import type { Stroke, TextAnnotation } from '../store';

const STORAGE_KEY = 'rustwhiteboard-autosave';
/** v1 held only strokes; v2 added text annotations. v1 payloads still load. */
const FORMAT_VERSION = 2;
const DEBOUNCE_MS = 600;

export interface CanvasContents {
  strokes: Stroke[];
  textAnnotations: TextAnnotation[];
}

interface AutosavePayload extends Partial<CanvasContents> {
  version: number;
  savedAt: number;
}

let timer: ReturnType<typeof setTimeout> | null = null;
let lastStrokes: Stroke[] | null = null;
let lastText: TextAnnotation[] | null = null;
/** Set after a write fails (usually a full localStorage) so we warn only once. */
let writesDisabled = false;

function isStroke(value: unknown): value is Stroke {
  if (typeof value !== 'object' || value === null) return false;
  const s = value as Partial<Stroke>;
  return (
    typeof s.id === 'string' &&
    typeof s.color === 'string' &&
    typeof s.width === 'number' &&
    typeof s.tool === 'string' &&
    Array.isArray(s.points) &&
    s.points.every(p => typeof p?.x === 'number' && typeof p?.y === 'number')
  );
}

function isTextAnnotation(value: unknown): value is TextAnnotation {
  if (typeof value !== 'object' || value === null) return false;
  const a = value as Partial<TextAnnotation>;
  return (
    typeof a.id === 'string' &&
    typeof a.text === 'string' &&
    typeof a.x === 'number' &&
    typeof a.y === 'number' &&
    typeof a.fontSize === 'number'
  );
}

/**
 * Canvas contents from the previous session, or empty lists when there is no
 * usable autosave. Anything malformed is discarded rather than crashing the boot.
 */
export function loadAutosavedCanvas(): CanvasContents {
  const empty: CanvasContents = { strokes: [], textAnnotations: [] };

  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return empty;

    const payload = JSON.parse(raw) as AutosavePayload;
    if (!payload || payload.version > FORMAT_VERSION) return empty;

    const strokes = Array.isArray(payload.strokes) ? payload.strokes.filter(isStroke) : [];
    const textAnnotations = Array.isArray(payload.textAnnotations)
      ? payload.textAnnotations.filter(isTextAnnotation)
      : [];

    if (strokes.length > 0 || textAnnotations.length > 0) {
      console.log(
        `[AUTOSAVE] restored ${strokes.length} strokes and ` +
          `${textAnnotations.length} text annotations from the last session`
      );
    }
    return { strokes, textAnnotations };
  } catch (error) {
    console.warn('[AUTOSAVE] failed to restore the previous session:', error);
    return empty;
  }
}

function write(contents: CanvasContents) {
  if (writesDisabled) return;

  try {
    const payload: AutosavePayload = {
      version: FORMAT_VERSION,
      savedAt: Date.now(),
      strokes: contents.strokes,
      textAnnotations: contents.textAnnotations,
    };
    localStorage.setItem(STORAGE_KEY, JSON.stringify(payload));
    lastStrokes = contents.strokes;
    lastText = contents.textAnnotations;
  } catch (error) {
    writesDisabled = true;
    console.warn(
      '[AUTOSAVE] could not save the canvas (storage is probably full); ' +
        'use Backup to keep this drawing:',
      error
    );
  }
}

/**
 * Queue an autosave. Cheap to call on every render: identical contents are
 * ignored, and the actual serialisation is debounced.
 */
export function scheduleAutosave(contents: CanvasContents) {
  if (contents.strokes === lastStrokes && contents.textAnnotations === lastText) return;

  if (timer) clearTimeout(timer);
  timer = setTimeout(() => {
    timer = null;
    write(contents);
  }, DEBOUNCE_MS);
}

/** Flush a pending autosave immediately (used when the window is closing). */
export function flushAutosave(contents: CanvasContents) {
  if (timer) {
    clearTimeout(timer);
    timer = null;
  }
  write(contents);
}

export function clearAutosave() {
  if (timer) {
    clearTimeout(timer);
    timer = null;
  }
  lastStrokes = null;
  lastText = null;
  try {
    localStorage.removeItem(STORAGE_KEY);
  } catch (error) {
    console.warn('[AUTOSAVE] failed to clear the saved session:', error);
  }
}
