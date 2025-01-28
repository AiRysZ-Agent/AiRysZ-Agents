use async_trait::async_trait;
use anyhow::{Result, anyhow};
use crate::providers::traits::CompletionProvider;
use async_openai::{
    types::{
        CreateEmbeddingRequestArgs, 
        EmbeddingInput, 
        CreateChatCompletionRequestArgs, 
        ChatCompletionRequestMessage,
        ChatCompletionRequestSystemMessage,
        ChatCompletionRequestUserMessage,
        ChatCompletionRequestUserMessageContent,
        Role,
    },
    Client, 
    config::OpenAIConfig,
};
use std::sync::{Arc, RwLock};
use std::env;

#[derive(Clone)]
pub struct OpenAIProvider {
    api_key: String,
    system_message: Arc<RwLock<String>>,
    client: Client<OpenAIConfig>,
    chat_model: String,
    embedding_model: String,
}

#[async_trait]
impl CompletionProvider for OpenAIProvider {
    async fn new(api_key: String, system_message: String) -> Result<Self> {
        let config = OpenAIConfig::new().with_api_key(api_key.clone());
        let client = Client::with_config(config);
        
        let chat_model = env::var("OPENAI_CHAT_MODEL").unwrap_or_else(|_| "gpt-4-turbo-preview".to_string());
        let embedding_model = env::var("OPENAI_EMBEDDING_MODEL").unwrap_or_else(|_| "text-embedding-3-small".to_string());
        
        Ok(Self {
            api_key,
            system_message: Arc::new(RwLock::new(system_message)),
            client,
            chat_model,
            embedding_model,
        })
    }

    async fn complete(&self, prompt: &str) -> Result<String> {
        let system_message = self.system_message.read()
            .map_err(|e| anyhow!("Failed to read system message: {}", e))?.clone();
        
        let request = CreateChatCompletionRequestArgs::default()
            .model(&self.chat_model)
            .messages(vec![
                ChatCompletionRequestMessage::System(
                    ChatCompletionRequestSystemMessage {
                        role: Role::System,
                        content: system_message,
                        name: None,
                    }
                ),
                ChatCompletionRequestMessage::User(
                    ChatCompletionRequestUserMessage {
                        role: Role::User,
                        content: ChatCompletionRequestUserMessageContent::Text(prompt.to_string()),
                        name: None,
                    }
                ),
            ])
            .build()?;

        let response = self.client.chat().create(request).await?;
        
        response.choices.first()
            .and_then(|choice| choice.message.content.clone())
            .ok_or_else(|| anyhow!("No response content"))
    }

    async fn generate_embedding(&self, text: &str) -> Result<Vec<f32>> {
        let request = CreateEmbeddingRequestArgs::default()
            .model(&self.embedding_model)
            .input(EmbeddingInput::String(text.to_string()))
            .build()?;

        let response = self.client.embeddings().create(request).await?;
        
        if let Some(embedding) = response.data.first() {
            Ok(embedding.embedding.clone())
        } else {
            Err(anyhow!("No embedding returned from OpenAI"))
        }
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
        Ok(self.chat_model.clone())
    }
}