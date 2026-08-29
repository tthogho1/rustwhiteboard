import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import { loadAutosavedCanvas } from './lib/autosave';

// Types
export interface Point {
  x: number;
  y: number;
  pressure?: number;
  timestamp: number;
}

export interface Stroke {
  id: string;
  points: Point[];
  color: string;
  width: number;
  tool: string;
}

/** Mirrors `shapes::ShapeType` (serde `rename_all = "lowercase"`). */
export type ShapeType =
  | 'rectangle'
  | 'circle'
  | 'ellipse'
  | 'triangle'
  | 'diamond'
  | 'arrow'
  | 'line'
  | 'connector'
  | 'freeform';

export const SHAPE_TYPES: ShapeType[] = [
  'rectangle',
  'circle',
  'ellipse',
  'triangle',
  'diamond',
  'arrow',
  'line',
  'connector',
  'freeform',
];

/** Mirrors `shapes::ShapeProperties`. Tuples arrive as `[x, y]` arrays. */
export interface ShapeProperties {
  center_x: number;
  center_y: number;
  radius?: number | null;
  start_point?: [number, number] | null;
  end_point?: [number, number] | null;
  corner_radius?: number | null;
  arrow_head?: { style: string; size: number; direction: number } | null;
}

export interface DetectedShape {
  id: string;
  shape_type: ShapeType;
  bounds: {
    x: number;
    y: number;
    width: number;
    height: number;
    rotation: number;
  };
  confidence: number;
  stroke_ids: string[];
  properties: ShapeProperties;
}

/** Text typed with the text tool. Position and size are in canvas space. */
export interface TextAnnotation {
  id: string;
  text: string;
  x: number;
  y: number;
  width: number;
  height: number;
  color: string;
  fontSize: number;
}

export interface TextRegion {
  id: string;
  text: string;
  bounds: {
    x: number;
    y: number;
    width: number;
    height: number;
  };
  confidence: number;
  font_size_estimate: number;
}

export interface ProcessingResult {
  shapes: DetectedShape[];
  text_regions: TextRegion[];
  suggested_diagram_type: string;
  confidence: number;
}

export type Tool = 'pen' | 'eraser' | 'text' | 'select' | 'pan';
export type Theme = 'light' | 'dark';

/** One undo step: everything the canvas holds. */
export interface CanvasSnapshot {
  strokes: Stroke[];
  textAnnotations: TextAnnotation[];
}

interface StoreState {
  // Canvas state
  strokes: Stroke[];
  currentStroke: Stroke | null;
  textAnnotations: TextAnnotation[];
  /** Annotation currently open in the inline editor, if any. */
  editingTextId: string | null;

  // Tool state
  tool: Tool;
  penColor: string;
  penWidth: number;
  eraserWidth: number;
  fontSize: number;

  // Selection state (select tool)
  /** Ids of the selected strokes *and* text annotations — the two id spaces
   *  are disjoint, so one list covers both. */
  selectedIds: string[];
  /** Detected shape picked on the detection overlay, for correcting it. */
  selectedShapeId: string | null;

  // View state
  zoom: number;
  panX: number;
  panY: number;

  // Processing state
  isProcessing: boolean;
  processingResult: ProcessingResult | null;
  /** True when the strokes changed after the last analysis, so the detected
   *  shapes / text held by the backend no longer describe the canvas. */
  isAnalysisStale: boolean;

  // UI state
  theme: Theme;
  showGrid: boolean;
  /** Draw the detected shape / text bounds on top of the canvas. */
  showDetection: boolean;
  /** Replace the freehand strokes with the clean detected geometry. */
  cleanupView: boolean;

  // History for undo/redo
  history: CanvasSnapshot[];
  historyIndex: number;

  // Actions
  setTool: (tool: Tool) => void;
  setPenColor: (color: string) => void;
  setPenWidth: (width: number) => void;
  setEraserWidth: (width: number) => void;
  setZoom: (zoom: number) => void;
  setPan: (x: number, y: number) => void;
  setTheme: (theme: Theme) => void;
  setShowGrid: (show: boolean) => void;
  setShowDetection: (show: boolean) => void;
  setCleanupView: (show: boolean) => void;
  setFontSize: (size: number) => void;

  // Stroke actions
  startStroke: (point: Point) => void;
  continueStroke: (point: Point) => void;
  endStroke: () => void;
  addStroke: (stroke: Stroke) => void;
  removeStroke: (id: string) => void;
  clearStrokes: () => void;
  setStrokes: (strokes: Stroke[]) => void;
  /** Swap in a whole canvas (backup restore) as one undo step. */
  replaceCanvas: (contents: CanvasSnapshot) => void;

  // Text annotation actions
  //
  // These are all live edits and deliberately skip history: a keystroke is not
  // an undo step. `finishTextEditing` closes the inline editor and pushes one
  // snapshot for the whole edit.
  addTextAnnotation: (annotation: TextAnnotation) => void;
  updateTextAnnotation: (id: string, patch: Partial<TextAnnotation>) => void;
  removeTextAnnotation: (id: string) => void;
  setTextAnnotations: (annotations: TextAnnotation[]) => void;
  moveTextAnnotation: (id: string, dx: number, dy: number) => void;
  setEditingTextId: (id: string | null) => void;
  finishTextEditing: () => void;

  // Selection actions
  setSelection: (ids: string[]) => void;
  toggleSelection: (id: string) => void;
  clearSelection: () => void;
  selectAll: () => void;
  /** Shift the selected strokes. Does not touch history — the caller commits
   *  once when the drag ends, so a drag is a single undo step. */
  moveSelected: (dx: number, dy: number) => void;
  deleteSelected: () => void;
  duplicateSelected: () => void;
  restyleSelected: (style: { color?: string; width?: number }) => void;
  setSelectedShapeId: (id: string | null) => void;

  // History actions
  undo: () => void;
  redo: () => void;
  saveHistory: () => void;

  // Processing actions
  setProcessing: (processing: boolean) => void;
  setProcessingResult: (result: ProcessingResult | null) => void;
  setAnalysisStale: (stale: boolean) => void;
}

const generateId = () => Math.random().toString(36).substring(2, 15);

/** Canvas-space offset applied to duplicated strokes so the copy is visible. */
const DUPLICATE_OFFSET = 16;

const restored = loadAutosavedCanvas();

export const useStore = create<StoreState>()(
  persist(
    (set, get) => ({
      // Initial state
      strokes: restored.strokes,
      currentStroke: null,
      textAnnotations: restored.textAnnotations,
      editingTextId: null,
      tool: 'pen',
      penColor: '#000000',
      penWidth: 3,
      eraserWidth: 20,
      fontSize: 16,
      selectedIds: [],
      selectedShapeId: null,
      zoom: 1,
      panX: 0,
      panY: 0,
      isProcessing: false,
      processingResult: null,
      isAnalysisStale: true,
      theme: 'light',
      showGrid: true,
      showDetection: false,
      cleanupView: false,
      history: [{ strokes: restored.strokes, textAnnotations: restored.textAnnotations }],
      historyIndex: 0,

      // Tool actions
      setTool: tool =>
        set(state => ({
          tool,
          // A selection only means something while the select tool is active.
          selectedIds: tool === 'select' ? state.selectedIds : [],
        })),
      setPenColor: color => set({ penColor: color }),
      setPenWidth: width => set({ penWidth: width }),
      setEraserWidth: width => set({ eraserWidth: width }),
      setZoom: zoom => set({ zoom: Math.max(0.1, Math.min(5, zoom)) }),
      setPan: (x, y) => set({ panX: x, panY: y }),
      setTheme: theme => set({ theme }),
      setShowGrid: show => set({ showGrid: show }),
      setShowDetection: show =>
        // Hiding the overlay must take the correction panel with it — the panel
        // is anchored to a box that is no longer on screen.
        set(show ? { showDetection: true } : { showDetection: false, selectedShapeId: null }),
      setCleanupView: show => set({ cleanupView: show }),
      setFontSize: size => set({ fontSize: size }),

      // Stroke actions
      startStroke: point => {
        const { tool, penColor, penWidth, eraserWidth } = get();
        set({
          currentStroke: {
            id: generateId(),
            points: [point],
            color: tool === 'eraser' ? '#ffffff' : penColor,
            width: tool === 'eraser' ? eraserWidth : penWidth,
            tool,
          },
        });
      },

      continueStroke: point => {
        const { currentStroke } = get();
        if (currentStroke) {
          set({
            currentStroke: {
              ...currentStroke,
              points: [...currentStroke.points, point],
            },
          });
        }
      },

      endStroke: () => {
        const { currentStroke, strokes, tool, selectedIds } = get();
        if (currentStroke && currentStroke.points.length > 1) {
          if (tool === 'eraser') {
            // For eraser, find and remove intersecting strokes
            const eraserPath = currentStroke.points;
            const remainingStrokes = strokes.filter(
              stroke => !strokesIntersect(stroke.points, eraserPath, currentStroke.width)
            );
            const remainingText = get().textAnnotations.filter(
              annotation => !pathCrossesBox(eraserPath, annotation, currentStroke.width)
            );
            set({
              strokes: remainingStrokes,
              textAnnotations: remainingText,
              currentStroke: null,
              isAnalysisStale: true,
              selectedIds: pruneSelection(selectedIds, remainingStrokes, remainingText),
            });
          } else {
            set({
              strokes: [...strokes, currentStroke],
              currentStroke: null,
              isAnalysisStale: true,
            });
          }
          get().saveHistory();
        } else {
          set({ currentStroke: null });
        }
      },

      addStroke: stroke => {
        set(state => ({
          strokes: [...state.strokes, stroke],
          isAnalysisStale: true,
        }));
        get().saveHistory();
      },

      removeStroke: id => {
        set(state => ({
          strokes: state.strokes.filter(s => s.id !== id),
          selectedIds: state.selectedIds.filter(sid => sid !== id),
          isAnalysisStale: true,
        }));
        get().saveHistory();
      },

      clearStrokes: () => {
        set({
          strokes: [],
          textAnnotations: [],
          editingTextId: null,
          selectedIds: [],
          selectedShapeId: null,
          processingResult: null,
          isAnalysisStale: true,
        });
        get().saveHistory();
      },

      setStrokes: strokes => {
        set({
          strokes,
          selectedIds: [],
          selectedShapeId: null,
          processingResult: null,
          isAnalysisStale: true,
        });
        get().saveHistory();
      },

      replaceCanvas: ({ strokes, textAnnotations }) => {
        set({
          strokes,
          textAnnotations,
          editingTextId: null,
          selectedIds: [],
          selectedShapeId: null,
          processingResult: null,
          isAnalysisStale: true,
        });
        get().saveHistory();
      },

      // Text annotation actions
      addTextAnnotation: annotation =>
        set(state => ({ textAnnotations: [...state.textAnnotations, annotation] })),

      updateTextAnnotation: (id, patch) =>
        set(state => ({
          textAnnotations: state.textAnnotations.map(a => (a.id === id ? { ...a, ...patch } : a)),
        })),

      removeTextAnnotation: id =>
        set(state => ({
          textAnnotations: state.textAnnotations.filter(a => a.id !== id),
          editingTextId: state.editingTextId === id ? null : state.editingTextId,
        })),

      setTextAnnotations: annotations => {
        set({ textAnnotations: annotations, editingTextId: null });
        get().saveHistory();
      },

      moveTextAnnotation: (id, dx, dy) => {
        if (dx === 0 && dy === 0) return;
        set(state => ({
          textAnnotations: state.textAnnotations.map(a =>
            a.id === id ? { ...a, x: a.x + dx, y: a.y + dy } : a
          ),
        }));
      },

      setEditingTextId: id => set({ editingTextId: id }),

      finishTextEditing: () => {
        const { editingTextId, textAnnotations } = get();
        if (!editingTextId) return;

        const annotation = textAnnotations.find(a => a.id === editingTextId);
        const remaining =
          annotation && annotation.text.trim() === ''
            ? textAnnotations.filter(a => a.id !== editingTextId)
            : textAnnotations;

        set({ editingTextId: null, textAnnotations: remaining });

        // Opening and closing the editor without changing anything must not
        // leave a no-op step in the undo stack.
        const snapshot = get().history[get().historyIndex];
        if (JSON.stringify(snapshot?.textAnnotations) !== JSON.stringify(remaining)) {
          get().saveHistory();
        }
      },

      // Selection actions
      setSelection: ids => set({ selectedIds: ids }),

      toggleSelection: id =>
        set(state => ({
          selectedIds: state.selectedIds.includes(id)
            ? state.selectedIds.filter(sid => sid !== id)
            : [...state.selectedIds, id],
        })),

      clearSelection: () => set({ selectedIds: [] }),

      selectAll: () =>
        set(state => ({
          selectedIds: [
            ...state.strokes.map(s => s.id),
            ...state.textAnnotations.map(a => a.id),
          ],
        })),

      moveSelected: (dx, dy) => {
        if (dx === 0 && dy === 0) return;
        set(state => {
          if (state.selectedIds.length === 0) return state;
          const selected = new Set(state.selectedIds);
          return {
            strokes: state.strokes.map(stroke =>
              selected.has(stroke.id)
                ? {
                    ...stroke,
                    points: stroke.points.map(p => ({ ...p, x: p.x + dx, y: p.y + dy })),
                  }
                : stroke
            ),
            textAnnotations: state.textAnnotations.map(annotation =>
              selected.has(annotation.id)
                ? { ...annotation, x: annotation.x + dx, y: annotation.y + dy }
                : annotation
            ),
            isAnalysisStale: true,
          };
        });
      },

      deleteSelected: () => {
        const { selectedIds } = get();
        if (selectedIds.length === 0) return;
        const selected = new Set(selectedIds);
        set(state => ({
          strokes: state.strokes.filter(s => !selected.has(s.id)),
          textAnnotations: state.textAnnotations.filter(a => !selected.has(a.id)),
          selectedIds: [],
          isAnalysisStale: true,
        }));
        get().saveHistory();
      },

      duplicateSelected: () => {
        const { strokes, textAnnotations, selectedIds } = get();
        if (selectedIds.length === 0) return;

        const selected = new Set(selectedIds);
        const strokeCopies = strokes
          .filter(s => selected.has(s.id))
          .map(stroke => ({
            ...stroke,
            id: generateId(),
            points: stroke.points.map(p => ({
              ...p,
              x: p.x + DUPLICATE_OFFSET,
              y: p.y + DUPLICATE_OFFSET,
            })),
          }));
        const textCopies = textAnnotations
          .filter(a => selected.has(a.id))
          .map(annotation => ({
            ...annotation,
            id: generateId(),
            x: annotation.x + DUPLICATE_OFFSET,
            y: annotation.y + DUPLICATE_OFFSET,
          }));

        set({
          strokes: [...strokes, ...strokeCopies],
          textAnnotations: [...textAnnotations, ...textCopies],
          // Leave the copies selected so they can be dragged straight away.
          selectedIds: [...strokeCopies.map(s => s.id), ...textCopies.map(a => a.id)],
          isAnalysisStale: true,
        });
        get().saveHistory();
      },

      restyleSelected: ({ color, width }) => {
        const { selectedIds } = get();
        if (selectedIds.length === 0 || (color === undefined && width === undefined)) return;

        const selected = new Set(selectedIds);
        set(state => ({
          strokes: state.strokes.map(stroke =>
            selected.has(stroke.id)
              ? {
                  ...stroke,
                  color: color ?? stroke.color,
                  width: width ?? stroke.width,
                }
              : stroke
          ),
          // Only the colour carries over to text; `width` is a pen concept.
          textAnnotations:
            color === undefined
              ? state.textAnnotations
              : state.textAnnotations.map(annotation =>
                  selected.has(annotation.id) ? { ...annotation, color } : annotation
                ),
          // Width changes the stroke bounds, so detection has to run again.
          isAnalysisStale: width !== undefined ? true : state.isAnalysisStale,
        }));
        get().saveHistory();
      },

      setSelectedShapeId: id => set({ selectedShapeId: id }),

      // History actions
      saveHistory: () => {
        const { strokes, textAnnotations, history, historyIndex } = get();
        const newHistory = history.slice(0, historyIndex + 1);
        newHistory.push({ strokes: [...strokes], textAnnotations: [...textAnnotations] });
        // Trim first, then index into the trimmed list — otherwise the index
        // points past the end once more than 50 states have accumulated.
        const trimmed = newHistory.slice(-50);
        set({
          history: trimmed,
          historyIndex: trimmed.length - 1,
        });
      },

      undo: () => {
        const { history, historyIndex } = get();
        if (historyIndex > 0) restoreSnapshot(set, history[historyIndex - 1], historyIndex - 1);
      },

      redo: () => {
        const { history, historyIndex } = get();
        if (historyIndex < history.length - 1) {
          restoreSnapshot(set, history[historyIndex + 1], historyIndex + 1);
        }
      },

      // Processing actions
      setProcessing: processing => set({ isProcessing: processing }),
      setProcessingResult: result =>
        set(state => ({
          processingResult: result,
          // The picked shape may not exist in the new result.
          selectedShapeId:
            state.selectedShapeId && result?.shapes.some(s => s.id === state.selectedShapeId)
              ? state.selectedShapeId
              : null,
        })),
      setAnalysisStale: stale => set({ isAnalysisStale: stale }),
    }),
    {
      name: 'rustwhiteboard-storage',
      // Strokes are deliberately *not* persisted here: this middleware writes on
      // every set(), which includes each pointermove. They go through the
      // debounced autosave in lib/autosave.ts instead.
      partialize: state => ({
        theme: state.theme,
        penColor: state.penColor,
        penWidth: state.penWidth,
        showGrid: state.showGrid,
      }),
    }
  )
);

type SetState = (
  updater: (state: StoreState) => Partial<StoreState>
) => void;

function restoreSnapshot(set: SetState, snapshot: CanvasSnapshot, index: number) {
  set(state => ({
    strokes: [...snapshot.strokes],
    textAnnotations: [...snapshot.textAnnotations],
    editingTextId: null,
    historyIndex: index,
    isAnalysisStale: true,
    selectedIds: pruneSelection(state.selectedIds, snapshot.strokes, snapshot.textAnnotations),
  }));
}

/** Drop selected ids whose stroke or annotation no longer exists. */
function pruneSelection(
  ids: string[],
  strokes: Stroke[],
  annotations: TextAnnotation[]
): string[] {
  if (ids.length === 0) return ids;
  const alive = new Set<string>();
  for (const stroke of strokes) alive.add(stroke.id);
  for (const annotation of annotations) alive.add(annotation.id);
  return ids.filter(id => alive.has(id));
}

/** True when any point of the path lands inside the box (plus threshold). */
function pathCrossesBox(
  path: Point[],
  box: { x: number; y: number; width: number; height: number },
  threshold: number
): boolean {
  return path.some(
    p =>
      p.x >= box.x - threshold &&
      p.x <= box.x + box.width + threshold &&
      p.y >= box.y - threshold &&
      p.y <= box.y + box.height + threshold
  );
}

// Helper function to check if two stroke paths intersect
function strokesIntersect(path1: Point[], path2: Point[], threshold: number): boolean {
  for (const p1 of path1) {
    for (const p2 of path2) {
      const dist = Math.sqrt(Math.pow(p1.x - p2.x, 2) + Math.pow(p1.y - p2.y, 2));
      if (dist < threshold) {
        return true;
      }
    }
  }
  return false;
}
