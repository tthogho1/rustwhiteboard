import { useStore } from '../store';
import { useCallback, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { save } from '@tauri-apps/plugin-dialog';

interface PreviewProps {
  onClose: () => void;
}

export function Preview({ onClose }: PreviewProps) {
  const { processingResult, theme } = useStore();
  const [activeTab, setActiveTab] = useState<'shapes' | 'text' | 'xml'>('shapes');
  const [xmlPreview, setXmlPreview] = useState<string>('');
  const [editedLabels, setEditedLabels] = useState<Record<string, string>>({});

  const handleGenerateXml = useCallback(async () => {
    try {
      const xml = await invoke<string>('generate_drawio', {
        options: {
          filename: 'preview',
          include_grid: true,
          page_width: 1920,
          page_height: 1080,
          theme: theme,
        },
      });
      setXmlPreview(xml);
      setActiveTab('xml');
    } catch (error) {
      console.error('XML generation failed:', error);
      alert(`Failed to generate XML: ${error}`);
    }
  }, [theme]);

  const handleExport = useCallback(async () => {
    try {
      const filePath = await save({
        filters: [{ name: 'Draw.io', extensions: ['drawio'] }],
        defaultPath: 'diagram.drawio',
      });

      if (!filePath) return;

      await invoke('export_drawio_file', {
        path: filePath,
        options: {
          filename: 'diagram',
          include_grid: true,
          page_width: 1920,
          page_height: 1080,
          theme: theme,
        },
      });

      alert(`Exported to ${filePath}`);
    } catch (error) {
      console.error('Export failed:', error);
      alert(`Export failed: ${error}`);
    }
  }, [theme]);

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
                    <span className="shape-type">{shape.shape_type}</span>
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
