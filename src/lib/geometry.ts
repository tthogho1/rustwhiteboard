import type { Point, Stroke, TextAnnotation } from '../store';

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

/** Union of several strokes' bounds, or null when the list is empty. */
export function unionBounds(strokes: Stroke[]): Rect | null {
  if (strokes.length === 0) return null;

  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;

  for (const stroke of strokes) {
    const b = strokeBounds(stroke);
    if (b.width === 0 && b.height === 0 && stroke.points.length === 0) continue;
    minX = Math.min(minX, b.x);
    minY = Math.min(minY, b.y);
    maxX = Math.max(maxX, b.x + b.width);
    maxY = Math.max(maxY, b.y + b.height);
  }

  if (minX === Infinity) return null;
  return { x: minX, y: minY, width: maxX - minX, height: maxY - minY };
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
