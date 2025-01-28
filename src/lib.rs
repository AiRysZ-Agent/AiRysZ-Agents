pub mod api;
pub mod config;
pub mod database;
pub mod knowledge_base;
pub mod learning;
pub mod llm;
pub mod personality;
pub mod providers;
pub mod commands;
pub mod food;
// pub mod memory;
pub mod completion;

// Re-export commonly used items
pub use personality::PersonalityProfile;
pub use providers::web_crawler::crawler_manager::WebCrawlerManager;
pub use providers::document::DocumentProcessor; 