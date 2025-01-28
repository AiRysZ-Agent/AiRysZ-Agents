#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;
    use crate::providers::document::DocumentProcessor;

    #[tokio::test]
    async fn test_text_file_processing() {
        // Create a temporary text file
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "This is a test document.\nIt has multiple lines.\nTesting 1-2-3.").unwrap();
        
        let api_key = std::env::var("DEEPSEEK_API_KEY").expect("DEEPSEEK_API_KEY must be set");
        let mut processor = DocumentProcessor::new(
            api_key,
            "You are a document analyzer.".to_string()
        ).await.unwrap();

        let result = processor.process_document(file.path().to_str().unwrap()).await;
        assert!(result.is_ok());
        
        let insights = result.unwrap();
        assert!(!insights.is_empty());
    }

    #[tokio::test]
    async fn test_image_processing() {
        // Skip if no DEEPSEEK_API_KEY
        if std::env::var("DEEPSEEK_API_KEY").is_err() {
            println!("Skipping image test - DEEPSEEK_API_KEY not set");
            return;
        }

        let test_image = "test_docs/sample.jpg";
        if !std::path::Path::new(test_image).exists() {
            println!("Skipping image test - test image not found");
            return;
        }

        let api_key = std::env::var("DEEPSEEK_API_KEY").unwrap();
        let mut processor = DocumentProcessor::new(
            api_key,
            "You are a document analyzer.".to_string()
        ).await.unwrap();

        let result = processor.process_image(test_image).await;
        assert!(result.is_ok());
        
        let insights = result.unwrap();
        assert!(!insights.is_empty());
    }

    #[tokio::test]
    async fn test_file_info() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "Test content").unwrap();
        
        let path = file.path().to_str().unwrap();
        let metadata = std::fs::metadata(path).unwrap();
        
        assert!(metadata.len() > 0);
        assert!(metadata.is_file());
    }

    #[tokio::test]
    async fn test_unsupported_file() {
        let api_key = std::env::var("DEEPSEEK_API_KEY").unwrap_or_else(|_| "dummy_key".to_string());
        let mut processor = DocumentProcessor::new(
            api_key,
            "You are a document analyzer.".to_string()
        ).await.unwrap();

        let result = processor.process_document("test.unsupported").await;
        assert!(matches!(result, Err(DocumentError::UnsupportedFileType(_))));
    }
} 