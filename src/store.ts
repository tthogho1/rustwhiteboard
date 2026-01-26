import { create } from 'zustand';
import { persist } from 'zustand/middleware';

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

export interface DetectedShape {
  id: string;
  shape_type: string;
  bounds: {
    x: number;
    y: number;
    width: number;
    height: number;
    rotation: number;
  };
  confidence: number;
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
}

export interface ProcessingResult {
  shapes: DetectedShape[];
  text_regions: TextRegion[];
  suggested_diagram_type: string;
  confidence: number;
}

export type Tool = 'pen' | 'eraser' | 'select' | 'pan';
export type Theme = 'light' | 'dark';

interface StoreState {
  // Canvas state
  strokes: Stroke[];
  currentStroke: Stroke | null;

  // Tool state
  tool: Tool;
  penColor: string;
  penWidth: number;
  eraserWidth: number;

  // View state
  zoom: number;
  panX: number;
  panY: number;

  // Processing state
  isProcessing: boolean;
  processingResult: ProcessingResult | null;

  // UI state
  theme: Theme;
  showGrid: boolean;

  // History for undo/redo
  history: Stroke[][];
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

  // Stroke actions
  startStroke: (point: Point) => void;
  continueStroke: (point: Point) => void;
  endStroke: () => void;
  addStroke: (stroke: Stroke) => void;
  removeStroke: (id: string) => void;
  clearStrokes: () => void;
  setStrokes: (strokes: Stroke[]) => void;

  // History actions
  undo: () => void;
  redo: () => void;
  saveHistory: () => void;

  // Processing actions
  setProcessing: (processing: boolean) => void;
  setProcessingResult: (result: ProcessingResult | null) => void;
}

const generateId = () => Math.random().toString(36).substring(2, 15);

export const useStore = create<StoreState>()(
  persist(
    (set, get) => ({
      // Initial state
      strokes: [],
      currentStroke: null,
      tool: 'pen',
      penColor: '#000000',
      penWidth: 3,
      eraserWidth: 20,
      zoom: 1,
      panX: 0,
      panY: 0,
      isProcessing: false,
      processingResult: null,
      theme: 'light',
      showGrid: true,
      history: [[]],
      historyIndex: 0,

      // Tool actions
      setTool: tool => set({ tool }),
      setPenColor: color => set({ penColor: color }),
      setPenWidth: width => set({ penWidth: width }),
      setEraserWidth: width => set({ eraserWidth: width }),
      setZoom: zoom => set({ zoom: Math.max(0.1, Math.min(5, zoom)) }),
      setPan: (x, y) => set({ panX: x, panY: y }),
      setTheme: theme => set({ theme }),
      setShowGrid: show => set({ showGrid: show }),

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
        const { currentStroke, strokes, tool } = get();
        if (currentStroke && currentStroke.points.length > 1) {
          if (tool === 'eraser') {
            // For eraser, find and remove intersecting strokes
            const eraserPath = currentStroke.points;
            const remainingStrokes = strokes.filter(
              stroke => !strokesIntersect(stroke.points, eraserPath, currentStroke.width)
            );
            set({
              strokes: remainingStrokes,
              currentStroke: null,
            });
          } else {
            set({
              strokes: [...strokes, currentStroke],
              currentStroke: null,
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
        }));
        get().saveHistory();
      },

      removeStroke: id => {
        set(state => ({
          strokes: state.strokes.filter(s => s.id !== id),
        }));
        get().saveHistory();
      },

      clearStrokes: () => {
        set({ strokes: [], processingResult: null });
        get().saveHistory();
      },

      setStrokes: strokes => {
        set({ strokes });
        get().saveHistory();
      },

      // History actions
      saveHistory: () => {
        const { strokes, history, historyIndex } = get();
        const newHistory = history.slice(0, historyIndex + 1);
        newHistory.push([...strokes]);
        set({
          history: newHistory.slice(-50), // Keep last 50 states
          historyIndex: newHistory.length - 1,
        });
      },

      undo: () => {
        const { history, historyIndex } = get();
        if (historyIndex > 0) {
          const newIndex = historyIndex - 1;
          set({
            strokes: [...history[newIndex]],
            historyIndex: newIndex,
          });
        }
      },

      redo: () => {
        const { history, historyIndex } = get();
        if (historyIndex < history.length - 1) {
          const newIndex = historyIndex + 1;
          set({
            strokes: [...history[newIndex]],
            historyIndex: newIndex,
          });
        }
      },

      // Processing actions
      setProcessing: processing => set({ isProcessing: processing }),
      setProcessingResult: result => set({ processingResult: result }),
    }),
    {
      name: 'rustwhiteboard-storage',
      partialize: state => ({
        theme: state.theme,
        penColor: state.penColor,
        penWidth: state.penWidth,
        showGrid: state.showGrid,
      }),
    }
  )
);

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
