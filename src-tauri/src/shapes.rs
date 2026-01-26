//! Shape detection and recognition module
//! 
//! Detects geometric shapes (rectangles, circles, arrows, lines) from strokes
//! and classifies diagram types.

use crate::{Point, Stroke};
use crate::ocr::TextRegion;
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

/// Types of shapes that can be detected
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ShapeType {
    Rectangle,
    Circle,
    Ellipse,
    Triangle,
    Diamond,
    Arrow,
    Line,
    Connector,
    Freeform,
}

/// A detected shape with its properties
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedShape {
    pub id: String,
    pub shape_type: ShapeType,
    pub bounds: ShapeBounds,
    pub confidence: f64,
    pub stroke_ids: Vec<String>,
    pub properties: ShapeProperties,
}

/// Bounding box of a shape
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShapeBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub rotation: f64,
}

/// Additional properties for shapes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShapeProperties {
    pub center_x: f64,
    pub center_y: f64,
    pub radius: Option<f64>,
    pub start_point: Option<(f64, f64)>,
    pub end_point: Option<(f64, f64)>,
    pub corner_radius: Option<f64>,
    pub arrow_head: Option<ArrowHead>,
}

/// Arrow head configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArrowHead {
    pub style: String,
    pub size: f64,
    pub direction: f64,
}

/// Shape detection parameters
#[derive(Debug, Clone)]
pub struct DetectionParams {
    pub min_points: usize,
    pub circularity_threshold: f64,
    pub rectangularity_threshold: f64,
    pub line_straightness_threshold: f64,
    pub arrow_angle_tolerance: f64,
}

impl Default for DetectionParams {
    fn default() -> Self {
        Self {
            min_points: 5,
            circularity_threshold: 0.85,
            rectangularity_threshold: 0.80,
            line_straightness_threshold: 0.95,
            arrow_angle_tolerance: 30.0,
        }
    }
}

/// Detect shapes from a collection of strokes
pub fn detect_shapes(strokes: &[Stroke]) -> Vec<DetectedShape> {
    let params = DetectionParams::default();
    let mut shapes = Vec::new();

    for stroke in strokes {
        if stroke.points.len() < params.min_points {
            continue;
        }

        if let Some(shape) = detect_shape_from_stroke(stroke, &params) {
            shapes.push(shape);
        }
    }

    // Try to detect compound shapes (connected shapes)
    let compound_shapes = detect_compound_shapes(&shapes, strokes);
    
    // Merge results, preferring compound shapes
    merge_shapes(shapes, compound_shapes)
}

/// Detect a single shape from a stroke
fn detect_shape_from_stroke(stroke: &Stroke, params: &DetectionParams) -> Option<DetectedShape> {
    let points = &stroke.points;
    
    // Calculate basic metrics
    let bounds = calculate_bounds(points);
    let center = calculate_centroid(points);
    let is_closed = is_stroke_closed(points, bounds.width.max(bounds.height) * 0.1);

    // Try to identify the shape type
    let (shape_type, confidence) = if is_closed {
        // Check for circle first
        let circularity = calculate_circularity(points, &center);
        if circularity > params.circularity_threshold {
            (ShapeType::Circle, circularity)
        } else {
            // Check for rectangle
            let rectangularity = calculate_rectangularity(points, &bounds);
            if rectangularity > params.rectangularity_threshold {
                // Check if it's a diamond (rotated 45 degrees)
                let is_diamond = check_diamond(points, &center);
                if is_diamond {
                    (ShapeType::Diamond, rectangularity * 0.95)
                } else {
                    (ShapeType::Rectangle, rectangularity)
                }
            } else {
                // Check for triangle
                let triangle_score = calculate_triangle_score(points);
                if triangle_score > 0.75 {
                    (ShapeType::Triangle, triangle_score)
                } else {
                    (ShapeType::Freeform, 0.5)
                }
            }
        }
    } else {
        // Open stroke - check for line or arrow
        let straightness = calculate_straightness(points);
        if straightness > params.line_straightness_threshold {
            // Check for arrow head
            if let Some(arrow_info) = detect_arrow_head(points, params.arrow_angle_tolerance) {
                (ShapeType::Arrow, straightness * 0.95)
            } else {
                (ShapeType::Line, straightness)
            }
        } else {
            // Could be a connector (curved line between shapes)
            (ShapeType::Connector, 0.6)
        }
    };

    let properties = ShapeProperties {
        center_x: center.0,
        center_y: center.1,
        radius: if shape_type == ShapeType::Circle {
            Some(calculate_average_radius(points, &center))
        } else {
            None
        },
        start_point: Some((points.first()?.x, points.first()?.y)),
        end_point: Some((points.last()?.x, points.last()?.y)),
        corner_radius: None,
        arrow_head: if shape_type == ShapeType::Arrow {
            detect_arrow_head(points, params.arrow_angle_tolerance)
        } else {
            None
        },
    };

    Some(DetectedShape {
        id: uuid::Uuid::new_v4().to_string(),
        shape_type,
        bounds,
        confidence,
        stroke_ids: vec![stroke.id.clone()],
        properties,
    })
}

/// Calculate bounding box of points
fn calculate_bounds(points: &[Point]) -> ShapeBounds {
    let mut min_x = f64::MAX;
    let mut min_y = f64::MAX;
    let mut max_x = f64::MIN;
    let mut max_y = f64::MIN;

    for p in points {
        min_x = min_x.min(p.x);
        min_y = min_y.min(p.y);
        max_x = max_x.max(p.x);
        max_y = max_y.max(p.y);
    }

    ShapeBounds {
        x: min_x,
        y: min_y,
        width: max_x - min_x,
        height: max_y - min_y,
        rotation: 0.0,
    }
}

/// Calculate centroid of points
fn calculate_centroid(points: &[Point]) -> (f64, f64) {
    let n = points.len() as f64;
    let sum_x: f64 = points.iter().map(|p| p.x).sum();
    let sum_y: f64 = points.iter().map(|p| p.y).sum();
    (sum_x / n, sum_y / n)
}

/// Check if stroke is closed (start and end points are close)
fn is_stroke_closed(points: &[Point], threshold: f64) -> bool {
    if points.len() < 3 {
        return false;
    }
    let start = &points[0];
    let end = &points[points.len() - 1];
    let distance = ((start.x - end.x).powi(2) + (start.y - end.y).powi(2)).sqrt();
    distance < threshold
}

/// Calculate circularity (how close to a circle)
fn calculate_circularity(points: &[Point], center: &(f64, f64)) -> f64 {
    let avg_radius = calculate_average_radius(points, center);
    
    if avg_radius == 0.0 {
        return 0.0;
    }

    let variance: f64 = points
        .iter()
        .map(|p| {
            let r = ((p.x - center.0).powi(2) + (p.y - center.1).powi(2)).sqrt();
            (r - avg_radius).powi(2)
        })
        .sum::<f64>()
        / points.len() as f64;

    let std_dev = variance.sqrt();
    let coefficient_of_variation = std_dev / avg_radius;
    
    // Lower variation = more circular
    (1.0 - coefficient_of_variation).max(0.0).min(1.0)
}

/// Calculate average radius from center
fn calculate_average_radius(points: &[Point], center: &(f64, f64)) -> f64 {
    let sum: f64 = points
        .iter()
        .map(|p| ((p.x - center.0).powi(2) + (p.y - center.1).powi(2)).sqrt())
        .sum();
    sum / points.len() as f64
}

/// Calculate rectangularity (how close to a rectangle)
fn calculate_rectangularity(points: &[Point], bounds: &ShapeBounds) -> f64 {
    let area = bounds.width * bounds.height;
    if area == 0.0 {
        return 0.0;
    }

    // Calculate convex hull area
    let hull_area = calculate_convex_hull_area(points);
    
    // Perfect rectangle has hull area equal to bounding box area
    let ratio = hull_area / area;
    
    // Also check for corner presence
    let corner_score = detect_corners(points, bounds);
    
    (ratio * 0.6 + corner_score * 0.4).min(1.0)
}

/// Simplified convex hull area calculation
fn calculate_convex_hull_area(points: &[Point]) -> f64 {
    // Shoelace formula for polygon area
    let n = points.len();
    if n < 3 {
        return 0.0;
    }

    let mut area = 0.0;
    for i in 0..n {
        let j = (i + 1) % n;
        area += points[i].x * points[j].y;
        area -= points[j].x * points[i].y;
    }
    
    (area / 2.0).abs()
}

/// Detect corners in the stroke
fn detect_corners(points: &[Point], bounds: &ShapeBounds) -> f64 {
    let corners = [
        (bounds.x, bounds.y),
        (bounds.x + bounds.width, bounds.y),
        (bounds.x + bounds.width, bounds.y + bounds.height),
        (bounds.x, bounds.y + bounds.height),
    ];

    let threshold = (bounds.width.max(bounds.height)) * 0.15;
    let mut found_corners = 0;

    for corner in &corners {
        for point in points {
            let dist = ((point.x - corner.0).powi(2) + (point.y - corner.1).powi(2)).sqrt();
            if dist < threshold {
                found_corners += 1;
                break;
            }
        }
    }

    found_corners as f64 / 4.0
}

/// Check if shape is a diamond (rhombus)
fn check_diamond(points: &[Point], center: &(f64, f64)) -> bool {
    // A diamond has points at cardinal directions from center
    let mut cardinal_scores = [0.0; 4]; // top, right, bottom, left
    
    for point in points {
        let dx = point.x - center.0;
        let dy = point.y - center.1;
        let angle = dy.atan2(dx);
        
        // Check proximity to cardinal directions
        let angles = [-PI / 2.0, 0.0, PI / 2.0, PI];
        for (i, &target_angle) in angles.iter().enumerate() {
            let diff = (angle - target_angle).abs();
            if diff < PI / 6.0 || (PI - diff).abs() < PI / 6.0 {
                cardinal_scores[i] += 1.0;
            }
        }
    }

    // Diamond should have points clustered at all 4 cardinal directions
    cardinal_scores.iter().all(|&s| s > 0.0)
}

/// Calculate triangle score
fn calculate_triangle_score(points: &[Point]) -> f64 {
    // Find the 3 most prominent corners
    let corners = find_prominent_corners(points, 3);
    
    if corners.len() < 3 {
        return 0.0;
    }

    // Check if points roughly lie on triangle edges
    let mut on_edge_count = 0;
    for point in points {
        for i in 0..3 {
            let j = (i + 1) % 3;
            let dist = point_to_line_distance(
                point,
                &corners[i],
                &corners[j],
            );
            if dist < 10.0 {
                on_edge_count += 1;
                break;
            }
        }
    }

    on_edge_count as f64 / points.len() as f64
}

/// Find prominent corners using angle changes
fn find_prominent_corners(points: &[Point], count: usize) -> Vec<Point> {
    if points.len() < 3 {
        return points.to_vec();
    }

    let mut angle_changes: Vec<(usize, f64)> = Vec::new();
    
    for i in 1..points.len() - 1 {
        let prev = &points[i - 1];
        let curr = &points[i];
        let next = &points[i + 1];
        
        let angle1 = (curr.y - prev.y).atan2(curr.x - prev.x);
        let angle2 = (next.y - curr.y).atan2(next.x - curr.x);
        let change = (angle2 - angle1).abs();
        
        angle_changes.push((i, change));
    }

    angle_changes.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    
    angle_changes
        .into_iter()
        .take(count)
        .map(|(i, _)| points[i].clone())
        .collect()
}

/// Calculate straightness of a stroke
fn calculate_straightness(points: &[Point]) -> f64 {
    if points.len() < 2 {
        return 1.0;
    }

    let start = &points[0];
    let end = &points[points.len() - 1];
    
    let direct_distance = ((end.x - start.x).powi(2) + (end.y - start.y).powi(2)).sqrt();
    
    if direct_distance == 0.0 {
        return 0.0;
    }

    // Calculate total stroke length
    let mut path_length = 0.0;
    for i in 1..points.len() {
        let dx = points[i].x - points[i - 1].x;
        let dy = points[i].y - points[i - 1].y;
        path_length += (dx * dx + dy * dy).sqrt();
    }

    // Straightness = direct distance / path length
    (direct_distance / path_length).min(1.0)
}

/// Detect arrow head at the end of a stroke
fn detect_arrow_head(points: &[Point], angle_tolerance: f64) -> Option<ArrowHead> {
    if points.len() < 5 {
        return None;
    }

    // Look at the last few points for arrow head pattern
    let n = points.len();
    let tip = &points[n - 1];
    
    // Check for sudden direction changes near the end
    let tail_start = n.saturating_sub(10);
    let main_direction = if tail_start > 0 {
        let mid = &points[tail_start];
        (tip.y - mid.y).atan2(tip.x - mid.x)
    } else {
        let start = &points[0];
        (tip.y - start.y).atan2(tip.x - start.x)
    };

    // Look for barbs (lines at angles to main direction)
    let barb_check_start = n.saturating_sub(5);
    let mut has_barb = false;
    
    for i in barb_check_start..n - 1 {
        let p1 = &points[i];
        let p2 = &points[i + 1];
        let segment_angle = (p2.y - p1.y).atan2(p2.x - p1.x);
        let angle_diff = ((segment_angle - main_direction).abs() * 180.0 / PI) % 180.0;
        
        if angle_diff > 30.0 && angle_diff < 150.0 {
            has_barb = true;
            break;
        }
    }

    if has_barb {
        Some(ArrowHead {
            style: "classic".to_string(),
            size: 10.0,
            direction: main_direction * 180.0 / PI,
        })
    } else {
        None
    }
}

/// Calculate point to line distance
fn point_to_line_distance(point: &Point, line_start: &Point, line_end: &Point) -> f64 {
    let dx = line_end.x - line_start.x;
    let dy = line_end.y - line_start.y;
    let length_sq = dx * dx + dy * dy;

    if length_sq == 0.0 {
        return ((point.x - line_start.x).powi(2) + (point.y - line_start.y).powi(2)).sqrt();
    }

    let t = ((point.x - line_start.x) * dx + (point.y - line_start.y) * dy) / length_sq;
    let t = t.clamp(0.0, 1.0);

    let proj_x = line_start.x + t * dx;
    let proj_y = line_start.y + t * dy;

    ((point.x - proj_x).powi(2) + (point.y - proj_y).powi(2)).sqrt()
}

/// Detect compound shapes (connected shapes)
fn detect_compound_shapes(shapes: &[DetectedShape], _strokes: &[Stroke]) -> Vec<DetectedShape> {
    // For now, return empty - can be extended to detect connected flowchart elements
    Vec::new()
}

/// Merge individual and compound shapes
fn merge_shapes(individual: Vec<DetectedShape>, compound: Vec<DetectedShape>) -> Vec<DetectedShape> {
    let mut result = individual;
    result.extend(compound);
    result
}

/// Classify the overall diagram type
pub fn classify_diagram(
    shapes: &[DetectedShape],
    text_regions: &[TextRegion],
) -> (String, f64) {
    let mut rectangle_count = 0;
    let mut diamond_count = 0;
    let mut arrow_count = 0;
    let mut circle_count = 0;
    let mut connector_count = 0;

    for shape in shapes {
        match shape.shape_type {
            ShapeType::Rectangle => rectangle_count += 1,
            ShapeType::Diamond => diamond_count += 1,
            ShapeType::Arrow | ShapeType::Line => arrow_count += 1,
            ShapeType::Circle | ShapeType::Ellipse => circle_count += 1,
            ShapeType::Connector => connector_count += 1,
            _ => {}
        }
    }

    // Check for flowchart indicators in text
    let flowchart_keywords = ["start", "end", "if", "yes", "no", "begin", "process"];
    let uml_keywords = ["class", "interface", "extends", "implements", "public", "private"];
    
    let text_content: String = text_regions
        .iter()
        .map(|t| t.text.to_lowercase())
        .collect::<Vec<_>>()
        .join(" ");

    let flowchart_text_score: f64 = flowchart_keywords
        .iter()
        .filter(|k| text_content.contains(*k))
        .count() as f64;
    
    let uml_text_score: f64 = uml_keywords
        .iter()
        .filter(|k| text_content.contains(*k))
        .count() as f64;

    // Determine diagram type
    let total_shapes = shapes.len() as f64;
    
    if diamond_count > 0 && arrow_count > 0 && rectangle_count > 0 {
        // Likely a flowchart
        let confidence = (0.3 + flowchart_text_score * 0.1 + 
            (diamond_count + arrow_count) as f64 / total_shapes.max(1.0) * 0.3)
            .min(0.95);
        ("flowchart".to_string(), confidence)
    } else if rectangle_count > 2 && arrow_count > 0 && uml_text_score > 0.0 {
        // Likely UML class diagram
        let confidence = (0.3 + uml_text_score * 0.15).min(0.9);
        ("uml_class".to_string(), confidence)
    } else if circle_count > rectangle_count && connector_count > 0 {
        // Likely state diagram or mind map
        ("state_diagram".to_string(), 0.6)
    } else if rectangle_count > 0 && arrow_count > 0 {
        // Generic block diagram
        ("block_diagram".to_string(), 0.5)
    } else {
        ("freeform".to_string(), 0.3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_bounds() {
        let points = vec![
            Point { x: 10.0, y: 20.0, pressure: None, timestamp: 0 },
            Point { x: 100.0, y: 200.0, pressure: None, timestamp: 1 },
            Point { x: 50.0, y: 100.0, pressure: None, timestamp: 2 },
        ];
        
        let bounds = calculate_bounds(&points);
        assert_eq!(bounds.x, 10.0);
        assert_eq!(bounds.y, 20.0);
        assert_eq!(bounds.width, 90.0);
        assert_eq!(bounds.height, 180.0);
    }

    #[test]
    fn test_calculate_centroid() {
        let points = vec![
            Point { x: 0.0, y: 0.0, pressure: None, timestamp: 0 },
            Point { x: 100.0, y: 0.0, pressure: None, timestamp: 1 },
            Point { x: 100.0, y: 100.0, pressure: None, timestamp: 2 },
            Point { x: 0.0, y: 100.0, pressure: None, timestamp: 3 },
        ];
        
        let center = calculate_centroid(&points);
        assert_eq!(center.0, 50.0);
        assert_eq!(center.1, 50.0);
    }

    #[test]
    fn test_is_stroke_closed() {
        let closed_points = vec![
            Point { x: 0.0, y: 0.0, pressure: None, timestamp: 0 },
            Point { x: 100.0, y: 0.0, pressure: None, timestamp: 1 },
            Point { x: 100.0, y: 100.0, pressure: None, timestamp: 2 },
            Point { x: 5.0, y: 5.0, pressure: None, timestamp: 3 },
        ];
        
        assert!(is_stroke_closed(&closed_points, 10.0));
        
        let open_points = vec![
            Point { x: 0.0, y: 0.0, pressure: None, timestamp: 0 },
            Point { x: 100.0, y: 100.0, pressure: None, timestamp: 1 },
        ];
        
        assert!(!is_stroke_closed(&open_points, 10.0));
    }

    #[test]
    fn test_calculate_straightness() {
        let straight_points: Vec<Point> = (0..10)
            .map(|i| Point {
                x: i as f64 * 10.0,
                y: i as f64 * 10.0,
                pressure: None,
                timestamp: i as u64,
            })
            .collect();
        
        let straightness = calculate_straightness(&straight_points);
        assert!(straightness > 0.99);
    }
}
