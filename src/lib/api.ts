import { invoke } from '@tauri-apps/api/core';
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
export const api = {
  // Stroke management
  async addStroke(stroke: Stroke): Promise<void> {
    return invoke('add_stroke', { stroke });
  },

  async clearStrokes(): Promise<void> {
    return invoke('clear_strokes');
  },

  async getStrokes(): Promise<Stroke[]> {
    return invoke('get_strokes');
  },

  // Processing
  async processCanvas(imageData: string, width: number, height: number): Promise<ProcessingResult> {
    return invoke('process_canvas', { imageData, width, height });
  },

  // LLM
  async enhanceWithLlm(prompt?: string): Promise<unknown> {
    return invoke('enhance_with_llm', { prompt });
  },

  async configureLlm(config: LlmConfig): Promise<void> {
    return invoke('configure_llm', { config });
  },

  // Export
  async generateDrawio(options: ExportOptions): Promise<string> {
    return invoke('generate_drawio', { options });
  },

  async exportDrawioFile(path: string, options: ExportOptions): Promise<void> {
    return invoke('export_drawio_file', { path, options });
  },

  // Backup
  async saveBackup(path: string): Promise<void> {
    return invoke('save_backup', { path });
  },

  async loadBackup(path: string): Promise<Stroke[]> {
    return invoke('load_backup', { path });
  },

  // Info
  async getAppInfo(): Promise<AppInfo> {
    return invoke('get_app_info');
  },
};

export default api;
