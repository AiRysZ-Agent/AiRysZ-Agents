use calamine::{open_workbook, Reader};
use std::error::Error;

pub struct ExcelExtractor;

impl ExcelExtractor {
    pub fn new() -> Self {
        Self
    }

    pub fn extract_text(&self, file_path: &str) -> Result<String, Box<dyn Error>> {
        let mut workbook: calamine::Xlsx<_> = open_workbook(file_path)?;
        let mut text = String::new();

        for sheet_name in workbook.sheet_names() {
            if let Some(Ok(range)) = workbook.worksheet_range(&sheet_name) {
                for row in range.rows() {
                    for cell in row {
                        text.push_str(&cell.to_string());
                        text.push(' ');
                    }
                    text.push('\n');
                }
            }
        }

        Ok(text)
    }
}
