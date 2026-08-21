import { useEffect, useRef } from 'react';
import { useStore, type Tool } from '../store';
import { exportDrawio, saveBackup } from './actions';

interface ShortcutHandlers {
  onTogglePreview: () => void;
}

/** Single-key tool switches, matching the toolbar order. */
const TOOL_KEYS: Record<string, Tool> = {
  p: 'pen',
  e: 'eraser',
  t: 'text',
  v: 'select',
  h: 'pan',
};

/** Typing in a field must never trigger a canvas shortcut. */
function isTextEntry(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target.isContentEditable) return true;
  return ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName);
}

/**
 * Global keyboard shortcuts. Reads the store imperatively so the listener is
 * installed once instead of being rebound on every state change.
 */
export function useKeyboardShortcuts({ onTogglePreview }: ShortcutHandlers) {
  const togglePreviewRef = useRef(onTogglePreview);
  togglePreviewRef.current = onTogglePreview;

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (isTextEntry(e.target)) return;

      const state = useStore.getState();
      const mod = e.ctrlKey || e.metaKey;
      const key = e.key.toLowerCase();

      if (mod) {
        switch (key) {
          case 'z':
            e.preventDefault();
            if (e.shiftKey) state.redo();
            else state.undo();
            return;
          case 'y':
            e.preventDefault();
            state.redo();
            return;
          case 'a':
            e.preventDefault();
            state.setTool('select');
            state.selectAll();
            return;
          case 'd':
            e.preventDefault();
            state.duplicateSelected();
            return;
          case 'p':
            e.preventDefault();
            togglePreviewRef.current();
            return;
          case 's':
            e.preventDefault();
            void saveBackup();
            return;
          case 'e':
            e.preventDefault();
            void exportDrawio();
            return;
          case '0':
            e.preventDefault();
            state.setZoom(1);
            state.setPan(0, 0);
            return;
          case '=':
          case '+':
            e.preventDefault();
            state.setZoom(state.zoom * 1.2);
            return;
          case '-':
            e.preventDefault();
            state.setZoom(state.zoom / 1.2);
            return;
          default:
            return;
        }
      }

      if (e.altKey) return;

      if (TOOL_KEYS[key] && !e.shiftKey) {
        e.preventDefault();
        state.setTool(TOOL_KEYS[key]);
        return;
      }

      switch (e.key) {
        case 'Delete':
        case 'Backspace':
          if (state.selectedIds.length > 0) {
            e.preventDefault();
            state.deleteSelected();
          }
          return;
        case 'Escape':
          if (state.selectedIds.length > 0) {
            e.preventDefault();
            state.clearSelection();
          }
          return;
        case 'g':
        case 'G':
          e.preventDefault();
          state.setShowGrid(!state.showGrid);
          return;
        case 'o':
        case 'O':
          if (!state.processingResult) return;
          e.preventDefault();
          state.setShowDetection(!state.showDetection);
          return;
        case 'c':
        case 'C':
          if (!state.processingResult) return;
          e.preventDefault();
          state.setCleanupView(!state.cleanupView);
          return;
        default:
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);
}

/** Shown in the shortcuts help popover. */
export const SHORTCUT_HELP: { keys: string; action: string }[] = [
  { keys: 'P / E / T / V / H', action: 'Pen / Eraser / Text / Select / Pan' },
  { keys: 'Ctrl+Z, Ctrl+Shift+Z', action: 'Undo / Redo' },
  { keys: 'Ctrl+A', action: 'Select all' },
  { keys: 'Ctrl+D', action: 'Duplicate selection' },
  { keys: 'Delete', action: 'Delete selection' },
  { keys: 'Esc', action: 'Clear selection' },
  { keys: 'Ctrl+E', action: 'Export .drawio' },
  { keys: 'Ctrl+S', action: 'Save backup' },
  { keys: 'Ctrl+0 / Ctrl +/-', action: 'Reset view / Zoom' },
  { keys: 'G', action: 'Toggle grid' },
  { keys: 'O', action: 'Detection overlay' },
  { keys: 'C', action: 'Clean-up view' },
  { keys: 'Ctrl+P', action: 'Toggle preview' },
];
