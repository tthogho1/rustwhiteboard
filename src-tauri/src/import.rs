//! draw.io (`.drawio` / mxGraph XML) import.
//!
//! The canvas has no concept of a "shape" — it holds freehand strokes — so an
//! imported diagram is turned back into strokes that trace each vertex and
//! edge, plus text annotations for the labels. That keeps the result editable
//! with every tool, undoable, and re-analysable: Analyze detects the traced
//! outlines back into shapes, so a file can round-trip through the app.

use crate::{Point, Stroke, TextAnnotation};
use quick_xml::events::Event;
use quick_xml::Reader;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// What an import produces: exactly what the frontend store holds.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CanvasData {
    pub strokes: Vec<Stroke>,
    pub text_annotations: Vec<TextAnnotation>,
}

/// Pen width used for the traced outlines, matching the frontend default.
const STROKE_WIDTH: f64 = 3.0;
const DEFAULT_COLOR: &str = "#000000";
const DEFAULT_FONT_SIZE: f64 = 16.0;
/// Points used to trace an ellipse. Enough for `detect_shapes` to measure
/// circularity without bloating the stroke.
const ELLIPSE_SEGMENTS: usize = 48;

#[derive(Debug, Clone, Default)]
struct Geometry {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl Geometry {
    fn center(&self) -> (f64, f64) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }
}

#[derive(Debug, Clone, Default)]
struct Cell {
    id: String,
    value: String,
    style: String,
    is_vertex: bool,
    is_edge: bool,
    source: Option<String>,
    target: Option<String>,
    geometry: Option<Geometry>,
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub fn parse_drawio(raw: &str) -> Result<CanvasData, String> {
    let model = extract_graph_model(raw)
        .ok_or_else(|| "No <mxGraphModel> found — is this a .drawio file?".to_string())?;

    let cells = parse_cells(&model)?;
    Ok(build_canvas(&cells))
}

/// The graph XML, decompressing the `<diagram>` payload when needed.
///
/// Files this app writes are plain XML. Files saved by draw.io itself usually
/// hold the model deflated and base64'd inside `<diagram>`, so both shapes have
/// to be accepted.
fn extract_graph_model(raw: &str) -> Option<String> {
    if raw.contains("<mxGraphModel") {
        return Some(raw.to_string());
    }

    let start = raw.find("<diagram")?;
    let open_end = raw[start..].find('>')? + start + 1;
    let close = raw[open_end..].find("</diagram>")? + open_end;
    let payload = raw[open_end..close].trim();

    let inflated = inflate_diagram(payload)?;
    inflated.contains("<mxGraphModel").then_some(inflated)
}

/// base64 -> raw deflate -> percent-decoding, which is how draw.io packs it.
fn inflate_diagram(payload: &str) -> Option<String> {
    use flate2::read::DeflateDecoder;
    use std::io::Read;

    let compressed =
        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, payload).ok()?;

    let mut decoder = DeflateDecoder::new(&compressed[..]);
    let mut encoded = String::new();
    decoder.read_to_string(&mut encoded).ok()?;

    Some(percent_decode(&encoded))
}

/// Minimal `decodeURIComponent`. '+' is left alone: `encodeURIComponent`
/// escapes a space as %20, so a '+' here is a literal one.
fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&input[i + 1..i + 3], 16) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }

    String::from_utf8_lossy(&out).into_owned()
}

// ---------------------------------------------------------------------------
// XML -> cells
// ---------------------------------------------------------------------------

fn parse_cells(xml: &str) -> Result<Vec<Cell>, String> {
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);

    let mut cells = Vec::new();
    let mut current: Option<Cell> = None;
    // draw.io wraps a labelled cell in <object label="..."><mxCell/></object>.
    let mut wrapper: Option<(String, String)> = None;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Err(e) => return Err(format!("Malformed XML: {}", e)),
            Ok(Event::Eof) => break,

            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let name = e.name();
                let tag = String::from_utf8_lossy(name.as_ref()).to_string();
                let attrs = read_attributes(&e);

                match tag.as_str() {
                    "object" | "UserObject" => {
                        wrapper = Some((
                            attrs.get("id").cloned().unwrap_or_default(),
                            attrs.get("label").cloned().unwrap_or_default(),
                        ));
                    }
                    "mxCell" => {
                        if let Some(cell) = current.take() {
                            cells.push(cell);
                        }
                        current = Some(cell_from_attributes(&attrs, wrapper.as_ref()));
                    }
                    "mxGeometry" => {
                        if let Some(cell) = current.as_mut() {
                            cell.geometry = Some(geometry_from_attributes(&attrs));
                        }
                    }
                    _ => {}
                }
            }

            Ok(Event::End(e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match tag.as_str() {
                    "mxCell" => {
                        if let Some(cell) = current.take() {
                            cells.push(cell);
                        }
                    }
                    "object" | "UserObject" => {
                        if let Some(cell) = current.take() {
                            cells.push(cell);
                        }
                        wrapper = None;
                    }
                    _ => {}
                }
            }

            _ => {}
        }
        buf.clear();
    }

    if let Some(cell) = current.take() {
        cells.push(cell);
    }

    Ok(cells)
}

fn read_attributes(e: &quick_xml::events::BytesStart) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for attr in e.attributes().flatten() {
        let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
        let value = attr
            .unescape_value()
            .map(|v| v.to_string())
            .unwrap_or_default();
        map.insert(key, value);
    }
    map
}

fn cell_from_attributes(
    attrs: &HashMap<String, String>,
    wrapper: Option<&(String, String)>,
) -> Cell {
    let mut value = attrs.get("value").cloned().unwrap_or_default();
    let mut id = attrs.get("id").cloned().unwrap_or_default();

    // The wrapping <object> carries the id and label for this cell.
    if let Some((wrapper_id, wrapper_label)) = wrapper {
        if value.is_empty() {
            value = wrapper_label.clone();
        }
        if !wrapper_id.is_empty() {
            id = wrapper_id.clone();
        }
    }

    Cell {
        id,
        value,
        style: attrs.get("style").cloned().unwrap_or_default(),
        is_vertex: attrs.get("vertex").map(|v| v == "1").unwrap_or(false),
        is_edge: attrs.get("edge").map(|v| v == "1").unwrap_or(false),
        source: attrs.get("source").cloned(),
        target: attrs.get("target").cloned(),
        geometry: None,
    }
}

fn geometry_from_attributes(attrs: &HashMap<String, String>) -> Geometry {
    let number = |key: &str| -> f64 {
        attrs
            .get(key)
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(0.0)
    };

    Geometry {
        x: number("x"),
        y: number("y"),
        width: number("width"),
        height: number("height"),
    }
}

// ---------------------------------------------------------------------------
// Cells -> canvas
// ---------------------------------------------------------------------------

fn build_canvas(cells: &[Cell]) -> CanvasData {
    let mut data = CanvasData::default();

    let boxes: HashMap<&str, &Geometry> = cells
        .iter()
        .filter(|c| c.is_vertex)
        .filter_map(|c| c.geometry.as_ref().map(|g| (c.id.as_str(), g)))
        .collect();

    for cell in cells.iter().filter(|c| c.is_vertex) {
        let Some(geo) = cell.geometry.as_ref() else {
            continue;
        };
        if geo.width <= 0.0 || geo.height <= 0.0 {
            continue;
        }

        // A `text` cell has no outline to trace — it is only its label.
        if !is_text_only(&cell.style) {
            data.strokes.push(trace_vertex(cell, geo));
        }
        if let Some(annotation) = label_for(cell, geo) {
            data.text_annotations.push(annotation);
        }
    }

    for cell in cells.iter().filter(|c| c.is_edge) {
        if let Some(stroke) = trace_edge(cell, &boxes) {
            data.strokes.push(stroke);
        }
    }

    data
}

fn is_text_only(style: &str) -> bool {
    style.starts_with("text;") || style.contains(";text;")
}

fn trace_vertex(cell: &Cell, geo: &Geometry) -> Stroke {
    let (cx, cy) = geo.center();
    let (hw, hh) = (geo.width / 2.0, geo.height / 2.0);

    let mut points: Vec<(f64, f64)> = if cell.style.contains("ellipse") {
        (0..ELLIPSE_SEGMENTS)
            .map(|i| {
                let t = (i as f64 / ELLIPSE_SEGMENTS as f64) * std::f64::consts::TAU;
                (cx + hw * t.cos(), cy + hh * t.sin())
            })
            .collect()
    } else if cell.style.contains("rhombus") {
        vec![
            (cx, geo.y),
            (geo.x + geo.width, cy),
            (cx, geo.y + geo.height),
            (geo.x, cy),
        ]
    } else if cell.style.contains("triangle") {
        vec![
            (cx, geo.y),
            (geo.x + geo.width, geo.y + geo.height),
            (geo.x, geo.y + geo.height),
        ]
    } else {
        vec![
            (geo.x, geo.y),
            (geo.x + geo.width, geo.y),
            (geo.x + geo.width, geo.y + geo.height),
            (geo.x, geo.y + geo.height),
        ]
    };

    // Close the outline so detection reads it back as a closed shape.
    if let Some(&first) = points.first() {
        points.push(first);
    }

    stroke_from_points(points, style_color(&cell.style, "strokeColor"))
}

fn trace_edge(cell: &Cell, boxes: &HashMap<&str, &Geometry>) -> Option<Stroke> {
    let source = cell.source.as_deref().and_then(|id| boxes.get(id))?;
    let target = cell.target.as_deref().and_then(|id| boxes.get(id))?;

    let from = source.center();
    let to = target.center();

    // Stop at the box edges rather than running to the centres, so the line
    // reads as a connector between two shapes.
    let start = exit_point(from, to, source);
    let end = exit_point(to, from, target);

    Some(stroke_from_points(
        vec![start, end],
        style_color(&cell.style, "strokeColor"),
    ))
}

/// Where the ray from a box's centre towards `towards` leaves that box.
fn exit_point(center: (f64, f64), towards: (f64, f64), geo: &Geometry) -> (f64, f64) {
    let dx = towards.0 - center.0;
    let dy = towards.1 - center.1;
    if dx == 0.0 && dy == 0.0 {
        return center;
    }

    let (hw, hh) = (geo.width / 2.0, geo.height / 2.0);
    let tx = if dx == 0.0 { f64::INFINITY } else { hw / dx.abs() };
    let ty = if dy == 0.0 { f64::INFINITY } else { hh / dy.abs() };
    // Never overshoot the other end of the segment.
    let t = tx.min(ty).min(1.0);

    (center.0 + dx * t, center.1 + dy * t)
}

fn stroke_from_points(points: Vec<(f64, f64)>, color: String) -> Stroke {
    Stroke {
        id: uuid::Uuid::new_v4().to_string(),
        points: points
            .into_iter()
            .map(|(x, y)| Point {
                x,
                y,
                pressure: None,
                timestamp: 0,
            })
            .collect(),
        color,
        width: STROKE_WIDTH,
        tool: "pen".to_string(),
    }
}

fn label_for(cell: &Cell, geo: &Geometry) -> Option<TextAnnotation> {
    let text = clean_label(&cell.value);
    if text.is_empty() {
        return None;
    }

    let font_size = style_number(&cell.style, "fontSize").unwrap_or(DEFAULT_FONT_SIZE);
    let lines = text.lines().count().max(1) as f64;
    // Rough: the frontend re-measures on import, this only has to be sane.
    let longest = text.lines().map(|l| l.chars().count()).max().unwrap_or(0) as f64;
    let width = longest * font_size * 0.6;
    let height = lines * font_size * 1.3;

    let (cx, cy) = geo.center();
    Some(TextAnnotation {
        id: uuid::Uuid::new_v4().to_string(),
        text,
        x: cx - width / 2.0,
        y: cy - height / 2.0,
        width,
        height,
        color: style_color(&cell.style, "fontColor"),
        font_size,
    })
}

/// draw.io labels are HTML fragments; the canvas draws plain text.
fn clean_label(value: &str) -> String {
    let mut out = value
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("</div>", "\n")
        .replace("</p>", "\n");

    // Drop any remaining tags.
    let mut stripped = String::with_capacity(out.len());
    let mut in_tag = false;
    for ch in out.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => stripped.push(ch),
            _ => {}
        }
    }
    out = stripped;

    out.replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        // &amp; last, so "&amp;lt;" does not become "<".
        .replace("&amp;", "&")
        .trim()
        .to_string()
}

/// Read `key=#rrggbb` out of an mxGraph style string.
fn style_color(style: &str, key: &str) -> String {
    for part in style.split(';') {
        if let Some(value) = part.strip_prefix(&format!("{}=", key)) {
            if value.starts_with('#') {
                return value.to_string();
            }
        }
    }
    DEFAULT_COLOR.to_string()
}

fn style_number(style: &str, key: &str) -> Option<f64> {
    for part in style.split(';') {
        if let Some(value) = part.strip_prefix(&format!("{}=", key)) {
            return value.parse().ok();
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain_model(cells: &str) -> String {
        format!(
            r#"<mxfile><diagram name="Page-1"><mxGraphModel><root>
<mxCell id="0"/><mxCell id="1" parent="0"/>{}
</root></mxGraphModel></diagram></mxfile>"#,
            cells
        )
    }

    #[test]
    fn test_rectangle_becomes_a_closed_stroke() {
        let xml = plain_model(
            r#"<mxCell id="2" value="Start" style="rounded=0;" vertex="1" parent="1">
                 <mxGeometry x="10" y="20" width="100" height="50" as="geometry"/>
               </mxCell>"#,
        );

        let data = parse_drawio(&xml).unwrap();
        assert_eq!(data.strokes.len(), 1);

        let points = &data.strokes[0].points;
        // Four corners plus the repeated first point.
        assert_eq!(points.len(), 5);
        assert_eq!((points[0].x, points[0].y), (10.0, 20.0));
        assert_eq!((points[2].x, points[2].y), (110.0, 70.0));
        assert_eq!((points[4].x, points[4].y), (points[0].x, points[0].y));

        assert_eq!(data.text_annotations.len(), 1);
        assert_eq!(data.text_annotations[0].text, "Start");
    }

    #[test]
    fn test_ellipse_is_traced_as_a_curve() {
        let xml = plain_model(
            r#"<mxCell id="2" style="ellipse;whiteSpace=wrap;" vertex="1" parent="1">
                 <mxGeometry x="0" y="0" width="100" height="100" as="geometry"/>
               </mxCell>"#,
        );

        let data = parse_drawio(&xml).unwrap();
        assert_eq!(data.strokes[0].points.len(), ELLIPSE_SEGMENTS + 1);
        // No label cell, so nothing to annotate.
        assert!(data.text_annotations.is_empty());
    }

    #[test]
    fn test_edge_is_clipped_to_both_boxes() {
        let xml = plain_model(
            r#"<mxCell id="2" vertex="1" parent="1">
                 <mxGeometry x="0" y="0" width="100" height="100" as="geometry"/>
               </mxCell>
               <mxCell id="3" vertex="1" parent="1">
                 <mxGeometry x="300" y="0" width="100" height="100" as="geometry"/>
               </mxCell>
               <mxCell id="4" edge="1" parent="1" source="2" target="3">
                 <mxGeometry relative="1" as="geometry"/>
               </mxCell>"#,
        );

        let data = parse_drawio(&xml).unwrap();
        let edge = data
            .strokes
            .iter()
            .find(|s| s.points.len() == 2)
            .expect("edge stroke");

        // Boxes span x 0..100 and 300..400 at the same height, so the line runs
        // between their facing edges rather than centre to centre.
        assert_eq!(edge.points[0].x, 100.0);
        assert_eq!(edge.points[1].x, 300.0);
        assert_eq!(edge.points[0].y, 50.0);
    }

    #[test]
    fn test_text_cell_makes_no_stroke() {
        let xml = plain_model(
            r#"<mxCell id="2" value="Note" style="text;html=1;strokeColor=none;" vertex="1" parent="1">
                 <mxGeometry x="5" y="5" width="40" height="20" as="geometry"/>
               </mxCell>"#,
        );

        let data = parse_drawio(&xml).unwrap();
        assert!(data.strokes.is_empty());
        assert_eq!(data.text_annotations.len(), 1);
        assert_eq!(data.text_annotations[0].text, "Note");
    }

    #[test]
    fn test_html_label_is_flattened() {
        assert_eq!(clean_label("A<br>B"), "A\nB");
        assert_eq!(clean_label("<b>Bold</b>"), "Bold");
        assert_eq!(clean_label("a&nbsp;&amp;&nbsp;b"), "a & b");
    }

    #[test]
    fn test_object_wrapper_supplies_the_label() {
        let xml = plain_model(
            r#"<object label="Wrapped" id="9">
                 <mxCell style="rounded=0;" vertex="1" parent="1">
                   <mxGeometry x="0" y="0" width="80" height="40" as="geometry"/>
                 </mxCell>
               </object>"#,
        );

        let data = parse_drawio(&xml).unwrap();
        assert_eq!(data.text_annotations.len(), 1);
        assert_eq!(data.text_annotations[0].text, "Wrapped");
    }

    #[test]
    fn test_round_trips_a_file_this_app_wrote() {
        use crate::drawio::generate_xml;
        use crate::shapes::{DetectedShape, ShapeBounds, ShapeProperties, ShapeType};

        let shape = DetectedShape {
            id: "s1".to_string(),
            shape_type: ShapeType::Rectangle,
            bounds: ShapeBounds {
                x: 40.0,
                y: 60.0,
                width: 200.0,
                height: 100.0,
                rotation: 0.0,
            },
            confidence: 1.0,
            stroke_ids: vec![],
            properties: ShapeProperties {
                center_x: 140.0,
                center_y: 110.0,
                radius: None,
                start_point: None,
                end_point: None,
                corner_radius: None,
                arrow_head: None,
            },
        };
        let label = text_region("t1", "Start", 100.0, 100.0);
        let options = crate::ExportOptions {
            filename: "test".to_string(),
            include_grid: true,
            page_width: 800.0,
            page_height: 600.0,
            theme: "light".to_string(),
        };

        let xml = generate_xml(&[shape], &[label], &options).unwrap();
        let data = parse_drawio(&xml).unwrap();

        assert_eq!(data.strokes.len(), 1, "the box comes back as one traced stroke");
        assert_eq!(data.text_annotations.len(), 1);
        assert_eq!(data.text_annotations[0].text, "Start");

        // generate_xml floors the geometry at 80x40; this box is bigger, so the
        // traced outline must land on the exported bounds exactly.
        let points = &data.strokes[0].points;
        assert_eq!((points[0].x, points[0].y), (40.0, 60.0));
        assert_eq!((points[2].x, points[2].y), (240.0, 160.0));
    }

    fn text_region(id: &str, text: &str, x: f64, y: f64) -> crate::ocr::TextRegion {
        crate::ocr::TextRegion {
            id: id.to_string(),
            text: text.to_string(),
            bounds: crate::ocr::TextBounds {
                x,
                y,
                width: 60.0,
                height: 20.0,
            },
            confidence: 1.0,
            font_size_estimate: 14.0,
        }
    }

    #[test]
    fn test_compressed_diagram_is_inflated() {
        use flate2::write::DeflateEncoder;
        use flate2::Compression;
        use std::io::Write;

        // draw.io stores the model URL-encoded, then raw-deflated, then base64.
        // The label is percent-encoded on top of its XML escaping, the way
        // encodeURIComponent leaves it: %26amp%3B -> &amp; -> &.
        let model = r#"<mxGraphModel><root><mxCell id="0"/><mxCell id="1" parent="0"/>
<mxCell id="2" value="Hi%20%26amp%3B%20bye" style="rounded=0;" vertex="1" parent="1">
<mxGeometry x="0" y="0" width="50" height="50" as="geometry"/></mxCell></root></mxGraphModel>"#;

        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(model.as_bytes()).unwrap();
        let payload = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            encoder.finish().unwrap(),
        );

        let xml = format!("<mxfile><diagram id=\"a\" name=\"P\">{}</diagram></mxfile>", payload);

        let data = parse_drawio(&xml).unwrap();
        assert_eq!(data.strokes.len(), 1);
        // %26 is the percent-encoding draw.io applies before compressing.
        assert_eq!(data.text_annotations[0].text, "Hi & bye");
    }

    #[test]
    fn test_non_drawio_input_is_rejected() {
        assert!(parse_drawio("<html><body>nope</body></html>").is_err());
    }

    #[test]
    fn test_style_helpers() {
        assert_eq!(style_color("a=1;strokeColor=#ff0000;b=2", "strokeColor"), "#ff0000");
        // `none` is not a colour we can draw with.
        assert_eq!(style_color("strokeColor=none;", "strokeColor"), DEFAULT_COLOR);
        assert_eq!(style_number("fontSize=22;", "fontSize"), Some(22.0));
        assert_eq!(style_number("rounded=1;", "fontSize"), None);
    }
}
