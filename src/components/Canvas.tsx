import { useRef, useEffect, useCallback, useState } from 'react';
import { useStore, Point } from '../store';
import { getStroke } from 'perfect-freehand';

const GRID_SIZE = 20;

export function Canvas() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const [isDrawing, setIsDrawing] = useState(false);
  const [isPanning, setIsPanning] = useState(false);
  const lastPanPoint = useRef<{ x: number; y: number } | null>(null);

  const {
    strokes,
    currentStroke,
    tool,
    zoom,
    panX,
    panY,
    showGrid,
    theme,
    startStroke,
    continueStroke,
    endStroke,
    setZoom,
    setPan,
  } = useStore();

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
    (ctx: CanvasRenderingContext2D, stroke: typeof currentStroke) => {
      if (!stroke || stroke.points.length < 2) return;

      ctx.save();
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

    // Draw all strokes
    for (const stroke of strokes) {
      drawStroke(ctx, stroke);
    }

    // Draw current stroke
    if (currentStroke) {
      drawStroke(ctx, currentStroke);
    }
  }, [getCtx, theme, drawGrid, strokes, currentStroke, drawStroke]);

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

      if (tool === 'pen' || tool === 'eraser') {
        const point = screenToCanvas(e.clientX, e.clientY);
        point.pressure = e.pressure;
        setIsDrawing(true);
        startStroke(point);
      }
    },
    [tool, screenToCanvas, startStroke]
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

      if (isDrawing && (tool === 'pen' || tool === 'eraser')) {
        const point = screenToCanvas(e.clientX, e.clientY);
        point.pressure = e.pressure;
        continueStroke(point);
      }
    },
    [isPanning, isDrawing, tool, panX, panY, setPan, screenToCanvas, continueStroke]
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

      if (isDrawing) {
        setIsDrawing(false);
        endStroke();
      }
    },
    [isPanning, isDrawing, endStroke]
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
      case 'select':
        return 'default';
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
    </div>
  );
}
