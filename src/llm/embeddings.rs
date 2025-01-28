use anyhow::{Result, Error};
use serde_json::Value;
use crate::providers::deepseek::deepseek::DeepSeekProvider;
use crate::providers::traits::CompletionProvider;

pub struct EmbeddingGenerator {
    provider: DeepSeekProvider,
}

impl EmbeddingGenerator {
    pub async fn new(api_key: String) -> Result<Self> {
        let provider = DeepSeekProvider::new(api_key, "You are a helpful assistant.".to_string()).await?;
        Ok(Self { provider })
    }

    pub async fn generate_embedding(&self, text: &str) -> Result<Vec<f32>> {
        let prompt = format!(
            "Convert this text into a numerical embedding vector that captures its semantic meaning. \
            Return ONLY a JSON array of 1536 float numbers:\n\n{}", 
            text
        );

        let response = self.provider.complete(&prompt).await?;
        
        // Clean the response to get just the JSON array
        let clean_response = response
            .trim()
            .trim_matches(|c| c == '[' || c == ']')
            .trim();

        // Parse the string of numbers into a Vec<f32>
        let numbers: Vec<f32> = clean_response
            .split(',')
            .map(|s| s.trim().parse::<f32>())
            .collect::<std::result::Result<Vec<f32>, _>>()
            .map_err(|e| Error::msg(format!("Failed to parse embedding numbers: {}", e)))?;

        // Validate vector size
        if numbers.len() != 1536 {
            return Err(Error::msg(format!(
                "Generated embedding has wrong size: {} (expected 1536)",
                numbers.len()
            )));
        }

        Ok(numbers)
    }

    pub async fn generate_batch_embeddings(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let mut embeddings = Vec::with_capacity(texts.len());
        for text in texts {
            let embedding = self.generate_embedding(text).await?;
            embeddings.push(embedding);
        }
        Ok(embeddings)
    }
} 