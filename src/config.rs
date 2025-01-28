use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub models: Vec<String>,
    pub api_url: String,
    pub temperature: f32,
}

impl ProviderConfig {
    pub fn from_env(provider: &str) -> Self {
        let prefix = provider.to_uppercase();
        
        // Get models from env or use defaults
        let models = env::var(format!("{}_MODELS", prefix))
            .map(|m| m.split(',').map(|s| s.trim().to_string()).collect())
            .unwrap_or_else(|_| match provider {
                "openrouter" => vec![
                    "anthropic/claude-2".to_string(),
                    "anthropic/claude-instant-1".to_string(),
                    "google/palm-2-chat-bison".to_string(),
                ],
                "openai" => vec![
                    "gpt-4-turbo-preview".to_string(),
                    "gpt-4".to_string(),
                    "gpt-3.5-turbo".to_string(),
                ],
                "mistral" => vec![
                    "mistral-large-latest".to_string(),
                    "mistral-small-latest".to_string(),
                ],
                "gemini" => vec![
                    "gemini-2.0-flash-exp".to_string(),
                    "gemini-1.5-flash-8b".to_string(),
                ],
                _ => vec![]
            });

        // Get API URL from env or use default
        let api_url = env::var(format!("{}_API_URL", prefix))
            .unwrap_or_else(|_| match provider {
                "openrouter" => "https://openrouter.ai/api/v1/chat/completions".to_string(),
                "openai" => "https://api.openai.com/v1/chat/completions".to_string(),
                "mistral" => "https://api.mistral.ai/v1/chat/completions".to_string(),
                "gemini" => "https://generativelanguage.googleapis.com/v1beta/models/gemini-pro:generateContent".to_string(),
                _ => String::new()
            });

        // Get temperature from env or use default
        let temperature = env::var(format!("{}_TEMPERATURE", prefix))
            .ok()
            .and_then(|t| t.parse().ok())
            .unwrap_or(0.7);

        Self {
            models,
            api_url,
            temperature,
        }
    }
} 