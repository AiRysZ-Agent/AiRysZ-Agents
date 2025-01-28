use async_trait::async_trait;
use anyhow::{Result, anyhow};
use crate::providers::traits::CompletionProvider;
use crate::providers::utils::get_placeholder_embedding;
use reqwest::Client;
use serde_json::{json, Value};
use std::sync::{Arc, RwLock};
use std::env;

#[derive(Clone)]
pub struct DeepSeekProvider {
    api_key: String,
    system_message: Arc<RwLock<String>>,
    client: Client,
    model: String,
}

impl DeepSeekProvider {
    pub fn clone_with_prompt(&self, system_prompt: &str) -> Self {
        Self {
            api_key: self.api_key.clone(),
            system_message: Arc::new(RwLock::new(system_prompt.to_string())),
            client: self.client.clone(),
            model: self.model.clone(),
        }
    }

    pub fn get_system_message(&self) -> String {
        self.system_message.read().unwrap().clone()
    }
}

#[async_trait]
impl CompletionProvider for DeepSeekProvider {
    async fn new(api_key: String, system_message: String) -> Result<Self> {
        let model = env::var("DEEPSEEK_MODEL").unwrap_or_else(|_| "deepseek-chat".to_string());
        
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
            .post("https://api.deepseek.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
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
                ],
                "temperature": 0.7
            }))
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("API request failed: Status {}, Body: {}", status, error_text));
        }

        let response_json: Value = response.json().await?;
        
        // Check for API-level errors
        if let Some(error) = response_json.get("error") {
            return Err(anyhow!("API returned error: {}", error));
        }

        // Extract the completion with better error handling
        response_json
            .get("choices")
            .and_then(|choices| choices.get(0))
            .and_then(|choice| choice.get("message"))
            .and_then(|message| message.get("content"))
            .and_then(|content| content.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| {
                let debug_json = serde_json::to_string_pretty(&response_json).unwrap_or_default();
                anyhow!("Invalid response format. Response JSON: {}", debug_json)
            })
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
