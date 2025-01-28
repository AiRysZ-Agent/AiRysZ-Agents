use async_trait::async_trait;
use anyhow::{Result, anyhow};
use crate::providers::traits::CompletionProvider;
use crate::providers::utils::get_placeholder_embedding;
use reqwest::Client;
use serde_json::{json, Value};
use std::sync::{Arc, RwLock};
use std::env;

#[derive(Clone)]
pub struct GeminiProvider {
    api_key: String,
    system_message: Arc<RwLock<String>>,
    client: Client,
    model: String,
}

#[async_trait]
impl CompletionProvider for GeminiProvider {
    async fn new(api_key: String, system_message: String) -> Result<Self> {
        let model = env::var("GEMINI_MODEL").unwrap_or_else(|_| "gemini-pro".to_string());
        
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
            .post("https://generativelanguage.googleapis.com/v1beta/models/gemini-pro:generateContent")
            .query(&[("key", self.api_key.as_str())])
            .json(&json!({
                "contents": [{
                    "role": "user",
                    "parts": [{
                        "text": format!("{}\n{}", system_message, prompt)
                    }]
                }]
            }))
            .send()
            .await?;

        let response_json: Value = response.json().await?;
        
        response_json["candidates"][0]["content"]["parts"][0]["text"]
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