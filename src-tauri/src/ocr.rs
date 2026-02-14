//! OCR (Optical Character Recognition) module
//! 
//! Provides text extraction from handwritten input using Tesseract
//! or fallback pattern-based recognition.

use image::DynamicImage;
use serde::{Deserialize, Serialize};

/// A detected text region with its content and location
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextRegion {
    pub id: String,
    pub text: String,
    pub bounds: TextBounds,
    pub confidence: f64,
    pub font_size_estimate: f64,
}

/// Bounding box for text region
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// OCR configuration options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrConfig {
    pub language: String,
    pub mode: OcrMode,
    pub whitelist: Option<String>,
    pub min_confidence: f64,
}

impl Default for OcrConfig {
    fn default() -> Self {
        Self {
            language: "eng".to_string(),
            mode: OcrMode::Auto,
            whitelist: None,
            min_confidence: 0.5,
        }
    }
}

/// OCR processing mode
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OcrMode {
    Auto,
    SingleLine,
    SingleWord,
    SingleChar,
    SparseText,
}

/// Extract text from an image
/// 
/// This function attempts to use Tesseract if available,
/// otherwise falls back to basic pattern recognition.
pub fn extract_text(
    image: &DynamicImage,
    _width: u32,
    _height: u32,
) -> Vec<TextRegion> {
    // Try Tesseract OCR if feature is enabled
    #[cfg(feature = "ocr")]
    {
        if let Ok(regions) = extract_text_tesseract(image) {
            if !regions.is_empty() {
                return regions;
            }
        }
    }

    // Fallback: basic image analysis for text-like regions
    extract_text_fallback(image)
}

/// Extract text using Tesseract OCR
#[cfg(feature = "ocr")]
fn extract_text_tesseract(image: &DynamicImage) -> Result<Vec<TextRegion>, String> {
    use tesseract::Tesseract;

    // Convert image to grayscale PNG bytes
    let gray = image.to_luma8();
    let mut png_bytes = Vec::new();
    
    let encoder = image::codecs::png::PngEncoder::new(&mut png_bytes);
    image::ImageEncoder::write_image(
        encoder,
        gray.as_raw(),
        gray.width(),
        gray.height(),
        image::ExtendedColorType::L8,
    )
    .map_err(|e| format!("Failed to encode image: {}", e))?;

    // Initialize Tesseract
    let mut tess = Tesseract::new(None, Some("eng"))
        .map_err(|e| format!("Failed to initialize Tesseract: {}", e))?
        .set_image_from_mem(&png_bytes)
        .map_err(|e| format!("Failed to set image: {}", e))?;

    // Get text
    let text = tess
        .get_text()
        .map_err(|e| format!("Failed to get text: {}", e))?;

    // Parse results into regions
    let regions = parse_tesseract_output(&text, image.width() as f64, image.height() as f64);

    Ok(regions)
}

/// Parse Tesseract output into text regions
#[cfg(feature = "ocr")]
fn parse_tesseract_output(text: &str, img_width: f64, img_height: f64) -> Vec<TextRegion> {
    let mut regions = Vec::new();
    
    // Simple parsing - each non-empty line is a region
    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if !trimmed.is_empty() && trimmed.len() > 1 {
            regions.push(TextRegion {
                id: uuid::Uuid::new_v4().to_string(),
                text: trimmed.to_string(),
                bounds: TextBounds {
                    // Estimate position based on line number
                    x: img_width * 0.1,
                    y: img_height * 0.1 + (i as f64 * 30.0),
                    width: trimmed.len() as f64 * 10.0,
                    height: 20.0,
                },
                confidence: 0.7,
                font_size_estimate: 14.0,
            });
        }
    }

    regions
}

/// Fallback text extraction using basic image analysis
fn extract_text_fallback(image: &DynamicImage) -> Vec<TextRegion> {
    let gray = image.to_luma8();
    let (width, height) = gray.dimensions();
    
    // Find connected dark regions that might be text
    let text_regions = find_text_like_regions(&gray, width, height);
    
    text_regions
}

/// Find regions that look like they might contain text
fn find_text_like_regions(
    gray: &image::GrayImage,
    width: u32,
    height: u32,
) -> Vec<TextRegion> {
    let mut regions = Vec::new();
    
    // Simple approach: divide into grid and find cells with significant dark pixels
    let cell_width = 100u32;
    let cell_height = 40u32;
    
    for row in 0..(height / cell_height) {
        for col in 0..(width / cell_width) {
            let x = col * cell_width;
            let y = row * cell_height;
            
            // Count dark pixels in this cell
            let mut dark_pixels = 0;
            let mut total_pixels = 0;
            
            for py in y..(y + cell_height).min(height) {
                for px in x..(x + cell_width).min(width) {
                    let pixel = gray.get_pixel(px, py);
                    if pixel.0[0] < 128 {
                        dark_pixels += 1;
                    }
                    total_pixels += 1;
                }
            }
            
            // If there's a reasonable amount of dark pixels, might be text
            let density = dark_pixels as f64 / total_pixels as f64;
            if density > 0.05 && density < 0.5 {
                // Likely text region
                regions.push(TextRegion {
                    id: uuid::Uuid::new_v4().to_string(),
                    text: "[Handwritten text]".to_string(), // Placeholder
                    bounds: TextBounds {
                        x: x as f64,
                        y: y as f64,
                        width: cell_width as f64,
                        height: cell_height as f64,
                    },
                    confidence: density * 2.0,
                    font_size_estimate: estimate_font_size(cell_height as f64, density),
                });
            }
        }
    }

    // Merge adjacent regions
    merge_adjacent_regions(regions)
}

/// Estimate font size based on region dimensions and density
fn estimate_font_size(height: f64, density: f64) -> f64 {
    // Rough estimation
    let base_size = height * 0.7;
    let adjusted = base_size * (1.0 + density);
    adjusted.clamp(8.0, 72.0)
}

/// Merge adjacent text regions
fn merge_adjacent_regions(regions: Vec<TextRegion>) -> Vec<TextRegion> {
    if regions.is_empty() {
        return regions;
    }

    let mut merged: Vec<TextRegion> = Vec::new();
    let mut current: Option<TextRegion> = None;

    for region in regions {
        match current {
            None => {
                current = Some(region);
            }
            Some(ref mut curr) => {
                // Check if regions are adjacent horizontally
                let gap = region.bounds.x - (curr.bounds.x + curr.bounds.width);
                let same_row = (region.bounds.y - curr.bounds.y).abs() < curr.bounds.height * 0.5;
                
                if gap < 20.0 && gap > -10.0 && same_row {
                    // Merge regions
                    curr.bounds.width = region.bounds.x + region.bounds.width - curr.bounds.x;
                    curr.text = format!("{} {}", curr.text, region.text);
                    curr.confidence = (curr.confidence + region.confidence) / 2.0;
                } else {
                    merged.push(current.take().unwrap());
                    current = Some(region);
                }
            }
        }
    }

    if let Some(last) = current {
        merged.push(last);
    }

    merged
}

/// Enhanced text extraction with preprocessing
pub fn extract_text_enhanced(
    image: &DynamicImage,
    config: &OcrConfig,
) -> Vec<TextRegion> {
    // Preprocess image
    let processed = preprocess_for_ocr(image);
    
    // Extract text
    let mut regions = extract_text(&processed, image.width(), image.height());
    
    // Filter by confidence
    regions.retain(|r| r.confidence >= config.min_confidence);
    
    regions
}

/// Preprocess image for better OCR results
fn preprocess_for_ocr(image: &DynamicImage) -> DynamicImage {
    let gray = image.to_luma8();
    
    // Apply basic thresholding
    let mut processed = gray.clone();
    let threshold = 128u8;
    
    for pixel in processed.pixels_mut() {
        pixel.0[0] = if pixel.0[0] < threshold { 0 } else { 255 };
    }
    
    DynamicImage::ImageLuma8(processed)
}

/// Detect handwriting characteristics
pub fn analyze_handwriting_style(regions: &[TextRegion]) -> HandwritingStyle {
    if regions.is_empty() {
        return HandwritingStyle::default();
    }

    let avg_height: f64 = regions.iter().map(|r| r.bounds.height).sum::<f64>() / regions.len() as f64;
    let avg_confidence: f64 = regions.iter().map(|r| r.confidence).sum::<f64>() / regions.len() as f64;

    HandwritingStyle {
        estimated_font_size: avg_height * 0.7,
        legibility_score: avg_confidence,
        style: if avg_confidence > 0.7 {
            "neat".to_string()
        } else if avg_confidence > 0.4 {
            "casual".to_string()
        } else {
            "rough".to_string()
        },
    }
}

/// Handwriting style analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandwritingStyle {
    pub estimated_font_size: f64,
    pub legibility_score: f64,
    pub style: String,
}

impl Default for HandwritingStyle {
    fn default() -> Self {
        Self {
            estimated_font_size: 14.0,
            legibility_score: 0.5,
            style: "unknown".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_region_creation() {
        let region = TextRegion {
            id: "test".to_string(),
            text: "Hello".to_string(),
            bounds: TextBounds {
                x: 0.0,
                y: 0.0,
                width: 50.0,
                height: 20.0,
            },
            confidence: 0.9,
            font_size_estimate: 14.0,
        };
        
        assert_eq!(region.text, "Hello");
        assert_eq!(region.confidence, 0.9);
    }

    #[test]
    fn test_ocr_config_default() {
        let config = OcrConfig::default();
        assert_eq!(config.language, "eng");
        assert_eq!(config.min_confidence, 0.5);
    }

    #[test]
    fn test_estimate_font_size() {
        let size = estimate_font_size(30.0, 0.2);
        assert!(size >= 8.0 && size <= 72.0);
    }

    #[test]
    fn test_handwriting_style_default() {
        let style = HandwritingStyle::default();
        assert_eq!(style.style, "unknown");
    }
}
