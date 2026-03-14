import { useCallback, useState } from 'react';
import { useStore, Tool } from '../store';
import { invoke } from '@tauri-apps/api/core';
import { save } from '@tauri-apps/plugin-dialog';

interface ToolbarProps {
  onTogglePreview: () => void;
}

export function Toolbar({ onTogglePreview }: ToolbarProps) {
  const {
    tool,
    setTool,
    penColor,
    setPenColor,
    penWidth,
    setPenWidth,
    zoom,
    setZoom,
    setPan,
    theme,
    setTheme,
    showGrid,
    setShowGrid,
    strokes,
    clearStrokes,
    undo,
    redo,
    history,
    historyIndex,
    isProcessing,
    setProcessing,
    setProcessingResult,
  } = useStore();

  const [llmPrompt, setLlmPrompt] = useState(
    'Convert this hand-drawn flowchart to a clean UML diagram'
  );

  const tools: { id: Tool; icon: string; label: string }[] = [
    { id: 'pen', icon: '✏️', label: 'Pen' },
    { id: 'eraser', icon: '🧹', label: 'Eraser' },
    { id: 'select', icon: '👆', label: 'Select' },
    { id: 'pan', icon: '✋', label: 'Pan' },
  ];

  const colors = [
    '#000000',
    '#ff0000',
    '#00aa00',
    '#0000ff',
    '#ff8800',
    '#8800ff',
    '#00aaaa',
    '#888888',
  ];

  const handleProcess = useCallback(async () => {
    if (strokes.length === 0) {
      alert('Please draw something first!');
      return;
    }

    setProcessing(true);
    try {
      // Get canvas image data
      const canvas = document.querySelector('.drawing-canvas') as HTMLCanvasElement;
      if (!canvas) throw new Error('Canvas not found');

      const imageData = canvas.toDataURL('image/png');

      // Send strokes to backend
      for (const stroke of strokes) {
        await invoke('add_stroke', { stroke });
      }

      // Process canvas
      const result = await invoke('process_canvas', {
        imageData,
        width: canvas.width,
        height: canvas.height,
      });

      setProcessingResult(result as any);
      onTogglePreview();
    } catch (error) {
      console.error('Processing failed:', error);
      alert(`Processing failed: ${error}`);
    } finally {
      setProcessing(false);
    }
  }, [strokes, setProcessing, setProcessingResult, onTogglePreview]);

  const handleEnhanceWithLLM = useCallback(async () => {
    if (strokes.length === 0) {
      alert('Please draw something first!');
      return;
    }

    setProcessing(true);
    try {
      const result = await invoke('enhance_with_llm', {
        prompt: llmPrompt,
      });
      console.log('LLM Enhancement result:', result);
      setProcessingResult(result as any);
      alert('LLM processing completed! Check the preview.');
      onTogglePreview();
    } catch (error) {
      console.error('LLM processing failed:', error);
      alert(`LLM processing failed: ${error}`);
    } finally {
      setProcessing(false);
    }
  }, [strokes, llmPrompt, setProcessing, onTogglePreview]);

  const handleExport = useCallback(async () => {
    if (strokes.length === 0) {
      alert('Please draw something first!');
      return;
    }

    try {
      const filePath = await save({
        filters: [
          {
            name: 'Draw.io',
            extensions: ['drawio'],
          },
        ],
        defaultPath: 'diagram.drawio',
      });

      if (!filePath) return;

      // Ensure backend has the strokes and detected shapes before exporting
      const canvas = document.querySelector('.drawing-canvas') as HTMLCanvasElement;
      if (canvas) {
        const imageData = canvas.toDataURL('image/png');
        for (const stroke of strokes) {
          await invoke('add_stroke', { stroke });
        }
        await invoke('process_canvas', {
          imageData,
          width: canvas.width,
          height: canvas.height,
        });
      }

      await invoke('export_drawio_file', {
        path: filePath,
        options: {
          filename: 'Untitled',
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
  }, [strokes, theme]);

  const handleSaveBackup = useCallback(async () => {
    try {
      const filePath = await save({
        filters: [
          {
            name: 'Whiteboard Backup',
            extensions: ['rwb.gz'],
          },
        ],
        defaultPath: 'whiteboard-backup.rwb.gz',
      });

      if (!filePath) return;

      await invoke('save_backup', { path: filePath });
      alert('Backup saved!');
    } catch (error) {
      console.error('Backup failed:', error);
      alert(`Backup failed: ${error}`);
    }
  }, []);

  const handleResetView = useCallback(() => {
    setZoom(1);
    setPan(0, 0);
  }, [setZoom, setPan]);

  return (
    <div className="toolbar">
      {/* Tool selection */}
      <div className="toolbar-group">
        <span className="toolbar-label">Tools</span>
        <div className="toolbar-buttons">
          {tools.map(t => (
            <button
              key={t.id}
              className={`toolbar-btn ${tool === t.id ? 'active' : ''}`}
              onClick={() => setTool(t.id)}
              title={t.label}
            >
              {t.icon}
            </button>
          ))}
        </div>
      </div>

      {/* Color picker */}
      <div className="toolbar-group">
        <span className="toolbar-label">Color</span>
        <div className="color-picker">
          {colors.map(color => (
            <button
              key={color}
              className={`color-btn ${penColor === color ? 'active' : ''}`}
              style={{ backgroundColor: color }}
              onClick={() => setPenColor(color)}
              title={color}
            />
          ))}
          <input
            type="color"
            value={penColor}
            onChange={e => setPenColor(e.target.value)}
            className="color-input"
            title="Custom color"
          />
        </div>
      </div>

      {/* Pen width */}
      <div className="toolbar-group">
        <span className="toolbar-label">Width</span>
        <input
          type="range"
          min="1"
          max="20"
          value={penWidth}
          onChange={e => setPenWidth(Number(e.target.value))}
          className="width-slider"
        />
        <span className="width-value">{penWidth}px</span>
      </div>

      {/* View controls */}
      <div className="toolbar-group">
        <span className="toolbar-label">View</span>
        <div className="toolbar-buttons">
          <button className="toolbar-btn" onClick={() => setZoom(zoom * 1.2)} title="Zoom In">
            🔍+
          </button>
          <button className="toolbar-btn" onClick={() => setZoom(zoom / 1.2)} title="Zoom Out">
            🔍-
          </button>
          <button className="toolbar-btn" onClick={handleResetView} title="Reset View">
            🎯
          </button>
          <button
            className={`toolbar-btn ${showGrid ? 'active' : ''}`}
            onClick={() => setShowGrid(!showGrid)}
            title="Toggle Grid"
          >
            #
          </button>
        </div>
        <span className="zoom-value">{Math.round(zoom * 100)}%</span>
      </div>

      {/* History */}
      <div className="toolbar-group">
        <span className="toolbar-label">History</span>
        <div className="toolbar-buttons">
          <button
            className="toolbar-btn"
            onClick={undo}
            disabled={historyIndex <= 0}
            title="Undo (Ctrl+Z)"
          >
            ↩️
          </button>
          <button
            className="toolbar-btn"
            onClick={redo}
            disabled={historyIndex >= history.length - 1}
            title="Redo (Ctrl+Y)"
          >
            ↪️
          </button>
          <button className="toolbar-btn danger" onClick={clearStrokes} title="Clear All">
            🗑️
          </button>
        </div>
      </div>

      {/* Processing */}
      <div className="toolbar-group">
        <span className="toolbar-label">Process</span>
        <div className="toolbar-buttons">
          <button
            className="toolbar-btn primary"
            onClick={handleProcess}
            disabled={isProcessing || strokes.length === 0}
            title="Detect Shapes & Text"
          >
            {isProcessing ? '⏳' : '🔍'} Analyze
          </button>
          <button className="toolbar-btn" onClick={onTogglePreview} title="Show Preview">
            👁️ Preview
          </button>
        </div>
      </div>

      {/* LLM */}
      <div className="toolbar-group llm-group">
        <span className="toolbar-label">AI Format</span>
        <input
          type="text"
          value={llmPrompt}
          onChange={e => setLlmPrompt(e.target.value)}
          placeholder="Enter formatting prompt..."
          className="llm-input"
        />
        <button
          className="toolbar-btn primary"
          onClick={handleEnhanceWithLLM}
          disabled={isProcessing}
          title="Enhance with LLM"
        >
          🤖 Format
        </button>
      </div>

      {/* Export */}
      <div className="toolbar-group">
        <span className="toolbar-label">Export</span>
        <div className="toolbar-buttons">
          <button className="toolbar-btn success" onClick={handleExport} title="Export to .drawio">
            📥 .drawio
          </button>
          <button className="toolbar-btn" onClick={handleSaveBackup} title="Save Backup">
            💾 Backup
          </button>
        </div>
      </div>

      {/* Theme */}
      <div className="toolbar-group">
        <button
          className="toolbar-btn"
          onClick={() => setTheme(theme === 'light' ? 'dark' : 'light')}
          title="Toggle Theme"
        >
          {theme === 'light' ? '🌙' : '☀️'}
        </button>
      </div>
    </div>
  );
}
