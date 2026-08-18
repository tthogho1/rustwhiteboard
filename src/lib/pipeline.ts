import { useStore } from '../store';
import type { ProcessingResult } from '../store';
import { api } from './api';

function getCanvas(): HTMLCanvasElement {
  const canvas = document.querySelector('.drawing-canvas') as HTMLCanvasElement | null;
  if (!canvas) throw new Error('Canvas not found');
  return canvas;
}

/**
 * Push the canvas strokes to the backend and make sure the shapes / text it
 * holds actually describe them.
 *
 * Detection is skipped when nothing changed since the last run, which is what
 * keeps label corrections made in the preview alive until export. Pass
 * `{ force: true }` for an explicit "Analyze" click.
 */
export async function ensureAnalyzed(
  options: { force?: boolean } = {}
): Promise<ProcessingResult | null> {
  const { strokes, processingResult, isAnalysisStale, setProcessingResult, setAnalysisStale } =
    useStore.getState();

  await api.syncStrokes(strokes);

  if (!options.force && processingResult && !isAnalysisStale) {
    return processingResult;
  }

  const canvas = getCanvas();
  const result = await api.processCanvas(
    canvas.toDataURL('image/png'),
    canvas.width,
    canvas.height
  );

  setProcessingResult(result);
  setAnalysisStale(false);
  return result;
}
