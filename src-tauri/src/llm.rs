//! LLM (Large Language Model) integration module
//! 
//! Provides diagram enhancement and formatting using local LLM inference
//! or optional Ollama API integration.

use crate::drawio::DiagramStructure;
use crate::ocr::TextRegion;
use crate::shapes::DetectedShape;
use serde::{Deserialize, Serialize};

/// LLM configuration options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub backend: LlmBackend,
    pub model_path: Option<String>,
    pub model_name: String,
    pub temperature: f32,
    pub max_tokens: usize,
    pub context_size: usize,
    pub ollama_url: Option<String>,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            backend: LlmBackend::Builtin,
            model_path: None,
            model_name: "llama3-8b-q4".to_string(),
            temperature: 0.7,
            max_tokens: 2048,
            context_size: 4096,
            ollama_url: Some("http://localhost:11434".to_string()),
        }
    }
}

/// LLM backend options
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LlmBackend {
    Builtin,   // Built-in rule-based processing
    Local,     // Local GGUF model via llm crate
    Ollama,    // Ollama API
    Disabled,  // No LLM processing
}

/// LLM inference result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    pub content: String,
    pub tokens_used: usize,
    pub processing_time_ms: u64,
}

/// Enhance diagram structure using LLM
pub async fn enhance_diagram(
    shapes: &[DetectedShape],
    text_regions: &[TextRegion],
    prompt: &str,
    config: &LlmConfig,
) -> Result<DiagramStructure, String> {
    // Build context from detected elements
    let context = build_diagram_context(shapes, text_regions);
    
    match config.backend {
        LlmBackend::Builtin => {
            // Use built-in rule-based enhancement
            enhance_with_rules(shapes, text_regions, &context)
        }
        LlmBackend::Local => {
            // Use local GGUF model
            enhance_with_local_llm(shapes, text_regions, prompt, &context, config).await
        }
        LlmBackend::Ollama => {
            // Use Ollama API
            #[cfg(feature = "ollama")]
            {
                enhance_with_ollama(shapes, text_regions, prompt, &context, config).await
            }
            #[cfg(not(feature = "ollama"))]
            {
                Err("Ollama feature not enabled".to_string())
            }
        }
        LlmBackend::Disabled => {
            // Return basic structure without enhancement
            Ok(create_basic_structure(shapes, text_regions))
        }
    }
}

/// Build context string from detected elements
fn build_diagram_context(shapes: &[DetectedShape], text_regions: &[TextRegion]) -> String {
    let mut context = String::new();
    
    context.push_str("Detected shapes:\n");
    for (i, shape) in shapes.iter().enumerate() {
        context.push_str(&format!(
            "  {}. {:?} at ({:.0}, {:.0}) size {:.0}x{:.0} (confidence: {:.2})\n",
            i + 1,
            shape.shape_type,
            shape.bounds.x,
            shape.bounds.y,
            shape.bounds.width,
            shape.bounds.height,
            shape.confidence
        ));
    }
    
    context.push_str("\nDetected text:\n");
    for (i, text) in text_regions.iter().enumerate() {
        context.push_str(&format!(
            "  {}. \"{}\" at ({:.0}, {:.0}) (confidence: {:.2})\n",
            i + 1,
            text.text,
            text.bounds.x,
            text.bounds.y,
            text.confidence
        ));
    }
    
    context
}

/// Rule-based diagram enhancement
fn enhance_with_rules(
    shapes: &[DetectedShape],
    text_regions: &[TextRegion],
    _context: &str,
) -> Result<DiagramStructure, String> {
    let mut structure = DiagramStructure {
        diagram_type: detect_diagram_type(shapes, text_regions),
        nodes: Vec::new(),
        edges: Vec::new(),
        metadata: DiagramMetadata::default(),
    };

    // Convert shapes to nodes
    for shape in shapes {
        if is_container_shape(&shape.shape_type) {
            let label = find_text_for_shape(shape, text_regions)
                .unwrap_or_else(|| "".to_string());
            
            structure.nodes.push(DiagramNode {
                id: shape.id.clone(),
                label,
                shape_type: map_shape_to_diagram_type(&shape.shape_type),
                x: shape.bounds.x,
                y: shape.bounds.y,
                width: shape.bounds.width.max(80.0),
                height: shape.bounds.height.max(40.0),
                style: get_default_style(&shape.shape_type),
            });
        }
    }

    // Convert arrows/lines to edges
    for shape in shapes {
        if is_connector_shape(&shape.shape_type) {
            if let (Some(start), Some(end)) = (
                &shape.properties.start_point,
                &shape.properties.end_point,
            ) {
                // Find connected nodes
                let source = find_node_at_point(&structure.nodes, *start);
                let target = find_node_at_point(&structure.nodes, *end);
                
                structure.edges.push(DiagramEdge {
                    id: shape.id.clone(),
                    source: source.unwrap_or_default(),
                    target: target.unwrap_or_default(),
                    label: None,
                    style: get_edge_style(&shape.shape_type),
                });
            }
        }
    }

    // Apply layout improvements
    improve_layout(&mut structure);

    Ok(structure)
}

/// Enhance diagram using local LLM
async fn enhance_with_local_llm(
    shapes: &[DetectedShape],
    text_regions: &[TextRegion],
    prompt: &str,
    context: &str,
    _config: &LlmConfig,
) -> Result<DiagramStructure, String> {
    // For now, fall back to rule-based enhancement
    // Full LLM integration would require loading GGUF model
    log::info!("Local LLM requested but falling back to rules");
    log::info!("Prompt: {}", prompt);
    log::info!("Context: {}", context);
    
    // In a full implementation, this would:
    // 1. Load the GGUF model if not already loaded
    // 2. Create a prompt combining the user prompt and context
    // 3. Run inference to get structured output
    // 4. Parse the LLM output into DiagramStructure
    
    enhance_with_rules(shapes, text_regions, context)
}

/// Enhance diagram using Ollama API
#[cfg(feature = "ollama")]
async fn enhance_with_ollama(
    shapes: &[DetectedShape],
    text_regions: &[TextRegion],
    prompt: &str,
    context: &str,
    config: &LlmConfig,
) -> Result<DiagramStructure, String> {
    let url = config.ollama_url.as_ref()
        .ok_or("Ollama URL not configured")?;
    
    let full_prompt = format!(
        "{}\n\nContext:\n{}\n\nUser request: {}\n\nRespond with a JSON structure describing the diagram.",
        SYSTEM_PROMPT,
        context,
        prompt
    );

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/api/generate", url))
        .json(&serde_json::json!({
            "model": config.model_name,
            "prompt": full_prompt,
            "stream": false,
            "options": {
                "temperature": config.temperature,
                "num_predict": config.max_tokens
            }
        }))
        .send()
        .await
        .map_err(|e| format!("Ollama request failed: {}", e))?;

    let result: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Ollama response: {}", e))?;

    let content = result["response"]
        .as_str()
        .ok_or("No response content")?;

    // Try to parse as JSON, fall back to rules if parsing fails
    parse_llm_output(content, shapes, text_regions)
        .or_else(|_| enhance_with_rules(shapes, text_regions, context))
}

/// System prompt for diagram enhancement
const SYSTEM_PROMPT: &str = r#"You are an expert at converting hand-drawn diagrams into structured formats.
Given the detected shapes and text, create a clean, organized diagram structure.

Rules:
1. Rectangles with text are process nodes
2. Diamonds are decision nodes (yes/no branches)
3. Circles/ovals at start/end are terminal nodes
4. Arrows indicate flow direction
5. Group related elements
6. Maintain logical flow (typically top-to-bottom or left-to-right)

Output format: JSON with nodes (id, label, type, x, y, width, height) and edges (source, target, label)."#;

/// Parse LLM output into diagram structure
fn parse_llm_output(
    content: &str,
    shapes: &[DetectedShape],
    text_regions: &[TextRegion],
) -> Result<DiagramStructure, String> {
    // Try to find JSON in the response
    let json_start = content.find('{');
    let json_end = content.rfind('}');
    
    if let (Some(start), Some(end)) = (json_start, json_end) {
        let json_str = &content[start..=end];
        if let Ok(parsed) = serde_json::from_str::<DiagramStructure>(json_str) {
            return Ok(parsed);
        }
    }
    
    // Fall back to rule-based if parsing fails
    Err("Failed to parse LLM output as JSON".to_string())
}

/// Create basic structure without enhancement
fn create_basic_structure(
    shapes: &[DetectedShape],
    text_regions: &[TextRegion],
) -> DiagramStructure {
    let mut structure = DiagramStructure {
        diagram_type: "generic".to_string(),
        nodes: Vec::new(),
        edges: Vec::new(),
        metadata: DiagramMetadata::default(),
    };

    for shape in shapes {
        if is_container_shape(&shape.shape_type) {
            let label = find_text_for_shape(shape, text_regions)
                .unwrap_or_default();
            
            structure.nodes.push(DiagramNode {
                id: shape.id.clone(),
                label,
                shape_type: format!("{:?}", shape.shape_type).to_lowercase(),
                x: shape.bounds.x,
                y: shape.bounds.y,
                width: shape.bounds.width,
                height: shape.bounds.height,
                style: String::new(),
            });
        }
    }

    structure
}

/// Detect overall diagram type
fn detect_diagram_type(shapes: &[DetectedShape], text_regions: &[TextRegion]) -> String {
    use crate::shapes::ShapeType;
    
    let has_diamonds = shapes.iter().any(|s| s.shape_type == ShapeType::Diamond);
    let has_arrows = shapes.iter().any(|s| s.shape_type == ShapeType::Arrow);
    let has_rectangles = shapes.iter().any(|s| s.shape_type == ShapeType::Rectangle);
    
    let text_lower: String = text_regions
        .iter()
        .map(|t| t.text.to_lowercase())
        .collect::<Vec<_>>()
        .join(" ");
    
    if has_diamonds && has_arrows {
        "flowchart".to_string()
    } else if text_lower.contains("class") || text_lower.contains("interface") {
        "uml_class".to_string()
    } else if has_rectangles && has_arrows {
        "block_diagram".to_string()
    } else {
        "generic".to_string()
    }
}

/// Check if shape type is a container (can hold content)
fn is_container_shape(shape_type: &crate::shapes::ShapeType) -> bool {
    use crate::shapes::ShapeType;
    matches!(
        shape_type,
        ShapeType::Rectangle
            | ShapeType::Circle
            | ShapeType::Ellipse
            | ShapeType::Diamond
            | ShapeType::Triangle
    )
}

/// Check if shape type is a connector
fn is_connector_shape(shape_type: &crate::shapes::ShapeType) -> bool {
    use crate::shapes::ShapeType;
    matches!(
        shape_type,
        ShapeType::Arrow | ShapeType::Line | ShapeType::Connector
    )
}

/// Map shape type to diagram node type
fn map_shape_to_diagram_type(shape_type: &crate::shapes::ShapeType) -> String {
    use crate::shapes::ShapeType;
    match shape_type {
        ShapeType::Rectangle => "process".to_string(),
        ShapeType::Diamond => "decision".to_string(),
        ShapeType::Circle | ShapeType::Ellipse => "terminator".to_string(),
        ShapeType::Triangle => "data".to_string(),
        _ => "shape".to_string(),
    }
}

/// Get default style for shape
fn get_default_style(shape_type: &crate::shapes::ShapeType) -> String {
    use crate::shapes::ShapeType;
    match shape_type {
        ShapeType::Rectangle => "rounded=0;whiteSpace=wrap;html=1;".to_string(),
        ShapeType::Diamond => "rhombus;whiteSpace=wrap;html=1;".to_string(),
        ShapeType::Circle => "ellipse;whiteSpace=wrap;html=1;aspect=fixed;".to_string(),
        ShapeType::Ellipse => "ellipse;whiteSpace=wrap;html=1;".to_string(),
        _ => "whiteSpace=wrap;html=1;".to_string(),
    }
}

/// Get edge style based on shape type
fn get_edge_style(shape_type: &crate::shapes::ShapeType) -> String {
    use crate::shapes::ShapeType;
    match shape_type {
        ShapeType::Arrow => {
            "edgeStyle=orthogonalEdgeStyle;rounded=0;orthogonalLoop=1;jettySize=auto;html=1;endArrow=classic;".to_string()
        }
        ShapeType::Line => {
            "edgeStyle=orthogonalEdgeStyle;rounded=0;orthogonalLoop=1;jettySize=auto;html=1;endArrow=none;".to_string()
        }
        _ => "edgeStyle=orthogonalEdgeStyle;html=1;".to_string(),
    }
}

/// Find text that belongs to a shape
fn find_text_for_shape(shape: &DetectedShape, text_regions: &[TextRegion]) -> Option<String> {
    // Find text regions that overlap with the shape bounds
    let mut matching_texts = Vec::new();
    
    for text in text_regions {
        if bounds_overlap(&shape.bounds, &text.bounds) {
            matching_texts.push(text.text.clone());
        }
    }
    
    if matching_texts.is_empty() {
        None
    } else {
        Some(matching_texts.join(" "))
    }
}

/// Check if two bounds overlap
fn bounds_overlap(shape_bounds: &crate::shapes::ShapeBounds, text_bounds: &crate::ocr::TextBounds) -> bool {
    let s_left = shape_bounds.x;
    let s_right = shape_bounds.x + shape_bounds.width;
    let s_top = shape_bounds.y;
    let s_bottom = shape_bounds.y + shape_bounds.height;
    
    let t_left = text_bounds.x;
    let t_right = text_bounds.x + text_bounds.width;
    let t_top = text_bounds.y;
    let t_bottom = text_bounds.y + text_bounds.height;
    
    s_left < t_right && s_right > t_left && s_top < t_bottom && s_bottom > t_top
}

/// Find node at a given point
fn find_node_at_point(nodes: &[DiagramNode], point: (f64, f64)) -> Option<String> {
    for node in nodes {
        if point.0 >= node.x
            && point.0 <= node.x + node.width
            && point.1 >= node.y
            && point.1 <= node.y + node.height
        {
            return Some(node.id.clone());
        }
    }
    
    // Find nearest node within threshold
    let threshold = 50.0;
    let mut nearest: Option<(String, f64)> = None;
    
    for node in nodes {
        let center_x = node.x + node.width / 2.0;
        let center_y = node.y + node.height / 2.0;
        let dist = ((point.0 - center_x).powi(2) + (point.1 - center_y).powi(2)).sqrt();
        
        if dist < threshold {
            if nearest.is_none() || dist < nearest.as_ref().unwrap().1 {
                nearest = Some((node.id.clone(), dist));
            }
        }
    }
    
    nearest.map(|(id, _)| id)
}

/// Improve layout by aligning and spacing elements
fn improve_layout(structure: &mut DiagramStructure) {
    if structure.nodes.is_empty() {
        return;
    }
    
    // Grid alignment
    let grid_size = 20.0;
    
    for node in &mut structure.nodes {
        node.x = (node.x / grid_size).round() * grid_size;
        node.y = (node.y / grid_size).round() * grid_size;
        node.width = (node.width / grid_size).round() * grid_size;
        node.height = (node.height / grid_size).round() * grid_size;
        
        // Ensure minimum size
        node.width = node.width.max(80.0);
        node.height = node.height.max(40.0);
    }
}

// Re-export types from drawio module
pub use crate::drawio::{DiagramMetadata, DiagramNode, DiagramEdge};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llm_config_default() {
        let config = LlmConfig::default();
        assert_eq!(config.backend, LlmBackend::Builtin);
        assert_eq!(config.temperature, 0.7);
    }

    #[test]
    fn test_build_context() {
        let shapes = vec![];
        let text_regions = vec![];
        let context = build_diagram_context(&shapes, &text_regions);
        assert!(context.contains("Detected shapes:"));
        assert!(context.contains("Detected text:"));
    }

    #[test]
    fn test_is_container_shape() {
        use crate::shapes::ShapeType;
        assert!(is_container_shape(&ShapeType::Rectangle));
        assert!(is_container_shape(&ShapeType::Circle));
        assert!(!is_container_shape(&ShapeType::Arrow));
    }

    #[test]
    fn test_is_connector_shape() {
        use crate::shapes::ShapeType;
        assert!(is_connector_shape(&ShapeType::Arrow));
        assert!(is_connector_shape(&ShapeType::Line));
        assert!(!is_connector_shape(&ShapeType::Rectangle));
    }
}
