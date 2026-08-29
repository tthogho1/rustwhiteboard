import { useCallback, useState } from 'react';
import { SHAPE_TYPES, useStore, type ShapeType } from '../store';
import { api } from '../lib/api';

/** Gap in screen pixels between the panel and the shape's box. */
const OFFSET_PX = 8;
const PANEL_HEIGHT_PX = 34;

/**
 * Correction panel for the detected shape picked on the detection overlay.
 *
 * Detection is never going to be exact on freehand input, and hunting the right
 * row in the preview list is slow — this sits on the box the user just clicked.
 */
export function ShapeInspector() {
  const selectedShapeId = useStore(state => state.selectedShapeId);
  const processingResult = useStore(state => state.processingResult);
  const zoom = useStore(state => state.zoom);
  const panX = useStore(state => state.panX);
  const panY = useStore(state => state.panY);
  const setProcessingResult = useStore(state => state.setProcessingResult);
  const setSelectedShapeId = useStore(state => state.setSelectedShapeId);

  const [busy, setBusy] = useState(false);

  const shape = processingResult?.shapes.find(s => s.id === selectedShapeId) ?? null;

  // The backend owns the shape list, so its response is the new truth in both
  // handlers below rather than a local patch.
  const handleTypeChange = useCallback(
    async (shapeType: ShapeType) => {
      if (!shape || !processingResult) return;
      setBusy(true);
      try {
        const shapes = await api.updateShapeType(shape.id, shapeType);
        setProcessingResult({ ...processingResult, shapes });
      } catch (error) {
        console.error('Failed to change shape type:', error);
        alert(`Failed to change shape type: ${error}`);
      } finally {
        setBusy(false);
      }
    },
    [shape, processingResult, setProcessingResult]
  );

  const handleDelete = useCallback(async () => {
    if (!shape || !processingResult) return;
    setBusy(true);
    try {
      const shapes = await api.deleteShape(shape.id);
      setProcessingResult({ ...processingResult, shapes });
      setSelectedShapeId(null);
    } catch (error) {
      console.error('Failed to delete the shape:', error);
      alert(`Failed to delete the shape: ${error}`);
    } finally {
      setBusy(false);
    }
  }, [shape, processingResult, setProcessingResult, setSelectedShapeId]);

  if (!shape) return null;

  // Sit above the box, or below it when there is no room at the top.
  const boxTop = shape.bounds.y * zoom + panY;
  const above = boxTop - PANEL_HEIGHT_PX - OFFSET_PX;
  const top = above >= 0 ? above : boxTop + shape.bounds.height * zoom + OFFSET_PX;

  return (
    <div
      className="shape-inspector"
      style={{ left: `${Math.max(0, shape.bounds.x * zoom + panX)}px`, top: `${top}px` }}
    >
      <select
        value={shape.shape_type}
        disabled={busy}
        onChange={e => handleTypeChange(e.target.value as ShapeType)}
        title="Correct the detected type"
      >
        {SHAPE_TYPES.map(type => (
          <option key={type} value={type}>
            {type}
          </option>
        ))}
      </select>
      <button
        className="shape-inspector-btn danger"
        onClick={handleDelete}
        disabled={busy}
        title="Drop this detection (the strokes stay)"
      >
        🗑️
      </button>
      <button
        className="shape-inspector-btn"
        onClick={() => setSelectedShapeId(null)}
        title="Close (Esc)"
      >
        ×
      </button>
    </div>
  );
}
