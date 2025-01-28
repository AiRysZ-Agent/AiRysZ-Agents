use std::fs;
use std::io;

pub struct TextExtractor;

impl TextExtractor {
    pub fn new() -> Self {
        Self
    }

    pub fn extract_text(&self, file_path: &str) -> io::Result<String> {
        fs::read_to_string(file_path)
    }
}

impl Default for TextExtractor {
    fn default() -> Self {
        Self::new()
    }
} 