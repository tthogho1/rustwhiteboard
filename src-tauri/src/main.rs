//! RustWhiteboard - Hand-drawn diagram to draw.io converter
//! 
//! This module provides the main entry point and Tauri command handlers
//! for the whiteboard application.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod canvas;
mod drawio;
mod import;
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
    /// Text typed with the frontend text tool.
    ///
    /// Kept apart from `ocr_text` because `process_canvas` overwrites that on
    /// every run — annotations are owned by the frontend and must survive
    /// re-analysis. The two are merged at export time.
    pub text_annotations: Mutex<Vec<TextAnnotation>>,
    /// LLM configuration
    pub llm_config: Mutex<llm::LlmConfig>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            strokes: Mutex::new(Vec::new()),
            detected_shapes: Mutex::new(Vec::new()),
            ocr_text: Mutex::new(Vec::new()),
            text_annotations: Mutex::new(Vec::new()),
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

/// Text typed with the frontend text tool.
///
/// Mirrors `TextAnnotation` in `src/store.ts`. `fontSize` keeps its camelCase
/// name (unlike the rest of the boundary) so annotations round-trip through
/// backups byte-for-byte instead of needing a conversion on each side.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextAnnotation {
    pub id: String,
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub color: String,
    #[serde(rename = "fontSize")]
    pub font_size: f64,
}

impl TextAnnotation {
    /// The shape the labelling and export code works in.
    fn to_text_region(&self) -> ocr::TextRegion {
        ocr::TextRegion {
            id: self.id.clone(),
            text: self.text.clone(),
            bounds: ocr::TextBounds {
                x: self.x,
                y: self.y,
                width: self.width,
                height: self.height,
            },
            // Typed text is exact, unlike an OCR guess.
            confidence: 1.0,
            font_size_estimate: self.font_size,
        }
    }
}

/// On-disk backup format.
///
/// v1 backups were a bare `Vec<Stroke>`; `load_backup` still reads those.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Backup {
    pub version: u32,
    pub strokes: Vec<Stroke>,
    #[serde(default)]
    pub text_annotations: Vec<TextAnnotation>,
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
    println!("[STROKE] add_stroke: id={}, points={}, tool={}", stroke.id, stroke.points.len(), stroke.tool);
    strokes.push(stroke);
    println!("[STROKE] Total strokes now: {}", strokes.len());
    Ok(())
}

/// Replace the backend stroke list with the frontend's current canvas.
///
/// The frontend store is the source of truth for what is on the canvas, so
/// replacing wholesale keeps repeated analysis runs idempotent. Pushing the
/// same strokes again with `add_stroke` would append duplicates.
#[tauri::command]
async fn sync_strokes(
    state: State<'_, AppState>,
    strokes: Vec<Stroke>,
) -> Result<usize, String> {
    let mut current = state.strokes.lock().map_err(|e| e.to_string())?;
    println!("[STROKE] sync_strokes: {} -> {} strokes", current.len(), strokes.len());
    *current = strokes;
    Ok(current.len())
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
    let mut annotations = state.text_annotations.lock().map_err(|e| e.to_string())?;
    annotations.clear();
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
    println!("[PROCESS] process_canvas called with image {}x{}", width, height);
    
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
    println!("[PROCESS] Found {} strokes in state", strokes.len());

    // Detect shapes from strokes
    let detected_shapes = shapes::detect_shapes(&strokes);
    println!("[PROCESS] Detected {} shapes", detected_shapes.len());
    
    // Store detected shapes
    {
        let mut shapes_state = state.detected_shapes.lock().map_err(|e| e.to_string())?;
        *shapes_state = detected_shapes.clone();
        println!("[PROCESS] Stored {} shapes in state", shapes_state.len());
    }

    // Perform OCR on the image
    let text_regions = ocr::extract_text(&img, width, height);
    println!("[PROCESS] Found {} text regions", text_regions.len());
    
    // Store OCR results
    {
        let mut ocr_state = state.ocr_text.lock().map_err(|e| e.to_string())?;
        *ocr_state = text_regions.clone();
    }

    // Determine diagram type
    let (diagram_type, confidence) = shapes::classify_diagram(&detected_shapes, &text_regions);
    println!("[PROCESS] Classified as {} with confidence {:.2}", diagram_type, confidence);

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

/// Overwrite the text of already-detected OCR regions, keyed by region id.
///
/// This is how corrections made in the preview reach the export: shape labels
/// in the generated XML come from `AppState.ocr_text`, so unedited regions keep
/// whatever OCR produced (`"[Handwritten text]"` when the `ocr` feature is off).
#[tauri::command]
async fn update_text_labels(
    state: State<'_, AppState>,
    labels: std::collections::HashMap<String, String>,
) -> Result<Vec<ocr::TextRegion>, String> {
    let mut regions = state.ocr_text.lock().map_err(|e| e.to_string())?;

    let mut updated = 0;
    for region in regions.iter_mut() {
        if let Some(text) = labels.get(&region.id) {
            region.text = text.clone();
            updated += 1;
        }
    }
    println!("[OCR] update_text_labels: {}/{} regions updated", updated, regions.len());

    Ok(regions.clone())
}

/// Replace the text typed with the frontend text tool.
///
/// Like `sync_strokes` this replaces wholesale: the frontend store owns the
/// annotations, so pushing them again must stay idempotent.
#[tauri::command]
async fn sync_text_annotations(
    state: State<'_, AppState>,
    annotations: Vec<TextAnnotation>,
) -> Result<usize, String> {
    let mut current = state.text_annotations.lock().map_err(|e| e.to_string())?;
    println!(
        "[TEXT] sync_text_annotations: {} -> {} annotations",
        current.len(),
        annotations.len()
    );
    *current = annotations;
    Ok(current.len())
}

/// Correct the type of an already-detected shape.
///
/// Detection is never going to be perfect on freehand input, so the preview
/// lets the user override a classification. Like the label corrections this
/// only survives while the analysis is not re-run (see `ensureAnalyzed`).
#[tauri::command]
async fn update_shape_type(
    state: State<'_, AppState>,
    shape_id: String,
    shape_type: shapes::ShapeType,
) -> Result<Vec<shapes::DetectedShape>, String> {
    let mut detected = state.detected_shapes.lock().map_err(|e| e.to_string())?;

    let shape = detected
        .iter_mut()
        .find(|s| s.id == shape_id)
        .ok_or_else(|| format!("No detected shape with id {}", shape_id))?;

    println!(
        "[PROCESS] update_shape_type: {} {:?} -> {:?}",
        shape_id, shape.shape_type, shape_type
    );
    shape.shape_type = shape_type;
    // A hand-picked type is certain by definition.
    shape.confidence = 1.0;

    Ok(detected.clone())
}

/// Drop a spurious detection.
///
/// Detection over-reports on freehand input — a stray mark becomes a
/// `freeform` shape and lands in the export. Removing it here keeps it out of
/// the generated XML without touching the strokes it came from. Like the other
/// corrections it lasts only until the analysis is re-run.
#[tauri::command]
async fn delete_shape(
    state: State<'_, AppState>,
    shape_id: String,
) -> Result<Vec<shapes::DetectedShape>, String> {
    let mut detected = state.detected_shapes.lock().map_err(|e| e.to_string())?;

    let before = detected.len();
    detected.retain(|s| s.id != shape_id);

    if detected.len() == before {
        return Err(format!("No detected shape with id {}", shape_id));
    }
    println!("[PROCESS] delete_shape: {} ({} -> {})", shape_id, before, detected.len());

    Ok(detected.clone())
}

/// Generate draw.io XML from the processed diagram
#[tauri::command]
async fn generate_drawio(
    state: State<'_, AppState>,
    options: ExportOptions,
) -> Result<String, String> {
    let shapes = state.detected_shapes.lock().map_err(|e| e.to_string())?;
    let ocr_regions = state.ocr_text.lock().map_err(|e| e.to_string())?;
    let annotations = state.text_annotations.lock().map_err(|e| e.to_string())?;

    // Typed text goes last so it wins the label slot when it overlaps an OCR
    // region for the same shape.
    let text_regions: Vec<ocr::TextRegion> = ocr_regions
        .iter()
        .cloned()
        .chain(annotations.iter().map(TextAnnotation::to_text_region))
        .collect();

    println!(
        "[DRAWIO] generate_drawio: {} shapes, {} ocr regions, {} annotations",
        shapes.len(),
        ocr_regions.len(),
        annotations.len()
    );
    for shape in shapes.iter() {
        println!("[DRAWIO]   Shape: {:?} at ({}, {}) {}x{}", 
            shape.shape_type, shape.bounds.x, shape.bounds.y, 
            shape.bounds.width, shape.bounds.height);
    }

    let result = drawio::generate_xml(&shapes, &text_regions, &options);
    match &result {
        Ok(xml) => println!("[DRAWIO] Generated XML length: {} bytes", xml.len()),
        Err(e) => println!("[DRAWIO] Error generating XML: {}", e),
    }
    result
}

/// Export the diagram to a .drawio file
#[tauri::command]
async fn export_drawio_file(
    state: State<'_, AppState>,
    path: String,
    options: ExportOptions,
) -> Result<(), String> {
    println!("[EXPORT] export_drawio_file called with path: {}", path);

    let xml = generate_drawio(state, options).await?;
    println!("[EXPORT] Generated {} bytes of XML", xml.len());
    println!("[EXPORT] XML preview:\n{}", &xml[..xml.len().min(500)]);

    std::fs::write(&path, &xml)
        .map_err(|e| {
            println!("[EXPORT] ❌ Failed to write file: {}", e);
            format!("Failed to write file: {}", e)
        })?;

    println!("[EXPORT] ✅ Successfully wrote file to {}", path);
    Ok(())
}

/// Write a PNG rendered by the frontend canvas.
///
/// The image arrives as a data URL because that is what `canvas.toDataURL`
/// produces; writing the bytes here keeps every export on the same path as
/// `export_drawio_file` and sidesteps the fs plugin's scope checks for a path
/// the user picked in a save dialog.
#[tauri::command]
async fn export_png_file(path: String, image_data: String) -> Result<(), String> {
    let base64_part = image_data
        .split_once("base64,")
        .map(|(_, data)| data)
        .unwrap_or(&image_data);

    let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, base64_part)
        .map_err(|e| format!("Failed to decode the image: {}", e))?;

    std::fs::write(&path, &bytes).map_err(|e| format!("Failed to write file: {}", e))?;
    println!("[EXPORT] wrote {} bytes of PNG to {}", bytes.len(), path);
    Ok(())
}

/// Write an SVG document built by the frontend.
#[tauri::command]
async fn export_svg_file(path: String, svg: String) -> Result<(), String> {
    std::fs::write(&path, svg.as_bytes()).map_err(|e| format!("Failed to write file: {}", e))?;
    println!("[EXPORT] wrote {} bytes of SVG to {}", svg.len(), path);
    Ok(())
}

/// Read a .drawio file back onto the canvas as strokes and text.
#[tauri::command]
async fn import_drawio_file(
    state: State<'_, AppState>,
    path: String,
) -> Result<import::CanvasData, String> {
    let xml = std::fs::read_to_string(&path).map_err(|e| format!("Failed to read file: {}", e))?;

    let data = import::parse_drawio(&xml)?;
    println!(
        "[IMPORT] {}: {} strokes, {} text annotations",
        path,
        data.strokes.len(),
        data.text_annotations.len()
    );

    // Mirror the new canvas into the backend so an export before the next
    // Analyze still has something to work from.
    {
        let mut strokes = state.strokes.lock().map_err(|e| e.to_string())?;
        *strokes = data.strokes.clone();
    }
    {
        let mut annotations = state.text_annotations.lock().map_err(|e| e.to_string())?;
        *annotations = data.text_annotations.clone();
    }
    {
        let mut shapes = state.detected_shapes.lock().map_err(|e| e.to_string())?;
        shapes.clear();
        let mut ocr = state.ocr_text.lock().map_err(|e| e.to_string())?;
        ocr.clear();
    }

    Ok(data)
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

    let backup = {
        let strokes = state.strokes.lock().map_err(|e| e.to_string())?;
        let annotations = state.text_annotations.lock().map_err(|e| e.to_string())?;
        Backup {
            version: 2,
            strokes: strokes.clone(),
            text_annotations: annotations.clone(),
        }
    };
    let json = serde_json::to_string(&backup)
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
) -> Result<Backup, String> {
    use flate2::read::GzDecoder;
    use std::io::Read;

    let file = std::fs::File::open(&path)
        .map_err(|e| format!("Failed to open file: {}", e))?;
    let mut decoder = GzDecoder::new(file);
    let mut json = String::new();
    decoder.read_to_string(&mut json)
        .map_err(|e| format!("Failed to read: {}", e))?;

    // v1 backups are a bare stroke array; fall back to that shape.
    let backup: Backup = match serde_json::from_str::<Backup>(&json) {
        Ok(backup) => backup,
        Err(_) => {
            let strokes: Vec<Stroke> = serde_json::from_str(&json)
                .map_err(|e| format!("Failed to deserialize: {}", e))?;
            println!("[BACKUP] loaded a v1 backup ({} strokes, no text)", strokes.len());
            Backup {
                version: 1,
                strokes,
                text_annotations: Vec::new(),
            }
        }
    };

    {
        let mut state_strokes = state.strokes.lock().map_err(|e| e.to_string())?;
        *state_strokes = backup.strokes.clone();
    }
    {
        let mut state_text = state.text_annotations.lock().map_err(|e| e.to_string())?;
        *state_text = backup.text_annotations.clone();
    }

    Ok(backup)
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
            sync_strokes,
            clear_strokes,
            get_strokes,
            process_canvas,
            enhance_with_llm,
            update_text_labels,
            sync_text_annotations,
            update_shape_type,
            delete_shape,
            generate_drawio,
            export_drawio_file,
            export_png_file,
            export_svg_file,
            import_drawio_file,
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
    fn test_v1_backup_without_text_still_loads() {
        // v1 files are a bare stroke array, which must not fail the v2 struct.
        // Extra hashes: the JSON itself contains a `"#` sequence.
        let legacy = r##"[{"id":"s1","points":[],"color":"#000","width":2.0,"tool":"pen"}]"##;

        assert!(serde_json::from_str::<Backup>(legacy).is_err());

        let strokes: Vec<Stroke> = serde_json::from_str(legacy).unwrap();
        assert_eq!(strokes.len(), 1);
    }

    #[test]
    fn test_backup_round_trips_text_annotations() {
        let backup = Backup {
            version: 2,
            strokes: vec![],
            text_annotations: vec![TextAnnotation {
                id: "a1".to_string(),
                text: "Start".to_string(),
                x: 10.0,
                y: 20.0,
                width: 40.0,
                height: 18.0,
                color: "#000000".to_string(),
                font_size: 16.0,
            }],
        };

        let json = serde_json::to_string(&backup).unwrap();
        // The frontend spells this one camelCase; keep it that way on disk.
        assert!(json.contains("\"fontSize\":16.0"));

        let parsed: Backup = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.text_annotations[0].text, "Start");
        assert_eq!(parsed.text_annotations[0].font_size, 16.0);
    }

    #[test]
    fn test_annotation_becomes_a_text_region() {
        let annotation = TextAnnotation {
            id: "a1".to_string(),
            text: "Done".to_string(),
            x: 5.0,
            y: 6.0,
            width: 30.0,
            height: 20.0,
            color: "#ff0000".to_string(),
            font_size: 14.0,
        };

        let region = annotation.to_text_region();
        assert_eq!(region.id, "a1");
        assert_eq!(region.bounds.x, 5.0);
        assert_eq!(region.bounds.height, 20.0);
        assert_eq!(region.font_size_estimate, 14.0);
        assert_eq!(region.confidence, 1.0);
    }

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
