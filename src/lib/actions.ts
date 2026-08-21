import { useStore } from '../store';
import { api } from './api';
import { ensureAnalyzed, syncCanvas } from './pipeline';
import { clearAutosave } from './autosave';
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
