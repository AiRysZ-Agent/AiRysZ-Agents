use std::error::Error;
use std::fs::File;
use std::io::Read;

pub struct WordExtractor;

impl WordExtractor {
    pub fn new() -> Self {
        Self
    }

    pub fn extract_text(&self, file_path: &str) -> Result<String, Box<dyn Error>> {
        // Simple text extraction for now
        let mut file = File::open(file_path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        
        // Try to decode as UTF-8, fallback to lossy conversion
        String::from_utf8(buffer.clone())
            .map_err(|_| Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid UTF-8 sequence"
            )))
            .or_else(|_| Ok(String::from_utf8_lossy(&buffer).into_owned()))
    }
}
