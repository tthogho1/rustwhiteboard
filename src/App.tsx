import { useState } from 'react';
import { Canvas } from './components/Canvas';
import { Toolbar } from './components/Toolbar';
import { Preview } from './components/Preview';
import { StatusBar } from './components/StatusBar';
import { useStore } from './store';
import './styles/global.css';

function App() {
  const { theme } = useStore();
  const [showPreview, setShowPreview] = useState(false);

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
