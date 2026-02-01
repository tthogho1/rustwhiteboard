# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

RustWhiteboard is a Tauri v2 desktop application that converts hand-drawn diagrams into editable draw.io files. It uses React + TypeScript for the frontend and Rust for the backend, with optional OCR (Tesseract) and LLM capabilities.

## Development Commands

### Frontend + Backend Development
```bash
# Run in development mode (starts Vite dev server + Tauri)
npm run tauri dev

# Build the frontend only
npm run build

# Preview the production frontend build
npm run preview
```

### Rust Backend
```bash
# Build Rust backend in development mode
cd src-tauri
cargo build

# Build with OCR support (requires system Tesseract)
cargo build --features ocr

# Build with Ollama API support
cargo build --features ollama

# Build with all features
cargo build --features ocr,ollama

# Run Rust tests
cargo test

# Check for compilation errors without building
cargo check
```

### Production Build
```bash
# Create production bundle for current platform
npm run tauri build
```

Note: Production builds require platform-specific Tauri prerequisites (code signing certificates, etc.).

## Architecture

### Frontend-Backend Communication
- Frontend invokes Rust commands via `@tauri-apps/api/core` `invoke()` function
- All Tauri commands are defined in `src-tauri/src/main.rs` with `#[tauri::command]` macro
- Type definitions are shared between Rust (serde Serialize/Deserialize) and TypeScript (in `store.ts`)
- API wrapper functions are in `src/lib/api.ts` for type-safe frontend calls

### State Management
- **Frontend**: Zustand store (`src/store.ts`) manages canvas state, tools, view transform (zoom/pan), history for undo/redo, and processing results
- **Backend**: `AppState` struct in `main.rs` holds shared state across commands using `Mutex` for thread-safe access to strokes, detected shapes, OCR text, and LLM config

### Core Modules (Rust Backend)

**`canvas.rs`**: Processes canvas image data, converts to internal format, applies filters
**`shapes.rs`**: Shape detection engine - analyzes strokes to detect rectangles, circles, diamonds, triangles, arrows, lines, and connectors using geometry algorithms
**`ocr.rs`**: Tesseract OCR integration (optional feature) - extracts text from image regions
**`llm.rs`**: LLM integration - supports local GGUF models, Ollama API, or builtin models for diagram refinement
**`drawio.rs`**: Generates mxGraph XML format compatible with draw.io/diagrams.net

### Frontend Components

**`Canvas.tsx`**: Main drawing surface - handles mouse/touch input, renders strokes using perfect-freehand, manages tools (pen, eraser, select, pan)
**`Toolbar.tsx`**: Tool selection and settings UI
**`Preview.tsx`**: Displays detected shapes and OCR results overlay
**`StatusBar.tsx`**: Shows app status, zoom level, coordinates

### Data Flow
1. User draws on Canvas → frontend records strokes → calls `add_stroke` Tauri command
2. User clicks "Analyze" → Canvas exports image data → calls `process_canvas` command
3. Backend processes image → detects shapes (shapes.rs) → runs OCR if enabled (ocr.rs) → returns results
4. Frontend displays overlay of detected shapes in Preview component
5. Optional: User runs "AI Format" → calls `enhance_with_llm` → LLM refines diagram structure
6. User exports → calls `export_to_drawio` → backend generates mxGraph XML → saves .drawio file

### Canvas Coordinate System
- Canvas uses an infinite coordinate space with pan and zoom transforms
- Frontend maintains `zoom`, `panX`, `panY` in store for view transform
- Strokes are stored in canvas coordinates (not screen coordinates)
- When exporting to image for processing, canvas is rendered to bitmap at current zoom level

### Shape Detection Pipeline
1. Strokes are grouped by proximity and timing
2. Each stroke group is analyzed for geometric properties (circularity, rectangularity, straightness)
3. Shapes are classified based on thresholds in `DetectionParams`
4. Connectors/arrows are detected by analyzing endpoints near other shapes
5. Results include bounding boxes, confidence scores, and shape-specific properties (radius, corner points, etc.)

## Configuration Files

**`tauri.conf.json`**: Tauri app configuration - window settings, bundle options, plugin permissions, CSP
**`vite.config.ts`**: Vite configuration - React plugin, path aliases, Tauri-specific build settings
**`Cargo.toml`**: Rust dependencies and optional features (`ocr`, `ollama`)
**`package.json`**: Frontend dependencies and npm scripts

## Feature Flags

Rust features are defined in `src-tauri/Cargo.toml`:
- `ocr`: Enables Tesseract OCR support (requires system Tesseract installation)
- `ollama`: Enables Ollama API client for remote LLM inference

Default build has no optional features enabled. Enable them with `--features` flag.

## Important Notes

- OCR functionality requires Tesseract to be installed on the host system with language data files (tessdata)
- LLM features can use local GGUF models (place in `models/` directory) or Ollama API
- Bundled resources (`models/*`, `tessdata/*`) are included in production bundles via `tauri.conf.json`
- Canvas rendering uses HTML5 Canvas 2D context with perfect-freehand for smooth stroke rendering
- File system operations use Tauri FS plugin with scoped access to user directories
- The app targets Chrome 105+ on Windows, Safari 13+ on macOS/Linux (ES2021)

## Testing Strategy

Currently no automated tests exist. When adding tests:
- Rust unit tests go alongside module code with `#[cfg(test)]`
- Frontend tests would use Vitest (add `vitest` to devDependencies)
- Integration tests for Tauri commands would use `tauri::test` module
