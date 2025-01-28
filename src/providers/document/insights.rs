use serde::{Deserialize, Serialize};
use crate::providers::deepseek::deepseek::DeepSeekProvider;
use crate::providers::openai::openai::OpenAIProvider;
use crate::providers::traits::CompletionProvider;
use std::fmt;
use anyhow::{Result, Error};
use qdrant_client::{
    qdrant::{
        PointStruct, SearchPoints, UpsertPoints,
        with_payload_selector::SelectorOptions, WithPayloadSelector,
        Value, PointId, point_id::PointIdOptions,
        Condition, Filter, MinShould,
        condition::{self, ConditionOneOf},
        Range,
    },
    Qdrant,
};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;
use log;
use crate::database::qdrant_config::create_qdrant_client;
use serde_json;
use serde_json::json;
use lru::LruCache;
use std::sync::Mutex;
use std::num::NonZeroUsize;
use std::cmp::Ordering;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Insight {
    pub text: String,
    pub relevance: f32,
    pub embedding: Option<Vec<f32>>,
    pub metadata: Option<serde_json::Value>,
}

impl fmt::Display for Insight {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Insight: {} (Relevance: {:.2})", self.text, self.relevance)
    }
}

#[derive(Debug, Clone)]
pub struct DocumentChunk {
    pub text: String,
    pub page_number: i32,
    pub chunk_index: i32,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Clone)]
struct ProcessedChunk {
    chunk: DocumentChunk,
    embedding: Option<Vec<f32>>,
    insights: Vec<Insight>,
}

pub struct InsightExtractor {
    deepseek_provider: DeepSeekProvider,
    embedding_provider: OpenAIProvider,
    client: Arc<Qdrant>,
    chunk_cache: Arc<Mutex<LruCache<String, ProcessedChunk>>>,
}

#[derive(Debug)]
pub struct SearchResult {
    pub text: String,
    pub context: String,
    pub score: f32,
    pub page_number: i32,
    pub chunk_index: i32,
}

impl InsightExtractor {
    pub async fn new(api_key: String, system_message: String) -> Result<Self, Box<dyn std::error::Error>> {
        let url = std::env::var("QDRANT_URL").unwrap_or_else(|_| "localhost:6333".to_string());
        let client = create_qdrant_client(&url).await?;
        
        let deepseek_provider = DeepSeekProvider::new(api_key.clone(), system_message.clone()).await
            .map_err(|e| Error::msg(format!("Failed to create DeepSeek provider: {}", e)))?;
            
        let embedding_provider = OpenAIProvider::new(api_key.clone(), system_message).await
            .map_err(|e| Error::msg(format!("Failed to create OpenAI provider: {}", e)))?;

        // Initialize cache with 100 item capacity
        let chunk_cache = Arc::new(Mutex::new(LruCache::new(NonZeroUsize::new(100).unwrap())));
        
        Ok(Self { 
            deepseek_provider,
            embedding_provider,
            client: Arc::new(client),
            chunk_cache,
        })
    }

    // Add cache helper methods
    fn cache_chunk(&self, key: String, chunk: ProcessedChunk) {
        if let Ok(mut cache) = self.chunk_cache.lock() {
            cache.put(key, chunk);
        }
    }

    fn get_cached_chunk(&self, key: &str) -> Option<ProcessedChunk> {
        self.chunk_cache.lock().ok()?.get(key).cloned()
    }

    pub async fn extract_insights(&self, text: &str) -> Result<Vec<Insight>> {
        let prompt = format!(
            r#"Extract key insights from the following text and format them as a JSON array.

Each insight must be an object with exactly these fields:
"text": (string) The insight text
"relevance": (number) Importance score between 0 and 1

Example format:
[
  {{"text": "First key insight here", "relevance": 0.95}},
  {{"text": "Second key insight here", "relevance": 0.85}}
]

Text to analyze:
{}

Respond ONLY with the JSON array. Do not add any explanations or additional text."#,
            text
        );

        let response = self.deepseek_provider.complete(&prompt).await
            .map_err(|e| Error::msg(format!("Failed to get completion: {}", e)))?;

        // Parse insights
        let mut insights: Vec<Insight> = self.parse_insights_response(&response)?;

        // Generate embeddings for each insight
        for insight in &mut insights {
            if let Ok(embedding) = self.generate_embedding(&insight.text).await {
                insight.embedding = Some(embedding.clone());
                
                // Store in Qdrant
                if let Err(e) = self.store_insight_vector(self.client.as_ref(), insight, &embedding).await {
                    eprintln!("Warning: Failed to store vector: {}", e);
                }
            }
        }

        Ok(insights)
    }

    async fn generate_embedding(&self, text: &str) -> Result<Vec<f32>> {
        // Use OpenAI for embeddings
        self.embedding_provider.generate_embedding(text).await
            .map_err(|e| Error::msg(format!("Failed to generate embedding: {}", e)))
    }

    async fn store_insight_vector(
        &self,
        client: &Qdrant,
        insight: &Insight,
        embedding: &[f32]
    ) -> Result<()> {
        let point_id = Uuid::new_v4().to_string();

        // Convert to qdrant::Value
        let mut payload = HashMap::new();
        payload.insert("text".to_string(), Value::from(insight.text.clone()));
        payload.insert("relevance".to_string(), Value::from(insight.relevance));
        if let Some(metadata) = &insight.metadata {
            payload.insert("metadata".to_string(), Value::from(metadata.clone()));
        }

        let point = PointStruct {
            id: Some(PointId {
                point_id_options: Some(PointIdOptions::Uuid(point_id.clone()))
            }),
            vectors: Some(embedding.to_vec().into()),
            payload,
        };

        let upsert_points = UpsertPoints {
            collection_name: "document_insights".to_string(),
            points: vec![point],
            ..Default::default()
        };

        client.upsert_points(upsert_points).await?;

        Ok(())
    }

    // Helper method to parse insights from AI response
    fn parse_insights_response(&self, response: &str) -> Result<Vec<Insight>> {
        let cleaned_response = response
            .trim()
            .trim_matches('`')
            .trim_start_matches("json")
            .trim_start_matches("JSON")
            .replace('\'', "\"")
            .trim()
            .to_string();

        match serde_json::from_str(&cleaned_response) {
            Ok(insights) => Ok(insights),
            Err(_) => {
                let fixed_response = if cleaned_response.starts_with("{") && cleaned_response.ends_with("}") {
                    format!("[{}]", cleaned_response)
                } else if !cleaned_response.starts_with("[") {
                    format!("[{}]", cleaned_response)
                } else {
                    cleaned_response
                };

                match serde_json::from_str(&fixed_response) {
                    Ok(insights) => Ok(insights),
                    Err(_) => {
                        Ok(response
                            .lines()
                            .filter(|line| !line.trim().is_empty())
                            .map(|line| Insight {
                                text: line.trim().to_string(),
                                relevance: 0.8,
                                embedding: None,
                                metadata: None,
                            })
                            .collect())
                    }
                }
            }
        }
    }

    // New method for quick, direct analysis without JSON
    pub async fn quick_analyze(&self, text: &str) -> Result<String> {
        let prompt = format!(
            "Please analyze this text and provide the key insights in a clear, concise way:\n\n{}",
            text
        );

        let response = self.deepseek_provider.complete(&prompt).await
            .map_err(|e| Error::msg(format!("Failed to get quick analysis: {}", e)))?;
        Ok(response)
    }

    // New method to search for similar insights
    pub async fn search_similar_insights(&self, query_text: &str) -> Result<Vec<(String, f32)>> {
        let embedding = self.generate_embedding(query_text).await?;

        let request = SearchPoints {
            collection_name: "document_insights".to_string(),
            vector: embedding,
            limit: 10,
            with_payload: Some(WithPayloadSelector {
                selector_options: Some(SelectorOptions::Enable(true)),
            }),
            ..Default::default()
        };

        let results = self.client.search_points(request).await
            .map_err(|e| Error::msg(format!("Failed to search vectors: {}", e)))?;

        let insights = results.result
            .into_iter()
            .filter_map(|point| {
                let score = point.score;
                let payload = point.payload;
                if let Some(Value { kind: Some(qdrant_client::qdrant::value::Kind::StringValue(text)) }) = payload.get("text") {
                    Some((text.clone(), score))
                } else {
                    None
                }
            })
            .collect();

        Ok(insights)
    }

    async fn store_document_vector(
        client: &Qdrant,
        collection: &str,
        vector: Vec<f32>,
        payload: HashMap<String, Value>,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let point_id = Uuid::new_v4().to_string();

        let point = PointStruct {
            id: Some(PointId {
                point_id_options: Some(PointIdOptions::Uuid(point_id.clone()))
            }),
            vectors: Some(vector.into()),
            payload,
        };

        let upsert_points = UpsertPoints {
            collection_name: collection.to_string(),
            points: vec![point],
            ..Default::default()
        };

        client.upsert_points(upsert_points).await?;

        Ok(point_id)
    }

    async fn search_similar_documents(
        client: &Qdrant,
        collection: &str,
        query_vector: Vec<f32>,
        limit: u64,
    ) -> Result<Vec<(String, f32, HashMap<String, Value>)>, Box<dyn std::error::Error>> {
        let request = SearchPoints {
            collection_name: collection.to_string(),
            vector: query_vector,
            limit,
            with_payload: Some(WithPayloadSelector {
                selector_options: Some(SelectorOptions::Enable(true)),
            }),
            ..Default::default()
        };

        let results = client.search_points(request).await?;

        Ok(results.result
            .into_iter()
            .map(|point| {
                let id = match point.id.and_then(|id| id.point_id_options) {
                    Some(PointIdOptions::Uuid(uuid)) => uuid,
                    _ => String::new(),
                };
                let score = point.score;
                (id, score, point.payload)
            })
            .collect())
    }

    pub async fn process_document(&self, text: &str, metadata: Option<serde_json::Value>) -> Result<Vec<Insight>> {
        let chunks = self.create_chunks(text, 1000);
        
        // Collect all texts for batch embedding
        let texts: Vec<String> = chunks.iter()
            .map(|c| c.text.clone())
            .collect();

        // Generate embeddings in batch
        let embeddings = if !texts.is_empty() {
            let mut batch_embeddings = Vec::new();
            for chunk in texts.chunks(20) {
                let chunk_embeddings = futures::future::join_all(
                    chunk.iter().map(|text| self.generate_embedding(text))
                ).await;
                batch_embeddings.extend(chunk_embeddings.into_iter().filter_map(Result::ok));
            }
            Some(batch_embeddings)
        } else {
            None
        };

        // Process chunks in parallel and cache them
        let mut tasks = Vec::new();
        for (i, chunk) in chunks.into_iter().enumerate() {
            let chunk_embedding = embeddings.as_ref().and_then(|e| e.get(i).cloned());
            let metadata = metadata.clone();
            tasks.push(self.process_chunk(chunk, chunk_embedding, metadata));
        }

        let chunk_results = futures::future::join_all(tasks).await;
        
        let mut all_insights = Vec::new();
        let mut points_to_store = Vec::new();

        for result in chunk_results {
            if let Ok((chunk_insights, processed_chunk)) = result {
                // Cache the processed chunk
                let cache_key = format!(
                    "page_{}_chunk_{}", 
                    processed_chunk.chunk.page_number,
                    processed_chunk.chunk.chunk_index
                );
                self.cache_chunk(cache_key, processed_chunk.clone());
                
                all_insights.extend(chunk_insights);
                
                if let Some(embedding) = processed_chunk.embedding {
                    let point_id = Uuid::new_v4().to_string();
                    let mut payload = HashMap::new();
                    payload.insert("text".to_string(), Value::from(processed_chunk.chunk.text));
                    payload.insert("page".to_string(), Value::from(processed_chunk.chunk.page_number as i64));
                    payload.insert("chunk".to_string(), Value::from(processed_chunk.chunk.chunk_index as i64));
                    
                    points_to_store.push(PointStruct {
                        id: Some(PointId {
                            point_id_options: Some(PointIdOptions::Uuid(point_id))
                        }),
                        vectors: Some(embedding.into()),
                        payload,
                    });
                }
            }
        }

        // Batch store vectors
        if !points_to_store.is_empty() {
            let upsert_points = UpsertPoints {
                collection_name: "document_chunks".to_string(),
                points: points_to_store,
                ..Default::default()
            };

            if let Err(e) = self.client.upsert_points(upsert_points).await {
                log::warn!("Failed to store vectors in batch: {}", e);
            }
        }

        Ok(all_insights)
    }

    async fn process_chunk(
        &self,
        chunk: DocumentChunk,
        embedding: Option<Vec<f32>>,
        metadata: Option<serde_json::Value>,
    ) -> Result<(Vec<Insight>, ProcessedChunk)> {
        let mut chunk_insights = self.extract_insights(&chunk.text).await?;
        
        // Add metadata and embedding to insights
        for insight in &mut chunk_insights {
            let mut meta = serde_json::Map::new();
            meta.insert("page".to_string(), json!(chunk.page_number));
            meta.insert("chunk".to_string(), json!(chunk.chunk_index));
            
            if let Some(doc_meta) = &metadata {
                if let Some(obj) = doc_meta.as_object() {
                    meta.extend(obj.clone());
                }
            }
            
            insight.metadata = Some(serde_json::Value::Object(meta));
            insight.embedding = embedding.clone();
        }

        let chunk_insights_clone = chunk_insights.clone();
        Ok((chunk_insights, ProcessedChunk {
            chunk,
            embedding,
            insights: chunk_insights_clone,
        }))
    }

    fn create_chunks(&self, text: &str, chunk_size: usize) -> Vec<DocumentChunk> {
        let mut chunks = Vec::new();
        let mut page = 1;
        let mut chunk_idx = 0;

        // Split text into pages if page markers exist
        let pages = text.split("\n\nPage ").collect::<Vec<_>>();
        
        for page_text in pages {
            let words: Vec<&str> = page_text.split_whitespace().collect();
            let mut start = 0;

            while start < words.len() {
                let end = (start + chunk_size).min(words.len());
                let chunk_text = words[start..end].join(" ");

                chunks.push(DocumentChunk {
                    text: chunk_text,
                    page_number: page,
                    chunk_index: chunk_idx,
                    metadata: None,
                });

                chunk_idx += 1;
                start = end;
            }
            page += 1;
        }

        chunks
    }

    // Improved search with context
    pub async fn search_document(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let embedding = self.generate_embedding(query).await?;

        let request = SearchPoints {
            collection_name: "document_chunks".to_string(),
            vector: embedding,
            limit: limit as u64,
            with_payload: Some(WithPayloadSelector {
                selector_options: Some(SelectorOptions::Enable(true)),
            }),
            ..Default::default()
        };

        let results = self.client.search_points(request).await?;
        
        let mut search_results = Vec::new();
        for point in results.result {
            let text = point.payload.get("text")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_default();
                
            let page = point.payload.get("page")
                .and_then(|v| match v.kind {
                    Some(qdrant_client::qdrant::value::Kind::IntegerValue(i)) => Some(i),
                    _ => None,
                })
                .unwrap_or(0) as i32;
                
            let chunk_idx = point.payload.get("chunk")
                .and_then(|v| match v.kind {
                    Some(qdrant_client::qdrant::value::Kind::IntegerValue(i)) => Some(i),
                    _ => None,
                })
                .unwrap_or(0) as i32;

            // Try to get context from cache
            let cache_key = format!("page_{}_chunk_{}", page, chunk_idx);
            let context = if let Some(cached) = self.get_cached_chunk(&cache_key) {
                cached.chunk.text
            } else {
                text.clone()
            };

            search_results.push(SearchResult {
                text,
                context,
                score: point.score,
                page_number: page,
                chunk_index: chunk_idx,
            });
        }

        Ok(search_results)
    }

    pub async fn get_document_summary(&self, page_range: Option<(i32, i32)>) -> Result<String> {
        let mut filter = None;
        if let Some((start, end)) = page_range {
            let mut conditions = Vec::new();
            conditions.push(Condition {
                condition_one_of: Some(condition::ConditionOneOf::Field(
                    qdrant_client::qdrant::FieldCondition {
                        key: "page".to_string(),
                        r#match: None,
                        range: Some(qdrant_client::qdrant::Range {
                            gt: Some((start - 1) as f64),  // -1 to make it inclusive
                            lt: Some((end + 1) as f64),    // +1 to make it inclusive
                            ..Default::default()
                        }),
                        ..Default::default()
                    }
                ))
            });

            filter = Some(Filter {
                should: vec![],
                must: conditions,
                must_not: vec![],
                min_should: Some(MinShould { 
                    conditions: vec![],
                    min_count: 0
                }),
            });
        }

        let request = SearchPoints {
            collection_name: "document_chunks".to_string(),
            vector: vec![0.0; 1536], // Dummy vector for getting all chunks
            limit: 100,
            with_payload: Some(WithPayloadSelector {
                selector_options: Some(SelectorOptions::Enable(true)),
            }),
            filter,
            ..Default::default()
        };

        let results = self.client.search_points(request).await?;
        
        let mut chunks_by_page: HashMap<i32, Vec<String>> = HashMap::new();
        for point in results.result {
            if let (Some(text), Some(page)) = (
                point.payload.get("text").and_then(|v| v.as_str()),
                point.payload.get("page").and_then(|v| match v.kind {
                    Some(qdrant_client::qdrant::value::Kind::IntegerValue(i)) => Some(i),
                    _ => None,
                })
            ) {
                chunks_by_page
                    .entry(page as i32)
                    .or_default()
                    .push(text.to_string());
            }
        }

        let mut summary_text = String::new();
        let mut sorted_pages: Vec<_> = chunks_by_page.iter().collect();
        sorted_pages.sort_by_key(|(&k, _)| k);
        
        for (page, chunks) in sorted_pages {
            let page_text = chunks.join(" ");
            let prompt = format!(
                "Summarize this text from page {} concisely:\n\n{}", 
                page, page_text
            );
            
            if let Ok(summary) = self.deepseek_provider.complete(&prompt).await {
                summary_text.push_str(&format!("\nPage {}: {}\n", page, summary));
            }
        }

        Ok(summary_text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_embedding_generation() {
        let api_key = std::env::var("OPENAI_API_KEY")
            .expect("OPENAI_API_KEY environment variable not set");
        let system_message = "You are a helpful assistant.".to_string();
        
        let extractor = InsightExtractor::new(api_key, system_message)
            .await
            .expect("Failed to create InsightExtractor");
            
        let text = "This is a test sentence for embedding generation.";
        let embedding = extractor.generate_embedding(text)
            .await
            .expect("Failed to generate embedding");
            
        assert_eq!(embedding.len(), 1536); // OpenAI ada-002 embeddings are 1536-dimensional
        assert!(embedding.iter().any(|&x| x != 0.0)); // Ensure we're not getting zero vectors
    }
}
