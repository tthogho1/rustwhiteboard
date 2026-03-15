# RustWhiteboard

A cross-platform desktop app that converts hand-drawn diagrams into editable **draw.io** (`.drawio`) files.  
Built with a React + TypeScript frontend and a Rust backend powered by [Tauri v2](https://tauri.app/).

## Features

- **Freehand drawing** — smooth, pressure-aware strokes (mouse / touch / stylus) via *perfect-freehand*
- **Infinite canvas** — pan, zoom, and grid overlay
- **Shape detection** — rectangles, circles, ellipses, diamonds, triangles, arrows, lines, connectors
  - Sharp-corner analysis to disambiguate squares from circles
  - PCA-based ellipse fitting for oval shapes
- **OCR** — extract handwritten / printed text with Tesseract (optional)
- **AI-powered formatting** — refine diagram structure and labels with:
  - Built-in rules (no setup needed)
  - **OpenAI API** (GPT-4o, GPT-4 Turbo, GPT-3.5 Turbo)
  - **Ollama** (local LLM server)
  - Local GGUF model (placeholder)
- **Export to draw.io** — generates mxGraph XML for editing in [diagrams.net](https://app.diagrams.net)
- **Backup / restore** — save and load whiteboard snapshots (`.rwb.gz`)
- **Dark / light theme**

## Technology Stack

| Component | Technology |
|---|---|
| Desktop shell | Tauri v2 |
| Frontend | React 18 + TypeScript, HTML5 Canvas |
| Drawing | perfect-freehand |
| State management | Zustand |
| Backend | Rust (Tauri commands) |
| Shape detection | geo (RDP simplification), nalgebra, custom PCA |
| OCR | tesseract-rs (optional, requires system Tesseract) |
| LLM integration | reqwest → OpenAI / Ollama APIs |
| XML export | quick-xml (mxGraph / draw.io format) |

## Requirements

- **Node.js 18+**
- **Rust 1.70+**
- npm (or pnpm)

Optional:

- **Tesseract** installed on the host (for OCR)
- **OpenAI API key** (for GPT-based diagram enhancement)
- **Ollama** running locally (for local LLM inference)

## Quick Start

```bash
# Clone and install
git clone <repo-url> rustwhiteboard
cd rustwhiteboard
npm install

# Development (hot-reload, includes OpenAI support)
npm run tauri:dev

# Production build
npm run tauri:build
```

### Build without OpenAI

If you don't need the OpenAI backend:

```bash
npx tauri dev          # dev
npx tauri build        # release
```

### Rust-only build (no bundling)

```bash
cargo build --manifest-path src-tauri/Cargo.toml --features openai
cargo test  --manifest-path src-tauri/Cargo.toml --features openai
```

## Usage

1. **Draw** shapes on the canvas using the pen tool.
2. Use the **eraser** to remove strokes or **pan** to navigate.
3. Click **🔍 Analyze** to detect shapes and text.
4. Click **⚙️** (next to 🤖 Format) to configure the AI backend:
   - Select **OpenAI** → paste your API key → choose a model → click **✅ Apply**.
   - Or select **Ollama** → enter model name (e.g. `llama2`) → **✅ Apply**.
   - Or leave on **Built-in (Rules)** for zero-config rule-based enhancement.
5. Click **🤖 Format** to enhance the diagram with AI.
6. Click **👁️ Preview** to see the result.
7. Click **📥 .drawio** to export for editing in diagrams.net.

### Keyboard Shortcuts

| Shortcut | Action |
|---|---|
| `Ctrl+Z` | Undo |
| `Ctrl+Y` | Redo |
| `Ctrl + Mouse Wheel` | Zoom |
| Middle mouse drag | Pan |

## LLM Configuration

The AI backend can be configured from the toolbar's ⚙️ settings panel or programmatically:

```typescript
import { api } from './lib/api';

// OpenAI
await api.configureLlm({
  backend: 'openai',
  api_key: 'sk-...',
  model_name: 'gpt-4o',
  temperature: 0.7,
  max_tokens: 2048,
  context_size: 4096,
});

// Ollama
await api.configureLlm({
  backend: 'ollama',
  model_name: 'llama2',
  temperature: 0.7,
  max_tokens: 2048,
  context_size: 4096,
  ollama_url: 'http://localhost:11434',
});

// Built-in (rules, no setup)
await api.configureLlm({
  backend: 'builtin',
  model_name: 'default',
  temperature: 0.7,
  max_tokens: 2048,
  context_size: 4096,
});
```

## Feature Flags (Cargo)

| Feature | Description |
|---|---|
| `openai` | Enables OpenAI API backend (adds `reqwest` dependency) |
| `ollama` | Enables Ollama API backend (adds `reqwest` dependency) |
| `ocr` | Enables Tesseract OCR (requires system Tesseract) |

Both `npm run tauri:dev` and `npm run tauri:build` include `openai` by default.

## Project Layout

```
rustwhiteboard/
├── src/                    # Frontend (React + TypeScript)
│   ├── components/         # UI components
│   │   ├── Canvas.tsx      #   Drawing canvas
│   │   ├── Toolbar.tsx     #   Toolbar with LLM settings
│   │   ├── Preview.tsx     #   Result preview
│   │   └── StatusBar.tsx   #   Status bar
│   ├── lib/
│   │   └── api.ts          # Tauri API wrapper
│   ├── styles/
│   │   └── global.css      # Styles + theme variables
│   ├── store.ts            # Zustand state management
│   └── main.tsx            # App entry point
├── src-tauri/              # Rust backend
│   ├── Cargo.toml          # Dependencies & feature flags
│   ├── tauri.conf.json     # Tauri configuration
│   └── src/
│       ├── main.rs         # Tauri commands & app setup
│       ├── canvas.rs       # Canvas image processing
│       ├── shapes.rs       # Shape detection (circle, rect, ellipse, etc.)
│       ├── ocr.rs          # OCR via Tesseract
│       ├── llm.rs          # LLM integration (OpenAI, Ollama, rules)
│       └── drawio.rs       # draw.io XML generation
├── package.json
├── tsconfig.json
├── vite.config.ts
└── public/                 # Static assets
```

## Notes

- **OCR** requires Tesseract installed on the host and tessdata language files.
- **OpenAI** requires a valid API key and internet access.
- **Ollama** requires the Ollama server running locally (`ollama serve`).
- The app is **local-first** — network calls only happen when OpenAI or Ollama is explicitly selected.
- Shape detection uses smoothing, RDP simplification, and sharp-corner counting to improve freehand recognition.

## License

MIT

## Contributing

Contributions welcome. For major changes, please open an issue first.

## Acknowledgements

- [Tauri](https://tauri.app/) — lightweight Rust desktop shell
- [perfect-freehand](https://github.com/steveruizok/perfect-freehand) — smooth stroke rendering
- [draw.io / diagrams.net](https://www.diagrams.net/) — target diagram editor
- [Tesseract OCR](https://github.com/tesseract-ocr/tesseract) — optional OCR engine
- [OpenAI API](https://platform.openai.com/) — optional AI diagram enhancement
