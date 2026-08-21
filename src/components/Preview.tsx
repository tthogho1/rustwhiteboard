import { SHAPE_TYPES, useStore, type ShapeType } from '../store';
import { useCallback, useState } from 'react';
import { api } from '../lib/api';
import { ensureAnalyzed } from '../lib/pipeline';
import { exportDrawio } from '../lib/actions';
import { measureText } from '../lib/text';

interface PreviewProps {
  onClose: () => void;
}

export function Preview({ onClose }: PreviewProps) {
  const {
    processingResult,
    theme,
    setProcessingResult,
    textAnnotations,
    updateTextAnnotation,
    removeTextAnnotation,
    saveHistory,
  } = useStore();
  const [activeTab, setActiveTab] = useState<'shapes' | 'text' | 'notes' | 'xml'>('shapes');
  const [xmlPreview, setXmlPreview] = useState<string>('');
  const [editedLabels, setEditedLabels] = useState<Record<string, string>>({});

  /**
   * Send edited labels to the backend, which is where `generate_drawio` reads
   * them from. Without this the edits would stay in this component only.
   */
  const commitLabels = useCallback(async () => {
    if (Object.keys(editedLabels).length === 0) return;

    try {
      const regions = await api.updateTextLabels(editedLabels);
      if (processingResult) {
        setProcessingResult({ ...processingResult, text_regions: regions });
      }
      setEditedLabels({});
    } catch (error) {
      console.error('Failed to save label edits:', error);
      alert(`Failed to save label edits: ${error}`);
      throw error;
    }
  }, [editedLabels, processingResult, setProcessingResult]);

  /**
   * Override a misdetected classification. The backend owns the shape list, so
   * take its response as the new truth rather than patching locally.
   */
  const handleShapeTypeChange = useCallback(
    async (shapeId: string, shapeType: ShapeType) => {
      try {
        const shapes = await api.updateShapeType(shapeId, shapeType);
        if (processingResult) {
          setProcessingResult({ ...processingResult, shapes });
        }
      } catch (error) {
        console.error('Failed to change shape type:', error);
        alert(`Failed to change shape type: ${error}`);
      }
    },
    [processingResult, setProcessingResult]
  );

  const handleAnnotationEdit = useCallback(
    (id: string, text: string, fontSize: number) => {
      const { width, height } = measureText(text, fontSize);
      updateTextAnnotation(id, { text, width, height });
    },
    [updateTextAnnotation]
  );

  const handleGenerateXml = useCallback(async () => {
    try {
      await commitLabels();
      await ensureAnalyzed();

      const xml = await api.generateDrawio({
        filename: 'preview',
        include_grid: true,
        page_width: 1920,
        page_height: 1080,
        theme,
      });
      setXmlPreview(xml);
      setActiveTab('xml');
    } catch (error) {
      console.error('XML generation failed:', error);
      alert(`Failed to generate XML: ${error}`);
    }
  }, [theme, commitLabels]);

  const handleExport = useCallback(async () => {
    try {
      await commitLabels();
    } catch {
      // commitLabels already reported the failure; don't export stale labels.
      return;
    }
    await exportDrawio();
  }, [commitLabels]);

  const handleLabelEdit = (id: string, value: string) => {
    setEditedLabels(prev => ({ ...prev, [id]: value }));
  };

  if (!processingResult) {
    return (
      <div className="preview-panel">
        <div className="preview-header">
          <h3>Preview</h3>
          <button className="close-btn" onClick={onClose}>
            ×
          </button>
        </div>
        <div className="preview-empty">
          <p>No processing results yet.</p>
          <p>Draw something and click "Analyze" to detect shapes and text.</p>
        </div>
      </div>
    );
  }

  return (
    <div className="preview-panel">
      <div className="preview-header">
        <h3>Preview</h3>
        <div className="preview-tabs">
          <button
            className={`tab-btn ${activeTab === 'shapes' ? 'active' : ''}`}
            onClick={() => setActiveTab('shapes')}
          >
            Shapes ({processingResult.shapes.length})
          </button>
          <button
            className={`tab-btn ${activeTab === 'text' ? 'active' : ''}`}
            onClick={() => setActiveTab('text')}
          >
            Text ({processingResult.text_regions.length})
          </button>
          <button
            className={`tab-btn ${activeTab === 'notes' ? 'active' : ''}`}
            onClick={() => setActiveTab('notes')}
          >
            Notes ({textAnnotations.length})
          </button>
          <button
            className={`tab-btn ${activeTab === 'xml' ? 'active' : ''}`}
            onClick={() => setActiveTab('xml')}
          >
            XML
          </button>
        </div>
        <button className="close-btn" onClick={onClose}>
          ×
        </button>
      </div>

      <div className="preview-info">
        <span className="diagram-type">
          Type: <strong>{processingResult.suggested_diagram_type}</strong>
        </span>
        <span className="confidence">
          Confidence: <strong>{Math.round(processingResult.confidence * 100)}%</strong>
        </span>
      </div>

      <div className="preview-content">
        {activeTab === 'shapes' && (
          <div className="shapes-list">
            {processingResult.shapes.length === 0 ? (
              <p className="empty-message">No shapes detected</p>
            ) : (
              processingResult.shapes.map((shape, index) => (
                <div key={shape.id} className="shape-item">
                  <div className="shape-header">
                    <span className="shape-index">#{index + 1}</span>
                    <select
                      className="shape-type-select"
                      value={shape.shape_type}
                      onChange={e =>
                        handleShapeTypeChange(shape.id, e.target.value as ShapeType)
                      }
                      title="Correct the detected type"
                    >
                      {SHAPE_TYPES.map(type => (
                        <option key={type} value={type}>
                          {type}
                        </option>
                      ))}
                    </select>
                    <span className="shape-confidence">{Math.round(shape.confidence * 100)}%</span>
                  </div>
                  <div className="shape-details">
                    <span>
                      Position: ({Math.round(shape.bounds.x)}, {Math.round(shape.bounds.y)})
                    </span>
                    <span>
                      Size: {Math.round(shape.bounds.width)} × {Math.round(shape.bounds.height)}
                    </span>
                  </div>
                </div>
              ))
            )}
          </div>
        )}

        {activeTab === 'text' && (
          <div className="text-list">
            {processingResult.text_regions.length === 0 ? (
              <p className="empty-message">No text detected</p>
            ) : (
              processingResult.text_regions.map((region, index) => (
                <div key={region.id} className="text-item">
                  <div className="text-header">
                    <span className="text-index">#{index + 1}</span>
                    <span className="text-confidence">{Math.round(region.confidence * 100)}%</span>
                  </div>
                  <input
                    type="text"
                    value={editedLabels[region.id] ?? region.text}
                    onChange={e => handleLabelEdit(region.id, e.target.value)}
                    onBlur={() => {
                      // Errors are already reported inside commitLabels().
                      commitLabels().catch(() => {});
                    }}
                    className="text-edit"
                    placeholder="Edit text..."
                  />
                  <div className="text-details">
                    <span>
                      Position: ({Math.round(region.bounds.x)}, {Math.round(region.bounds.y)})
                    </span>
                  </div>
                </div>
              ))
            )}
          </div>
        )}

        {activeTab === 'notes' && (
          <div className="text-list">
            {textAnnotations.length === 0 ? (
              <p className="empty-message">
                No typed text. Pick the Text tool (T) and click the canvas.
              </p>
            ) : (
              textAnnotations.map((annotation, index) => (
                <div key={annotation.id} className="text-item">
                  <div className="text-header">
                    <span className="text-index">#{index + 1}</span>
                    <button
                      className="text-delete"
                      onClick={() => {
                        removeTextAnnotation(annotation.id);
                        saveHistory();
                      }}
                      title="Delete this text"
                    >
                      ×
                    </button>
                  </div>
                  <input
                    type="text"
                    value={annotation.text}
                    onChange={e =>
                      handleAnnotationEdit(annotation.id, e.target.value, annotation.fontSize)
                    }
                    onBlur={saveHistory}
                    className="text-edit"
                    placeholder="Edit text..."
                  />
                  <div className="text-details">
                    <span>
                      Position: ({Math.round(annotation.x)}, {Math.round(annotation.y)})
                    </span>
                  </div>
                </div>
              ))
            )}
          </div>
        )}

        {activeTab === 'xml' && (
          <div className="xml-preview">
            {xmlPreview ? (
              <pre className="xml-content">{xmlPreview}</pre>
            ) : (
              <div className="xml-empty">
                <p>Click "Generate XML" to preview the draw.io format</p>
                <button className="generate-btn" onClick={handleGenerateXml}>
                  Generate XML
                </button>
              </div>
            )}
          </div>
        )}
      </div>

      <div className="preview-actions">
        <button className="action-btn" onClick={handleGenerateXml}>
          🔄 Regenerate
        </button>
        <button className="action-btn primary" onClick={handleExport}>
          📥 Export .drawio
        </button>
      </div>
    </div>
  );
}
