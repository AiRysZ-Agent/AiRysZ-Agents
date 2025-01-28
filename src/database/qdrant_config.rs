use qdrant_client::{Qdrant, config::QdrantConfig};
use std::time::Duration;

pub async fn create_qdrant_client(url: &str) -> Result<Qdrant, Box<dyn std::error::Error>> {
    // Clean the URL
    let clean_url = if url.contains("://") {
        url.split("://").nth(1).unwrap_or(url).to_string()
    } else {
        url.to_string()
    };

    // Replace port 6333 with 6334 for gRPC if needed
    let grpc_url = if clean_url.ends_with(":6333") {
        clean_url.replace(":6333", ":6334")
    } else {
        clean_url
    };

    let url_with_scheme = format!("http://{}", grpc_url);
    log::info!("Attempting to connect to Qdrant with URL: {}", url_with_scheme);

    // Configure Qdrant
    let mut config = QdrantConfig::from_url(&url_with_scheme);
    config.check_compatibility = false;
    config.timeout = Duration::from_secs(30);
    config.connect_timeout = Duration::from_secs(10);
    
    let client = Qdrant::new(config)?;
    
    // Test the connection
    match client.list_collections().await {
        Ok(_) => {
            log::info!("Successfully connected to Qdrant");
            Ok(client)
        }
        Err(e) => {
            log::error!("Connection test failed: {}", e);
            Err(format!("Failed to connect to Qdrant: {}", e).into())
        }
    }
} 