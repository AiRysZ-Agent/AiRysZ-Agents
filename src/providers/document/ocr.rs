use image::DynamicImage;
use tesseract::Tesseract;
use std::ffi::CString;
use crate::providers::document::error::DocumentError;

pub struct OcrExtractor {
    tesseract: Tesseract,
    supported_formats: Vec<String>,
}

impl OcrExtractor {
    pub fn new() -> Result<Self, DocumentError> {
        let tesseract = Tesseract::new(None, Some("eng"))
            .map_err(|e| DocumentError::OcrError(e.to_string()))?;
            
        let supported_formats = vec![
            "jpg", "jpeg", "png", "gif", "bmp", "tiff",
            "webp", "ico", "tga"
        ].into_iter().map(String::from).collect();

        Ok(Self { 
            tesseract,
            supported_formats 
        })
    }

    pub fn is_supported(&self, extension: &str) -> bool {
        self.supported_formats.contains(&extension.to_lowercase())
    }

    pub fn extract_text(&self, file_path: &str) -> Result<String, DocumentError> {
        // Pre-process image for better OCR results
        let img = image::open(file_path)
            .map_err(|e| DocumentError::OcrError(format!("Failed to open image: {}", e)))?;
            
        // Enhance image for better OCR
        let processed = self.preprocess_image(img)?;
        
        // Convert to temporary file that Tesseract can read
        let temp_path = format!("{}.enhanced.png", file_path);
        processed.save(&temp_path)
            .map_err(|e| DocumentError::OcrError(e.to_string()))?;

        // Create new Tesseract instance for this operation
        let tesseract = Tesseract::new(None, Some("eng"))
            .map_err(|e| DocumentError::OcrError(e.to_string()))?;

        // Perform OCR
        let c_path = CString::new(temp_path.clone())
            .map_err(|e| DocumentError::OcrError(e.to_string()))?;

        let text = tesseract
            .set_image(c_path.to_str().unwrap())
            .map_err(|e| DocumentError::OcrError(e.to_string()))?
            .get_text()
            .map_err(|e| DocumentError::OcrError(e.to_string()))?;

        // Cleanup temporary file
        std::fs::remove_file(temp_path).ok();

        Ok(text)
    }

    fn preprocess_image(&self, img: DynamicImage) -> Result<DynamicImage, DocumentError> {
        // Convert to grayscale
        let gray = img.grayscale();
        
        // Increase contrast
        let contrast = gray.adjust_contrast(1.5);
        
        // Optional: Add more preprocessing steps like:
        // - Noise reduction
        // - Thresholding
        // - Deskewing
        // - Resolution adjustment

        Ok(contrast)
    }
}

impl Default for OcrExtractor {
    fn default() -> Self {
        Self::new().unwrap_or_else(|e| {
            eprintln!("Warning: Failed to create default OCR extractor: {}", e);
            Self {
                tesseract: Tesseract::new(None, Some("eng")).unwrap(),
                supported_formats: vec![
                    "jpg", "jpeg", "png", "gif", "bmp", "tiff",
                    "webp", "ico", "tga"
                ].into_iter().map(String::from).collect(),
            }
        })
    }
}
