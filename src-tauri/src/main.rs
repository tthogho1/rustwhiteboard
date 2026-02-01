//! RustWhiteboard - Hand-drawn diagram to draw.io converter
//! 
//! This module provides the main entry point and Tauri command handlers
//! for the whiteboard application.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod canvas;
mod drawio;
mod llm;
mod ocr;
mod shapes;

use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::State;

/// Application state shared across commands
pub struct AppState {
    /// Current canvas strokes
    pub strokes: Mutex<Vec<Stroke>>,
    /// Detected shapes from the canvas
    pub detected_shapes: Mutex<Vec<shapes::DetectedShape>>,
    /// OCR results
    pub ocr_text: Mutex<Vec<ocr::TextRegion>>,
    /// LLM configuration
    pub llm_config: Mutex<llm::LlmConfig>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            strokes: Mutex::new(Vec::new()),
            detected_shapes: Mutex::new(Vec::new()),
            ocr_text: Mutex::new(Vec::new()),
            llm_config: Mutex::new(llm::LlmConfig::default()),
        }
    }
}

/// A single point in a stroke
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
    pub pressure: Option<f64>,
    pub timestamp: u64,
}

/// A stroke consisting of multiple points
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stroke {
    pub id: String,
    pub points: Vec<Point>,
    pub color: String,
    pub width: f64,
    pub tool: String,
}

/// Result of diagram processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingResult {
    pub shapes: Vec<shapes::DetectedShape>,
    pub text_regions: Vec<ocr::TextRegion>,
    pub suggested_diagram_type: String,
    pub confidence: f64,
}

/// Draw.io export options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportOptions {
    pub filename: String,
    pub include_grid: bool,
    pub page_width: f64,
    pub page_height: f64,
    pub theme: String,
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// Receive strokes from the frontend canvas
#[tauri::command]
async fn add_stroke(
    state: State<'_, AppState>,
    stroke: Stroke,
) -> Result<(), String> {
    let mut strokes = state.strokes.lock().map_err(|e| e.to_string())?;
    strokes.push(stroke);
    Ok(())
}

/// Clear all strokes from the canvas
#[tauri::command]
async fn clear_strokes(state: State<'_, AppState>) -> Result<(), String> {
    let mut strokes = state.strokes.lock().map_err(|e| e.to_string())?;
    strokes.clear();
    let mut shapes = state.detected_shapes.lock().map_err(|e| e.to_string())?;
    shapes.clear();
    let mut text = state.ocr_text.lock().map_err(|e| e.to_string())?;
    text.clear();
    Ok(())
}

/// Get all current strokes
#[tauri::command]
async fn get_strokes(state: State<'_, AppState>) -> Result<Vec<Stroke>, String> {
    let strokes = state.strokes.lock().map_err(|e| e.to_string())?;
    Ok(strokes.clone())
}

/// Process the canvas strokes to detect shapes and text
#[tauri::command]
async fn process_canvas(
    state: State<'_, AppState>,
    image_data: String,
    width: u32,
    height: u32,
) -> Result<ProcessingResult, String> {
    // Decode base64 image data
    let image_bytes = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        &image_data.replace("data:image/png;base64,", ""),
    )
    .map_err(|e| format!("Failed to decode image: {}", e))?;

    // Convert to image
    let img = image::load_from_memory(&image_bytes)
        .map_err(|e| format!("Failed to load image: {}", e))?;

    // Get strokes for shape detection
    let strokes = state.strokes.lock().map_err(|e| e.to_string())?;

    // Detect shapes from strokes
    let detected_shapes = shapes::detect_shapes(&strokes);
    
    // Store detected shapes
    {
        let mut shapes_state = state.detected_shapes.lock().map_err(|e| e.to_string())?;
        *shapes_state = detected_shapes.clone();
    }

    // Perform OCR on the image
    let text_regions = ocr::extract_text(&img, width, height);
    
    // Store OCR results
    {
        let mut ocr_state = state.ocr_text.lock().map_err(|e| e.to_string())?;
        *ocr_state = text_regions.clone();
    }

    // Determine diagram type
    let (diagram_type, confidence) = shapes::classify_diagram(&detected_shapes, &text_regions);

    Ok(ProcessingResult {
        shapes: detected_shapes,
        text_regions,
        suggested_diagram_type: diagram_type,
        confidence,
    })
}

/// Use LLM to enhance and format the diagram structure
#[tauri::command]
async fn enhance_with_llm(
    state: State<'_, AppState>,
    prompt: Option<String>,
) -> Result<drawio::DiagramStructure, String> {
    // Clone state out of the mutexes so we don't hold MutexGuards across await points.
    let shapes = {
        let guard = state.detected_shapes.lock().map_err(|e| e.to_string())?;
        guard.clone()
    };
    let text_regions = {
        let guard = state.ocr_text.lock().map_err(|e| e.to_string())?;
        guard.clone()
    };
    let config = {
        let guard = state.llm_config.lock().map_err(|e| e.to_string())?;
        guard.clone()
    };

    let custom_prompt = prompt.unwrap_or_else(|| {
        "Convert this hand-drawn flowchart to a clean, structured UML diagram".to_string()
    });

    llm::enhance_diagram(&shapes, &text_regions, &custom_prompt, &config).await
}

/// Generate draw.io XML from the processed diagram
#[tauri::command]
async fn generate_drawio(
    state: State<'_, AppState>,
    options: ExportOptions,
) -> Result<String, String> {
    let shapes = state.detected_shapes.lock().map_err(|e| e.to_string())?;
    let text_regions = state.ocr_text.lock().map_err(|e| e.to_string())?;

    drawio::generate_xml(&shapes, &text_regions, &options)
}

/// Export the diagram to a .drawio file
#[tauri::command]
async fn export_drawio_file(
    state: State<'_, AppState>,
    path: String,
    options: ExportOptions,
) -> Result<(), String> {
    log::info!("🔹 export_drawio_file called with path: {}", path);

    let xml = generate_drawio(state, options).await?;
    log::info!("🔹 Generated {} bytes of XML", xml.len());

    std::fs::write(&path, &xml)
        .map_err(|e| {
            log::error!("❌ Failed to write file: {}", e);
            format!("Failed to write file: {}", e)
        })?;

    log::info!("✅ Successfully wrote file to {}", path);
    Ok(())
}

/// Configure LLM settings
#[tauri::command]
async fn configure_llm(
    state: State<'_, AppState>,
    config: llm::LlmConfig,
) -> Result<(), String> {
    let mut llm_config = state.llm_config.lock().map_err(|e| e.to_string())?;
    *llm_config = config;
    Ok(())
}

/// Save canvas state as JSON backup
#[tauri::command]
async fn save_backup(
    state: State<'_, AppState>,
    path: String,
) -> Result<(), String> {
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::Write;

    let strokes = state.strokes.lock().map_err(|e| e.to_string())?;
    let json = serde_json::to_string(&*strokes)
        .map_err(|e| format!("Failed to serialize: {}", e))?;

    let file = std::fs::File::create(&path)
        .map_err(|e| format!("Failed to create file: {}", e))?;
    let mut encoder = GzEncoder::new(file, Compression::default());
    encoder.write_all(json.as_bytes())
        .map_err(|e| format!("Failed to write: {}", e))?;
    encoder.finish()
        .map_err(|e| format!("Failed to finish compression: {}", e))?;

    Ok(())
}

/// Load canvas state from JSON backup
#[tauri::command]
async fn load_backup(
    state: State<'_, AppState>,
    path: String,
) -> Result<Vec<Stroke>, String> {
    use flate2::read::GzDecoder;
    use std::io::Read;

    let file = std::fs::File::open(&path)
        .map_err(|e| format!("Failed to open file: {}", e))?;
    let mut decoder = GzDecoder::new(file);
    let mut json = String::new();
    decoder.read_to_string(&mut json)
        .map_err(|e| format!("Failed to read: {}", e))?;

    let strokes: Vec<Stroke> = serde_json::from_str(&json)
        .map_err(|e| format!("Failed to deserialize: {}", e))?;

    let mut state_strokes = state.strokes.lock().map_err(|e| e.to_string())?;
    *state_strokes = strokes.clone();

    Ok(strokes)
}

/// Get application info
#[tauri::command]
fn get_app_info() -> serde_json::Value {
    serde_json::json!({
        "name": "RustWhiteboard",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "Hand-drawn diagram to draw.io converter",
        "features": {
            "ocr": cfg!(feature = "ocr"),
            "ollama": cfg!(feature = "ollama"),
        }
    })
}

fn main() {
    env_logger::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            add_stroke,
            clear_strokes,
            get_strokes,
            process_canvas,
            enhance_with_llm,
            generate_drawio,
            export_drawio_file,
            configure_llm,
            save_backup,
            load_backup,
            get_app_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_creation() {
        let point = Point {
            x: 100.0,
            y: 200.0,
            pressure: Some(0.5),
            timestamp: 12345,
        };
        assert_eq!(point.x, 100.0);
        assert_eq!(point.y, 200.0);
    }

    #[test]
    fn test_stroke_creation() {
        let stroke = Stroke {
            id: "test-1".to_string(),
            points: vec![],
            color: "#000000".to_string(),
            width: 2.0,
            tool: "pen".to_string(),
        };
        assert_eq!(stroke.id, "test-1");
    }
}
