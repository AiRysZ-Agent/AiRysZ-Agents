use thiserror::Error;

#[derive(Error, Debug)]
pub enum DocumentError {
    #[error("Invalid file extension")]
    InvalidExtension,
    
    #[error("File too large: {0} bytes")]
    FileTooLarge(u64),
    
    #[error("Unsupported file type: {0}")]
    UnsupportedFileType(String),
    
    #[error("PDF extraction error: {0}")]
    PdfError(String),
    
    #[error("Excel extraction error: {0}")]
    ExcelError(String),
    
    #[error("Word extraction error: {0}")]
    WordError(String),
    
    #[error("OCR extraction error: {0}")]
    OcrError(String),
    
    #[error("Text extraction error: {0}")]
    TextError(String),
    
    #[error("Insight extraction error: {0}")]
    InsightError(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Other error: {0}")]
    Other(String),
}

impl From<Box<dyn std::error::Error>> for DocumentError {
    fn from(err: Box<dyn std::error::Error>) -> Self {
        DocumentError::Other(err.to_string())
    }
}
