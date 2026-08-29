import { useStore } from '../store';
import { api } from './api';
import { ensureAnalyzed, syncCanvas } from './pipeline';
import { clearAutosave } from './autosave';
import { renderPngDataUrl, type ExportSource } from './imageExport';
import { toSvg } from './svg';
import { withMeasuredSize } from './text';
import type { ExportOptions } from './api';

/**
 * Canvas-level commands shared by the toolbar buttons and the keyboard
 * shortcuts. They read the store directly, so they take no arguments and can be
 * called from anywhere.
 */

function exportOptions(): ExportOptions {
  return {
    filename: 'Untitled',
    include_grid: true,
    page_width: 1920,
    page_height: 1080,
    theme: useStore.getState().theme,
  };
}

export async function exportDrawio(): Promise<void> {
  const { strokes } = useStore.getState();
  if (strokes.length === 0) {
    alert('Please draw something first!');
    return;
  }

  try {
    const filePath = await api.saveFile('diagram.drawio', [
      { name: 'Draw.io', extensions: ['drawio'] },
    ]);
    if (!filePath) return;

    // Ensure the backend holds shapes matching the canvas. Already-analyzed
    // canvases are not re-detected, so label corrections survive the export.
    await ensureAnalyzed();
    await api.exportDrawioFile(filePath, exportOptions());

    alert(`Exported to ${filePath}`);
  } catch (error) {
    console.error('Export failed:', error);
    alert(`Export failed: ${error}`);
  }
}

/**
 * What the image exports draw. Mirrors the canvas: with the clean-up view on
 * they get the straightened geometry, otherwise the freehand strokes.
 */
function exportSource(): ExportSource {
  const { strokes, textAnnotations, theme, cleanupView, processingResult } = useStore.getState();
  const shapes = processingResult?.shapes ?? [];

  return {
    strokes,
    textAnnotations,
    cleanShapes: cleanupView && shapes.length > 0 ? shapes : null,
    theme,
  };
}

function isEmptyCanvas(): boolean {
  const { strokes, textAnnotations } = useStore.getState();
  if (strokes.length === 0 && textAnnotations.length === 0) {
    alert('Please draw something first!');
    return true;
  }
  return false;
}

export async function exportPng(): Promise<void> {
  if (isEmptyCanvas()) return;

  try {
    const dataUrl = renderPngDataUrl(exportSource());
    if (!dataUrl) {
      alert('Nothing to export.');
      return;
    }

    const filePath = await api.saveFile('diagram.png', [
      { name: 'PNG Image', extensions: ['png'] },
    ]);
    if (!filePath) return;

    await api.exportPngFile(filePath, dataUrl);
    alert(`Exported to ${filePath}`);
  } catch (error) {
    console.error('PNG export failed:', error);
    alert(`PNG export failed: ${error}`);
  }
}

export async function exportSvg(): Promise<void> {
  if (isEmptyCanvas()) return;

  try {
    const svg = toSvg(exportSource());
    if (!svg) {
      alert('Nothing to export.');
      return;
    }

    const filePath = await api.saveFile('diagram.svg', [
      { name: 'SVG Image', extensions: ['svg'] },
    ]);
    if (!filePath) return;

    await api.exportSvgFile(filePath, svg);
    alert(`Exported to ${filePath}`);
  } catch (error) {
    console.error('SVG export failed:', error);
    alert(`SVG export failed: ${error}`);
  }
}

/**
 * Read a .drawio file onto the canvas. The shapes come back as strokes tracing
 * their outlines, so everything stays editable and a re-run of Analyze detects
 * them again.
 */
export async function importDrawio(): Promise<void> {
  const { strokes, textAnnotations, replaceCanvas } = useStore.getState();

  try {
    const filePath = await api.openFile([
      { name: 'Draw.io', extensions: ['drawio', 'xml'] },
    ]);
    if (!filePath) return;

    if (strokes.length > 0 || textAnnotations.length > 0) {
      const proceed = await api.confirm(
        'Import .drawio',
        'This replaces everything currently on the canvas. Continue?'
      );
      if (!proceed) return;
    }

    const data = await api.importDrawioFile(filePath);
    replaceCanvas({
      strokes: data.strokes,
      // The backend can only estimate text sizes; measure them for real so hit
      // testing and the exported bounds match what gets drawn.
      textAnnotations: data.text_annotations.map(withMeasuredSize),
    });

    alert(
      `Imported ${data.strokes.length} shapes and ${data.text_annotations.length} labels.`
    );
  } catch (error) {
    console.error('Import failed:', error);
    alert(`Import failed: ${error}`);
  }
}

export async function saveBackup(): Promise<void> {
  try {
    const filePath = await api.saveFile('whiteboard-backup.rwb.gz', [
      { name: 'Whiteboard Backup', extensions: ['rwb.gz'] },
    ]);
    if (!filePath) return;

    // save_backup serialises the *backend* copy, so push the canvas first.
    await syncCanvas();
    await api.saveBackup(filePath);
    alert('Backup saved!');
  } catch (error) {
    console.error('Backup failed:', error);
    alert(`Backup failed: ${error}`);
  }
}

export async function restoreBackup(): Promise<void> {
  const { strokes, textAnnotations, replaceCanvas } = useStore.getState();

  try {
    const filePath = await api.openFile([
      { name: 'Whiteboard Backup', extensions: ['gz', 'rwb'] },
    ]);
    if (!filePath) return;

    if (strokes.length > 0 || textAnnotations.length > 0) {
      const proceed = await api.confirm(
        'Restore Backup',
        'This replaces everything currently on the canvas. Continue?'
      );
      if (!proceed) return;
    }

    const backup = await api.loadBackup(filePath);
    // One call, so the restore is a single undo step.
    replaceCanvas({
      strokes: backup.strokes,
      textAnnotations: backup.text_annotations,
    });

    const textNote =
      backup.version < 2
        ? ' (this backup predates the text tool, so it carries no text)'
        : ` and ${backup.text_annotations.length} text annotations`;
    alert(`Restored ${backup.strokes.length} strokes${textNote}.`);
  } catch (error) {
    console.error('Restore failed:', error);
    alert(`Restore failed: ${error}`);
  }
}

export async function clearCanvas(): Promise<void> {
  useStore.getState().clearStrokes();
  // Drop the saved session straight away rather than waiting for the debounce,
  // so a crash right after Clear cannot resurrect the drawing.
  clearAutosave();

  try {
    await api.clearStrokes();
  } catch (error) {
    // The canvas is already cleared; a stale backend only matters on re-analysis.
    console.error('Failed to clear backend state:', error);
  }
}
