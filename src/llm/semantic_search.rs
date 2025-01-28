use anyhow::{Result, Error};
use serde::{Deserialize, Serialize};
use crate::database::vector_db::VectorDB;
use std::collections::HashMap;
use crate::llm::memory::{Memory, MemoryManager};
use crate::providers::traits::CompletionProvider;
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResult {
    pub text: String,
    pub score: f32,
    pub source: String,
    pub metadata: Option<HashMap<String, String>>,
}

pub struct SemanticSearch {
    vector_db: VectorDB,
    collection_name: String,
    provider: Arc<dyn CompletionProvider>,
    memory: MemoryManager,
}

impl SemanticSearch {
    pub async fn new(vector_db: VectorDB, provider: Arc<dyn CompletionProvider>, memory: MemoryManager) -> Result<Self> {
        let collection_name = "semantic_search";
        
        // Create collection if it doesn't exist
        if let Err(e) = vector_db.create_collection(collection_name, 1536).await {
            eprintln!("Note: Collection may already exist: {}", e);
        }

        Ok(Self {
            vector_db,
            collection_name: collection_name.to_string(),
            provider,
            memory,
        })
    }

    pub async fn index_text(&self, text: &str, source: &str, embedding: Vec<f32>, metadata: Option<HashMap<String, String>>) -> Result<String> {
        let mut payload = HashMap::new();
        payload.insert("text".to_string(), serde_json::Value::String(text.to_string()));
        payload.insert("source".to_string(), serde_json::Value::String(source.to_string()));
        
        if let Some(meta) = metadata {
            payload.insert("metadata".to_string(), serde_json::to_value(meta)?);
        }

        self.vector_db.store_vector(&self.collection_name, embedding, payload).await
            .map_err(|e| Error::msg(format!("Failed to index text: {}", e)))
    }

    pub async fn search(&self, query_embedding: Vec<f32>, limit: u64) -> Result<Vec<SearchResult>> {
        let results = self.vector_db.search_vectors(&self.collection_name, query_embedding, limit).await
            .map_err(|e| Error::msg(format!("Failed to search: {}", e)))?;

        let search_results = results.into_iter()
            .filter_map(|(_, score, payload)| {
                let text = payload.get("text")?.as_str()?.to_string();
                let source = payload.get("source")?.as_str()?.to_string();
                let metadata = payload.get("metadata")
                    .and_then(|m| serde_json::from_value(m.clone()).ok());

                Some(SearchResult {
                    text,
                    score,
                    source,
                    metadata,
                })
            })
            .collect();

        Ok(search_results)
    }

    pub async fn search_by_source(&self, query_embedding: Vec<f32>, source: &str, limit: u64) -> Result<Vec<SearchResult>> {
        // This is a basic implementation - in a real system, you'd want to use Qdrant's filtering capabilities
        let mut results = self.search(query_embedding, limit * 2).await?;
        
        results.retain(|r| r.source == source);
        results.truncate(limit as usize);
        
        Ok(results)
    }

    pub async fn format_results(&self, results: &[SearchResult]) -> String {
        let mut formatted = String::new();
        
        for (i, result) in results.iter().enumerate() {
            formatted.push_str(&format!("{}. [Score: {:.2}] {} (Source: {})\n", 
                i + 1,
                result.score,
                result.text,
                result.source
            ));
        }
        
        formatted
    }

    pub async fn chat(&self, user_message: &str) -> Result<String> {
        let user_embedding = self.provider.as_ref().generate_embedding(user_message).await?;
        
        // Get relevant search results
        let search_results = self.search(user_embedding.clone(), 5).await?;
        let formatted_results = self.format_results(&search_results).await;
        
        // Build prompt with search results
        let prompt = format!(
            "Relevant search results:\n{}\n\nUser: {}\nAssistant:",
            formatted_results,
            user_message
        );

        let response = self.provider.as_ref().complete(&prompt).await?;
        let response_embedding = self.provider.as_ref().generate_embedding(&response).await?;

        // Store the interaction in memory
        self.memory.store_memory(
            &format!("Q: {}\nA: {}", user_message, response),
            "chat",
            response_embedding,
            None
        ).await?;

        Ok(response)
    }

    fn format_memories(recent: &[Memory], similar: &[Memory]) -> String {
        let mut context = String::new();
        
        // Add recent messages first
        context.push_str("Recent messages:\n");
        for memory in recent {
            context.push_str(&format!("{}: {}\n", memory.role, memory.text));
        }
        
        // Add relevant similar messages
        context.push_str("\nRelevant previous messages:\n");
        for memory in similar {
            context.push_str(&format!("{}: {}\n", memory.role, memory.text));
        }
        
        context
    }

    pub async fn get_conversation_summary(&self) -> Result<String> {
        let memories = self.memory.get_recent_memories(10).await?;
        Ok(self.memory.summarize_memories(&memories).await)
    }
} 