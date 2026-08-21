import { useRef, useEffect, useCallback, useMemo, useState } from 'react';
import { useStore, Point } from '../store';
import { getStroke } from 'perfect-freehand';
import {
  hitTestAnnotation,
  hitTestStroke,
  rectContains,
  rectFromCorners,
  strokeBounds,
  strokeIntersectsRect,
  unionBounds,
  type Rect,
} from '../lib/geometry';
import {
  drawCleanShapes,
  drawDetectionOverlay,
  drawTextAnnotations,
  leftoverStrokes,
  type View,
} from '../lib/render';
import { createTextAnnotation } from '../lib/text';
import { TextEditor } from './TextEditor';

const GRID_SIZE = 20;
/** Screen-pixel slack around a stroke that still counts as a click on it. */
const HIT_SLOP_PX = 6;

type DragMode = 'move' | 'marquee' | 'text';

interface DragState {
  mode: DragMode;
  /** Canvas-space anchor the marquee is drawn from. */
  origin: Point;
  /** Canvas-space position of the previous move event (for move deltas). */
  last: Point;
  moved: boolean;
  /** Selection to keep when shift-dragging a marquee. */
  baseSelection: string[];
  /** Annotation being dragged, for `mode === 'text'`. */
  textId?: string;
}

export function Canvas() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const [isDrawing, setIsDrawing] = useState(false);
  const [isPanning, setIsPanning] = useState(false);
  const lastPanPoint = useRef<{ x: number; y: number } | null>(null);
  const dragState = useRef<DragState | null>(null);
  const [marquee, setMarquee] = useState<Rect | null>(null);
  const [isDraggingSelection, setIsDraggingSelection] = useState(false);

  const {
    strokes,
    currentStroke,
    tool,
    zoom,
    panX,
    panY,
    showGrid,
    theme,
    selectedIds,
    textAnnotations,
    editingTextId,
    penColor,
    fontSize,
    showDetection,
    cleanupView,
    processingResult,
    isAnalysisStale,
    addTextAnnotation,
    setEditingTextId,
    finishTextEditing,
    moveTextAnnotation,
    startStroke,
    continueStroke,
    endStroke,
    setSelection,
    toggleSelection,
    clearSelection,
    moveSelected,
    saveHistory,
    setZoom,
    setPan,
  } = useStore();

  const accentColor = theme === 'dark' ? '#4da6ff' : '#0066cc';
  // Memoised so `render` (and the effects keyed on it) don't churn every render.
  const view: View = useMemo(() => ({ zoom, panX, panY }), [zoom, panX, panY]);
  // The clean-up view needs something to draw; fall back to the raw strokes.
  const showClean = cleanupView && (processingResult?.shapes.length ?? 0) > 0;

  // Get canvas context
  const getCtx = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas) return null;
    return canvas.getContext('2d');
  }, []);

  // Convert screen coordinates to canvas coordinates
  const screenToCanvas = useCallback(
    (screenX: number, screenY: number): Point => {
      const canvas = canvasRef.current;
      if (!canvas) return { x: 0, y: 0, timestamp: Date.now() };

      const rect = canvas.getBoundingClientRect();
      const x = (screenX - rect.left - panX) / zoom;
      const y = (screenY - rect.top - panY) / zoom;

      return { x, y, timestamp: Date.now() };
    },
    [zoom, panX, panY]
  );

  // Draw grid
  const drawGrid = useCallback(
    (ctx: CanvasRenderingContext2D, width: number, height: number) => {
      if (!showGrid) return;

      ctx.save();
      ctx.strokeStyle = theme === 'dark' ? '#333333' : '#e0e0e0';
      ctx.lineWidth = 0.5;

      const gridSizeScaled = GRID_SIZE * zoom;
      const offsetX = panX % gridSizeScaled;
      const offsetY = panY % gridSizeScaled;

      // Vertical lines
      for (let x = offsetX; x < width; x += gridSizeScaled) {
        ctx.beginPath();
        ctx.moveTo(x, 0);
        ctx.lineTo(x, height);
        ctx.stroke();
      }

      // Horizontal lines
      for (let y = offsetY; y < height; y += gridSizeScaled) {
        ctx.beginPath();
        ctx.moveTo(0, y);
        ctx.lineTo(width, y);
        ctx.stroke();
      }

      ctx.restore();
    },
    [showGrid, theme, zoom, panX, panY]
  );

  // Draw a single stroke
  const drawStroke = useCallback(
    (ctx: CanvasRenderingContext2D, stroke: typeof currentStroke, alpha = 1) => {
      if (!stroke || stroke.points.length < 2) return;

      ctx.save();
      ctx.globalAlpha = alpha;
      ctx.translate(panX, panY);
      ctx.scale(zoom, zoom);

      // Use perfect-freehand for smooth stroke rendering
      const strokePoints = stroke.points.map(p => [p.x, p.y, p.pressure ?? 0.5]);

      const outlinePoints = getStroke(strokePoints, {
        size: stroke.width,
        thinning: 0.5,
        smoothing: 0.5,
        streamline: 0.5,
        simulatePressure: !stroke.points[0]?.pressure,
      });

      if (outlinePoints.length < 2) {
        ctx.restore();
        return;
      }

      ctx.fillStyle = stroke.color;
      ctx.beginPath();
      ctx.moveTo(outlinePoints[0][0], outlinePoints[0][1]);

      for (let i = 1; i < outlinePoints.length; i++) {
        ctx.lineTo(outlinePoints[i][0], outlinePoints[i][1]);
      }

      ctx.closePath();
      ctx.fill();
      ctx.restore();
    },
    [zoom, panX, panY]
  );

  // Selection highlight + rubber-band rectangle, both in canvas coordinates
  const drawSelectionOverlay = useCallback(
    (ctx: CanvasRenderingContext2D) => {
      const selectedSet = new Set(selectedIds);
      const selected = strokes.filter(s => selectedSet.has(s.id));
      if (selected.length === 0 && !marquee) return;

      ctx.save();
      ctx.translate(panX, panY);
      ctx.scale(zoom, zoom);
      // Keep outlines one screen pixel wide whatever the zoom is.
      ctx.lineWidth = 1 / zoom;

      if (selected.length > 0) {
        ctx.fillStyle = theme === 'dark' ? 'rgba(77, 166, 255, 0.14)' : 'rgba(0, 102, 204, 0.10)';
        for (const stroke of selected) {
          const b = strokeBounds(stroke);
          ctx.fillRect(b.x, b.y, b.width, b.height);
        }

        const union = unionBounds(selected);
        if (union) {
          ctx.setLineDash([6 / zoom, 4 / zoom]);
          ctx.strokeStyle = accentColor;
          ctx.strokeRect(union.x, union.y, union.width, union.height);
          ctx.setLineDash([]);
        }
      }

      if (marquee) {
        ctx.fillStyle = theme === 'dark' ? 'rgba(77, 166, 255, 0.12)' : 'rgba(0, 102, 204, 0.08)';
        ctx.fillRect(marquee.x, marquee.y, marquee.width, marquee.height);
        ctx.setLineDash([4 / zoom, 3 / zoom]);
        ctx.strokeStyle = accentColor;
        ctx.strokeRect(marquee.x, marquee.y, marquee.width, marquee.height);
        ctx.setLineDash([]);
      }

      ctx.restore();
    },
    [strokes, selectedIds, marquee, theme, accentColor, zoom, panX, panY]
  );

  // Main render function
  const render = useCallback(() => {
    const ctx = getCtx();
    const canvas = canvasRef.current;
    if (!ctx || !canvas) return;

    const { width, height } = canvas;

    // Clear canvas
    ctx.fillStyle = theme === 'dark' ? '#1a1a1a' : '#ffffff';
    ctx.fillRect(0, 0, width, height);

    // Draw grid
    drawGrid(ctx, width, height);

    if (showClean && processingResult) {
      // Straightened geometry stands in for the strokes it was detected from;
      // anything unrecognised stays visible, faded, so nothing disappears.
      drawCleanShapes(ctx, processingResult.shapes, view, theme);
      for (const stroke of leftoverStrokes(strokes, processingResult.shapes)) {
        drawStroke(ctx, stroke, 0.25);
      }
    } else {
      for (const stroke of strokes) {
        drawStroke(ctx, stroke);
      }
    }

    // Draw current stroke
    if (currentStroke) {
      drawStroke(ctx, currentStroke);
    }

    drawTextAnnotations(ctx, textAnnotations, view, editingTextId);

    if (showDetection && processingResult) {
      drawDetectionOverlay(ctx, processingResult, view, isAnalysisStale);
    }

    drawSelectionOverlay(ctx);
  }, [
    getCtx,
    theme,
    drawGrid,
    strokes,
    currentStroke,
    drawStroke,
    drawSelectionOverlay,
    textAnnotations,
    editingTextId,
    showDetection,
    showClean,
    processingResult,
    isAnalysisStale,
    view,
  ]);

  // Handle canvas resize
  useEffect(() => {
    const handleResize = () => {
      const canvas = canvasRef.current;
      const container = containerRef.current;
      if (!canvas || !container) return;

      const rect = container.getBoundingClientRect();
      canvas.width = rect.width;
      canvas.height = rect.height;
      render();
    };

    handleResize();
    window.addEventListener('resize', handleResize);
    return () => window.removeEventListener('resize', handleResize);
  }, [render]);

  // Re-render when state changes
  useEffect(() => {
    render();
  }, [render]);

  // Decide what a press with the select tool starts: dragging the selection or
  // rubber-banding a new one.
  const beginSelectGesture = useCallback(
    (point: Point, additive: boolean) => {
      const hit = hitTestStroke(strokes, point.x, point.y, HIT_SLOP_PX / zoom);
      const wasSelected = hit ? selectedIds.includes(hit.id) : false;

      let mode: DragMode = 'marquee';

      if (hit) {
        if (additive) {
          toggleSelection(hit.id);
          if (wasSelected) {
            // Shift+click took it out of the selection — no drag follows.
            dragState.current = null;
            setIsDraggingSelection(false);
            return;
          }
        } else if (!wasSelected) {
          setSelection([hit.id]);
        }
        mode = 'move';
      } else {
        const selectionBounds = unionBounds(strokes.filter(s => selectedIds.includes(s.id)));
        if (!additive && selectionBounds && rectContains(selectionBounds, point.x, point.y)) {
          // Empty spot inside the selection box still grabs the selection.
          mode = 'move';
        } else if (!additive) {
          clearSelection();
        }
      }

      dragState.current = {
        mode,
        origin: point,
        last: point,
        moved: false,
        baseSelection: additive ? selectedIds : [],
      };
      setIsDraggingSelection(mode === 'move');
    },
    [strokes, selectedIds, zoom, setSelection, toggleSelection, clearSelection]
  );

  // Pointer event handlers
  const handlePointerDown = useCallback(
    (e: React.PointerEvent) => {
      e.preventDefault();
      const canvas = canvasRef.current;
      if (!canvas) return;

      canvas.setPointerCapture(e.pointerId);

      if (tool === 'pan' || e.button === 1 || (e.button === 0 && e.ctrlKey)) {
        setIsPanning(true);
        lastPanPoint.current = { x: e.clientX, y: e.clientY };
        return;
      }

      if (tool === 'select') {
        beginSelectGesture(screenToCanvas(e.clientX, e.clientY), e.shiftKey);
        return;
      }

      if (tool === 'text') {
        const point = screenToCanvas(e.clientX, e.clientY);
        // Commit whatever was open before this press changes the target.
        finishTextEditing();

        // Re-read: finishTextEditing may have just dropped a blank annotation.
        const current = useStore.getState().textAnnotations;
        const hit = hitTestAnnotation(current, point.x, point.y, HIT_SLOP_PX / zoom);
        if (hit) {
          // Click edits, drag moves — decided on pointerup.
          dragState.current = {
            mode: 'text',
            origin: point,
            last: point,
            moved: false,
            baseSelection: [],
            textId: hit.id,
          };
        } else {
          const annotation = createTextAnnotation(point.x, point.y, penColor, fontSize);
          addTextAnnotation(annotation);
          setEditingTextId(annotation.id);
        }
        return;
      }

      if (tool === 'pen' || tool === 'eraser') {
        const point = screenToCanvas(e.clientX, e.clientY);
        point.pressure = e.pressure;
        setIsDrawing(true);
        startStroke(point);
      }
    },
    [
      tool,
      screenToCanvas,
      startStroke,
      beginSelectGesture,
      zoom,
      penColor,
      fontSize,
      addTextAnnotation,
      setEditingTextId,
      finishTextEditing,
    ]
  );

  const handlePointerMove = useCallback(
    (e: React.PointerEvent) => {
      if (isPanning && lastPanPoint.current) {
        const dx = e.clientX - lastPanPoint.current.x;
        const dy = e.clientY - lastPanPoint.current.y;
        setPan(panX + dx, panY + dy);
        lastPanPoint.current = { x: e.clientX, y: e.clientY };
        return;
      }

      const drag = dragState.current;

      if (drag?.mode === 'text' && drag.textId) {
        const point = screenToCanvas(e.clientX, e.clientY);
        const dx = point.x - drag.last.x;
        const dy = point.y - drag.last.y;
        if (dx !== 0 || dy !== 0) {
          moveTextAnnotation(drag.textId, dx, dy);
          drag.moved = true;
        }
        drag.last = point;
        return;
      }

      if (tool === 'select' && drag) {
        const point = screenToCanvas(e.clientX, e.clientY);

        if (drag.mode === 'move') {
          const dx = point.x - drag.last.x;
          const dy = point.y - drag.last.y;
          if (dx !== 0 || dy !== 0) {
            moveSelected(dx, dy);
            drag.moved = true;
          }
          drag.last = point;
        } else {
          drag.moved = true;
          drag.last = point;
          setMarquee(rectFromCorners(drag.origin, point));
        }
        return;
      }

      if (isDrawing && (tool === 'pen' || tool === 'eraser')) {
        const point = screenToCanvas(e.clientX, e.clientY);
        point.pressure = e.pressure;
        continueStroke(point);
      }
    },
    [
      isPanning,
      isDrawing,
      tool,
      panX,
      panY,
      setPan,
      screenToCanvas,
      continueStroke,
      moveSelected,
      moveTextAnnotation,
    ]
  );

  const handlePointerUp = useCallback(
    (e: React.PointerEvent) => {
      const canvas = canvasRef.current;
      if (canvas) {
        canvas.releasePointerCapture(e.pointerId);
      }

      if (isPanning) {
        setIsPanning(false);
        lastPanPoint.current = null;
        return;
      }

      const drag = dragState.current;
      if (drag) {
        dragState.current = null;
        setIsDraggingSelection(false);

        if (drag.mode === 'text') {
          // A drag repositions the text; a plain click opens the editor.
          if (drag.moved) saveHistory();
          else if (drag.textId) setEditingTextId(drag.textId);
        } else if (drag.mode === 'move') {
          // One undo step for the whole drag, not one per pointermove.
          if (drag.moved) saveHistory();
        } else {
          if (drag.moved) {
            const rect = rectFromCorners(drag.origin, drag.last);
            const hits = strokes.filter(s => strokeIntersectsRect(s, rect)).map(s => s.id);
            const merged = new Set([...drag.baseSelection, ...hits]);
            setSelection([...merged]);
          }
          setMarquee(null);
        }
        return;
      }

      if (isDrawing) {
        setIsDrawing(false);
        endStroke();
      }
    },
    [isPanning, isDrawing, endStroke, strokes, setSelection, saveHistory, setEditingTextId]
  );

  // Wheel handler for zoom
  const handleWheel = useCallback(
    (e: React.WheelEvent) => {
      e.preventDefault();

      if (e.ctrlKey || e.metaKey) {
        // Zoom
        const delta = e.deltaY > 0 ? 0.9 : 1.1;
        const newZoom = zoom * delta;

        // Zoom toward cursor position
        const rect = canvasRef.current?.getBoundingClientRect();
        if (rect) {
          const cursorX = e.clientX - rect.left;
          const cursorY = e.clientY - rect.top;

          const newPanX = cursorX - (cursorX - panX) * delta;
          const newPanY = cursorY - (cursorY - panY) * delta;

          setZoom(newZoom);
          setPan(newPanX, newPanY);
        } else {
          setZoom(newZoom);
        }
      } else {
        // Pan
        setPan(panX - e.deltaX, panY - e.deltaY);
      }
    },
    [zoom, panX, panY, setZoom, setPan]
  );

  // Get cursor style based on tool
  const getCursor = () => {
    switch (tool) {
      case 'pen':
        return 'crosshair';
      case 'eraser':
        return 'cell';
      case 'text':
        return 'text';
      case 'select':
        return isDraggingSelection ? 'move' : 'crosshair';
      case 'pan':
        return isPanning ? 'grabbing' : 'grab';
      default:
        return 'default';
    }
  };

  return (
    <div ref={containerRef} className="canvas-container">
      <canvas
        ref={canvasRef}
        className="drawing-canvas"
        style={{ cursor: getCursor() }}
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={handlePointerUp}
        onPointerLeave={handlePointerUp}
        onWheel={handleWheel}
        onContextMenu={e => e.preventDefault()}
      />
      <TextEditor />
    </div>
  );
}
