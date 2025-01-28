use anyhow::Result;
use crate::llm::memory::{Memory, MemoryManager};
use crate::providers::traits::CompletionProvider;
use crate::database::vector_db::VectorDB;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct ChatManager<T: CompletionProvider> {
    provider: Arc<T>,
    memory: Arc<Mutex<MemoryManager>>,
    context_window: usize,
    max_context_length: usize,
}

impl<T: CompletionProvider> ChatManager<T> {
    pub async fn new(provider: T, vector_db: VectorDB, context_window: usize) -> Result<Self> {
        let vector_db = Arc::new(vector_db);
        let memory = Arc::new(Mutex::new(MemoryManager::new(vector_db).await?));
        
        Ok(Self {
            provider: Arc::new(provider),
            memory,
            context_window,
            max_context_length: 4000, // Adjust based on your model's limits
        })
    }

    pub async fn start_conversation(&self, topic: Option<&str>) -> Result<String> {
        let mut memory = self.memory.lock().await;
        memory.start_new_session(topic.unwrap_or("General Conversation")).await
    }

    pub async fn chat(&self, user_message: &str) -> Result<String> {
        // Generate embedding for user message
        let user_embedding = self.provider.generate_embedding(user_message).await?;
        
        // Get or create session
        let session_id = {
            let mut memory = self.memory.lock().await;
            memory.get_or_create_session(None).await?
        };

        // Store user message in memory
        {
            let memory = self.memory.lock().await;
            memory.store_memory(
                user_message,
                "user",
                user_embedding.clone(),
                None
            ).await?;
        }

        // Build context from various sources
        let context = self.build_conversation_context(user_message, &user_embedding).await?;
        
        // Generate response with rich context
        let prompt = format!(
            "Conversation Context:\n{}\n\n\
             Current Session ID: {}\n\n\
             User: {}\nAssistant:",
            context,
            session_id,
            user_message
        );

        let response = self.provider.complete(&prompt).await?;

        // Store assistant's response
        let response_embedding = self.provider.generate_embedding(&response).await?;
        {
            let memory = self.memory.lock().await;
            memory.store_memory(
                &response,
                "assistant",
                response_embedding,
                None
            ).await?;
        }

        Ok(response)
    }

    async fn build_conversation_context(&self, user_message: &str, user_embedding: &[f32]) -> Result<String> {
        let memory = self.memory.lock().await;
        
        // Get recent and similar messages
        let similar_memories = memory.search_similar(user_embedding.to_vec(), 10).await?;
        let recent_memories = memory.get_recent_memories(5).await?;
        
        // Build context sections
        let mut context = String::new();
        
        // Add recent conversation
        context.push_str("Recent Conversation:\n");
        for mem in recent_memories.iter().rev() {
            context.push_str(&format!("{}: {}\n", mem.role, mem.text));
        }
        
        // Add relevant past messages
        context.push_str("\nRelevant Past Messages:\n");
        for mem in similar_memories.iter() {
            if !recent_memories.iter().any(|m| m.text == mem.text) {
                context.push_str(&format!("[Previous] {}: {}\n", mem.role, mem.text));
            }
        }
        
        // Truncate if too long while preserving recent messages
        if context.len() > self.max_context_length {
            let recent_part = context.split("\nRelevant Past Messages:\n").next().unwrap_or("");
            context = format!("{}\nRelevant Past Messages: [Truncated for length]", recent_part);
        }
        
        Ok(context)
    }

    pub async fn get_conversation_summary(&self) -> Result<String> {
        let memory = self.memory.lock().await;
        let memories = memory.get_recent_memories(10).await?;
        Ok(memory.summarize_memories(&memories).await)
    }
} 