use rust_ai_agent::providers::traits::CompletionProvider;
use rust_ai_agent::providers::openai::openai::OpenAIProvider;
use rust_ai_agent::providers::openrouter::openrouter::OpenRouterProvider;
use rust_ai_agent::providers::mistral::mistral::MistralProvider;
use rust_ai_agent::providers::gemini::gemini::GeminiProvider;
use rust_ai_agent::providers::deepseek::deepseek::DeepSeekProvider;
use rust_ai_agent::knowledge_base::knowledge_base::KnowledgeBaseHandler;
use rust_ai_agent::database::Database;
use rust_ai_agent::learning::LearningManager;
use rust_ai_agent::personality::{Personality, PersonalityProfile};
use rust_ai_agent::providers::twitter::manager::ConversationManager;
use rust_ai_agent::providers::web_crawler::crawler_manager::WebCrawlerManager;
use rust_ai_agent::commands::CommandHandler;
use rust_ai_agent::llm::MemoryManager;
use rust_ai_agent::api;
use std::env;
use std::io::Write;
use std::path::Path;
use std::fs::File;
use std::net::SocketAddr;
use clap::Parser;
use colored::Colorize;
use dotenv::dotenv;
use rustyline::error::ReadlineError;
use rustyline::Editor;
use rustyline::history::DefaultHistory;
use axum::serve;
use tokio::net::TcpListener;
use std::time::Duration;
use tokio::time::timeout;
use thiserror::Error;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(feature = "food")]
mod food;

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    api_key: Option<String>,

    #[arg(long)]
    provider: Option<String>,

    #[arg(long)]
    twitter: bool,

    #[arg(long)]
    crawler: bool,

    #[arg(long)]
    character: Option<String>,

    #[arg(long)]
    twitter_cookie: Option<String>,

    #[arg(long)]
    twitter_username: Option<String>,

    #[arg(long)]
    twitter_password: Option<String>,

    #[arg(long)]
    twitter_email: Option<String>,

    #[arg(long)]
    api: bool,

    #[arg(long, default_value = "3000")]
    port: u16,

    #[arg(long)]
    server: bool,

    #[cfg(feature = "food")]
    #[arg(long)]
    food_mode: bool,
}

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Provider error: {0}")]
    ProviderError(String),
    #[error("Database error: {0}")]
    DatabaseError(String),
    #[error("API error: {0}")]
    ApiError(String),
    #[error("Timeout error")]
    TimeoutError,
}

#[derive(Clone)]
struct ProviderFactory {
    api_key: String,
    system_prompt: String,
    active_provider: Arc<RwLock<Box<dyn CompletionProvider + Send + Sync>>>,
    backup_providers: Vec<Box<dyn CompletionProvider + Send + Sync>>,
}

impl ProviderFactory {
    async fn new(api_key: String, system_prompt: String) -> Result<Self, AppError> {
        // Initialize with DeepSeek as primary and others as backup
        let primary = Box::new(DeepSeekProvider::new(api_key.clone(), system_prompt.clone()).await
            .map_err(|e| AppError::ProviderError(e.to_string()))?);
            
        let mut backup_providers: Vec<Box<dyn CompletionProvider + Send + Sync>> = Vec::new();
        
        // Initialize backup providers
        if let Ok(provider) = OpenAIProvider::new(api_key.clone(), system_prompt.clone()).await {
            backup_providers.push(Box::new(provider) as Box<dyn CompletionProvider + Send + Sync>);
        }
        if let Ok(provider) = MistralProvider::new(api_key.clone(), system_prompt.clone()).await {
            backup_providers.push(Box::new(provider) as Box<dyn CompletionProvider + Send + Sync>);
        }
        
        Ok(Self {
            api_key,
            system_prompt,
            active_provider: Arc::new(RwLock::new(primary)),
            backup_providers,
        })
    }
    
    async fn get_provider(&self) -> Box<dyn CompletionProvider + Send + Sync> {
        self.active_provider.read().await.as_ref().clone_box()
    }
    
    async fn health_check(&self) -> bool {
        let provider = self.active_provider.read().await;
        provider.as_ref().get_model_info().await.is_ok()
    }
    
    async fn fallback_if_needed(&self) -> Result<(), AppError> {
        if !self.health_check().await {
            let mut active = self.active_provider.write().await;
            
            // Try each backup provider
            for backup in &self.backup_providers {
                if backup.get_model_info().await.is_ok() {
                    *active = backup.clone_box();
                    return Ok(());
                }
            }
            
            return Err(AppError::ProviderError("All providers failed".to_string()));
        }
        Ok(())
    }
}

#[derive(Clone)]
struct MemoryMonitor {
    total_tokens: Arc<AtomicUsize>,
    last_cleanup: Arc<RwLock<SystemTime>>,
    max_tokens: usize,
    cleanup_interval: Duration,
    recent_context: Arc<RwLock<Vec<String>>>,
    context_window: usize,
}

impl MemoryMonitor {
    fn new(max_tokens: usize, cleanup_interval: Duration) -> Self {
        Self {
            total_tokens: Arc::new(AtomicUsize::new(0)),
            last_cleanup: Arc::new(RwLock::new(SystemTime::now())),
            max_tokens,
            cleanup_interval,
            recent_context: Arc::new(RwLock::new(Vec::new())),
            context_window: 20,  // Keep last 20 messages by default
        }
    }
    
    fn add_tokens(&self, tokens: usize) {
        self.total_tokens.fetch_add(tokens, Ordering::SeqCst);
    }
    
    fn get_total_tokens(&self) -> usize {
        self.total_tokens.load(Ordering::SeqCst)
    }
    
    async fn needs_cleanup(&self) -> bool {
        let last_cleanup = self.last_cleanup.read().await;
        let elapsed = last_cleanup.elapsed().unwrap_or(Duration::from_secs(0));
        
        elapsed >= self.cleanup_interval || self.get_total_tokens() >= self.max_tokens
    }
    
    async fn add_context(&self, message: String) {
        let mut context = self.recent_context.write().await;
        context.push(message);
        
        // Keep only the most recent messages within context window
        if context.len() > self.context_window {
            context.remove(0);
        }
    }
    
    async fn get_recent_context(&self) -> Vec<String> {
        self.recent_context.read().await.clone()
    }
    
    async fn perform_cleanup(&self, memory_manager: &MemoryManager) -> Result<(), AppError> {
        if self.needs_cleanup().await {
            let mut last_cleanup = self.last_cleanup.write().await;
            *last_cleanup = SystemTime::now();
            
            let recent_context = self.get_recent_context().await;
            let context_tokens = recent_context.iter()
                .map(|msg| msg.split_whitespace().count())
                .sum::<usize>();
            
            self.total_tokens.store(context_tokens, Ordering::SeqCst);
            
            // Use cleanup_old_memories instead of smart_cleanup
            memory_manager.cleanup_old_memories().await
                .map_err(|e| AppError::DatabaseError(format!("Memory cleanup failed: {}", e)))?;
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Initialize colored output
    colored::control::set_override(true);

    // Load environment variables
    dotenv().ok();

    // Parse command line arguments
    let args = Args::parse();

    if args.api {
        run_api_server(args).await
    } else {
        run_cli_mode(&args).await
    }
}

async fn run_cli_mode(args: &Args) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Get API key from command line or environment
    let api_key = match &args.api_key {
        Some(key) => key.clone(),
        None => env::var("API_KEY").expect("API key must be provided via --api-key or API_KEY env var"),
    };

    // Initialize personality
    let personality = if let Some(character_file) = &args.character {
        if let Some(Personality::Dynamic(profile)) = load_personality_from_filename(character_file) {
            profile
        } else {
            match create_default_personality() {
                Personality::Dynamic(profile) => profile
            }
        }
    } else {
        match create_default_personality() {
            Personality::Dynamic(profile) => profile
        }
    };

    // Initialize provider factory instead of single provider
    let provider_factory = ProviderFactory::new(api_key, personality.generate_system_prompt()).await?;
    
    // Initialize database
    let db = Database::new("data/agent.db").await?
        .with_vector_db(&env::var("QDRANT_URL").unwrap_or_else(|_| "http://localhost:6333".to_string()))
        .await?;

    // Initialize knowledge base handler
    let knowledge_base_handler = KnowledgeBaseHandler::new("data/knowledge_base.json");

    // Initialize learning manager
    let learning_manager = LearningManager::new(db.clone(), knowledge_base_handler.clone());

    // Initialize memory monitor with context handling
    let memory_monitor = Arc::new(MemoryMonitor::new(
        1_000_000, // 1M tokens max
        Duration::from_secs(3600), // Cleanup every hour
    ));
    
    // Initialize memory manager with cloned VectorDB
    let vector_db = db.get_vector_db().await.ok_or("Failed to get vector database")?;
    let memory_manager = MemoryManager::new(Arc::new((*vector_db).clone())).await?;
    let memory_manager_clone = memory_manager.clone();
    
    // Start memory monitoring loop
    let memory_monitor_clone = memory_monitor.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(300)).await;
            if let Err(e) = memory_monitor_clone.perform_cleanup(&memory_manager_clone).await {
                eprintln!("Memory cleanup failed: {}", e);
            }
            
            let total_tokens = memory_monitor_clone.get_total_tokens();
            println!("Current memory usage: {} tokens", total_tokens);
        }
    });
    
    // Update command handler with provider
    let mut command_handler = CommandHandler::new(
        personality.clone(),
        if args.twitter {
            Some(ConversationManager::new(personality.clone()).await?)
        } else {
            None
        },
        if args.crawler {
            Some(WebCrawlerManager::new(personality.clone()).await?)
        } else {
            None
        },
        provider_factory.get_provider().await,
    ).await?;

    // Add message tracking (if CommandHandler supports it)
    let memory_monitor_clone = memory_monitor.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            // Track messages through memory monitor
            memory_monitor_clone.add_tokens(1); // Example token tracking
        }
    });

    // Start health check loop
    let provider_factory_clone = provider_factory.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(300)).await; // Check every 5 minutes
            if let Err(e) = provider_factory_clone.fallback_if_needed().await {
                eprintln!("Provider health check failed: {}", e);
            }
        }
    });

    // Show initial help menu
    command_handler.handle_command("help").await?;

    // Initialize rustyline editor
    let mut rl = Editor::<(), DefaultHistory>::new()?;

    // Main input loop
    loop {
        match rl.readline("👤 ") {
            Ok(line) => {
                let input = line.trim();
                rl.add_history_entry(input);

                if let Err(e) = command_handler.handle_command(input).await {
                    println!("{}", e.red());
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("CTRL-C");
                break;
            }
            Err(ReadlineError::Eof) => {
                println!("CTRL-D");
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }
    Ok(())
}

fn load_personality_from_filename(filename: &str) -> Option<Personality> {
    let path = Path::new("characters").join(filename);
    if path.exists() {
        if let Ok(file) = File::open(path) {
            if let Ok(profile) = serde_json::from_reader::<_, PersonalityProfile>(file) {
                return Some(Personality::Dynamic(profile));
            }
        }
    }
    None
}

fn create_default_personality() -> Personality {
    Personality::Dynamic(PersonalityProfile {
        name: "Helpful Assistant".to_string(),
        attributes: serde_json::json!({
            "description": "a helpful AI coding assistant",
            "style": "professional and technically precise",
            "expertise": "programming, software development, and technical problem-solving",
            "motto": "Always here to help with your coding needs",
            "example_code": [
                "```python\n# Example function\ndef greet(name):\n    return f'Hello, {name}!'\n```",
                "```rust\n// Example struct\nstruct User {\n    name: String,\n    age: u32\n}\n```"
            ]
        }),
    })
}

async fn run_api_server(args: Args) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr: SocketAddr = format!("0.0.0.0:{}", args.port)
        .parse()
        .expect("Failed to parse address");

    println!("Starting API server on {}", addr);

    // Get API key from command line or environment
    let api_key = match &args.api_key {
        Some(key) => key.clone(),
        None => env::var("API_KEY").expect("API key must be provided via --api-key or API_KEY env var"),
    };

    // Initialize personality
    let personality = if let Some(character_file) = &args.character {
        if let Some(Personality::Dynamic(profile)) = load_personality_from_filename(character_file) {
            profile
        } else {
            match create_default_personality() {
                Personality::Dynamic(profile) => profile
            }
        }
    } else {
        match create_default_personality() {
            Personality::Dynamic(profile) => profile
        }
    };

    // Initialize database
    let db = Database::new("data/agent.db").await?
        .with_vector_db(&env::var("QDRANT_URL").unwrap_or_else(|_| "http://localhost:6333".to_string()))
        .await?;

    println!("Initializing API routes...");

    // Create web crawler manager if enabled
    let crawler = if args.crawler {
        Some(WebCrawlerManager::new(personality.clone()).await?)
    } else {
        None
    };

    // Create memory manager with vector database
    let vector_db = db.get_vector_db().await.expect("Failed to get vector database");
    let memory_manager = MemoryManager::new(Arc::new((*vector_db).clone())).await?;

    // Create a new DeepSeek provider for the API
    let api_key = std::env::var("DEEPSEEK_API_KEY")
        .expect("DEEPSEEK_API_KEY environment variable not set");
    let deepseek_provider = DeepSeekProvider::new(api_key, personality.generate_system_prompt()).await?;

    let app = api::create_api(deepseek_provider, personality, db, crawler, memory_manager).await;

    println!("API routes configured, attempting to bind to address...");

    let listener = TcpListener::bind(&addr).await
        .map_err(|e| format!("Failed to bind to {}: {}", addr, e))?;

    println!("Server successfully bound to {}", addr);
    println!("Ready to accept connections!");

    axum::serve(listener, app)
        .await
        .map_err(|e| format!("Server error: {}", e))?;

    Ok(())
}
