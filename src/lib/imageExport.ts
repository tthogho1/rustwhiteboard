import type { DetectedShape, Stroke, TextAnnotation, Theme } from '../store';
import { contentBounds, type Rect } from './geometry';
import { drawCleanShapes, drawStroke, drawTextAnnotations, leftoverStrokes } from './render';

/** What the image exports draw, mirroring what the canvas is showing. */
export interface ExportSource {
  strokes: Stroke[];
  textAnnotations: TextAnnotation[];
  /** Detected shapes when the clean-up view is on, otherwise null. */
  cleanShapes: DetectedShape[] | null;
  theme: Theme;
}

/** Guard against a huge canvas allocation on a very large drawing. */
const MAX_PIXELS = 8192;

export function backgroundFor(theme: Theme): string {
  return theme === 'dark' ? '#1a1a1a' : '#ffffff';
}

/** Framing shared by both exports: the drawing, not the current viewport. */
export function exportBounds(source: ExportSource): Rect | null {
  return contentBounds(source.strokes, source.textAnnotations);
}

/**
 * Render the drawing to a PNG data URL, cropped to its contents.
 *
 * `scale` is capped so a large drawing cannot ask for a canvas the browser
 * refuses to allocate — exceeding it silently produces a blank image.
 */
export function renderPngDataUrl(source: ExportSource, scale = 2): string | null {
  const bounds = exportBounds(source);
  if (!bounds) return null;

  const fit = Math.min(
    scale,
    MAX_PIXELS / Math.max(bounds.width, 1),
    MAX_PIXELS / Math.max(bounds.height, 1)
  );
  const effective = Math.max(0.1, fit);

  const canvas = document.createElement('canvas');
  canvas.width = Math.max(1, Math.ceil(bounds.width * effective));
  canvas.height = Math.max(1, Math.ceil(bounds.height * effective));

  const ctx = canvas.getContext('2d');
  if (!ctx) return null;

  ctx.fillStyle = backgroundFor(source.theme);
  ctx.fillRect(0, 0, canvas.width, canvas.height);

  // Same view maths as the on-screen canvas, with the crop as the pan.
  const view = {
    zoom: effective,
    panX: -bounds.x * effective,
    panY: -bounds.y * effective,
  };

  if (source.cleanShapes) {
    drawCleanShapes(ctx, source.cleanShapes, view, source.theme);
    for (const stroke of leftoverStrokes(source.strokes, source.cleanShapes)) {
      drawStroke(ctx, stroke, view, 0.25);
    }
  } else {
    for (const stroke of source.strokes) {
      drawStroke(ctx, stroke, view);
    }
  }

  drawTextAnnotations(ctx, source.textAnnotations, view);

  return canvas.toDataURL('image/png');
}
