import type { DetectedShape } from '../store';
import { strokeOutline } from './render';
import { FONT_FAMILY, LINE_HEIGHT } from './text';
import { backgroundFor, exportBounds, type ExportSource } from './imageExport';

/** Two decimals is well under a pixel and keeps the file small. */
function n(value: number): string {
  return (Math.round(value * 100) / 100).toString();
}

function escapeXml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

/** Freehand strokes are filled outlines on the canvas, so they are here too. */
function strokePath(outline: number[][], color: string, opacity: number): string {
  if (outline.length < 2) return '';

  const d =
    `M ${n(outline[0][0])} ${n(outline[0][1])} ` +
    outline
      .slice(1)
      .map(p => `L ${n(p[0])} ${n(p[1])}`)
      .join(' ') +
    ' Z';

  const alpha = opacity < 1 ? ` opacity="${opacity}"` : '';
  return `  <path d="${d}" fill="${color}"${alpha}/>`;
}

function cleanShapeElement(shape: DetectedShape, color: string): string {
  const { x, y, width, height } = shape.bounds;
  const cx = x + width / 2;
  const cy = y + height / 2;
  const common = `fill="none" stroke="${color}" stroke-width="2" stroke-linejoin="round"`;

  switch (shape.shape_type) {
    case 'rectangle':
      return `  <rect x="${n(x)}" y="${n(y)}" width="${n(width)}" height="${n(height)}" ${common}/>`;

    case 'circle':
    case 'ellipse':
      return `  <ellipse cx="${n(cx)}" cy="${n(cy)}" rx="${n(width / 2)}" ry="${n(height / 2)}" ${common}/>`;

    case 'diamond':
      return `  <polygon points="${n(cx)},${n(y)} ${n(x + width)},${n(cy)} ${n(cx)},${n(y + height)} ${n(x)},${n(cy)}" ${common}/>`;

    case 'triangle':
      return `  <polygon points="${n(cx)},${n(y)} ${n(x + width)},${n(y + height)} ${n(x)},${n(y + height)}" ${common}/>`;

    case 'freeform':
      // Nothing to straighten — mark the area, as the canvas does.
      return `  <rect x="${n(x)}" y="${n(y)}" width="${n(width)}" height="${n(height)}" ${common} stroke-dasharray="4 4"/>`;

    default: {
      const start = shape.properties?.start_point;
      const end = shape.properties?.end_point;
      const [x1, y1, x2, y2] = start && end ? [...start, ...end] : [x, y, x + width, y + height];

      const line = `  <line x1="${n(x1)}" y1="${n(y1)}" x2="${n(x2)}" y2="${n(y2)}" ${common}/>`;
      if (shape.shape_type !== 'arrow' && shape.shape_type !== 'connector') return line;

      const angle = Math.atan2(y2 - y1, x2 - x1);
      const spread = Math.PI / 7;
      const size = 12;
      const points = [
        `${n(x2)},${n(y2)}`,
        `${n(x2 - size * Math.cos(angle - spread))},${n(y2 - size * Math.sin(angle - spread))}`,
        `${n(x2 - size * Math.cos(angle + spread))},${n(y2 - size * Math.sin(angle + spread))}`,
      ].join(' ');

      return `${line}\n  <polygon points="${points}" fill="${color}"/>`;
    }
  }
}

/**
 * Vector export of what the canvas is showing, cropped to the drawing.
 *
 * Returns null when there is nothing to export.
 */
export function toSvg(source: ExportSource): string | null {
  const bounds = exportBounds(source);
  if (!bounds) return null;

  const body: string[] = [];
  const lineColor = source.theme === 'dark' ? '#e0e0e0' : '#333333';

  if (source.cleanShapes) {
    for (const shape of source.cleanShapes) {
      body.push(cleanShapeElement(shape, lineColor));
    }
    const consumed = new Set(source.cleanShapes.flatMap(s => s.stroke_ids ?? []));
    for (const stroke of source.strokes) {
      if (consumed.has(stroke.id)) continue;
      body.push(strokePath(strokeOutline(stroke), stroke.color, 0.25));
    }
  } else {
    for (const stroke of source.strokes) {
      body.push(strokePath(strokeOutline(stroke), stroke.color, 1));
    }
  }

  for (const annotation of source.textAnnotations) {
    if (annotation.text.length === 0) continue;

    const lines = annotation.text.split('\n');
    const spans = lines
      .map((line, i) => {
        // The canvas draws with textBaseline 'top'; SVG positions the baseline,
        // so drop by roughly the ascent to land in the same place.
        const y = annotation.y + i * annotation.fontSize * LINE_HEIGHT + annotation.fontSize * 0.8;
        return `<tspan x="${n(annotation.x)}" y="${n(y)}">${escapeXml(line)}</tspan>`;
      })
      .join('');

    body.push(
      `  <text font-family="${escapeXml(FONT_FAMILY)}" font-size="${n(annotation.fontSize)}" ` +
        `fill="${annotation.color}">${spans}</text>`
    );
  }

  return [
    '<?xml version="1.0" encoding="UTF-8"?>',
    `<svg xmlns="http://www.w3.org/2000/svg" width="${n(bounds.width)}" height="${n(bounds.height)}" ` +
      `viewBox="${n(bounds.x)} ${n(bounds.y)} ${n(bounds.width)} ${n(bounds.height)}">`,
    `  <rect x="${n(bounds.x)}" y="${n(bounds.y)}" width="${n(bounds.width)}" ` +
      `height="${n(bounds.height)}" fill="${backgroundFor(source.theme)}"/>`,
    ...body.filter(Boolean),
    '</svg>',
    '',
  ].join('\n');
}
