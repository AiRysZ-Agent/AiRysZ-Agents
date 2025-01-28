use axum::{
    routing::{get, post},
    Router,
    Json,
    extract::State,
    response::{IntoResponse, Response},
    http::{Method, header, StatusCode},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::{CorsLayer, Any};
use std::error::Error;
use std::fmt;
use tokio::fs;
use tower::limit::RateLimitLayer;
use validator::Validate;
use anyhow;

use crate::personality::PersonalityProfile;
use crate::providers::deepseek::deepseek::DeepSeekProvider;
use crate::database::Database;
use crate::providers::web_crawler::crawler_manager::WebCrawlerManager;
use crate::providers::traits::CompletionProvider;
use crate::llm::memory::MemoryManager;
use crate::llm::EmbeddingGenerator;
use crate::providers::openai::openai::OpenAIProvider;
use crate::providers::openrouter::openrouter::OpenRouterProvider;
use crate::providers::mistral::mistral::MistralProvider;

#[derive(Debug, Deserialize, Clone)]
pub enum LLMProvider {
    DeepSeek,
    OpenAI,
    OpenRouter,
    Mistral,
}

impl Default for LLMProvider {
    fn default() -> Self {
        LLMProvider::DeepSeek
    }
}

#[derive(Clone)]
pub struct AppState {
    deepseek: Arc<DeepSeekProvider>,
    openai: Arc<RwLock<Option<OpenAIProvider>>>,
    openrouter: Arc<RwLock<Option<OpenRouterProvider>>>,
    mistral: Arc<RwLock<Option<MistralProvider>>>,
    personality: Arc<RwLock<PersonalityProfile>>,
    db: Arc<Database>,
    crawler: Arc<RwLock<Option<WebCrawlerManager>>>,
    memory: Arc<RwLock<MemoryManager>>,
    embedding_generator: Arc<EmbeddingGenerator>,
}

#[derive(Deserialize, Validate)]
pub struct ChatRequest {
    #[validate(length(min = 1, max = 1000))]
    message: String,
    #[validate(length(min = 1, max = 100))]
    character: Option<String>,
    #[serde(default)]
    provider: LLMProvider,
}

#[derive(Deserialize)]
pub struct CharacterRequest {
    character: String,
}

#[derive(Deserialize)]
pub struct WebRequest {
    command: String,
}

#[derive(Serialize)]
pub struct ChatResponse {
    response: String,
    tokens: TokenInfo,
}

#[derive(Serialize)]
pub struct TokenInfo {
    input: usize,
    response: usize,
    total: usize,
}

#[derive(Serialize)]
pub struct CharacterResponse {
    status: String,
}

#[derive(Serialize)]
struct ApiResponse {
    status: String,
}

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<ApiResponse>)>;

#[derive(Debug)]
struct ApiError(String);

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for ApiError {}

/// Create and configure the API router
pub async fn create_api(
    deepseek: DeepSeekProvider,
    personality: PersonalityProfile,
    db: Database,
    crawler: Option<WebCrawlerManager>,
    memory: MemoryManager,
) -> Router {
    // Create embedding generator
    let api_key = std::env::var("DEEPSEEK_API_KEY")
        .expect("DEEPSEEK_API_KEY must be set");
    let embedding_generator = EmbeddingGenerator::new(api_key).await
        .expect("Failed to create embedding generator");

    // Initialize optional providers
    let openai = if let Ok(api_key) = std::env::var("OPENAI_API_KEY") {
        Some(OpenAIProvider::new(api_key, "You are a helpful assistant.".to_string()).await
            .expect("Failed to create OpenAI provider"))
    } else {
        None
    };

    let openrouter = if let Ok(api_key) = std::env::var("OPENROUTER_API_KEY") {
        Some(OpenRouterProvider::new(api_key, "You are a helpful assistant.".to_string()).await
            .expect("Failed to create OpenRouter provider"))
    } else {
        None
    };

    let mistral = if let Ok(api_key) = std::env::var("MISTRAL_API_KEY") {
        Some(MistralProvider::new(api_key, "You are a helpful assistant.".to_string()).await
            .expect("Failed to create Mistral provider"))
    } else {
        None
    };

    let state = AppState {
        deepseek: Arc::new(deepseek),
        openai: Arc::new(RwLock::new(openai)),
        openrouter: Arc::new(RwLock::new(openrouter)),
        mistral: Arc::new(RwLock::new(mistral)),
        personality: Arc::new(RwLock::new(personality)),
        db: Arc::new(db),
        crawler: Arc::new(RwLock::new(crawler)),
        memory: Arc::new(RwLock::new(memory)),
        embedding_generator: Arc::new(embedding_generator),
    };

    println!("Setting up API server with CORS...");

    // Fully permissive CORS configuration
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any)
        .max_age(std::time::Duration::from_secs(3600));

    println!("CORS configured with permissive settings");

    // Create the router with middleware
    Router::new()
        .route("/chat", post(chat_handler))
        .route("/character", post(character_handler))
        .route("/health", get(health_check))
        .route("/web", post(web_handler))
        .layer(cors)
        .with_state(state)
}

async fn chat_handler(
    State(state): State<AppState>,
    Json(request): Json<ChatRequest>,
) -> Response {
    let input_tokens = request.message.split_whitespace().count();
    
    // Get recent conversations from database
    let recent_convos = match state.db.get_recent_conversations(5).await {
        Ok(convos) => convos,
        Err(e) => {
            eprintln!("Database error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse { status: "Database error".to_string() })
            ).into_response();
        }
    };
    
    // Get current personality and build context
    let personality = state.personality.read().await;
    println!("Generating response as character: {}", personality.name);
    
    // Get system prompt
    let system_prompt = personality.generate_system_prompt();

    // Select provider based on request
    let response = match request.provider {
        LLMProvider::DeepSeek => {
            match std::env::var("DEEPSEEK_API_KEY") {
                Ok(api_key) => {
                    match DeepSeekProvider::new(api_key, system_prompt).await {
                        Ok(provider) => provider.complete(&request.message).await,
                        Err(e) => Err(anyhow::Error::msg(format!("Failed to create DeepSeek provider: {}", e)))
                    }
                },
                Err(_) => Err(anyhow::Error::msg("DEEPSEEK_API_KEY not set"))
            }
        },
        LLMProvider::OpenAI => {
            let provider = state.openai.read().await;
            if let Some(provider) = provider.as_ref() {
                provider.complete(&request.message).await
            } else {
                Err(anyhow::Error::msg("OpenAI provider not initialized"))
            }
        },
        LLMProvider::OpenRouter => {
            let provider = state.openrouter.read().await;
            if let Some(provider) = provider.as_ref() {
                provider.complete(&request.message).await
            } else {
                Err(anyhow::Error::msg("OpenRouter provider not initialized"))
            }
        },
        LLMProvider::Mistral => {
            let provider = state.mistral.read().await;
            if let Some(provider) = provider.as_ref() {
                provider.complete(&request.message).await
            } else {
                Err(anyhow::Error::msg("Mistral provider not initialized"))
            }
        }
    };

    let response = match response {
        Ok(text) => text,
        Err(e) => {
            eprintln!("AI error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse { status: format!("AI error: {}", e) })
            ).into_response();
        }
    };

    let response_tokens = response.split_whitespace().count();
    
    // Save conversation to database with current personality
    if let Err(e) = state.db.save_conversation(
        request.message.clone(),
        response.clone(),
        personality.name.clone(),
    ).await {
        eprintln!("Warning: Failed to save conversation to database: {}", e);
    }

    // Store in memory with proper embeddings
    let mut memory = state.memory.write().await;
    let chat_text = format!("User: {}\nAI: {}", request.message, response);
    
    // Generate embedding for the chat
    let embedding = match state.embedding_generator.generate_embedding(&chat_text).await {
        Ok(emb) => emb,
        Err(e) => {
            eprintln!("Warning: Failed to generate embedding: {}", e);
            vec![0.0; 1536] // Fallback to zero vector
        }
    };

    if let Err(e) = memory.store_memory(
        &chat_text,
        "chat",
        embedding,
        None
    ).await {
        eprintln!("Warning: Failed to store memory: {}", e);
    }

    Json(ChatResponse {
        response,
        tokens: TokenInfo {
            input: input_tokens,
            response: response_tokens,
            total: input_tokens + response_tokens,
        },
    }).into_response()
}

async fn character_handler(
    State(mut state): State<AppState>,
    Json(request): Json<CharacterRequest>,
) -> Response {
    println!("Changing character to: {}", request.character);
    
    // Load character profile
    let file_path = format!("/root/RUSTV3-MULTILLM/characters/{}.json", request.character);
    let profile = match tokio::fs::read_to_string(&file_path).await {
        Ok(content) => {
            match serde_json::from_str::<PersonalityProfile>(&content) {
                Ok(profile) => profile,
                Err(e) => {
                    eprintln!("Error parsing character profile: {}", e);
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ApiResponse { status: format!("Error parsing character profile: {}", e) })
                    ).into_response();
                }
            }
        },
        Err(e) => {
            eprintln!("Error reading character file: {}", e);
            return (
                StatusCode::NOT_FOUND,
                Json(ApiResponse { status: format!("Character file not found: {}", e) })
            ).into_response();
        }
    };

    // Update the personality
    *state.personality.write().await = profile.clone();

    // Create new provider with updated character
    let api_key = match std::env::var("DEEPSEEK_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse { status: "DEEPSEEK_API_KEY not set".to_string() })
            ).into_response();
        }
    };

    let system_prompt = profile.generate_system_prompt();
    let new_provider = match DeepSeekProvider::new(api_key, system_prompt).await {
        Ok(provider) => provider,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse { status: format!("Failed to create provider: {}", e) })
            ).into_response();
        }
    };

    // Update the provider
    state.deepseek = Arc::new(new_provider);

    Json(CharacterResponse {
        status: "Character updated successfully".to_string(),
    }).into_response()
}

async fn health_check() -> Response {
    println!("Health check requested");
    Json(ApiResponse { 
        status: "Server is running and healthy".to_string() 
    }).into_response()
} 

async fn web_handler(
    State(state): State<AppState>,
    Json(request): Json<WebRequest>,
) -> Response {
    let command = request.command.as_str();
    
    let mut crawler = state.crawler.write().await;
    let mut memory = state.memory.write().await;
    let personality = state.personality.read().await;
    
    match handle_web_command(
        command,
        &mut crawler,
        &state.deepseek,
        &mut memory,
        &personality,
        &state.embedding_generator
    ).await {
        Ok(result) => Json(ApiResponse { 
            status: result 
        }).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse { status: e })
        ).into_response()
    }
}

async fn handle_web_command(
    command: &str,
    crawler: &mut Option<WebCrawlerManager>,
    provider: &DeepSeekProvider,
    memory: &mut MemoryManager,
    personality: &PersonalityProfile,
    embedding_generator: &EmbeddingGenerator,
) -> Result<String, String> {
    if let Some(crawler) = crawler {
        match command {
            s if s.starts_with("analyze ") => {
                let url = s.trim_start_matches("analyze ").trim();
                if url.is_empty() {
                    return Err("Please provide a URL to analyze.".to_string());
                }

                let content = crawler.analyze_url(url).await
                    .map_err(|e| format!("Failed to analyze webpage: {}", e))?;

                // Store the webpage content in memory with embedding
                let content_text = format!("Webpage being discussed: {}\nContent:\n{}", url, content);
                let content_embedding = embedding_generator.generate_embedding(&content_text).await
                    .map_err(|e| format!("Failed to generate embedding: {}", e))?;

                memory.store_memory(
                    &content_text,
                    "system",
                    content_embedding,
                    None
                ).await.map_err(|e| format!("Failed to store memory: {}", e))?;

                // Create new provider with current personality
                let system_prompt = personality.generate_system_prompt();
                let new_provider = provider.clone_with_prompt(&system_prompt);

                let analysis_prompt = format!(
                    "{}\n\n\
                    Analyze this webpage content and provide your unique perspective. \
                    Consider your personality traits and expertise. \
                    Be creative and stay true to your character's style:\n\n{}",
                    new_provider.get_system_message(),
                    content
                );

                let analysis = new_provider.complete(&analysis_prompt).await
                    .map_err(|e| format!("Failed to analyze content: {}", e))?;

                // Store the analysis in memory with embedding
                let analysis_text = format!("Analysis of webpage: {}\n{}", url, analysis);
                let analysis_embedding = embedding_generator.generate_embedding(&analysis_text).await
                    .map_err(|e| format!("Failed to generate embedding: {}", e))?;

                memory.store_memory(
                    &analysis_text,
                    "assistant",
                    analysis_embedding,
                    None
                ).await.map_err(|e| format!("Failed to store memory: {}", e))?;

                Ok(analysis)
            },
            s if s.starts_with("research ") => {
                let topic = s.trim_start_matches("research ").trim();
                if topic.is_empty() {
                    return Err("Please provide a topic to research.".to_string());
                }

                let results = crawler.research_topic(topic).await
                    .map_err(|e| format!("Failed to research topic: {}", e))?;

                // Store research request in memory
                memory.store_memory(
                    &format!("Research topic: {}", topic),
                    "user",
                    vec![0.0; 1536],
                    None
                ).await.map_err(|e| format!("Failed to store memory: {}", e))?;

                // Create new provider with current personality
                let system_prompt = personality.generate_system_prompt();
                let new_provider = provider.clone_with_prompt(&system_prompt);

                let research_prompt = format!(
                    "{}\n\n\
                    Analyze and synthesize the research about '{}' in your unique style. \
                    Structure your response in these sections:\n\
                    1. Key Findings (3-10 main points)\n\
                    2. Analysis (from your unique perspective)\n\
                    Keep each section focused and insightful. \
                    Stay true to your character's expertise and communication style.\n\n\
                    3. Then make a quick summary of all of these, short and insightful with your own unique style:\n{}",  
                    new_provider.get_system_message(),
                    topic,
                    results.join("\n")
                );

                let analysis = new_provider.complete(&research_prompt).await
                    .map_err(|e| format!("Failed to synthesize research: {}", e))?;

                // Store research results in memory
                memory.store_memory(
                    &format!("Research findings for {}: {}", topic, analysis),
                    "assistant",
                    vec![0.0; 1536],
                    None
                ).await.map_err(|e| format!("Failed to store memory: {}", e))?;

                Ok(analysis)
            },
            s if s.starts_with("links ") => {
                let url = s.trim_start_matches("links ").trim();
                if url.is_empty() {
                    return Err("Please provide a URL to extract links from.".to_string());
                }

                let links = crawler.extract_links(url).await
                    .map_err(|e| format!("Failed to extract links: {}", e))?;

                Ok(format!("Links found:\n{}", links.join("\n")))
            },
            _ => Err("Unknown web command. Available commands: analyze <url>, research <topic>, links <url>".to_string())
        }
    } else {
        Err("Web crawler not initialized. Use --crawler flag to enable web features.".to_string())
    }
} 