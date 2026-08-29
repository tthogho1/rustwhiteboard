import { useCallback, useState, useRef } from 'react';
import { useStore, Tool } from '../store';
import { invoke } from '@tauri-apps/api/core';
import { api } from '../lib/api';
import { ensureAnalyzed } from '../lib/pipeline';
import {
  clearCanvas,
  exportDrawio,
  exportPng,
  exportSvg,
  importDrawio,
  restoreBackup,
  saveBackup,
} from '../lib/actions';
import { SHORTCUT_HELP } from '../lib/useKeyboardShortcuts';
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
    showDetection,
    setShowDetection,
    cleanupView,
    setCleanupView,
    fontSize,
    setFontSize,
    processingResult,
    strokes,
    selectedIds,
    deleteSelected,
    duplicateSelected,
    restyleSelected,
    clearSelection,
    undo,
    redo,
    history,
    historyIndex,
    isProcessing,
    setProcessing,
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
  const [showShortcuts, setShowShortcuts] = useState(false);
  const settingsBtnRef = useRef<HTMLButtonElement>(null);
  const shortcutsBtnRef = useRef<HTMLButtonElement>(null);

  const tools: { id: Tool; icon: string; label: string }[] = [
    { id: 'pen', icon: '✏️', label: 'Pen (P)' },
    { id: 'eraser', icon: '🧹', label: 'Eraser (E)' },
    { id: 'text', icon: '🔤', label: 'Text (T)' },
    { id: 'select', icon: '👆', label: 'Select (V)' },
    { id: 'pan', icon: '✋', label: 'Pan (H)' },
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
      await ensureAnalyzed({ force: true });
      onTogglePreview();
    } catch (error) {
      console.error('Processing failed:', error);
      alert(`Processing failed: ${error}`);
    } finally {
      setProcessing(false);
    }
  }, [strokes, setProcessing, onTogglePreview]);

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
      // The backend enhances the *detected* shapes, so detection has to have run.
      await ensureAnalyzed();

      const structure = await invoke<{ nodes?: unknown[]; edges?: unknown[] }>(
        'enhance_with_llm',
        { prompt: llmPrompt }
      );
      console.log('LLM Enhancement result:', structure);
      alert(
        `AI formatting done: ${structure.nodes?.length ?? 0} nodes, ` +
          `${structure.edges?.length ?? 0} edges (see the console).\n` +
          'Note: the .drawio export still uses the detected shapes.'
      );
      onTogglePreview();
    } catch (error) {
      console.error('LLM processing failed:', error);
      alert(`LLM processing failed: ${error}`);
    } finally {
      setProcessing(false);
    }
  }, [strokes, llmPrompt, llmBackend, llmApiKey, llmModelName, llmConfigured, setProcessing, onTogglePreview]);

  const handleApplyStyleToSelection = useCallback(() => {
    restyleSelected({ color: penColor, width: penWidth });
  }, [restyleSelected, penColor, penWidth]);

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

      {/* Selection actions (select tool only) */}
      {tool === 'select' && (
        <div className="toolbar-group">
          <span className="toolbar-label">Selection ({selectedIds.length})</span>
          <div className="toolbar-buttons">
            <button
              className="toolbar-btn"
              onClick={duplicateSelected}
              disabled={selectedIds.length === 0}
              title="Duplicate selection (Ctrl+D)"
            >
              ⧉
            </button>
            <button
              className="toolbar-btn"
              onClick={handleApplyStyleToSelection}
              disabled={selectedIds.length === 0}
              title="Apply the current color and width to the selection"
            >
              🎨
            </button>
            <button
              className="toolbar-btn"
              onClick={clearSelection}
              disabled={selectedIds.length === 0}
              title="Clear selection (Esc)"
            >
              ✖
            </button>
            <button
              className="toolbar-btn danger"
              onClick={deleteSelected}
              disabled={selectedIds.length === 0}
              title="Delete selection (Delete)"
            >
              🗑️
            </button>
          </div>
        </div>
      )}

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

      {/* Pen width / font size */}
      {tool === 'text' ? (
        <div className="toolbar-group">
          <span className="toolbar-label">Font</span>
          <input
            type="range"
            min="10"
            max="48"
            value={fontSize}
            onChange={e => setFontSize(Number(e.target.value))}
            className="width-slider"
          />
          <span className="width-value">{fontSize}px</span>
        </div>
      ) : (
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
      )}

      {/* View controls */}
      <div className="toolbar-group">
        <span className="toolbar-label">View</span>
        <div className="toolbar-buttons">
          <button className="toolbar-btn" onClick={() => setZoom(zoom * 1.2)} title="Zoom In (Ctrl +)">
            🔍+
          </button>
          <button className="toolbar-btn" onClick={() => setZoom(zoom / 1.2)} title="Zoom Out (Ctrl -)">
            🔍-
          </button>
          <button className="toolbar-btn" onClick={handleResetView} title="Reset View (Ctrl+0)">
            🎯
          </button>
          <button
            className={`toolbar-btn ${showGrid ? 'active' : ''}`}
            onClick={() => setShowGrid(!showGrid)}
            title="Toggle Grid (G)"
          >
            #
          </button>
          <button
            className={`toolbar-btn ${showDetection ? 'active' : ''}`}
            onClick={() => setShowDetection(!showDetection)}
            disabled={!processingResult}
            title="Show what was detected, on the canvas (O) — run Analyze first"
          >
            🔎
          </button>
          <button
            className={`toolbar-btn ${cleanupView ? 'active' : ''}`}
            onClick={() => setCleanupView(!cleanupView)}
            disabled={!processingResult}
            title="Clean-up view: draw the detected shapes straightened (C)"
          >
            ✨
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
            title="Redo (Ctrl+Shift+Z)"
          >
            ↪️
          </button>
          <button className="toolbar-btn danger" onClick={clearCanvas} title="Clear All">
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
          <button className="toolbar-btn" onClick={onTogglePreview} title="Show Preview (Ctrl+P)">
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
          <button className="toolbar-btn success" onClick={exportDrawio} title="Export to .drawio (Ctrl+E)">
            📥 .drawio
          </button>
          <button
            className="toolbar-btn"
            onClick={exportPng}
            title="Export a PNG, cropped to the drawing (follows the clean-up view)"
          >
            🖼️ PNG
          </button>
          <button
            className="toolbar-btn"
            onClick={exportSvg}
            title="Export an SVG, cropped to the drawing (follows the clean-up view)"
          >
            ⬡ SVG
          </button>
        </div>
      </div>

      {/* File */}
      <div className="toolbar-group">
        <span className="toolbar-label">File</span>
        <div className="toolbar-buttons">
          <button
            className="toolbar-btn"
            onClick={importDrawio}
            title="Import a .drawio file back onto the canvas"
          >
            📄 Import
          </button>
          <button className="toolbar-btn" onClick={saveBackup} title="Save Backup (Ctrl+S)">
            💾 Backup
          </button>
          <button className="toolbar-btn" onClick={restoreBackup} title="Restore Backup">
            📂 Restore
          </button>
        </div>
      </div>

      {/* Theme + shortcuts */}
      <div className="toolbar-group">
        <div className="toolbar-buttons">
          <button
            className="toolbar-btn"
            onClick={() => setTheme(theme === 'light' ? 'dark' : 'light')}
            title="Toggle Theme"
          >
            {theme === 'light' ? '🌙' : '☀️'}
          </button>
          <button
            ref={shortcutsBtnRef}
            className={`toolbar-btn ${showShortcuts ? 'active' : ''}`}
            onClick={() => setShowShortcuts(!showShortcuts)}
            title="Keyboard shortcuts"
          >
            ⌨️
          </button>
        </div>
      </div>

      {/* Shortcut cheat sheet */}
      {showShortcuts && (() => {
        const rect = shortcutsBtnRef.current?.getBoundingClientRect();
        const top = rect ? rect.bottom + 4 : 60;
        const right = rect ? Math.max(8, window.innerWidth - rect.right) : 16;
        return (
          <>
            <div
              style={{ position: 'fixed', inset: 0, zIndex: 9998 }}
              onClick={() => setShowShortcuts(false)}
            />
            <div className="shortcut-help" style={{ top: `${top}px`, right: `${right}px` }}>
              <div className="shortcut-help-title">Keyboard shortcuts</div>
              <table>
                <tbody>
                  {SHORTCUT_HELP.map(item => (
                    <tr key={item.keys}>
                      <th>{item.keys}</th>
                      <td>{item.action}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
              <div className="shortcut-help-note">
                Select tool: drag to rubber-band, Shift+click to add, drag the box to move.
                Strokes and text select together.
                <br />
                With the detection overlay on (O), click a box&rsquo;s outline to fix its type
                or drop it.
              </div>
            </div>
          </>
        );
      })()}

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
