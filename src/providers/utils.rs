use anyhow::Result;

/// Returns a placeholder embedding vector for testing purposes.
/// This should be replaced with proper embeddings in production.
pub async fn get_placeholder_embedding(_text: &str) -> Result<Vec<f32>> {
    // Return a dummy embedding vector of size 1536 (standard embedding dimension)
    Ok(vec![0.0; 1536])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_placeholder_embedding() {
        let result = get_placeholder_embedding("test text").await.unwrap();
        assert_eq!(result.len(), 1536);
        assert!(result.iter().all(|&x| x == 0.0));
    }
} 