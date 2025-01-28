use anyhow::{Result, Error};
use serde::{Deserialize, Serialize};
use crate::database::vector_db::VectorDB;
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use uuid;
use crate::providers::traits::CompletionProvider;
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize)]
pub struct Memory {
    pub text: String,
    pub timestamp: DateTime<Utc>,
    pub role: String,
    pub session_id: String,
    pub importance: f32,
    pub topic_tags: Vec<String>,
    pub metadata: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConversationSession {
    pub id: String,
    pub start_time: DateTime<Utc>,
    pub topic: String,
    pub summary: String,
    pub last_active: DateTime<Utc>,
}

#[derive(Clone)]
pub struct MemoryManager {
    vector_db: Arc<VectorDB>,
    collection_name: String,
    current_session: Option<ConversationSession>,
}

impl MemoryManager {
    pub async fn new(vector_db: Arc<VectorDB>) -> Result<Self> {
        let collection_name = "conversation_memory";
        
        // Create collection if it doesn't exist
        if let Err(e) = vector_db.create_collection(collection_name, 1536).await {
            eprintln!("Note: Collection may already exist: {}", e);
        }

        Ok(Self {
            vector_db,
            collection_name: collection_name.to_string(),
            current_session: None,
        })
    }

    pub async fn start_new_session(&mut self, topic: &str) -> Result<String> {
        let session = ConversationSession {
            id: uuid::Uuid::new_v4().to_string(),
            start_time: Utc::now(),
            topic: topic.to_string(),
            summary: String::new(),
            last_active: Utc::now(),
        };
        
        self.current_session = Some(session.clone());
        Ok(session.id)
    }

    pub async fn get_or_create_session(&mut self, topic: Option<&str>) -> Result<String> {
        if let Some(session) = &mut self.current_session {
            if Utc::now().signed_duration_since(session.last_active).num_minutes() < 30 {
                session.last_active = Utc::now();
                return Ok(session.id.clone());
            }
        }
        
        self.start_new_session(topic.unwrap_or("General Conversation")).await
    }

    pub async fn store_memory(&self, text: &str, role: &str, embedding: Vec<f32>, metadata: Option<HashMap<String, String>>) -> Result<String> {
        let session_id = if let Some(session) = &self.current_session {
            session.id.clone()
        } else {
            "default".to_string()
        };

        let memory = Memory {
            text: text.to_string(),
            timestamp: Utc::now(),
            role: role.to_string(),
            session_id,
            importance: 1.0, // Default importance
            topic_tags: vec![], // Will be filled by analyze_and_tag
            metadata,
        };

        let mut payload = HashMap::new();
        payload.insert("text".to_string(), serde_json::Value::String(memory.text.clone()));
        payload.insert("timestamp".to_string(), serde_json::Value::String(memory.timestamp.to_rfc3339()));
        payload.insert("role".to_string(), serde_json::Value::String(memory.role.clone()));
        payload.insert("session_id".to_string(), serde_json::Value::String(memory.session_id.clone()));
        payload.insert("importance".to_string(), serde_json::Value::Number(serde_json::Number::from_f64(memory.importance as f64).unwrap()));
        payload.insert("topic_tags".to_string(), serde_json::to_value(memory.topic_tags.clone())?);
        
        if let Some(meta) = memory.metadata {
            payload.insert("metadata".to_string(), serde_json::to_value(meta)?);
        }

        self.vector_db.store_vector(&self.collection_name, embedding, payload).await
            .map_err(|e| Error::msg(format!("Failed to store memory: {}", e)))
    }

    pub async fn search_similar(&self, query_embedding: Vec<f32>, limit: u64) -> Result<Vec<Memory>> {
        let results = self.vector_db.search_vectors(&self.collection_name, query_embedding, limit).await
            .map_err(|e| Error::msg(format!("Failed to search memories: {}", e)))?;

        let memories = results.into_iter()
            .filter_map(|(_, _, payload)| {
                let text = payload.get("text")?.as_str()?.to_string();
                let timestamp = payload.get("timestamp")?.as_str()
                    .and_then(|ts| DateTime::parse_from_rfc3339(ts).ok())
                    .map(|dt| dt.with_timezone(&Utc))?;
                let role = payload.get("role")?.as_str()?.to_string();
                let metadata = payload.get("metadata")
                    .and_then(|m| serde_json::from_value(m.clone()).ok());

                Some(Memory {
                    text,
                    timestamp,
                    role,
                    metadata,
                    session_id: String::new(),
                    importance: 1.0,
                    topic_tags: vec![],
                })
            })
            .collect();

        Ok(memories)
    }

    pub async fn get_recent_memories(&self, limit: u64) -> Result<Vec<Memory>> {
        // For recent memories, we'll use a zero vector to get all memories
        // and sort by timestamp (this could be optimized with a proper database query)
        let zero_vector = vec![0.0; 1536];
        let mut memories = self.search_similar(zero_vector, limit).await?;
        
        // Sort by timestamp, most recent first
        memories.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        
        Ok(memories)
    }

    pub async fn summarize_memories(&self, memories: &[Memory]) -> String {
        let mut summary = String::new();
        
        for memory in memories {
            summary.push_str(&format!("[{}] {}: {}\n", 
                memory.timestamp.format("%Y-%m-%d %H:%M:%S"),
                memory.role,
                memory.text
            ));
        }
        
        summary
    }

    pub async fn analyze_and_tag(&self, text: &str, provider: &dyn CompletionProvider) -> Result<(Vec<String>, f32)> {
        let prompt = format!(
            "Analyze the following message and:\n\
             1. Extract 1-3 topic tags (single words)\n\
             2. Rate its importance (0.0-1.0) for future context\n\
             Format: tag1,tag2,tag3|importance\n\n\
             Message: {}\n\n\
             Tags|Importance:",
            text
        );

        let response = provider.complete(&prompt).await?;
        let parts: Vec<&str> = response.split('|').collect();
        
        if parts.len() != 2 {
            return Ok((vec![], 1.0));
        }

        let tags: Vec<String> = parts[0]
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
            
        let importance = parts[1]
            .trim()
            .parse::<f32>()
            .unwrap_or(1.0)
            .clamp(0.0, 1.0);

        Ok((tags, importance))
    }

    pub async fn get_session_summary(&self, session_id: &str, provider: &dyn CompletionProvider) -> Result<String> {
        let memories = self.search_by_session(session_id).await?;
        
        if memories.is_empty() {
            return Ok("No conversation found for this session.".to_string());
        }

        let conversation = memories.iter()
            .map(|m| format!("{}: {}", m.role, m.text))
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = format!(
            "Summarize the key points of this conversation in 2-3 sentences:\n\n{}",
            conversation
        );

        provider.complete(&prompt).await
    }

    pub async fn search_by_session(&self, session_id: &str) -> Result<Vec<Memory>> {
        // For now, we'll retrieve all memories and filter
        // This could be optimized with proper database filtering
        let zero_vector = vec![0.0; 1536];
        let all_memories = self.search_similar(zero_vector, 100).await?;
        
        Ok(all_memories
            .into_iter()
            .filter(|m| m.session_id == session_id)
            .collect())
    }

    pub async fn update_session_summary(&mut self, provider: &dyn CompletionProvider) -> Result<()> {
        // First get the session ID if there is one
        let session_id = match &self.current_session {
            Some(session) => session.id.clone(),
            None => return Ok(()),
        };

        // Get the summary using the session ID
        let summary = self.get_session_summary(&session_id, provider).await?;

        // Now update the session with the new summary
        if let Some(session) = &mut self.current_session {
            session.summary = summary;
        }

        Ok(())
    }

    pub async fn get_topic_context(&self, topic: &str, limit: u64) -> Result<Vec<Memory>> {
        let zero_vector = vec![0.0; 1536];
        let all_memories = self.search_similar(zero_vector, 100).await?;
        
        let mut topic_memories: Vec<Memory> = all_memories
            .into_iter()
            .filter(|m| m.topic_tags.contains(&topic.to_string()))
            .collect();
            
        topic_memories.sort_by(|a, b| b.importance.partial_cmp(&a.importance).unwrap());
        topic_memories.truncate(limit as usize);
        
        Ok(topic_memories)
    }

    pub async fn cleanup_old_memories(&self) -> Result<()> {
        // Delete memories older than 30 days
        let thirty_days_ago = Utc::now() - chrono::Duration::days(30);
        
        // For now, just return Ok since we don't have direct timestamp filtering in VectorDB
        // In a real implementation, you would want to filter and delete old vectors
        Ok(())
    }
} 