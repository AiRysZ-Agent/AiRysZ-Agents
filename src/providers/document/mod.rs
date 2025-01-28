pub mod pdf;
pub mod excel;
pub mod word;
pub mod ocr;
pub mod insights;
pub mod error;
pub mod text;

pub use pdf::PdfExtractor;
pub use excel::ExcelExtractor;
pub use word::WordExtractor;
pub use ocr::OcrExtractor;
pub use insights::InsightExtractor;
pub use error::DocumentError;
pub use text::TextExtractor;

use indicatif::{ProgressBar, ProgressStyle};

pub struct DocumentProcessor {
    pdf_extractor: PdfExtractor,
    excel_extractor: ExcelExtractor,
    word_extractor: WordExtractor,
    ocr_extractor: OcrExtractor,
    text_extractor: TextExtractor,
    pub insight_extractor: InsightExtractor,
}

impl DocumentProcessor {
    const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024; // 10MB limit

    pub async fn new(api_key: String, system_message: String) -> Result<Self, DocumentError> {
        Ok(Self {
            pdf_extractor: PdfExtractor::new(),
            excel_extractor: ExcelExtractor::new(),
            word_extractor: WordExtractor::new(),
            ocr_extractor: OcrExtractor::new()
                .map_err(|e| DocumentError::OcrError(e.to_string()))?,
            text_extractor: TextExtractor::new(),
            insight_extractor: InsightExtractor::new(api_key, system_message)
                .await
                .map_err(|e| DocumentError::InsightError(e.to_string()))?,
        })
    }

    pub async fn process_document(&mut self, file_path: &str) -> Result<Vec<insights::Insight>, DocumentError> {
        let extension = std::path::Path::new(file_path)
            .extension()
            .and_then(|ext| ext.to_str())
            .ok_or(DocumentError::InvalidExtension)?;

        let text = match extension.to_lowercase().as_str() {
            "pdf" => self.pdf_extractor.extract_text(file_path)
                .map_err(|e| DocumentError::PdfError(e.to_string()))?,
            "xlsx" | "xls" => self.excel_extractor.extract_text(file_path)
                .map_err(|e| DocumentError::ExcelError(e.to_string()))?,
            "docx" | "doc" => self.word_extractor.extract_text(file_path)
                .map_err(|e| DocumentError::WordError(e.to_string()))?,
            "png" | "jpg" | "jpeg" => {
                let extractor = std::mem::replace(&mut self.ocr_extractor, OcrExtractor::default());
                extractor.extract_text(file_path)
            }
                .map_err(|e| DocumentError::OcrError(e.to_string()))?,
            "txt" | "md" | "rs" | "py" | "js" | "json" | "yaml" | "yml" => self.text_extractor.extract_text(file_path)
                .map_err(|e| DocumentError::TextError(e.to_string()))?,
            _ => return Err(DocumentError::UnsupportedFileType(extension.to_string())),
        };

        let insights = self.insight_extractor.extract_insights(&text).await
            .map_err(|e| DocumentError::InsightError(e.to_string()))?;
        Ok(insights)
    }

    pub async fn quick_analyze(&mut self, file_path: &str) -> Result<String, DocumentError> {
        let extension = std::path::Path::new(file_path)
            .extension()
            .and_then(|ext| ext.to_str())
            .ok_or(DocumentError::InvalidExtension)?;

        let text = match extension.to_lowercase().as_str() {
            "pdf" => self.pdf_extractor.extract_text(file_path)
                .map_err(|e| DocumentError::PdfError(e.to_string()))?,
            "xlsx" | "xls" => self.excel_extractor.extract_text(file_path)
                .map_err(|e| DocumentError::ExcelError(e.to_string()))?,
            "docx" | "doc" => self.word_extractor.extract_text(file_path)
                .map_err(|e| DocumentError::WordError(e.to_string()))?,
            "png" | "jpg" | "jpeg" => {
                let extractor = std::mem::replace(&mut self.ocr_extractor, OcrExtractor::default());
                extractor.extract_text(file_path)
            }
                .map_err(|e| DocumentError::OcrError(e.to_string()))?,
            "txt" | "md" | "rs" | "py" | "js" | "json" | "yaml" | "yml" => self.text_extractor.extract_text(file_path)
                .map_err(|e| DocumentError::TextError(e.to_string()))?,
            _ => return Err(DocumentError::UnsupportedFileType(extension.to_string())),
        };

        self.insight_extractor.quick_analyze(&text).await
            .map_err(|e| DocumentError::InsightError(e.to_string()))
    }

    async fn validate_file(&self, file_path: &str) -> Result<(), DocumentError> {
        let metadata = std::fs::metadata(file_path)
            .map_err(|e| DocumentError::IoError(e))?;
            
        if metadata.len() > Self::MAX_FILE_SIZE {
            return Err(DocumentError::FileTooLarge(metadata.len()));
        }
        Ok(())
    }

    pub async fn process_image(&mut self, file_path: &str) -> Result<Vec<insights::Insight>, DocumentError> {
        let pb = ProgressBar::new_spinner();
        pb.set_style(ProgressStyle::default_spinner()
            .template("{spinner:.green} [{elapsed_precise}] {msg}")
            .unwrap());

        // Validate file
        pb.set_message("Validating file...");
        self.validate_file(file_path).await?;

        // Extract text using OCR
        pb.set_message("Performing OCR...");
        let text = self.ocr_extractor.extract_text(file_path)
            .map_err(|e| DocumentError::OcrError(e.to_string()))?;

        // Generate insights
        pb.set_message("Analyzing content...");
        let insights = self.insight_extractor.extract_insights(&text).await
            .map_err(|e| DocumentError::InsightError(e.to_string()))?;

        pb.finish_with_message("Processing complete!");
        Ok(insights)
    }
}
