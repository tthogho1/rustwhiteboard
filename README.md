# RustWhiteboard

RustWhiteboard is a cross-platform desktop app that converts hand-drawn diagrams into editable draw.io (.drawio) files. It combines a responsive HTML5 canvas frontend with a Rust backend (via Tauri) for image processing, OCR, shape detection, and optional local LLM-based formatting.

## Key Features

- Smooth, pressure-aware freehand drawing (mouse / touch / stylus)
- Infinite canvas with pan and zoom
- Toolset: pen, eraser, select, pan, undo / redo
- Shape detection: rectangles, diamonds, circles, triangles, arrows, connectors
- OCR support (Tesseract) for extracting handwritten text
- Optional local LLM or Ollama API for structure/label refinement
- Export to draw.io (mxGraph XML) for further editing in draw.io / diagrams.net
- Local-first: works offline and can use GGUF models for on-device LLM

## Technology Stack

| Component         | Technology                                         |
| ----------------- | -------------------------------------------------- |
| Desktop shell     | Tauri v2                                           |
| Frontend          | React + TypeScript, HTML5 Canvas                   |
| Drawing smoothing | perfect-freehand                                   |
| Backend           | Rust (tauri commands)                              |
| OCR               | tesseract-rs (optional, requires system tesseract) |
| XML export        | quick-xml (mxGraph / draw.io format)               |
| Optional LLM      | llm / candle (GGUF models) or Ollama API           |

## Requirements

- Node.js 18+ (for frontend tooling)
- Rust 1.70+ (for building the Tauri backend)
- npm or pnpm

Optional for OCR:

- Tesseract installed on the host system (and tessdata available)

Optional for local LLMs:

- GGUF model files (models/\*) and enough disk/CPU resources

## Quick Start (development)

Clone and install:

```bash
git clone <repo-url> rustwhiteboard
cd rustwhiteboard
npm install
```

Run the app in development (starts Vite and Tauri dev):

```bash
npm run tauri dev
```

Create a production build:

```bash
npm run tauri build
```

Note: building a release bundle requires Tauri prerequisites for your platform (see Tauri docs).

## Usage Overview

1. Draw shapes on the canvas using the pen tool.
2. Use the eraser to remove strokes or the pan tool to move the view.
3. Click Analyze to perform shape detection and OCR.
4. Optionally run the AI Format action to refine structure and labels.
5. Export the result to a .drawio file for editing in draw.io / diagrams.net.

Keyboard shortcuts:

- `Ctrl+Z` — Undo
- `Ctrl+Y` — Redo
- `Ctrl + Mouse Wheel` — Zoom
- Middle mouse button drag — Pan

## Configuration Examples

LLM config (example):

```json
{
  "backend": "local", // "builtin" | "local" | "ollama" | "disabled"
  "model_name": "llama3-8b-q4",
  "temperature": 0.7,
  "max_tokens": 2048,
  "ollama_url": "http://localhost:11434"
}
```

Export options (example):

```json
{
  "filename": "diagram",
  "include_grid": true,
  "page_width": 1920,
  "page_height": 1080,
  "theme": "light"
}
```

## Project Layout

```
rustwhiteboard/
├── src/                # Frontend (React + TypeScript)
│   ├── components/     # UI components (Canvas, Toolbar, Preview, StatusBar)
│   ├── lib/            # Frontend helpers
│   ├── styles/         # CSS
│   └── store.ts        # Zustand state
├── src-tauri/          # Tauri / Rust backend
│   ├── Cargo.toml
   └── src/
       ├── main.rs     # Tauri commands
       ├── canvas.rs
       ├── shapes.rs
       ├── ocr.rs
       ├── llm.rs
       └── drawio.rs
└── public/             # Static files (icons, favicon)
```

## Notes & Caveats

- OCR requires Tesseract to be installed on the host and may need tessdata language files.
- Local LLM inference (GGUF) can be resource intensive; smaller models or Ollama API are viable alternatives for low-spec machines.
- The app attempts to be local-first and offline-capable; network calls are only used when an external LLM API is selected.

## Non-functional Goals

- Cross-platform (Windows / macOS / Linux)
- Small binary and low memory footprint where possible
- Real-time drawing responsiveness
- Local-first privacy by default

## License

MIT

## Contributing

Contributions welcome. For major changes, please open an issue to discuss the proposal before submitting a pull request.

## Acknowledgements

- Tauri — lightweight Rust desktop shell
- perfect-freehand — smooth stroke rendering
- draw.io / diagrams.net — target editor
- Tesseract OCR — optional OCR engine
