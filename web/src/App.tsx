import { useCallback, useEffect, useState } from 'react';
import { DetailsPanel } from './components/DetailsPanel.tsx';
import { Toolbar } from './components/Toolbar.tsx';
import type { CesiumMap, ViewMode } from './map/CesiumMap.ts';
import { MapView } from './map/MapView.tsx';
import { useCore } from './worker/useCore.ts';

const MAX_PROBLEMS = 5;

export function App() {
  const core = useCore();
  const [map, setMap] = useState<CesiumMap | null>(null);
  const [mode, setMode] = useState<ViewMode>('3d');
  const [switching, setSwitching] = useState(false);
  const [problems, setProblems] = useState<readonly string[]>([]);
  const site = core.state === 'ready' ? core.site : null;
  const controlsDisabled = map === null || site === null;

  const onModeChange = useCallback((m: ViewMode) => {
    setMode(m);
    setSwitching(false);
  }, []);
  const onRendererProblem = useCallback((message: string) => {
    setProblems((p) => [...p.slice(1 - MAX_PROBLEMS), message]);
  }, []);
  const requestMode = useCallback(
    (m: ViewMode) => {
      if (map?.setMode(m)) setSwitching(true);
    },
    [map],
  );

  useEffect(() => {
    if (controlsDisabled) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.altKey || e.ctrlKey || e.metaKey || e.repeat) return;
      const target = e.target;
      if (
        target instanceof HTMLElement &&
        target.closest('input, textarea, select, [contenteditable]')
      ) {
        return;
      }
      const key = e.key.toLowerCase();
      if (key === '2') requestMode('2d');
      else if (key === '3') requestMode('3d');
      else if (key === 'h') map.fitSite();
      else if (key === 'n') map.northUp();
    };
    window.addEventListener('keydown', onKey);
    return () => {
      window.removeEventListener('keydown', onKey);
    };
  }, [controlsDisabled, map, requestMode]);

  return (
    <div className="app">
      <main className="map-area">
        <MapView
          site={site}
          onMap={setMap}
          onModeChange={onModeChange}
          onRendererProblem={onRendererProblem}
        />
        <Toolbar
          mode={mode}
          switching={switching}
          disabled={controlsDisabled}
          onMode={requestMode}
          onHome={() => map?.fitSite()}
          onNorthUp={() => map?.northUp()}
        />
      </main>
      <DetailsPanel core={core} mode={mode} switching={switching} problems={problems} />
    </div>
  );
}
