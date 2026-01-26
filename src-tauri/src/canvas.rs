//! Canvas processing module
//! 
//! Handles canvas data transformation, stroke analysis, and image processing.

use crate::{Point, Stroke};
use image::{DynamicImage, Rgba, RgbaImage};
use serde::{Deserialize, Serialize};

/// Canvas configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasConfig {
    pub width: u32,
    pub height: u32,
    pub background_color: String,
    pub grid_size: Option<u32>,
}

impl Default for CanvasConfig {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            background_color: "#ffffff".to_string(),
            grid_size: Some(20),
        }
    }
}

/// Render strokes to an image
pub fn render_strokes_to_image(
    strokes: &[Stroke],
    config: &CanvasConfig,
) -> DynamicImage {
    let mut img = RgbaImage::from_pixel(
        config.width,
        config.height,
        parse_color(&config.background_color),
    );

    for stroke in strokes {
        let color = parse_color(&stroke.color);
        draw_stroke(&mut img, stroke, color);
    }

    DynamicImage::ImageRgba8(img)
}

/// Parse hex color string to Rgba
fn parse_color(color: &str) -> Rgba<u8> {
    let color = color.trim_start_matches('#');
    let r = u8::from_str_radix(&color[0..2], 16).unwrap_or(0);
    let g = u8::from_str_radix(&color[2..4], 16).unwrap_or(0);
    let b = u8::from_str_radix(&color[4..6], 16).unwrap_or(0);
    let a = if color.len() >= 8 {
        u8::from_str_radix(&color[6..8], 16).unwrap_or(255)
    } else {
        255
    };
    Rgba([r, g, b, a])
}

/// Draw a single stroke on the image using Bresenham's line algorithm
fn draw_stroke(img: &mut RgbaImage, stroke: &Stroke, color: Rgba<u8>) {
    if stroke.points.len() < 2 {
        return;
    }

    let width = stroke.width as i32;
    
    for window in stroke.points.windows(2) {
        let p1 = &window[0];
        let p2 = &window[1];
        draw_line_thick(
            img,
            p1.x as i32,
            p1.y as i32,
            p2.x as i32,
            p2.y as i32,
            width,
            color,
        );
    }
}

/// Draw a thick line using filled circles along the line path
fn draw_line_thick(
    img: &mut RgbaImage,
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
    width: i32,
    color: Rgba<u8>,
) {
    let dx = (x2 - x1).abs();
    let dy = (y2 - y1).abs();
    let sx = if x1 < x2 { 1 } else { -1 };
    let sy = if y1 < y2 { 1 } else { -1 };
    let mut err = dx - dy;

    let mut x = x1;
    let mut y = y1;

    loop {
        draw_circle_filled(img, x, y, width / 2, color);

        if x == x2 && y == y2 {
            break;
        }

        let e2 = 2 * err;
        if e2 > -dy {
            err -= dy;
            x += sx;
        }
        if e2 < dx {
            err += dx;
            y += sy;
        }
    }
}

/// Draw a filled circle at the given position
fn draw_circle_filled(img: &mut RgbaImage, cx: i32, cy: i32, radius: i32, color: Rgba<u8>) {
    let (w, h) = img.dimensions();
    let r_sq = radius * radius;

    for dy in -radius..=radius {
        for dx in -radius..=radius {
            if dx * dx + dy * dy <= r_sq {
                let px = cx + dx;
                let py = cy + dy;
                if px >= 0 && px < w as i32 && py >= 0 && py < h as i32 {
                    img.put_pixel(px as u32, py as u32, color);
                }
            }
        }
    }
}

/// Calculate the bounding box of all strokes
pub fn calculate_bounding_box(strokes: &[Stroke]) -> Option<(f64, f64, f64, f64)> {
    if strokes.is_empty() {
        return None;
    }

    let mut min_x = f64::MAX;
    let mut min_y = f64::MAX;
    let mut max_x = f64::MIN;
    let mut max_y = f64::MIN;

    for stroke in strokes {
        for point in &stroke.points {
            min_x = min_x.min(point.x);
            min_y = min_y.min(point.y);
            max_x = max_x.max(point.x);
            max_y = max_y.max(point.y);
        }
    }

    Some((min_x, min_y, max_x, max_y))
}

/// Normalize strokes to fit within a given viewport
pub fn normalize_strokes(
    strokes: &[Stroke],
    target_width: f64,
    target_height: f64,
    padding: f64,
) -> Vec<Stroke> {
    let bbox = match calculate_bounding_box(strokes) {
        Some(b) => b,
        None => return strokes.to_vec(),
    };

    let (min_x, min_y, max_x, max_y) = bbox;
    let width = max_x - min_x;
    let height = max_y - min_y;

    if width == 0.0 || height == 0.0 {
        return strokes.to_vec();
    }

    let scale_x = (target_width - 2.0 * padding) / width;
    let scale_y = (target_height - 2.0 * padding) / height;
    let scale = scale_x.min(scale_y);

    let offset_x = padding + (target_width - 2.0 * padding - width * scale) / 2.0;
    let offset_y = padding + (target_height - 2.0 * padding - height * scale) / 2.0;

    strokes
        .iter()
        .map(|stroke| Stroke {
            id: stroke.id.clone(),
            color: stroke.color.clone(),
            width: stroke.width * scale,
            tool: stroke.tool.clone(),
            points: stroke
                .points
                .iter()
                .map(|p| Point {
                    x: (p.x - min_x) * scale + offset_x,
                    y: (p.y - min_y) * scale + offset_y,
                    pressure: p.pressure,
                    timestamp: p.timestamp,
                })
                .collect(),
        })
        .collect()
}

/// Simplify strokes using Douglas-Peucker algorithm
pub fn simplify_stroke(points: &[Point], epsilon: f64) -> Vec<Point> {
    if points.len() < 3 {
        return points.to_vec();
    }

    let mut result = douglas_peucker(points, epsilon);
    
    // Ensure we keep at least the start and end points
    if result.len() < 2 && points.len() >= 2 {
        result = vec![points.first().unwrap().clone(), points.last().unwrap().clone()];
    }
    
    result
}

/// Douglas-Peucker algorithm implementation
fn douglas_peucker(points: &[Point], epsilon: f64) -> Vec<Point> {
    if points.len() < 3 {
        return points.to_vec();
    }

    let mut max_distance = 0.0;
    let mut max_index = 0;

    let start = &points[0];
    let end = &points[points.len() - 1];

    for (i, point) in points.iter().enumerate().skip(1).take(points.len() - 2) {
        let distance = perpendicular_distance(point, start, end);
        if distance > max_distance {
            max_distance = distance;
            max_index = i;
        }
    }

    if max_distance > epsilon {
        let mut left = douglas_peucker(&points[..=max_index], epsilon);
        let right = douglas_peucker(&points[max_index..], epsilon);
        
        left.pop(); // Remove duplicate point at junction
        left.extend(right);
        left
    } else {
        vec![start.clone(), end.clone()]
    }
}

/// Calculate perpendicular distance from a point to a line
fn perpendicular_distance(point: &Point, line_start: &Point, line_end: &Point) -> f64 {
    let dx = line_end.x - line_start.x;
    let dy = line_end.y - line_start.y;

    let length_sq = dx * dx + dy * dy;
    
    if length_sq == 0.0 {
        return ((point.x - line_start.x).powi(2) + (point.y - line_start.y).powi(2)).sqrt();
    }

    let t = ((point.x - line_start.x) * dx + (point.y - line_start.y) * dy) / length_sq;
    let t = t.clamp(0.0, 1.0);

    let nearest_x = line_start.x + t * dx;
    let nearest_y = line_start.y + t * dy;

    ((point.x - nearest_x).powi(2) + (point.y - nearest_y).powi(2)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_color() {
        let color = parse_color("#ff0000");
        assert_eq!(color, Rgba([255, 0, 0, 255]));
        
        let color2 = parse_color("#00ff00ff");
        assert_eq!(color2, Rgba([0, 255, 0, 255]));
    }

    #[test]
    fn test_bounding_box() {
        let strokes = vec![Stroke {
            id: "1".to_string(),
            points: vec![
                Point { x: 10.0, y: 20.0, pressure: None, timestamp: 0 },
                Point { x: 100.0, y: 200.0, pressure: None, timestamp: 1 },
            ],
            color: "#000000".to_string(),
            width: 2.0,
            tool: "pen".to_string(),
        }];
        
        let bbox = calculate_bounding_box(&strokes);
        assert_eq!(bbox, Some((10.0, 20.0, 100.0, 200.0)));
    }

    #[test]
    fn test_simplify_stroke() {
        let points = vec![
            Point { x: 0.0, y: 0.0, pressure: None, timestamp: 0 },
            Point { x: 1.0, y: 0.1, pressure: None, timestamp: 1 },
            Point { x: 2.0, y: 0.0, pressure: None, timestamp: 2 },
            Point { x: 3.0, y: 0.1, pressure: None, timestamp: 3 },
            Point { x: 4.0, y: 0.0, pressure: None, timestamp: 4 },
        ];
        
        let simplified = simplify_stroke(&points, 0.5);
        assert!(simplified.len() <= points.len());
    }
}
