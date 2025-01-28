use pdf_extract::extract_text;
use std::error::Error;

pub struct PdfExtractor;

impl PdfExtractor {
    pub fn new() -> Self {
        Self
    }

    pub fn extract_text(&self, file_path: &str) -> Result<String, Box<dyn Error>> {
        let text = extract_text(file_path)?;
        Ok(text)
    }
}
