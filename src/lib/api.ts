import { invoke as tauriInvoke } from '@tauri-apps/api/core';
import { open, save, message as showDialog, ask } from '@tauri-apps/plugin-dialog';
import type { Stroke, ProcessingResult } from '../store';

export interface ExportOptions {
  filename: string;
  include_grid: boolean;
  page_width: number;
  page_height: number;
  theme: string;
}

export interface LlmConfig {
  backend: 'builtin' | 'local' | 'ollama' | 'disabled';
  model_path?: string;
  model_name: string;
  temperature: number;
  max_tokens: number;
  context_size: number;
  ollama_url?: string;
}

export interface AppInfo {
  name: string;
  version: string;
  description: string;
  features: {
    ocr: boolean;
    ollama: boolean;
  };
}

// Tauri API wrapper functions
function safeInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (typeof window !== 'undefined' && !(window as any).__TAURI_INTERNALS__) {
    return Promise.reject(new Error('Tauri API not available. Start the app with `npm run tauri:dev`.'));
  }
  return tauriInvoke<T>(cmd, args);
}

export const api = {
  // Stroke management
  async addStroke(stroke: Stroke): Promise<void> {
    return safeInvoke('add_stroke', { stroke });
  },

  async clearStrokes(): Promise<void> {
    return safeInvoke('clear_strokes');
  },

  async getStrokes(): Promise<Stroke[]> {
    return safeInvoke('get_strokes');
  },

  // Processing
  async processCanvas(imageData: string, width: number, height: number): Promise<ProcessingResult> {
    return safeInvoke('process_canvas', { imageData, width, height });
  },

  // LLM
  async enhanceWithLlm(prompt?: string): Promise<unknown> {
    return safeInvoke('enhance_with_llm', { prompt });
  },

  async configureLlm(config: LlmConfig): Promise<void> {
    return safeInvoke('configure_llm', { config });
  },

  // Export
  async generateDrawio(options: ExportOptions): Promise<string> {
    return safeInvoke('generate_drawio', { options });
  },

  async exportDrawioFile(path: string, options: ExportOptions): Promise<void> {
    return safeInvoke('export_drawio_file', { path, options });
  },

  // Backup
  async saveBackup(path: string): Promise<void> {
    return safeInvoke('save_backup', { path });
  },

  async loadBackup(path: string): Promise<Stroke[]> {
    return safeInvoke('load_backup', { path });
  },

  // Info
  async getAppInfo(): Promise<AppInfo> {
    return safeInvoke('get_app_info');
  },

  // Dialog utilities (Tauri v2 style)
  async openFile(filters?: { name: string; extensions: string[] }[]): Promise<string | null> {
    return open({
      multiple: false,
      directory: false,
      filters,
    }) as Promise<string | null>;
  },

  async saveFile(defaultPath?: string, filters?: { name: string; extensions: string[] }[]): Promise<string | null> {
    return save({
      defaultPath,
      filters,
    });
  },

  async showMessage(title: string, msgText: string, kind?: 'info' | 'warning' | 'error'): Promise<void> {
    await showDialog(msgText, { title, kind: kind || 'info' });
  },

  async confirm(title: string, msgText: string): Promise<boolean> {
    return ask(msgText, { title, kind: 'info' });
  },
};

export default api;
