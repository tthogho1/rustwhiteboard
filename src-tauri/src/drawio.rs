//! Draw.io (mxGraph XML) generation module
//! 
//! Generates mxGraph XML format compatible with draw.io/diagrams.net
//! for exporting hand-drawn diagrams.

use crate::ocr::TextRegion;
use crate::shapes::DetectedShape;
use crate::ExportOptions;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Writer;
use serde::{Deserialize, Serialize};
use std::io::Cursor;

/// Diagram structure for export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagramStructure {
    pub diagram_type: String,
    pub nodes: Vec<DiagramNode>,
    pub edges: Vec<DiagramEdge>,
    pub metadata: DiagramMetadata,
}

/// A node in the diagram
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagramNode {
    pub id: String,
    pub label: String,
    pub shape_type: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub style: String,
}

/// An edge/connection in the diagram
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagramEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub label: Option<String>,
    pub style: String,
}

/// Diagram metadata
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DiagramMetadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub created_at: Option<String>,
    pub version: Option<String>,
}

/// Style presets for different shape types
pub struct StylePresets;

impl StylePresets {
    pub fn rectangle() -> &'static str {
        "rounded=0;whiteSpace=wrap;html=1;fillColor=#dae8fc;strokeColor=#6c8ebf;"
    }

    pub fn rounded_rectangle() -> &'static str {
        "rounded=1;whiteSpace=wrap;html=1;fillColor=#dae8fc;strokeColor=#6c8ebf;"
    }

    pub fn diamond() -> &'static str {
        "rhombus;whiteSpace=wrap;html=1;fillColor=#fff2cc;strokeColor=#d6b656;"
    }

    pub fn circle() -> &'static str {
        "ellipse;whiteSpace=wrap;html=1;aspect=fixed;fillColor=#d5e8d4;strokeColor=#82b366;"
    }

    pub fn ellipse() -> &'static str {
        "ellipse;whiteSpace=wrap;html=1;fillColor=#d5e8d4;strokeColor=#82b366;"
    }

    pub fn terminator() -> &'static str {
        "ellipse;whiteSpace=wrap;html=1;fillColor=#f8cecc;strokeColor=#b85450;"
    }

    pub fn arrow() -> &'static str {
        "edgeStyle=orthogonalEdgeStyle;rounded=0;orthogonalLoop=1;jettySize=auto;html=1;endArrow=classic;endFill=1;"
    }

    pub fn line() -> &'static str {
        "edgeStyle=orthogonalEdgeStyle;rounded=0;orthogonalLoop=1;jettySize=auto;html=1;endArrow=none;"
    }

    pub fn dashed_arrow() -> &'static str {
        "edgeStyle=orthogonalEdgeStyle;rounded=0;orthogonalLoop=1;jettySize=auto;html=1;endArrow=classic;endFill=1;dashed=1;"
    }
}

/// Generate mxGraph XML from detected shapes and text
pub fn generate_xml(
    shapes: &[DetectedShape],
    text_regions: &[TextRegion],
    options: &ExportOptions,
) -> Result<String, String> {
    let mut writer = Writer::new(Cursor::new(Vec::new()));

    // XML declaration
    writer
        .write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))
        .map_err(|e| e.to_string())?;

    // mxfile root element
    let mut mxfile = BytesStart::new("mxfile");
    mxfile.push_attribute(("host", "RustWhiteboard"));
    mxfile.push_attribute(("modified", chrono_timestamp().as_str()));
    mxfile.push_attribute(("agent", "RustWhiteboard/0.1.0"));
    mxfile.push_attribute(("version", "24.0.0"));
    mxfile.push_attribute(("type", "device"));
    writer
        .write_event(Event::Start(mxfile))
        .map_err(|e| e.to_string())?;

    // diagram element
    let diagram_id = uuid::Uuid::new_v4().to_string();
    let mut diagram = BytesStart::new("diagram");
    diagram.push_attribute(("id", diagram_id.as_str()));
    diagram.push_attribute(("name", &options.filename[..]));
    writer
        .write_event(Event::Start(diagram))
        .map_err(|e| e.to_string())?;

    // mxGraphModel
    let mut graph_model = BytesStart::new("mxGraphModel");
    graph_model.push_attribute(("dx", "0"));
    graph_model.push_attribute(("dy", "0"));
    graph_model.push_attribute(("grid", if options.include_grid { "1" } else { "0" }));
    graph_model.push_attribute(("gridSize", "10"));
    graph_model.push_attribute(("guides", "1"));
    graph_model.push_attribute(("tooltips", "1"));
    graph_model.push_attribute(("connect", "1"));
    graph_model.push_attribute(("arrows", "1"));
    graph_model.push_attribute(("fold", "1"));
    graph_model.push_attribute(("page", "1"));
    graph_model.push_attribute(("pageScale", "1"));
    let page_width = options.page_width.to_string();
    let page_height = options.page_height.to_string();
    graph_model.push_attribute(("pageWidth", page_width.as_str()));
    graph_model.push_attribute(("pageHeight", page_height.as_str()));
    graph_model.push_attribute(("math", "0"));
    graph_model.push_attribute(("shadow", "0"));
    writer
        .write_event(Event::Start(graph_model))
        .map_err(|e| e.to_string())?;

    // root element
    writer
        .write_event(Event::Start(BytesStart::new("root")))
        .map_err(|e| e.to_string())?;

    // Default parent cells (required by draw.io)
    write_cell(&mut writer, "0", "", "")?;
    write_cell_with_parent(&mut writer, "1", "0")?;

    // Cell ID counter
    let mut cell_id = 2;

    // Convert shapes to cells
    let shape_id_map = write_shapes(&mut writer, shapes, text_regions, &mut cell_id)?;

    // Write connectors
    write_connectors(&mut writer, shapes, &shape_id_map, &mut cell_id)?;

    // Close root
    writer
        .write_event(Event::End(BytesEnd::new("root")))
        .map_err(|e| e.to_string())?;

    // Close mxGraphModel
    writer
        .write_event(Event::End(BytesEnd::new("mxGraphModel")))
        .map_err(|e| e.to_string())?;

    // Close diagram
    writer
        .write_event(Event::End(BytesEnd::new("diagram")))
        .map_err(|e| e.to_string())?;

    // Close mxfile
    writer
        .write_event(Event::End(BytesEnd::new("mxfile")))
        .map_err(|e| e.to_string())?;

    let xml_bytes = writer.into_inner().into_inner();
    String::from_utf8(xml_bytes).map_err(|e| e.to_string())
}

/// Write a basic cell element
fn write_cell(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    id: &str,
    parent: &str,
    value: &str,
) -> Result<(), String> {
    let mut cell = BytesStart::new("mxCell");
    cell.push_attribute(("id", id));
    if !parent.is_empty() {
        cell.push_attribute(("parent", parent));
    }
    if !value.is_empty() {
        cell.push_attribute(("value", value));
    }
    writer
        .write_event(Event::Empty(cell))
        .map_err(|e| e.to_string())
}

/// Write a cell with just parent
fn write_cell_with_parent(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    id: &str,
    parent: &str,
) -> Result<(), String> {
    let mut cell = BytesStart::new("mxCell");
    cell.push_attribute(("id", id));
    cell.push_attribute(("parent", parent));
    writer
        .write_event(Event::Empty(cell))
        .map_err(|e| e.to_string())
}

/// Write shape cells and return a mapping of original IDs to cell IDs
fn write_shapes(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    shapes: &[DetectedShape],
    text_regions: &[TextRegion],
    cell_id: &mut i32,
) -> Result<std::collections::HashMap<String, String>, String> {
    use crate::shapes::ShapeType;
    let mut id_map = std::collections::HashMap::new();

    for shape in shapes {
        // Skip connector shapes (handled separately)
        if matches!(
            shape.shape_type,
            ShapeType::Arrow | ShapeType::Line | ShapeType::Connector
        ) {
            continue;
        }

        let current_id = cell_id.to_string();
        id_map.insert(shape.id.clone(), current_id.clone());

        // Find label text for this shape
        let label = find_label_for_shape(shape, text_regions);

        // Get style based on shape type
        let style = get_style_for_shape(&shape.shape_type);

        // Write cell with geometry
        write_shape_cell(
            writer,
            &current_id,
            "1",
            &label,
            &style,
            shape.bounds.x,
            shape.bounds.y,
            shape.bounds.width.max(80.0),
            shape.bounds.height.max(40.0),
        )?;

        *cell_id += 1;
    }

    Ok(id_map)
}

/// Write a shape cell with geometry
fn write_shape_cell(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    id: &str,
    parent: &str,
    value: &str,
    style: &str,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), String> {
    let mut cell = BytesStart::new("mxCell");
    cell.push_attribute(("id", id));
    cell.push_attribute(("value", value));
    cell.push_attribute(("style", style));
    cell.push_attribute(("vertex", "1"));
    cell.push_attribute(("parent", parent));
    writer
        .write_event(Event::Start(cell))
        .map_err(|e| e.to_string())?;

    // Geometry
    let mut geometry = BytesStart::new("mxGeometry");
    let x_str = x.to_string();
    let y_str = y.to_string();
    let w_str = width.to_string();
    let h_str = height.to_string();
    geometry.push_attribute(("x", x_str.as_str()));
    geometry.push_attribute(("y", y_str.as_str()));
    geometry.push_attribute(("width", w_str.as_str()));
    geometry.push_attribute(("height", h_str.as_str()));
    geometry.push_attribute(("as", "geometry"));
    writer
        .write_event(Event::Empty(geometry))
        .map_err(|e| e.to_string())?;

    writer
        .write_event(Event::End(BytesEnd::new("mxCell")))
        .map_err(|e| e.to_string())
}

/// Write connector/edge cells
fn write_connectors(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    shapes: &[DetectedShape],
    shape_id_map: &std::collections::HashMap<String, String>,
    cell_id: &mut i32,
) -> Result<(), String> {
    use crate::shapes::ShapeType;

    for shape in shapes {
        if !matches!(
            shape.shape_type,
            ShapeType::Arrow | ShapeType::Line | ShapeType::Connector
        ) {
            continue;
        }

        // Find source and target based on proximity
        let (source_id, target_id) = find_connection_endpoints(shape, shapes, shape_id_map);

        let current_id = cell_id.to_string();
        let style = get_connector_style(&shape.shape_type);

        write_edge_cell(
            writer,
            &current_id,
            "1",
            "",
            &style,
            &source_id.unwrap_or_default(),
            &target_id.unwrap_or_default(),
        )?;

        *cell_id += 1;
    }

    Ok(())
}

/// Write an edge cell
fn write_edge_cell(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    id: &str,
    parent: &str,
    value: &str,
    style: &str,
    source: &str,
    target: &str,
) -> Result<(), String> {
    let mut cell = BytesStart::new("mxCell");
    cell.push_attribute(("id", id));
    cell.push_attribute(("value", value));
    cell.push_attribute(("style", style));
    cell.push_attribute(("edge", "1"));
    cell.push_attribute(("parent", parent));
    if !source.is_empty() {
        cell.push_attribute(("source", source));
    }
    if !target.is_empty() {
        cell.push_attribute(("target", target));
    }
    writer
        .write_event(Event::Start(cell))
        .map_err(|e| e.to_string())?;

    // Geometry for edge
    let mut geometry = BytesStart::new("mxGeometry");
    geometry.push_attribute(("relative", "1"));
    geometry.push_attribute(("as", "geometry"));
    writer
        .write_event(Event::Empty(geometry))
        .map_err(|e| e.to_string())?;

    writer
        .write_event(Event::End(BytesEnd::new("mxCell")))
        .map_err(|e| e.to_string())
}

/// Find label text that belongs to a shape
fn find_label_for_shape(shape: &DetectedShape, text_regions: &[TextRegion]) -> String {
    let mut labels = Vec::new();

    for text in text_regions {
        // Check if text center is within shape bounds (with some margin)
        let text_cx = text.bounds.x + text.bounds.width / 2.0;
        let text_cy = text.bounds.y + text.bounds.height / 2.0;

        let margin = 20.0;
        if text_cx >= shape.bounds.x - margin
            && text_cx <= shape.bounds.x + shape.bounds.width + margin
            && text_cy >= shape.bounds.y - margin
            && text_cy <= shape.bounds.y + shape.bounds.height + margin
        {
            labels.push(text.text.clone());
        }
    }

    labels.join("\\n")
}

/// Get draw.io style string for shape type
fn get_style_for_shape(shape_type: &crate::shapes::ShapeType) -> String {
    use crate::shapes::ShapeType;
    match shape_type {
        ShapeType::Rectangle => StylePresets::rounded_rectangle().to_string(),
        ShapeType::Diamond => StylePresets::diamond().to_string(),
        ShapeType::Circle => StylePresets::circle().to_string(),
        ShapeType::Ellipse => StylePresets::ellipse().to_string(),
        ShapeType::Triangle => {
            "triangle;whiteSpace=wrap;html=1;fillColor=#ffe6cc;strokeColor=#d79b00;".to_string()
        }
        ShapeType::Freeform => "shape=curlyBracket;whiteSpace=wrap;html=1;".to_string(),
        _ => StylePresets::rectangle().to_string(),
    }
}

/// Get connector style string
fn get_connector_style(shape_type: &crate::shapes::ShapeType) -> String {
    use crate::shapes::ShapeType;
    match shape_type {
        ShapeType::Arrow => StylePresets::arrow().to_string(),
        ShapeType::Line => StylePresets::line().to_string(),
        _ => StylePresets::arrow().to_string(),
    }
}

/// Find source and target shapes for a connector
fn find_connection_endpoints(
    connector: &DetectedShape,
    all_shapes: &[DetectedShape],
    id_map: &std::collections::HashMap<String, String>,
) -> (Option<String>, Option<String>) {
    use crate::shapes::ShapeType;

    let start = connector.properties.start_point;
    let end = connector.properties.end_point;

    let mut source_id = None;
    let mut target_id = None;

    // Find shapes that contain or are near the endpoints
    for shape in all_shapes {
        if matches!(
            shape.shape_type,
            ShapeType::Arrow | ShapeType::Line | ShapeType::Connector
        ) {
            continue;
        }

        if let Some(mapped_id) = id_map.get(&shape.id) {
            if let Some((sx, sy)) = start {
                if point_near_shape(sx, sy, shape, 30.0) && source_id.is_none() {
                    source_id = Some(mapped_id.clone());
                }
            }

            if let Some((ex, ey)) = end {
                if point_near_shape(ex, ey, shape, 30.0) && target_id.is_none() {
                    target_id = Some(mapped_id.clone());
                }
            }
        }
    }

    (source_id, target_id)
}

/// Check if a point is near a shape
fn point_near_shape(px: f64, py: f64, shape: &DetectedShape, threshold: f64) -> bool {
    let shape_cx = shape.bounds.x + shape.bounds.width / 2.0;
    let shape_cy = shape.bounds.y + shape.bounds.height / 2.0;

    // Check if point is inside shape bounds (expanded by threshold)
    let in_x = px >= shape.bounds.x - threshold
        && px <= shape.bounds.x + shape.bounds.width + threshold;
    let in_y = py >= shape.bounds.y - threshold
        && py <= shape.bounds.y + shape.bounds.height + threshold;

    in_x && in_y
}

/// Generate timestamp string
fn chrono_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}", duration.as_secs())
}

/// Generate draw.io XML from a DiagramStructure
pub fn generate_xml_from_structure(
    structure: &DiagramStructure,
    options: &ExportOptions,
) -> Result<String, String> {
    let mut writer = Writer::new(Cursor::new(Vec::new()));

    // XML declaration
    writer
        .write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))
        .map_err(|e| e.to_string())?;

    // mxfile root
    let mut mxfile = BytesStart::new("mxfile");
    mxfile.push_attribute(("host", "RustWhiteboard"));
    mxfile.push_attribute(("modified", chrono_timestamp().as_str()));
    writer
        .write_event(Event::Start(mxfile))
        .map_err(|e| e.to_string())?;

    // diagram
    let mut diagram = BytesStart::new("diagram");
    diagram.push_attribute(("id", "diagram-1"));
    diagram.push_attribute(("name", options.filename.as_str()));
    writer
        .write_event(Event::Start(diagram))
        .map_err(|e| e.to_string())?;

    // mxGraphModel
    let mut model = BytesStart::new("mxGraphModel");
    model.push_attribute(("grid", "1"));
    model.push_attribute(("gridSize", "10"));
    let pw = options.page_width.to_string();
    let ph = options.page_height.to_string();
    model.push_attribute(("pageWidth", pw.as_str()));
    model.push_attribute(("pageHeight", ph.as_str()));
    writer
        .write_event(Event::Start(model))
        .map_err(|e| e.to_string())?;

    // root
    writer
        .write_event(Event::Start(BytesStart::new("root")))
        .map_err(|e| e.to_string())?;

    // Default cells
    write_cell(&mut writer, "0", "", "")?;
    write_cell_with_parent(&mut writer, "1", "0")?;

    // Nodes
    for (i, node) in structure.nodes.iter().enumerate() {
        let id = format!("{}", i + 2);
        let style = if node.style.is_empty() {
            get_style_for_type(&node.shape_type)
        } else {
            node.style.clone()
        };

        write_shape_cell(
            &mut writer,
            &id,
            "1",
            &node.label,
            &style,
            node.x,
            node.y,
            node.width,
            node.height,
        )?;
    }

    // Edges
    let node_offset = structure.nodes.len() + 2;
    for (i, edge) in structure.edges.iter().enumerate() {
        let id = format!("{}", node_offset + i);
        let style = if edge.style.is_empty() {
            StylePresets::arrow().to_string()
        } else {
            edge.style.clone()
        };

        // Map source/target IDs
        let source_idx = structure
            .nodes
            .iter()
            .position(|n| n.id == edge.source)
            .map(|i| format!("{}", i + 2))
            .unwrap_or_default();
        let target_idx = structure
            .nodes
            .iter()
            .position(|n| n.id == edge.target)
            .map(|i| format!("{}", i + 2))
            .unwrap_or_default();

        write_edge_cell(
            &mut writer,
            &id,
            "1",
            edge.label.as_deref().unwrap_or(""),
            &style,
            &source_idx,
            &target_idx,
        )?;
    }

    // Close elements
    writer
        .write_event(Event::End(BytesEnd::new("root")))
        .map_err(|e| e.to_string())?;
    writer
        .write_event(Event::End(BytesEnd::new("mxGraphModel")))
        .map_err(|e| e.to_string())?;
    writer
        .write_event(Event::End(BytesEnd::new("diagram")))
        .map_err(|e| e.to_string())?;
    writer
        .write_event(Event::End(BytesEnd::new("mxfile")))
        .map_err(|e| e.to_string())?;

    let xml_bytes = writer.into_inner().into_inner();
    String::from_utf8(xml_bytes).map_err(|e| e.to_string())
}

/// Get style string for node type
fn get_style_for_type(shape_type: &str) -> String {
    match shape_type {
        "process" | "rectangle" => StylePresets::rounded_rectangle().to_string(),
        "decision" | "diamond" => StylePresets::diamond().to_string(),
        "terminator" | "circle" | "ellipse" => StylePresets::terminator().to_string(),
        "data" | "triangle" => {
            "shape=parallelogram;whiteSpace=wrap;html=1;fillColor=#ffe6cc;strokeColor=#d79b00;"
                .to_string()
        }
        _ => StylePresets::rectangle().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_empty_xml() {
        let shapes = vec![];
        let text_regions = vec![];
        let options = ExportOptions {
            filename: "test".to_string(),
            include_grid: true,
            page_width: 800.0,
            page_height: 600.0,
            theme: "light".to_string(),
        };

        let result = generate_xml(&shapes, &text_regions, &options);
        assert!(result.is_ok());
        let xml = result.unwrap();
        assert!(xml.contains("mxfile"));
        assert!(xml.contains("mxGraphModel"));
    }

    #[test]
    fn test_style_presets() {
        assert!(StylePresets::rectangle().contains("rounded=0"));
        assert!(StylePresets::diamond().contains("rhombus"));
        assert!(StylePresets::circle().contains("ellipse"));
        assert!(StylePresets::arrow().contains("endArrow=classic"));
    }

    #[test]
    fn test_diagram_structure_serialization() {
        let structure = DiagramStructure {
            diagram_type: "flowchart".to_string(),
            nodes: vec![DiagramNode {
                id: "1".to_string(),
                label: "Start".to_string(),
                shape_type: "terminator".to_string(),
                x: 100.0,
                y: 100.0,
                width: 100.0,
                height: 50.0,
                style: "".to_string(),
            }],
            edges: vec![],
            metadata: DiagramMetadata::default(),
        };

        let json = serde_json::to_string(&structure);
        assert!(json.is_ok());
    }
}
