use async_trait::async_trait;
use std::any::Any;
use anyhow::Result;
use std::sync::{Arc, RwLock};

#[async_trait]
pub trait CompletionProvider: Any + Send + Sync {
    async fn new(api_key: String, system_message: String) -> Result<Self>
    where
        Self: Sized;

    async fn complete(&self, prompt: &str) -> Result<String>;

    async fn generate_embedding(&self, text: &str) -> Result<Vec<f32>>;

    async fn update_personality(&self, system_message: String) -> Result<()>;

    async fn get_model_info(&self) -> Result<String>;

    fn get_system_message(&self) -> String;

    fn get_api_key(&self) -> &String;

    fn clone_box(&self) -> Box<dyn CompletionProvider + Send + Sync>;
}

impl Clone for Box<dyn CompletionProvider + Send + Sync> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}