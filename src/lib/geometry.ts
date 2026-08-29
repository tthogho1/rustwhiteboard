import type { DetectedShape, Point, Stroke, TextAnnotation } from '../store';

export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** Axis-aligned bounds of one stroke, in canvas coordinates. */
export function strokeBounds(stroke: Stroke): Rect {
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;

  for (const p of stroke.points) {
    if (p.x < minX) minX = p.x;
    if (p.y < minY) minY = p.y;
    if (p.x > maxX) maxX = p.x;
    if (p.y > maxY) maxY = p.y;
  }

  if (minX === Infinity) return { x: 0, y: 0, width: 0, height: 0 };

  // Half the pen width bleeds outside the point path on every side.
  const pad = stroke.width / 2;
  return {
    x: minX - pad,
    y: minY - pad,
    width: maxX - minX + stroke.width,
    height: maxY - minY + stroke.width,
  };
}

/** Smallest rect containing all of them, or null when there are none. */
export function unionRects(rects: Rect[]): Rect | null {
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;

  for (const r of rects) {
    minX = Math.min(minX, r.x);
    minY = Math.min(minY, r.y);
    maxX = Math.max(maxX, r.x + r.width);
    maxY = Math.max(maxY, r.y + r.height);
  }

  if (minX === Infinity) return null;
  return { x: minX, y: minY, width: maxX - minX, height: maxY - minY };
}

/**
 * Bounding box of the current selection. The selection spans both strokes and
 * text annotations, so both have to be measured to draw one box around it.
 */
export function selectionBounds(
  strokes: Stroke[],
  annotations: TextAnnotation[],
  selectedIds: string[]
): Rect | null {
  if (selectedIds.length === 0) return null;
  const selected = new Set(selectedIds);

  const rects: Rect[] = [];
  for (const stroke of strokes) {
    if (selected.has(stroke.id) && stroke.points.length > 0) rects.push(strokeBounds(stroke));
  }
  for (const annotation of annotations) {
    if (selected.has(annotation.id)) rects.push(annotationBounds(annotation));
  }

  return unionRects(rects);
}

export function rectContains(rect: Rect, x: number, y: number, pad = 0): boolean {
  return (
    x >= rect.x - pad &&
    x <= rect.x + rect.width + pad &&
    y >= rect.y - pad &&
    y <= rect.y + rect.height + pad
  );
}

/** Rect spanning two corner points, normalised so width/height are positive. */
export function rectFromCorners(a: { x: number; y: number }, b: { x: number; y: number }): Rect {
  return {
    x: Math.min(a.x, b.x),
    y: Math.min(a.y, b.y),
    width: Math.abs(a.x - b.x),
    height: Math.abs(a.y - b.y),
  };
}

function distanceToSegment(px: number, py: number, a: Point, b: Point): number {
  const dx = b.x - a.x;
  const dy = b.y - a.y;
  const lenSq = dx * dx + dy * dy;

  if (lenSq === 0) return Math.hypot(px - a.x, py - a.y);

  let t = ((px - a.x) * dx + (py - a.y) * dy) / lenSq;
  t = Math.max(0, Math.min(1, t));

  return Math.hypot(px - (a.x + t * dx), py - (a.y + t * dy));
}

/**
 * Shortest distance from a canvas-space point to the stroke's path. Used for
 * hit testing, so an unfilled shape is only picked up near its outline rather
 * than anywhere inside its bounding box.
 */
export function distanceToStroke(stroke: Stroke, x: number, y: number): number {
  const pts = stroke.points;
  if (pts.length === 0) return Infinity;
  if (pts.length === 1) return Math.hypot(x - pts[0].x, y - pts[0].y);

  let min = Infinity;
  for (let i = 1; i < pts.length; i++) {
    const d = distanceToSegment(x, y, pts[i - 1], pts[i]);
    if (d < min) min = d;
  }
  return min;
}

/** True when any point of the stroke falls inside the rect (marquee semantics). */
export function strokeIntersectsRect(stroke: Stroke, rect: Rect): boolean {
  for (const p of stroke.points) {
    if (rectContains(rect, p.x, p.y)) return true;
  }
  return false;
}

/**
 * Topmost stroke within `threshold` canvas units of the point, or null.
 * Later strokes are drawn on top, so the search runs back to front.
 */
export function hitTestStroke(strokes: Stroke[], x: number, y: number, threshold: number): Stroke | null {
  for (let i = strokes.length - 1; i >= 0; i--) {
    const stroke = strokes[i];
    if (distanceToStroke(stroke, x, y) <= threshold + stroke.width / 2) {
      return stroke;
    }
  }
  return null;
}

/** Clickable box of a text annotation; empty ones still need a grab target. */
export function annotationBounds(annotation: TextAnnotation): Rect {
  return {
    x: annotation.x,
    y: annotation.y,
    width: Math.max(annotation.width, 24),
    height: Math.max(annotation.height, annotation.fontSize),
  };
}

/** Topmost annotation under the point, or null. Later ones draw on top. */
export function hitTestAnnotation(
  annotations: TextAnnotation[],
  x: number,
  y: number,
  pad = 0
): TextAnnotation | null {
  for (let i = annotations.length - 1; i >= 0; i--) {
    if (rectContains(annotationBounds(annotations[i]), x, y, pad)) return annotations[i];
  }
  return null;
}

/** Shortest distance from a point to the rectangle's outline (inside or out). */
export function distanceToRectBorder(rect: Rect, x: number, y: number): number {
  const { x: rx, y: ry, width, height } = rect;
  const corners: Point[] = [
    { x: rx, y: ry, timestamp: 0 },
    { x: rx + width, y: ry, timestamp: 0 },
    { x: rx + width, y: ry + height, timestamp: 0 },
    { x: rx, y: ry + height, timestamp: 0 },
  ];

  let min = Infinity;
  for (let i = 0; i < corners.length; i++) {
    const d = distanceToSegment(x, y, corners[i], corners[(i + 1) % corners.length]);
    if (d < min) min = d;
  }
  return min;
}

/**
 * Detected shape whose overlay box the point is on, or null.
 *
 * Deliberately keyed on the *border* rather than the whole box: the overlay
 * covers the drawing, and a shape's interior has to stay clickable for
 * selecting the strokes underneath it. Smaller boxes win, so a shape nested
 * inside another is still reachable.
 */
export function hitTestDetection(
  shapes: DetectedShape[],
  x: number,
  y: number,
  threshold: number
): DetectedShape | null {
  let best: DetectedShape | null = null;
  let bestArea = Infinity;

  for (const shape of shapes) {
    const rect: Rect = {
      x: shape.bounds.x,
      y: shape.bounds.y,
      width: shape.bounds.width,
      height: shape.bounds.height,
    };
    if (distanceToRectBorder(rect, x, y) > threshold) continue;

    const area = rect.width * rect.height;
    if (area < bestArea) {
      best = shape;
      bestArea = area;
    }
  }

  return best;
}

/** True when two rects share any area (marquee test for boxed items). */
export function rectsOverlap(a: Rect, b: Rect): boolean {
  return (
    a.x < b.x + b.width &&
    a.x + a.width > b.x &&
    a.y < b.y + b.height &&
    a.y + a.height > b.y
  );
}
