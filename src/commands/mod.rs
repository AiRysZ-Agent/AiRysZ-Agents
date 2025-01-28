use colored::Colorize;
use crate::providers::traits::CompletionProvider;
use crate::providers::openai::openai::OpenAIProvider;
use crate::providers::openrouter::openrouter::OpenRouterProvider;
use crate::providers::mistral::mistral::MistralProvider;
use crate::providers::gemini::gemini::GeminiProvider;
use crate::personality::PersonalityProfile;
use crate::providers::twitter::manager::ConversationManager;
use crate::providers::web_crawler::crawler_manager::WebCrawlerManager;
use crate::llm::memory::MemoryManager;
use crate::database::Database;
use crate::database::vector_db::VectorDB;
use std::sync::Arc;
use std::collections::HashMap;
use std::env;
use std::any::Any;
use std::any::TypeId;

mod character;
mod twitter;
mod web;
mod system;
mod document;

#[cfg(feature = "food")]
pub mod food_cmd;

pub struct CommandHandler {
    twitter_manager: Option<ConversationManager>,
    web_crawler: Option<WebCrawlerManager>,
    provider: Box<dyn CompletionProvider + Send + Sync>,
    personality: PersonalityProfile,
    memory_manager: MemoryManager,
    db: Arc<Database>,
    crawler: WebCrawlerManager,
    // Store API keys for different providers
    provider_keys: HashMap<String, String>,
}

impl CommandHandler {
    pub async fn new(
        personality: PersonalityProfile,
        twitter_manager: Option<ConversationManager>,
        web_crawler: Option<WebCrawlerManager>,
        provider: Box<dyn CompletionProvider + Send + Sync>,
    ) -> Result<Self, String> {
        let db = Database::new("agent.db")
            .await
            .map_err(|e| format!("Failed to initialize database: {}", e))?;

        // Initialize vector database
        let vector_db = VectorDB::new("http://localhost:6333")
            .await
            .map_err(|e| format!("Failed to initialize vector database: {}", e))?;

        // Initialize memory manager with vector database
        let vector_db = Arc::new(vector_db);
        let memory_manager = MemoryManager::new(vector_db)
            .await
            .map_err(|e| format!("Failed to initialize memory manager: {}", e))?;

        // Load API keys from environment
        let mut provider_keys = HashMap::new();
        for provider_name in ["openai", "openrouter", "mistral", "gemini"] {
            let key_var = format!("{}_API_KEY", provider_name.to_uppercase());
            if let Ok(api_key) = env::var(&key_var) {
                provider_keys.insert(provider_name.to_string(), api_key);
            }
        }

        Ok(Self {
            twitter_manager,
            web_crawler,
            provider,
            personality: personality.clone(),
            memory_manager,
            db: Arc::new(db),
            crawler: WebCrawlerManager::new(personality)
                .await
                .map_err(|e| format!("Failed to initialize web crawler: {}", e))?,
            provider_keys,
        })
    }

    pub async fn handle_command(&mut self, input: &str) -> Result<(), String> {
        if input.is_empty() {
            return Ok(());
        }

        let input = input.trim();

        // Handle single-word commands first
        match input.to_lowercase().as_str() {
            "help" | "exit" | "quit" => return self.handle_system_command(input).await,
            "chars" | "characters" | "load" => return self.handle_character_command(input).await,
            "providers" => return self.list_providers(),
            _ => {}
        }

        // Handle food commands if the feature is enabled
        #[cfg(feature = "food")]
        if input.starts_with("nutrition ") || input.starts_with("recipe ") {
            return food_cmd::handle_command(input, &self.provider).await;
        }

        // Handle command prefixes
        if input.starts_with("load ") {
            return self.handle_character_command(input).await;
        }

        if input.starts_with("use ") {
            return self.switch_provider(input.trim_start_matches("use ").trim()).await;
        }

        // Document commands
        if input.starts_with("doc ") {
            return document::handle_command(
                input,
                &self.provider,
                &mut self.memory_manager,
                &self.db
            ).await;
        }

        // Twitter commands
        if input.starts_with("tweet ") ||
           input.starts_with("autopost ") ||
           input.eq_ignore_ascii_case("tweet") ||
           input.eq_ignore_ascii_case("autopost") ||
           input.starts_with("reply ") ||
           input.starts_with("dm @") {
            return self.handle_twitter_command(input).await;
        }

        // Web commands
        if input.starts_with("web ") {
            if let Some(ref crawler) = self.web_crawler {
                let result = web::handle_command(
                    input.trim_start_matches("web ").trim(),
                    crawler,
                    &self.provider,
                    &mut self.memory_manager,
                ).await?;
                println!("{}", result);
                return Ok(());
            } else {
                return Err("Web crawler not initialized. Use --crawler flag to enable web features.".to_string());
            }
        }

        // Default to chat completion if no command matches
        self.handle_chat(input).await
    }

    async fn handle_twitter_command(&mut self, input: &str) -> Result<(), String> {
        if input.eq_ignore_ascii_case("tweet") {
            println!("Please provide a message to tweet.");
            println!("Usage: tweet <message>");
            return Ok(());
        }
        if input.eq_ignore_ascii_case("autopost") {
            println!("Please specify start or stop for autopost.");
            println!("Usage: autopost start <minutes> or autopost stop");
            return Ok(());
        }
        twitter::handle_command(input, &mut self.twitter_manager).await
    }

    async fn handle_character_command(&mut self, input: &str) -> Result<(), String> {
        let result = character::handle_command(input, &mut self.personality);
        if result.is_ok() {
            // Update provider with new personality
            if let Err(e) = self.provider.update_personality(
                self.personality.generate_system_prompt()
            ).await {
                return Err(format!("Failed to update personality: {}", e));
            }
        }
        result
    }

    async fn handle_system_command(&mut self, input: &str) -> Result<(), String> {
        system::handle_command(input)
    }

    async fn handle_chat(&mut self, input: &str) -> Result<(), String> {
        // Count input tokens
        let input_tokens = input.split_whitespace().count();
        println!("📥 Input tokens: {}", input_tokens.to_string().cyan());

        // Get response from AI
        match self.provider.complete(input).await {
            Ok(response) => {
                let response_tokens = response.split_whitespace().count();
                self.print_response("", &response, input_tokens, response_tokens);
                Ok(())
            }
            Err(e) => Err(format!("Failed to get AI response: {}", e))
        }
    }

    fn print_response(&self, _character_name: &str, response: &str, input_tokens: usize, response_tokens: usize) {
        println!("{}", response.truecolor(255, 236, 179));

        println!("\n📊 Tokens: 📥 Input: {} | 📤 Response: {} | 📈 Total: {}",
            input_tokens.to_string().cyan(),
            response_tokens.to_string().cyan(),
            (input_tokens + response_tokens).to_string().cyan()
        );
        println!();
    }

    fn list_providers(&self) -> Result<(), String> {
        println!("\n🤖 Available AI Providers:");
        println!("  Currently using: {}", self.get_current_provider_name().cyan());
        println!("\n  Available providers:");
        
        for provider in ["openai", "openrouter", "mistral", "gemini"] {
            let status = if self.provider_keys.contains_key(provider) {
                "✅ Ready".green()
            } else {
                "❌ No API key".red()
            };
            println!("  • {} - {}", provider, status);
        }
        
        println!("\nTo switch providers, use: use <provider>");
        println!("Example: use openai");
        
        Ok(())
    }

    fn get_current_provider_name(&self) -> String {
        let type_id = self.provider.type_id();
        
        if type_id == TypeId::of::<OpenAIProvider>() {
            "OpenAI"
        } else if type_id == TypeId::of::<OpenRouterProvider>() {
            "OpenRouter"
        } else if type_id == TypeId::of::<MistralProvider>() {
            "Mistral"
        } else if type_id == TypeId::of::<GeminiProvider>() {
            "Gemini"
        } else {
            "Unknown"
        }.to_string()
    }

    async fn switch_provider(&mut self, provider_name: &str) -> Result<(), String> {
        let provider_name = provider_name.to_lowercase();
        
        // Get API key for the requested provider
        let api_key = self.provider_keys.get(&provider_name)
            .ok_or_else(|| format!("No API key found for {}. Set {}_API_KEY in your environment.", 
                provider_name, provider_name.to_uppercase()))?
            .clone();

        // Create the new provider
        let new_provider: Box<dyn CompletionProvider + Send + Sync> = match provider_name.as_str() {
            "openai" => Box::new(OpenAIProvider::new(api_key, self.personality.generate_system_prompt()).await
                .map_err(|e| format!("Failed to initialize OpenAI provider: {}", e))?),
            "openrouter" => Box::new(OpenRouterProvider::new(api_key, self.personality.generate_system_prompt()).await
                .map_err(|e| format!("Failed to initialize OpenRouter provider: {}", e))?),
            "mistral" => Box::new(MistralProvider::new(api_key, self.personality.generate_system_prompt()).await
                .map_err(|e| format!("Failed to initialize Mistral provider: {}", e))?),
            "gemini" => Box::new(GeminiProvider::new(api_key, self.personality.generate_system_prompt()).await
                .map_err(|e| format!("Failed to initialize Gemini provider: {}", e))?),
            _ => return Err(format!("Unknown provider: {}. Available providers: openai, openrouter, mistral, gemini", provider_name))
        };

        // Switch to the new provider
        self.provider = new_provider;
        println!("🔄 Switched to {} provider", provider_name.cyan());
        
        Ok(())
    }
}

pub use document::handle_command as handle_document_command;
