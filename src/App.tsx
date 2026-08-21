import { useEffect, useState } from 'react';
import { Canvas } from './components/Canvas';
import { Toolbar } from './components/Toolbar';
import { Preview } from './components/Preview';
import { StatusBar } from './components/StatusBar';
import { useStore } from './store';
import { useKeyboardShortcuts } from './lib/useKeyboardShortcuts';
import { flushAutosave, scheduleAutosave } from './lib/autosave';
import './styles/global.css';

function App() {
  const theme = useStore(state => state.theme);
  const strokes = useStore(state => state.strokes);
  const textAnnotations = useStore(state => state.textAnnotations);
  const [showPreview, setShowPreview] = useState(false);

  useKeyboardShortcuts({ onTogglePreview: () => setShowPreview(prev => !prev) });

  // `strokes` keeps its identity while a stroke is in progress, so this only
  // fires once a stroke is committed, undone, moved, or deleted.
  useEffect(() => {
    scheduleAutosave({ strokes, textAnnotations });
  }, [strokes, textAnnotations]);

  // Don't lose the last few hundred milliseconds of work when the window closes.
  useEffect(() => {
    const flush = () => {
      const { strokes: s, textAnnotations: t } = useStore.getState();
      flushAutosave({ strokes: s, textAnnotations: t });
    };
    window.addEventListener('beforeunload', flush);
    return () => window.removeEventListener('beforeunload', flush);
  }, []);

  return (
    <div className={`app ${theme}`}>
      <Toolbar onTogglePreview={() => setShowPreview(!showPreview)} />
      <div className="main-content">
        <Canvas />
        {showPreview && <Preview onClose={() => setShowPreview(false)} />}
      </div>
      <StatusBar />
    </div>
  );
}

export default App;
