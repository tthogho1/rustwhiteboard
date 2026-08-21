import type { TextAnnotation } from '../store';

/** Canvas-space line spacing multiplier used for both rendering and measuring. */
export const LINE_HEIGHT = 1.3;

export const FONT_FAMILY = "-apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif";

export function fontFor(fontSize: number): string {
  return `${fontSize}px ${FONT_FAMILY}`;
}

/** Shared offscreen context — measuring must not disturb the visible canvas. */
let measureCtx: CanvasRenderingContext2D | null = null;

function getMeasureCtx(): CanvasRenderingContext2D | null {
  if (measureCtx) return measureCtx;
  measureCtx = document.createElement('canvas').getContext('2d');
  return measureCtx;
}

/**
 * Canvas-space size of a text annotation. Stored on the annotation itself so
 * hit testing and the exported bounds agree with what was drawn, without
 * re-measuring on every frame.
 */
export function measureText(text: string, fontSize: number): { width: number; height: number } {
  const lines = text.split('\n');
  const height = Math.max(1, lines.length) * fontSize * LINE_HEIGHT;

  const ctx = getMeasureCtx();
  if (!ctx) {
    // Rough fallback: average glyph width is around 0.6em.
    const longest = lines.reduce((max, line) => Math.max(max, line.length), 0);
    return { width: longest * fontSize * 0.6, height };
  }

  ctx.font = fontFor(fontSize);
  const width = lines.reduce((max, line) => Math.max(max, ctx.measureText(line).width), 0);
  return { width, height };
}

/** Re-measure an annotation after its text or size changed. */
export function withMeasuredSize(annotation: TextAnnotation): TextAnnotation {
  const { width, height } = measureText(annotation.text, annotation.fontSize);
  return { ...annotation, width, height };
}

/** A blank annotation anchored so the click sits on its first line. */
export function createTextAnnotation(
  x: number,
  y: number,
  color: string,
  fontSize: number
): TextAnnotation {
  return {
    id: Math.random().toString(36).substring(2, 15),
    text: '',
    x,
    y: y - (fontSize * LINE_HEIGHT) / 2,
    width: 0,
    height: fontSize * LINE_HEIGHT,
    color,
    fontSize,
  };
}
