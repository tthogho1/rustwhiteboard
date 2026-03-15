import { useCallback, useState, useRef } from 'react';
import { useStore, Tool } from '../store';
import { invoke } from '@tauri-apps/api/core';
import { save } from '@tauri-apps/plugin-dialog';
import { api } from '../lib/api';
import type { LlmConfig } from '../lib/api';

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

  // LLM configuration state
  const [showLlmSettings, setShowLlmSettings] = useState(false);
  const [llmBackend, setLlmBackend] = useState<LlmConfig['backend']>('builtin');
  const [llmApiKey, setLlmApiKey] = useState('');
  const [llmModelName, setLlmModelName] = useState('gpt-4o');
  const [llmConfigured, setLlmConfigured] = useState(false);
  const settingsBtnRef = useRef<HTMLButtonElement>(null);

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

  const handleConfigureLlm = useCallback(async () => {
    try {
      const config: LlmConfig = {
        backend: llmBackend,
        model_name: llmModelName,
        temperature: 0.7,
        max_tokens: 2048,
        context_size: 4096,
        ...(llmBackend === 'openai' && llmApiKey ? { api_key: llmApiKey } : {}),
        ...(llmBackend === 'ollama' ? { ollama_url: 'http://localhost:11434' } : {}),
      };
      await api.configureLlm(config);
      setLlmConfigured(true);
      setShowLlmSettings(false);
      alert(`LLM configured: ${llmBackend} / ${llmModelName}`);
    } catch (error) {
      console.error('LLM configuration failed:', error);
      alert(`LLM configuration failed: ${error}`);
    }
  }, [llmBackend, llmApiKey, llmModelName]);

  const handleEnhanceWithLLM = useCallback(async () => {
    if (strokes.length === 0) {
      alert('Please draw something first!');
      return;
    }

    // Auto-configure if not yet configured
    if (!llmConfigured) {
      try {
        const config: LlmConfig = {
          backend: llmBackend,
          model_name: llmModelName,
          temperature: 0.7,
          max_tokens: 2048,
          context_size: 4096,
          ...(llmBackend === 'openai' && llmApiKey ? { api_key: llmApiKey } : {}),
          ...(llmBackend === 'ollama' ? { ollama_url: 'http://localhost:11434' } : {}),
        };
        await api.configureLlm(config);
        setLlmConfigured(true);
      } catch (error) {
        console.error('LLM auto-configure failed:', error);
        alert(`LLM configuration failed: ${error}`);
        return;
      }
    }

    setProcessing(true);
    try {
      // Send strokes to backend first
      for (const stroke of strokes) {
        await invoke('add_stroke', { stroke });
      }

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
  }, [strokes, llmPrompt, llmBackend, llmApiKey, llmModelName, llmConfigured, setProcessing, onTogglePreview]);

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
      <div className="toolbar-group llm-group" style={{ position: 'relative' }}>
        <span className="toolbar-label">AI Format</span>
        <div style={{ display: 'flex', alignItems: 'center', gap: '4px', flexWrap: 'wrap' }}>
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
          <button
            ref={settingsBtnRef}
            className={`toolbar-btn ${showLlmSettings ? 'active' : ''}`}
            onClick={() => setShowLlmSettings(!showLlmSettings)}
            title="LLM Settings"
          >
            ⚙️
          </button>
        </div>
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

      {/* LLM Settings Panel - rendered as fixed overlay to escape toolbar overflow clipping */}
      {showLlmSettings && (() => {
        const rect = settingsBtnRef.current?.getBoundingClientRect();
        const top = rect ? rect.bottom + 4 : 60;
        const right = rect ? window.innerWidth - rect.right : 16;
        return (
          <>
            <div
              style={{
                position: 'fixed',
                inset: 0,
                zIndex: 9998,
              }}
              onClick={() => setShowLlmSettings(false)}
            />
            <div className="llm-settings" style={{
              position: 'fixed',
              top: `${top}px`,
              right: `${right}px`,
              zIndex: 9999,
              background: 'var(--bg-primary, #fff)',
              border: '1px solid var(--border-color, #ccc)',
              borderRadius: '8px',
              padding: '12px',
              minWidth: '280px',
              boxShadow: '0 4px 12px rgba(0,0,0,0.15)',
              color: 'var(--text-primary, #333)',
            }}>
              <div style={{ marginBottom: '8px', fontWeight: 'bold' }}>LLM Settings</div>

              <label style={{ display: 'block', marginBottom: '6px', fontSize: '12px' }}>
                Backend
                <select
                  value={llmBackend}
                  onChange={e => {
                    setLlmBackend(e.target.value as LlmConfig['backend']);
                    setLlmConfigured(false);
                  }}
                  style={{ display: 'block', width: '100%', padding: '4px', marginTop: '2px' }}
                >
                  <option value="builtin">Built-in (Rules)</option>
                  <option value="openai">OpenAI (GPT-4o, etc.)</option>
                  <option value="ollama">Ollama (Local)</option>
                  <option value="local">Local GGUF Model</option>
                  <option value="disabled">Disabled</option>
                </select>
              </label>

              {llmBackend === 'openai' && (
                <>
                  <label style={{ display: 'block', marginBottom: '6px', fontSize: '12px' }}>
                    API Key
                    <input
                      type="password"
                      value={llmApiKey}
                      onChange={e => {
                        setLlmApiKey(e.target.value);
                        setLlmConfigured(false);
                      }}
                      placeholder="sk-..."
                      style={{ display: 'block', width: '100%', padding: '4px', marginTop: '2px' }}
                    />
                  </label>
                  <label style={{ display: 'block', marginBottom: '6px', fontSize: '12px' }}>
                    Model
                    <select
                      value={llmModelName}
                      onChange={e => {
                        setLlmModelName(e.target.value);
                        setLlmConfigured(false);
                      }}
                      style={{ display: 'block', width: '100%', padding: '4px', marginTop: '2px' }}
                    >
                      <option value="gpt-4o">GPT-4o</option>
                      <option value="gpt-4-turbo">GPT-4 Turbo</option>
                      <option value="gpt-4o-mini">GPT-4o Mini</option>
                      <option value="gpt-3.5-turbo">GPT-3.5 Turbo</option>
                    </select>
                  </label>
                </>
              )}

              {llmBackend === 'ollama' && (
                <label style={{ display: 'block', marginBottom: '6px', fontSize: '12px' }}>
                  Model Name
                  <input
                    type="text"
                    value={llmModelName}
                    onChange={e => {
                      setLlmModelName(e.target.value);
                      setLlmConfigured(false);
                    }}
                    placeholder="llama2, mistral, etc."
                    style={{ display: 'block', width: '100%', padding: '4px', marginTop: '2px' }}
                  />
                </label>
              )}

              <div style={{ display: 'flex', gap: '6px', marginTop: '10px' }}>
                <button
                  className="toolbar-btn primary"
                  onClick={handleConfigureLlm}
                  style={{ flex: 1 }}
                >
                  ✅ Apply
                </button>
                <button
                  className="toolbar-btn"
                  onClick={() => setShowLlmSettings(false)}
                  style={{ flex: 1 }}
                >
                  Cancel
                </button>
              </div>

              {llmConfigured && (
                <div style={{ marginTop: '6px', fontSize: '11px', color: 'green' }}>
                  ✓ Configured: {llmBackend} / {llmModelName}
                </div>
              )}
            </div>
          </>
        );
      })()}
    </div>
  );
}
