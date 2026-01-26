import { useStore } from '../store';

export function StatusBar() {
  const { strokes, zoom, panX, panY, tool, isProcessing, processingResult } = useStore();

  const strokeCount = strokes.length;
  const pointCount = strokes.reduce((sum, s) => sum + s.points.length, 0);

  return (
    <div className="status-bar">
      <div className="status-section">
        <span className="status-item">
          Tool: <strong>{tool}</strong>
        </span>
      </div>

      <div className="status-section">
        <span className="status-item">
          Strokes: <strong>{strokeCount}</strong>
        </span>
        <span className="status-item">
          Points: <strong>{pointCount}</strong>
        </span>
      </div>

      <div className="status-section">
        <span className="status-item">
          Zoom: <strong>{Math.round(zoom * 100)}%</strong>
        </span>
        <span className="status-item">
          Pan: ({Math.round(panX)}, {Math.round(panY)})
        </span>
      </div>

      {processingResult && (
        <div className="status-section">
          <span className="status-item">
            Shapes: <strong>{processingResult.shapes.length}</strong>
          </span>
          <span className="status-item">
            Text: <strong>{processingResult.text_regions.length}</strong>
          </span>
          <span className="status-item">
            Type: <strong>{processingResult.suggested_diagram_type}</strong>
          </span>
        </div>
      )}

      {isProcessing && (
        <div className="status-section processing">
          <span className="status-item">⏳ Processing...</span>
        </div>
      )}

      <div className="status-section right">
        <span className="status-item hint">
          Ctrl+Scroll: Zoom | Middle-click: Pan | Ctrl+Z/Y: Undo/Redo
        </span>
      </div>
    </div>
  );
}
