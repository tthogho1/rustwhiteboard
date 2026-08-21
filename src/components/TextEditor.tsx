import { useEffect, useRef } from 'react';
import { useStore } from '../store';
import { FONT_FAMILY, LINE_HEIGHT, measureText } from '../lib/text';

/**
 * Inline editor for the annotation currently being typed. It sits on top of the
 * canvas at the annotation's position and writes straight into the store on
 * every keystroke, so nothing has to be flushed when it closes — the canvas
 * simply skips drawing the annotation this editor is covering.
 */
export function TextEditor() {
  const editingTextId = useStore(state => state.editingTextId);
  const annotation = useStore(
    state => state.textAnnotations.find(a => a.id === state.editingTextId) ?? null
  );
  const zoom = useStore(state => state.zoom);
  const panX = useStore(state => state.panX);
  const panY = useStore(state => state.panY);
  const updateTextAnnotation = useStore(state => state.updateTextAnnotation);
  const finishTextEditing = useStore(state => state.finishTextEditing);

  const ref = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    el.focus();
    el.setSelectionRange(el.value.length, el.value.length);
  }, [editingTextId]);

  if (!annotation) return null;

  const handleChange = (text: string) => {
    // Re-measure so hit testing and the exported bounds track what is typed.
    const { width, height } = measureText(text, annotation.fontSize);
    updateTextAnnotation(annotation.id, { text, width, height });
  };

  const lineHeightPx = annotation.fontSize * LINE_HEIGHT * zoom;

  return (
    <textarea
      ref={ref}
      className="text-editor"
      value={annotation.text}
      spellCheck={false}
      placeholder="Type…"
      onChange={e => handleChange(e.target.value)}
      onBlur={finishTextEditing}
      onKeyDown={e => {
        // Enter inserts a newline; Escape is how you finish.
        if (e.key === 'Escape') {
          e.preventDefault();
          finishTextEditing();
        }
        // The canvas shortcuts already ignore text fields, but the wheel/zoom
        // handlers do not — keep key events local either way.
        e.stopPropagation();
      }}
      style={{
        left: `${annotation.x * zoom + panX}px`,
        top: `${annotation.y * zoom + panY}px`,
        width: `${Math.max(annotation.width, 60) * zoom + 8}px`,
        height: `${Math.max(annotation.height, annotation.fontSize * LINE_HEIGHT) * zoom + 4}px`,
        fontFamily: FONT_FAMILY,
        fontSize: `${annotation.fontSize * zoom}px`,
        lineHeight: `${lineHeightPx}px`,
        color: annotation.color,
      }}
    />
  );
}
