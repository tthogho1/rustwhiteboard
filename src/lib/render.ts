import { getStroke } from 'perfect-freehand';
import type { DetectedShape, ProcessingResult, Stroke, TextAnnotation, Theme } from '../store';
import { fontFor, LINE_HEIGHT } from './text';

/** perfect-freehand settings. Shared so the canvas, PNG and SVG all agree. */
export const FREEHAND_OPTIONS = {
  thinning: 0.5,
  smoothing: 0.5,
  streamline: 0.5,
};

/** Outline polygon of a stroke, in canvas coordinates. */
export function strokeOutline(stroke: Stroke): number[][] {
  return getStroke(
    stroke.points.map(p => [p.x, p.y, p.pressure ?? 0.5]),
    {
      ...FREEHAND_OPTIONS,
      size: stroke.width,
      simulatePressure: !stroke.points[0]?.pressure,
    }
  );
}

export interface View {
  zoom: number;
  panX: number;
  panY: number;
}

/** Confidence bands used to colour the detection overlay. */
const CONFIDENCE_COLORS = {
  high: '#28a745',
  medium: '#ffc107',
  low: '#dc3545',
};

function confidenceColor(confidence: number): string {
  if (confidence >= 0.8) return CONFIDENCE_COLORS.high;
  if (confidence >= 0.5) return CONFIDENCE_COLORS.medium;
  return CONFIDENCE_COLORS.low;
}

function begin(ctx: CanvasRenderingContext2D, view: View) {
  ctx.save();
  ctx.translate(view.panX, view.panY);
  ctx.scale(view.zoom, view.zoom);
}

/** Canvas-space size that renders at `px` screen pixels whatever the zoom. */
function screenUnits(px: number, view: View): number {
  return px / view.zoom;
}

/** Draw one freehand stroke. `alpha` fades the strokes no shape claimed. */
export function drawStroke(
  ctx: CanvasRenderingContext2D,
  stroke: Stroke | null,
  view: View,
  alpha = 1
) {
  if (!stroke || stroke.points.length < 2) return;

  const outline = strokeOutline(stroke);
  if (outline.length < 2) return;

  ctx.save();
  ctx.globalAlpha = alpha;
  ctx.translate(view.panX, view.panY);
  ctx.scale(view.zoom, view.zoom);

  ctx.fillStyle = stroke.color;
  ctx.beginPath();
  ctx.moveTo(outline[0][0], outline[0][1]);
  for (let i = 1; i < outline.length; i++) {
    ctx.lineTo(outline[i][0], outline[i][1]);
  }
  ctx.closePath();
  ctx.fill();
  ctx.restore();
}

// ---------------------------------------------------------------------------
// Text annotations
// ---------------------------------------------------------------------------

export function drawTextAnnotations(
  ctx: CanvasRenderingContext2D,
  annotations: TextAnnotation[],
  view: View,
  skipId?: string | null
) {
  if (annotations.length === 0) return;

  begin(ctx, view);
  ctx.textBaseline = 'top';

  for (const annotation of annotations) {
    // The one being edited is covered by the HTML editor; drawing it too would
    // show the text twice.
    if (annotation.id === skipId || annotation.text.length === 0) continue;

    ctx.font = fontFor(annotation.fontSize);
    ctx.fillStyle = annotation.color;

    const lines = annotation.text.split('\n');
    lines.forEach((line, i) => {
      ctx.fillText(line, annotation.x, annotation.y + i * annotation.fontSize * LINE_HEIGHT);
    });
  }

  ctx.restore();
}

// ---------------------------------------------------------------------------
// Detection overlay
// ---------------------------------------------------------------------------

/**
 * Draw what the backend detected on top of the drawing: shape bounds coloured
 * by confidence with their classification, and the OCR text regions. This is
 * the only way to see a misdetection without opening the preview panel.
 */
export function drawDetectionOverlay(
  ctx: CanvasRenderingContext2D,
  result: ProcessingResult,
  view: View,
  isStale: boolean,
  selectedShapeId?: string | null
) {
  begin(ctx, view);
  ctx.lineWidth = screenUnits(1.5, view);
  ctx.textBaseline = 'bottom';
  // A stale result no longer describes the canvas, so show it faded.
  ctx.globalAlpha = isStale ? 0.4 : 1;

  const labelFont = fontFor(screenUnits(11, view));
  const labelGap = screenUnits(3, view);

  for (const shape of result.shapes) {
    const color = confidenceColor(shape.confidence);
    const { x, y, width, height } = shape.bounds;
    const picked = shape.id === selectedShapeId;

    // The one being corrected gets a solid, heavier outline so it stands out
    // from the dashed boxes around it.
    ctx.strokeStyle = color;
    ctx.lineWidth = screenUnits(picked ? 3 : 1.5, view);
    if (!picked) ctx.setLineDash([screenUnits(5, view), screenUnits(3, view)]);
    ctx.strokeRect(x, y, width, height);
    ctx.setLineDash([]);

    ctx.font = labelFont;
    ctx.fillStyle = color;
    ctx.fillText(
      `${shape.shape_type} ${Math.round(shape.confidence * 100)}%`,
      x,
      y - labelGap
    );
  }

  ctx.lineWidth = screenUnits(1.5, view);

  for (const region of result.text_regions) {
    const { x, y, width, height } = region.bounds;

    ctx.strokeStyle = '#8800ff';
    ctx.setLineDash([screenUnits(2, view), screenUnits(2, view)]);
    ctx.strokeRect(x, y, width, height);
    ctx.setLineDash([]);
  }

  ctx.restore();
}

// ---------------------------------------------------------------------------
// Clean-up view
// ---------------------------------------------------------------------------

function pathForShape(ctx: CanvasRenderingContext2D, shape: DetectedShape): boolean {
  const { x, y, width, height } = shape.bounds;
  const cx = x + width / 2;
  const cy = y + height / 2;

  switch (shape.shape_type) {
    case 'rectangle':
      ctx.beginPath();
      ctx.rect(x, y, width, height);
      return true;

    case 'circle':
    case 'ellipse':
      ctx.beginPath();
      ctx.ellipse(cx, cy, width / 2, height / 2, 0, 0, Math.PI * 2);
      return true;

    case 'diamond':
      ctx.beginPath();
      ctx.moveTo(cx, y);
      ctx.lineTo(x + width, cy);
      ctx.lineTo(cx, y + height);
      ctx.lineTo(x, cy);
      ctx.closePath();
      return true;

    case 'triangle':
      ctx.beginPath();
      ctx.moveTo(cx, y);
      ctx.lineTo(x + width, y + height);
      ctx.lineTo(x, y + height);
      ctx.closePath();
      return true;

    default:
      return false;
  }
}

/** Endpoints of a line-like shape, falling back to the bounds diagonal. */
function endpointsOf(shape: DetectedShape): [number, number, number, number] {
  const start = shape.properties?.start_point;
  const end = shape.properties?.end_point;
  if (start && end) return [start[0], start[1], end[0], end[1]];

  const { x, y, width, height } = shape.bounds;
  return [x, y, x + width, y + height];
}

function drawArrowHead(
  ctx: CanvasRenderingContext2D,
  x1: number,
  y1: number,
  x2: number,
  y2: number,
  size: number
) {
  const angle = Math.atan2(y2 - y1, x2 - x1);
  const spread = Math.PI / 7;

  ctx.beginPath();
  ctx.moveTo(x2, y2);
  ctx.lineTo(x2 - size * Math.cos(angle - spread), y2 - size * Math.sin(angle - spread));
  ctx.lineTo(x2 - size * Math.cos(angle + spread), y2 - size * Math.sin(angle + spread));
  ctx.closePath();
  ctx.fill();
}

/**
 * Redraw the detected shapes as clean geometry — what the .drawio export will
 * roughly look like. Strokes that no shape claimed are drawn by the caller.
 */
export function drawCleanShapes(
  ctx: CanvasRenderingContext2D,
  shapes: DetectedShape[],
  view: View,
  theme: Theme
) {
  if (shapes.length === 0) return;

  const line = theme === 'dark' ? '#e0e0e0' : '#333333';
  const fill = theme === 'dark' ? 'rgba(77, 166, 255, 0.12)' : 'rgba(0, 102, 204, 0.07)';

  begin(ctx, view);
  ctx.lineWidth = screenUnits(2, view);
  ctx.lineJoin = 'round';

  for (const shape of shapes) {
    ctx.strokeStyle = line;
    ctx.fillStyle = fill;

    if (pathForShape(ctx, shape)) {
      ctx.fill();
      ctx.stroke();
      continue;
    }

    if (shape.shape_type === 'freeform') {
      // Nothing sensible to straighten — just mark the area.
      ctx.setLineDash([screenUnits(4, view), screenUnits(4, view)]);
      ctx.strokeRect(shape.bounds.x, shape.bounds.y, shape.bounds.width, shape.bounds.height);
      ctx.setLineDash([]);
      continue;
    }

    const [x1, y1, x2, y2] = endpointsOf(shape);
    ctx.beginPath();
    ctx.moveTo(x1, y1);
    ctx.lineTo(x2, y2);
    ctx.stroke();

    if (shape.shape_type === 'arrow' || shape.shape_type === 'connector') {
      ctx.fillStyle = line;
      drawArrowHead(ctx, x1, y1, x2, y2, screenUnits(12, view));
    }
  }

  ctx.restore();
}

/** Ids of every stroke that a detected shape absorbed. */
export function consumedStrokeIds(shapes: DetectedShape[]): Set<string> {
  const ids = new Set<string>();
  for (const shape of shapes) {
    for (const id of shape.stroke_ids ?? []) ids.add(id);
  }
  return ids;
}

/** Strokes left over once the detected shapes have taken theirs. */
export function leftoverStrokes(strokes: Stroke[], shapes: DetectedShape[]): Stroke[] {
  const consumed = consumedStrokeIds(shapes);
  return strokes.filter(s => !consumed.has(s.id));
}
