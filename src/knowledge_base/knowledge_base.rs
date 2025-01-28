// src/knowledge_base/knowledge_base.rs
use serde::Deserialize;
use std::fs;
use tokio::fs as tokio_fs;
use serde_json;

#[derive(Debug, Deserialize, Clone)]
pub struct KnowledgeEntry {
    pub keywords: Vec<String>,
    pub content: String,
}

#[derive(Clone)]
pub struct KnowledgeBaseHandler {
    knowledge_base: Vec<KnowledgeEntry>,
    file_path: String,
}

impl KnowledgeBaseHandler {
    pub fn new(file_path: &str) -> Self {
        // Load the knowledge base from the file
        let knowledge_base = fs::read_to_string(file_path)
            .map_err(|e| {
                eprintln!("Failed to read knowledge base file: {}", e);
                std::process::exit(1);
            })
            .and_then(|data| {
                serde_json::from_str(&data).map_err(|e| {
                    eprintln!("Failed to parse knowledge base file: {}", e);
                    std::process::exit(1);
                })
            })
            .unwrap_or_else(|_| vec![]);

        Self {
            knowledge_base,
            file_path: file_path.to_string(),
        }
    }

    pub fn retrieve_information(&self, query: &str) -> String {
        // Tokenize the query into words
        let query_words: Vec<_> = query.split_whitespace().map(|s| s.to_lowercase()).collect();

        // Find entries with matching keywords
        let relevant_entries: Vec<_> = self
            .knowledge_base
            .iter()
            .filter(|entry| {
                entry
                    .keywords
                    .iter()
                    .any(|kw| query_words.contains(&kw.to_lowercase()))
            })
            .collect();

        // Combine the content of relevant entries
        relevant_entries
            .iter()
            .map(|entry| entry.content.clone())
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub async fn get_entry(&self, key: &str) -> Result<Option<String>, Box<dyn std::error::Error>> {
        let content = tokio_fs::read_to_string(&self.file_path).await?;
        let data: serde_json::Value = serde_json::from_str(&content)?;
        
        if let Some(value) = data.get(key) {
            if let Some(str_value) = value.as_str() {
                return Ok(Some(str_value.to_string()));
            }
        }
        
        Ok(None)
    }

    pub async fn add_entry(&self, key: &str, value: &str) -> Result<(), Box<dyn std::error::Error>> {
        let content = tokio_fs::read_to_string(&self.file_path).await?;
        let mut data: serde_json::Value = serde_json::from_str(&content)?;
        
        if let serde_json::Value::Object(ref mut map) = data {
            map.insert(key.to_string(), serde_json::Value::String(value.to_string()));
        }
        
        tokio_fs::write(&self.file_path, serde_json::to_string_pretty(&data)?).await?;
        Ok(())
    }

    pub async fn update_entry(&self, key: &str, value: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.add_entry(key, value).await
    }
}