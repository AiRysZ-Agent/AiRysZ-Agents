use async_trait::async_trait;
use anyhow::{Result, anyhow};
use crate::providers::traits::CompletionProvider;
use crate::providers::utils::get_placeholder_embedding;
use reqwest::Client;
use serde_json::{json, Value};
use std::sync::{Arc, RwLock};
use std::env;

#[derive(Clone)]
pub struct OpenRouterProvider {
    api_key: String,
    system_message: Arc<RwLock<String>>,
    client: Client,
    model: String,
}

#[async_trait]
impl CompletionProvider for OpenRouterProvider {
    async fn new(api_key: String, system_message: String) -> Result<Self> {
        let model = env::var("OPENROUTER_MODEL").unwrap_or_else(|_| "anthropic/claude-3-opus".to_string());
        
        Ok(Self {
            api_key,
            system_message: Arc::new(RwLock::new(system_message)),
            client: Client::new(),
            model,
        })
    }

    async fn complete(&self, prompt: &str) -> Result<String> {
        let system_message = self.system_message.read().map_err(|e| anyhow!("Failed to read system message: {}", e))?.clone();
        
        let response = self.client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("HTTP-Referer", "https://github.com/your-repo")
            .header("X-Title", "AI Agent")
            .json(&json!({
                "model": self.model,
                "messages": [
                    {
                        "role": "system",
                        "content": system_message
                    },
                    {
                        "role": "user",
                        "content": prompt
                    }
                ]
            }))
            .send()
            .await?;

        let response_json: Value = response.json().await?;
        
        response_json["choices"][0]["message"]["content"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow!("Invalid response format"))
    }

    async fn generate_embedding(&self, text: &str) -> Result<Vec<f32>> {
        // Use placeholder embeddings for now
        get_placeholder_embedding(text).await
    }

    async fn update_personality(&self, system_message: String) -> Result<()> {
        let mut guard = self.system_message.write().map_err(|e| anyhow!("Lock error: {}", e))?;
        *guard = system_message;
        Ok(())
    }

    fn get_system_message(&self) -> String {
        self.system_message.read().unwrap().clone()
    }

    fn get_api_key(&self) -> &String {
        &self.api_key
    }

    fn clone_box(&self) -> Box<dyn CompletionProvider + Send + Sync> {
        Box::new(self.clone())
    }

    async fn get_model_info(&self) -> Result<String> {
        Ok(self.model.clone())
    }
}