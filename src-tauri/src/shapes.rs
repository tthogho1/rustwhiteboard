//! Shape detection and recognition module
//! 
//! Detects geometric shapes (rectangles, circles, arrows, lines) from strokes
//! and classifies diagram types.

use crate::{Point, Stroke};
use crate::ocr::TextRegion;
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;
use geo::{LineString, Simplify};

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
    pub ellipse_threshold: f64,
    pub rectangularity_threshold: f64,
    pub line_straightness_threshold: f64,
    pub arrow_angle_tolerance: f64,
}

impl Default for DetectionParams {
    fn default() -> Self {
        Self {
            min_points: 5,
            circularity_threshold: 0.60,       // フリーハンド用にさらに緩和 (0.70 → 0.60)
            ellipse_threshold: 0.50,           // 楕円検出の閾値
            rectangularity_threshold: 0.50,    // フリーハンド用にさらに緩和 (0.65 → 0.50)
            line_straightness_threshold: 0.75, // 直線検出を緩和 (0.80 → 0.75)
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

/// Smooth a stroke using a moving average to reduce freehand jitter.
/// Runs `passes` times; 2 passes works well for typical freehand noise.
fn smooth_stroke(points: &[Point], window: usize, passes: usize) -> Vec<Point> {
    let mut pts: Vec<Point> = points.to_vec();
    let half = window / 2;
    for _ in 0..passes {
        let src = pts.clone();
        for i in 0..src.len() {
            let lo = i.saturating_sub(half);
            let hi = (i + half + 1).min(src.len());
            let count = (hi - lo) as f64;
            let sx: f64 = src[lo..hi].iter().map(|p| p.x).sum();
            let sy: f64 = src[lo..hi].iter().map(|p| p.y).sum();
            pts[i].x = sx / count;
            pts[i].y = sy / count;
        }
    }
    pts
}

/// Simplify a stroke with Ramer-Douglas-Peucker, removing near-collinear points.
/// `epsilon` is adaptive: ~1.5% of the larger bounding dimension.
fn simplify_stroke(points: &[Point]) -> Vec<Point> {
    if points.len() < 4 {
        return points.to_vec();
    }

    // Compute adaptive epsilon from bounding box
    let min_x = points.iter().map(|p| p.x).fold(f64::MAX, f64::min);
    let max_x = points.iter().map(|p| p.x).fold(f64::MIN, f64::max);
    let min_y = points.iter().map(|p| p.y).fold(f64::MAX, f64::min);
    let max_y = points.iter().map(|p| p.y).fold(f64::MIN, f64::max);
    let max_dim = (max_x - min_x).max(max_y - min_y);
    let epsilon = (max_dim * 0.015).max(1.5); // at least 1.5 px

    let line: LineString<f64> = points
        .iter()
        .map(|p| geo::coord! { x: p.x, y: p.y })
        .collect();

    let simplified = line.simplify(&epsilon);

    simplified
        .coords()
        .map(|c| Point { x: c.x, y: c.y, pressure: None, timestamp: 0 })
        .collect()
}

/// Detect a single shape from a stroke
fn detect_shape_from_stroke(stroke: &Stroke, params: &DetectionParams) -> Option<DetectedShape> {
    let raw_points = &stroke.points;

    // --- Pre-processing pipeline ---
    // 1. Smooth to remove freehand jitter (3-point window, 2 passes)
    let smoothed = smooth_stroke(raw_points, 3, 2);
    // 2. Simplify with RDP to collapse near-collinear points
    let points_vec = simplify_stroke(&smoothed);
    let points = &points_vec;

    log::debug!(
        "Stroke pre-process: {} raw → {} smoothed → {} simplified points",
        raw_points.len(), smoothed.len(), points.len()
    );

    // Calculate basic metrics on the cleaned stroke
    let bounds = calculate_bounds(points);
    let center = calculate_centroid(points);

    // 閉じたストロークの判定（閾値を緩和）
    let close_threshold = bounds.width.max(bounds.height) * 0.15;
    let is_closed = is_stroke_closed(points, close_threshold);

    // 各スコアを計算
    let circularity = calculate_circularity(points, &center);
    let rectangularity = calculate_rectangularity(points, &bounds);
    let straightness = calculate_straightness(points);

    // Count sharp corners to disambiguate circle vs square/rectangle.
    // A true circle has 0–1 sharp corners; a freehand square typically has 3–4.
    // Threshold raised to 70°: a circle simplified to ~5-6 segments has ~60° corners,
    // while a rectangle has ~90° corners — 70° cleanly separates them.
    let sharp_corners = count_sharp_corners(points, 70.0);
    let adjusted_circularity = if sharp_corners >= 3 {
        // Strong corner evidence → suppress circularity significantly
        circularity * (1.0 - (sharp_corners as f64 - 2.0) * 0.25).max(0.0)
    } else if sharp_corners == 2 {
        circularity * 0.75
    } else {
        circularity
    };

    // Compute ellipticality (PCA-based axis fit)
    let ellipticality = calculate_ellipticality(points, &center);

    println!("[SHAPE] Stroke {} raw→{} simplified, bounds: ({:.0}, {:.0}, {:.0}x{:.0})",
        raw_points.len(), points.len(), bounds.x, bounds.y, bounds.width, bounds.height);
    println!("[SHAPE] Metrics: closed={}, circularity={:.2}, adjusted_circularity={:.2} (sharp_corners={}), ellipticality={:.2}, rectangularity={:.2}, straightness={:.2}",
        is_closed, circularity, adjusted_circularity, sharp_corners, ellipticality, rectangularity, straightness);
    println!("[SHAPE] Line check: !is_closed={}, straightness({:.2}) > threshold({:.2}) = {}",
        !is_closed, straightness, params.line_straightness_threshold,
        straightness > params.line_straightness_threshold);
    
    let (shape_type, confidence) = {
        // 開いたストロークの場合、まず直線/矢印をチェック（矩形より優先）
        if !is_closed && straightness > params.line_straightness_threshold {
            println!("[SHAPE] → Detected as LINE or ARROW (open stroke with high straightness)");
            if let Some(_arrow_info) = detect_arrow_head(points, params.arrow_angle_tolerance) {
                (ShapeType::Arrow, straightness * 0.95)
            } else {
                (ShapeType::Line, straightness)
            }
        } else if adjusted_circularity > params.circularity_threshold && (is_closed || adjusted_circularity > 0.8) {
            println!("[SHAPE] → Detected as CIRCLE (high adjusted_circularity={:.2}, sharp_corners={})", adjusted_circularity, sharp_corners);
            // 円形度が高ければ円として判定（ダイヤモンドより優先）
            (ShapeType::Circle, adjusted_circularity)
        } else if ellipticality > params.ellipse_threshold && is_closed && sharp_corners <= 1 {
            println!("[SHAPE] → Detected as ELLIPSE (ellipticality={:.2})", ellipticality);
            // 楕円: 円形度は低いが楕円フィットが高く、コーナーが少ない
            (ShapeType::Ellipse, ellipticality)
        } else if rectangularity > params.rectangularity_threshold && is_closed {
            println!("[SHAPE] → Detected as RECTANGLE or DIAMOND (closed with high rectangularity)");
            // 閉じたストロークで矩形スコアが高い場合
            // ダイヤモンド判定は円形度が低い場合のみ（円をダイヤモンドと誤判定しないため）
            let is_diamond = circularity < 0.5 && check_diamond(points, &center);
            if is_diamond {
                (ShapeType::Diamond, rectangularity * 0.95)
            } else {
                (ShapeType::Rectangle, rectangularity)
            }
        } else if is_closed {
            println!("[SHAPE] → Detected as TRIANGLE or FREEFORM (closed but not rect/circle)");
            // 閉じているが矩形でも円でもない場合
            let triangle_score = calculate_triangle_score(points);
            if triangle_score > 0.75 {
                (ShapeType::Triangle, triangle_score)
            } else {
                (ShapeType::Freeform, 0.5)
            }
        } else {
            println!("[SHAPE] → Detected as CONNECTOR (open with low straightness={:.2})", straightness);
            // 開いたストロークで直線度が低い場合 - コネクタ
            (ShapeType::Connector, 0.6)
        }
    };

    // Log detected shape type and confidence
    println!("[SHAPE] Final result: {:?} with confidence {:.2}", shape_type, confidence);

    let properties = ShapeProperties {
        center_x: center.0,
        center_y: center.1,
        radius: if shape_type == ShapeType::Circle {
            Some(calculate_average_radius(points, &center))
        } else {
            None
        },
        start_point: Some((raw_points.first()?.x, raw_points.first()?.y)),
        end_point: Some((raw_points.last()?.x, raw_points.last()?.y)),
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
    // 最大閾値を設定してフリーハンド誤判定を防ぐ
    let max_threshold = 50.0;
    distance < threshold.min(max_threshold)
}

/// Count sharp corners (direction changes exceeding `angle_threshold_deg` degrees).
/// Used to distinguish squares/rectangles from circles after smoothing.
fn count_sharp_corners(points: &[Point], angle_threshold_deg: f64) -> usize {
    if points.len() < 3 {
        return 0;
    }
    let threshold = angle_threshold_deg.to_radians();
    // Adaptive look-ahead window: ~5% of total points, at least 2
    let window = (points.len() / 20).max(2);
    let mut count = 0;
    let mut i = window;
    while i < points.len().saturating_sub(window) {
        let prev = &points[i - window];
        let curr = &points[i];
        let next = &points[i + window];

        let a1 = (curr.y - prev.y).atan2(curr.x - prev.x);
        let a2 = (next.y - curr.y).atan2(next.x - curr.x);
        let mut diff = (a2 - a1).abs();
        if diff > PI {
            diff = (2.0 * PI) - diff;
        }

        if diff > threshold {
            count += 1;
            i += window; // skip ahead to avoid counting the same corner twice
        } else {
            i += 1;
        }
    }
    count
}

/// Calculate circularity (how close to a circle)
/// サンプリングを使用してフリーハンドのノイズを軽減
fn calculate_circularity(points: &[Point], center: &(f64, f64)) -> f64 {
    let avg_radius = calculate_average_radius(points, center);
    
    if avg_radius == 0.0 {
        return 0.0;
    }

    // サンプリングしてノイズを減らす（最大20点）
    let sample_step = (points.len() / 20).max(1);
    let sampled: Vec<_> = points.iter().step_by(sample_step).collect();

    let variance: f64 = sampled
        .iter()
        .map(|p| {
            let r = ((p.x - center.0).powi(2) + (p.y - center.1).powi(2)).sqrt();
            (r - avg_radius).powi(2)
        })
        .sum::<f64>()
        / sampled.len() as f64;

    let std_dev = variance.sqrt();
    let coefficient_of_variation = std_dev / avg_radius;
    
    // フリーハンド用に許容範囲を広げる（係数を2倍にして緩和）
    (1.0 - coefficient_of_variation * 2.0).max(0.0).min(1.0)
}

/// Calculate ellipticality score using PCA-based axis fitting.
/// Returns 0..1 where higher means a strong ellipse fit with non-circular aspect ratio.
fn calculate_ellipticality(points: &[Point], center: &(f64, f64)) -> f64 {
    if points.len() < 5 {
        return 0.0;
    }

    let cx = center.0;
    let cy = center.1;

    // Covariance matrix of centred points
    let mut sxx = 0.0;
    let mut sxy = 0.0;
    let mut syy = 0.0;
    for p in points {
        let dx = p.x - cx;
        let dy = p.y - cy;
        sxx += dx * dx;
        sxy += dx * dy;
        syy += dy * dy;
    }
    let n = points.len() as f64;
    sxx /= n;
    sxy /= n;
    syy /= n;

    // Eigenvalues of the 2×2 covariance matrix
    let trace = sxx + syy;
    let det = sxx * syy - sxy * sxy;
    let disc = (trace * trace / 4.0 - det).max(0.0).sqrt();
    let lambda1 = trace / 2.0 + disc; // major
    let lambda2 = trace / 2.0 - disc; // minor
    if lambda1 <= 0.0 || lambda2 <= 0.0 {
        return 0.0;
    }

    let major = lambda1.sqrt();
    let minor = lambda2.sqrt();
    if major == 0.0 {
        return 0.0;
    }

    // Aspect ratio (0 = degenerate line, 1 = circle)
    let aspect = (minor / major).clamp(0.0, 1.0);

    // Eigenvector for the major axis
    let (vx, vy) = if sxy.abs() < 1e-12 && sxx >= syy {
        (1.0, 0.0)
    } else if sxy.abs() < 1e-12 {
        (0.0, 1.0)
    } else {
        let v_x = lambda1 - syy;
        let v_y = sxy;
        let norm = (v_x * v_x + v_y * v_y).sqrt().max(1e-12);
        (v_x / norm, v_y / norm)
    };
    // Orthogonal minor-axis direction
    let (ux, uy) = (-vy, vx);

    // Fit score: project each point onto the PCA axes, normalise
    // by the respective semi-axis length, then measure how close
    // the normalised radius is to 1.0 (perfect ellipse).
    let mut sum_r = 0.0;
    let mut sum_r2 = 0.0;
    for p in points {
        let dx = p.x - cx;
        let dy = p.y - cy;
        let proj_major = dx * vx + dy * vy;
        let proj_minor = dx * ux + dy * uy;
        let r = ((proj_major / major.max(1e-6)).powi(2)
               + (proj_minor / minor.max(1e-6)).powi(2))
            .sqrt();
        sum_r += r;
        sum_r2 += r * r;
    }
    let mean_r = sum_r / n;
    let var_r = (sum_r2 / n) - mean_r * mean_r;
    let std_r = var_r.max(0.0).sqrt();

    // fit_score: tight ellipse ⇒ low std_r ⇒ high score
    let fit_score = (1.0 - std_r).clamp(0.0, 1.0);

    // Penalise near-circular shapes (those should stay Circle).
    // aspect close to 1 → near-circle → low eccentricity bonus
    let eccentricity_bonus = if aspect > 0.85 {
        // Very round – prefer Circle classification
        0.0
    } else if aspect > 0.70 {
        0.5
    } else {
        1.0
    };

    (fit_score * (0.5 + 0.5 * eccentricity_bonus)).clamp(0.0, 1.0)
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
/// 辺に沿ったポイント分布とコーナー検出を組み合わせて判定
fn calculate_rectangularity(points: &[Point], bounds: &ShapeBounds) -> f64 {
    let area = bounds.width * bounds.height;
    if area == 0.0 {
        return 0.0;
    }

    // ポイントがバウンディングボックスの辺に沿っているかをチェック
    let edge_threshold = bounds.width.max(bounds.height) * 0.15;
    let mut on_edge_count = 0;

    for p in points {
        let near_left = (p.x - bounds.x).abs() < edge_threshold;
        let near_right = (p.x - (bounds.x + bounds.width)).abs() < edge_threshold;
        let near_top = (p.y - bounds.y).abs() < edge_threshold;
        let near_bottom = (p.y - (bounds.y + bounds.height)).abs() < edge_threshold;

        if near_left || near_right || near_top || near_bottom {
            on_edge_count += 1;
        }
    }

    let edge_ratio = on_edge_count as f64 / points.len() as f64;
    
    // アスペクト比もチェック（極端に細長いものは除外）
    let aspect = bounds.width.min(bounds.height) / bounds.width.max(bounds.height);
    let aspect_score = if aspect > 0.3 { 1.0 } else { aspect / 0.3 };

    // コーナー検出スコア
    let corner_score = detect_corners(points, bounds);

    (edge_ratio * 0.4 + corner_score * 0.4 + aspect_score * 0.2).min(1.0)
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
    // A diamond has points clustered at cardinal directions (corners), not distributed evenly like a circle
    let mut cardinal_scores = [0.0; 4]; // top, right, bottom, left
    let mut diagonal_scores = [0.0; 4]; // top-right, bottom-right, bottom-left, top-left
    
    for point in points {
        let dx = point.x - center.0;
        let dy = point.y - center.1;
        let angle = dy.atan2(dx);
        
        // Check proximity to cardinal directions (diamond corners)
        let cardinal_angles = [-PI / 2.0, 0.0, PI / 2.0, PI];
        for (i, &target_angle) in cardinal_angles.iter().enumerate() {
            let diff = (angle - target_angle).abs();
            if diff < PI / 8.0 || (PI - diff).abs() < PI / 8.0 {
                cardinal_scores[i] += 1.0;
            }
        }
        
        // Check proximity to diagonal directions (should be low for diamond)
        let diagonal_angles = [-PI / 4.0, PI / 4.0, 3.0 * PI / 4.0, -3.0 * PI / 4.0];
        for (i, &target_angle) in diagonal_angles.iter().enumerate() {
            let diff = (angle - target_angle).abs();
            if diff < PI / 8.0 || (PI - diff).abs() < PI / 8.0 {
                diagonal_scores[i] += 1.0;
            }
        }
    }

    let total_cardinal: f64 = cardinal_scores.iter().sum();
    let total_diagonal: f64 = diagonal_scores.iter().sum();
    
    // Diamond should have significantly more points at cardinal directions than diagonals
    // And should have points at all 4 cardinal directions
    let has_all_cardinals = cardinal_scores.iter().all(|&s| s > 0.0);
    let cardinal_dominant = total_cardinal > total_diagonal * 1.5;
    
    has_all_cardinals && cardinal_dominant
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

/// One open stroke that may be a piece of a larger, multi-stroke figure.
struct ChainCandidate<'a> {
    shape: &'a DetectedShape,
    points: &'a [Point],
}

impl<'a> ChainCandidate<'a> {
    /// First point of the stroke, or the last one when walked in reverse.
    fn head(&self, reversed: bool) -> (f64, f64) {
        let p = if reversed { self.points.last() } else { self.points.first() };
        let p = p.expect("candidates always have at least two points");
        (p.x, p.y)
    }

    /// The endpoint the chain continues from after consuming this stroke.
    fn tail(&self, reversed: bool) -> (f64, f64) {
        let p = if reversed { self.points.first() } else { self.points.last() };
        let p = p.expect("candidates always have at least two points");
        (p.x, p.y)
    }

    /// Straight-line distance between the endpoints, used to size tolerances.
    fn span(&self) -> f64 {
        let a = self.head(false);
        let b = self.tail(false);
        distance(a, b)
    }
}

fn distance(a: (f64, f64), b: (f64, f64)) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// How far apart two endpoints may be and still count as the same corner.
/// Scaled to the shorter of the two strokes so that small figures are not
/// glued together by a tolerance meant for large ones.
fn junction_tolerance(span_a: f64, span_b: f64) -> f64 {
    (0.25 * span_a.min(span_b)).clamp(8.0, 40.0)
}

/// Detect figures drawn with several separate strokes.
///
/// A rectangle drawn as four flicks of the pen reaches this point as four
/// `Line` shapes, and would otherwise be exported as four loose connectors.
/// Strokes whose endpoints chain back to where the chain started are merged
/// into a single closed shape. Arrows are never candidates: they are drawn as
/// connectors on purpose.
fn detect_compound_shapes(shapes: &[DetectedShape], strokes: &[Stroke]) -> Vec<DetectedShape> {
    let params = DetectionParams::default();
    let stroke_map: std::collections::HashMap<&str, &Stroke> =
        strokes.iter().map(|s| (s.id.as_str(), s)).collect();

    let candidates: Vec<ChainCandidate> = shapes
        .iter()
        .filter(|s| matches!(s.shape_type, ShapeType::Line | ShapeType::Connector))
        .filter_map(|shape| {
            let stroke = stroke_map.get(shape.stroke_ids.first()?.as_str())?;
            if stroke.points.len() < 2 {
                return None;
            }
            Some(ChainCandidate { shape, points: &stroke.points })
        })
        .collect();

    if candidates.len() < 2 {
        return Vec::new();
    }

    let mut used = vec![false; candidates.len()];
    let mut compound = Vec::new();

    for start in 0..candidates.len() {
        if used[start] {
            continue;
        }
        let Some(chain) = build_closed_chain(&candidates, &used, start) else {
            continue;
        };
        let Some(shape) = shape_from_chain(&candidates, &chain, &params) else {
            continue;
        };
        for &(idx, _) in &chain {
            used[idx] = true;
        }
        println!(
            "[SHAPE] Compound {:?} from {} strokes (confidence {:.2})",
            shape.shape_type, chain.len(), shape.confidence
        );
        compound.push(shape);
    }

    compound
}

/// Walk from `start` along touching endpoints until the chain closes on itself.
///
/// Returns the members in traversal order with a flag saying whether the stroke
/// has to be walked backwards, or `None` when the strokes form an open path.
fn build_closed_chain(
    candidates: &[ChainCandidate],
    used: &[bool],
    start: usize,
) -> Option<Vec<(usize, bool)>> {
    // A chain longer than this is noise rather than a hand-drawn outline.
    const MAX_CHAIN: usize = 12;

    let start_point = candidates[start].head(false);
    let mut chain = vec![(start, false)];
    let mut in_chain = vec![false; candidates.len()];
    in_chain[start] = true;
    let mut current = candidates[start].tail(false);

    loop {
        if chain.len() >= 2 {
            let closing = junction_tolerance(
                candidates[start].span(),
                candidates[chain.last()?.0].span(),
            );
            if distance(current, start_point) <= closing {
                return Some(chain);
            }
        }
        if chain.len() >= MAX_CHAIN {
            return None;
        }

        // Nearest unused stroke whose endpoint meets the one we stopped at.
        let mut best: Option<(usize, bool, f64)> = None;
        for (j, candidate) in candidates.iter().enumerate() {
            if used[j] || in_chain[j] {
                continue;
            }
            let tolerance = junction_tolerance(candidates[chain.last()?.0].span(), candidate.span());
            for reversed in [false, true] {
                let d = distance(current, candidate.head(reversed));
                if d <= tolerance && best.map_or(true, |(_, _, best_d)| d < best_d) {
                    best = Some((j, reversed, d));
                }
            }
        }

        let (next, reversed, _) = best?;
        in_chain[next] = true;
        chain.push((next, reversed));
        current = candidates[next].tail(reversed);
    }
}

/// Turn a closed chain of strokes into a single shape.
fn shape_from_chain(
    candidates: &[ChainCandidate],
    chain: &[(usize, bool)],
    params: &DetectionParams,
) -> Option<DetectedShape> {
    let mut points: Vec<Point> = Vec::new();
    for &(idx, reversed) in chain {
        let member = &candidates[idx];
        if reversed {
            points.extend(member.points.iter().rev().cloned());
        } else {
            points.extend(member.points.iter().cloned());
        }
    }
    if points.len() < 5 {
        return None;
    }

    let bounds = calculate_bounds(&points);
    if bounds.width < 15.0 || bounds.height < 15.0 {
        return None;
    }
    let center = calculate_centroid(&points);

    // Unlike a single stroke, the members here are separate open strokes, so a
    // polygon is by far the likeliest reading: the number of straight members
    // decides the shape, and the metric scores only arbitrate the rest.
    let all_straight = chain
        .iter()
        .all(|&(idx, _)| candidates[idx].shape.shape_type == ShapeType::Line);
    let member_confidence: f64 = chain
        .iter()
        .map(|&(idx, _)| candidates[idx].shape.confidence)
        .sum::<f64>()
        / chain.len() as f64;

    let circularity = calculate_circularity(&points, &center);
    let (shape_type, confidence) = match (chain.len(), all_straight) {
        (3, true) => (ShapeType::Triangle, member_confidence * 0.95),
        (4, true) => {
            if circularity < 0.5 && check_diamond(&points, &center) {
                (ShapeType::Diamond, member_confidence * 0.9)
            } else {
                (ShapeType::Rectangle, member_confidence * 0.95)
            }
        }
        _ => {
            let rectangularity = calculate_rectangularity(&points, &bounds);
            let triangle_score = calculate_triangle_score(&points);
            if rectangularity > params.rectangularity_threshold {
                (ShapeType::Rectangle, rectangularity)
            } else if triangle_score > 0.75 {
                (ShapeType::Triangle, triangle_score)
            } else if circularity > params.circularity_threshold {
                (ShapeType::Circle, circularity)
            } else {
                (ShapeType::Freeform, 0.5)
            }
        }
    };

    let properties = ShapeProperties {
        center_x: center.0,
        center_y: center.1,
        radius: if shape_type == ShapeType::Circle {
            Some(calculate_average_radius(&points, &center))
        } else {
            None
        },
        // A closed figure has no meaningful start/end for connector routing.
        start_point: None,
        end_point: None,
        corner_radius: None,
        arrow_head: None,
    };

    Some(DetectedShape {
        id: uuid::Uuid::new_v4().to_string(),
        shape_type,
        bounds,
        confidence: confidence.clamp(0.0, 1.0),
        stroke_ids: chain
            .iter()
            .flat_map(|&(idx, _)| candidates[idx].shape.stroke_ids.clone())
            .collect(),
        properties,
    })
}

/// Merge individual and compound shapes, dropping the individual strokes that
/// were absorbed into a compound one.
fn merge_shapes(individual: Vec<DetectedShape>, compound: Vec<DetectedShape>) -> Vec<DetectedShape> {
    let consumed: std::collections::HashSet<String> = compound
        .iter()
        .flat_map(|shape| shape.stroke_ids.iter().cloned())
        .collect();

    let mut result: Vec<DetectedShape> = individual
        .into_iter()
        .filter(|shape| !shape.stroke_ids.iter().any(|id| consumed.contains(id)))
        .collect();
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

    /// A straight pen stroke from `from` to `to`, dense enough to be detected.
    fn line_stroke(id: &str, from: (f64, f64), to: (f64, f64)) -> Stroke {
        let steps = 20;
        let points = (0..=steps)
            .map(|i| {
                let t = i as f64 / steps as f64;
                Point {
                    x: from.0 + (to.0 - from.0) * t,
                    y: from.1 + (to.1 - from.1) * t,
                    pressure: None,
                    timestamp: i as u64,
                }
            })
            .collect();

        Stroke {
            id: id.to_string(),
            points,
            color: "#000000".to_string(),
            width: 2.0,
            tool: "pen".to_string(),
        }
    }

    #[test]
    fn test_rectangle_from_four_strokes() {
        let strokes = vec![
            line_stroke("top", (0.0, 0.0), (200.0, 0.0)),
            line_stroke("right", (200.0, 0.0), (200.0, 150.0)),
            line_stroke("bottom", (200.0, 150.0), (0.0, 150.0)),
            line_stroke("left", (0.0, 150.0), (0.0, 0.0)),
        ];

        let shapes = detect_shapes(&strokes);

        assert_eq!(shapes.len(), 1, "the four sides should merge into one shape");
        assert_eq!(shapes[0].shape_type, ShapeType::Rectangle);
        assert_eq!(shapes[0].stroke_ids.len(), 4);
        assert_eq!(shapes[0].bounds.width, 200.0);
        assert_eq!(shapes[0].bounds.height, 150.0);
    }

    #[test]
    fn test_rectangle_from_strokes_drawn_in_any_direction() {
        // Sides drawn away from a shared corner, so two of them need reversing.
        let strokes = vec![
            line_stroke("top", (0.0, 0.0), (200.0, 0.0)),
            line_stroke("left", (0.0, 0.0), (0.0, 150.0)),
            line_stroke("bottom", (0.0, 150.0), (200.0, 150.0)),
            line_stroke("right", (200.0, 0.0), (200.0, 150.0)),
        ];

        let shapes = detect_shapes(&strokes);

        assert_eq!(shapes.len(), 1);
        assert_eq!(shapes[0].shape_type, ShapeType::Rectangle);
    }

    #[test]
    fn test_triangle_from_three_strokes() {
        let strokes = vec![
            line_stroke("a", (100.0, 0.0), (200.0, 160.0)),
            line_stroke("b", (200.0, 160.0), (0.0, 160.0)),
            line_stroke("c", (0.0, 160.0), (100.0, 0.0)),
        ];

        let shapes = detect_shapes(&strokes);

        assert_eq!(shapes.len(), 1);
        assert_eq!(shapes[0].shape_type, ShapeType::Triangle);
    }

    #[test]
    fn test_open_corner_is_not_merged() {
        // Two sides of a box: a chain that never closes stays two lines.
        let strokes = vec![
            line_stroke("top", (0.0, 0.0), (200.0, 0.0)),
            line_stroke("right", (200.0, 0.0), (200.0, 150.0)),
        ];

        let shapes = detect_shapes(&strokes);

        assert_eq!(shapes.len(), 2);
        assert!(shapes.iter().all(|s| s.shape_type == ShapeType::Line));
    }

    #[test]
    fn test_distant_lines_are_not_merged() {
        let strokes = vec![
            line_stroke("a", (0.0, 0.0), (200.0, 0.0)),
            line_stroke("b", (0.0, 300.0), (200.0, 300.0)),
        ];

        let shapes = detect_shapes(&strokes);

        assert_eq!(shapes.len(), 2);
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
