import { useCallback, useEffect, useState } from 'react';
import { DetailsPanel } from './components/DetailsPanel.tsx';
import { PlacementBar, type Placing } from './components/PlacementBar.tsx';
import { EntityDetails, MarkerDetails } from './components/SelectionDetails.tsx';
import { Timeline } from './components/Timeline.tsx';
import { Toolbar } from './components/Toolbar.tsx';
import type { CesiumMap, Pick, ViewMode } from './map/CesiumMap.ts';
import { MapView } from './map/MapView.tsx';
import { useEntityDetail, useSession } from './session/useSession.ts';

const MAX_PROBLEMS = 5;
const DEFAULT_MARKER_LABEL = 'Marker';

function typingIn(target: EventTarget | null): boolean {
  return (
    target instanceof HTMLElement &&
    target.closest('input, textarea, select, [contenteditable]') !== null
  );
}

export function App() {
  const session = useSession();
  const { core, scene } = session;
  const [map, setMap] = useState<CesiumMap | null>(null);
  const [mapKey, setMapKey] = useState(0);
  const [mode, setMode] = useState<ViewMode>('3d');
  const [switching, setSwitching] = useState(false);
  const [problems, setProblems] = useState<readonly string[]>([]);
  const [selection, setSelection] = useState<Pick | null>(null);
  const [placingState, setPlacing] = useState<Placing | null>(null);
  const [busy, setBusy] = useState(false);
  const site = core.state === 'ready' ? core.site : null;
  const controlsDisabled = map === null || site === null;
  const live = scene?.timeline.mode === 'live';
  const editable = core.state === 'ready' && live;
  // Placement only makes sense while editing is possible, and a move only
  // while its marker exists.
  const placing =
    editable &&
    placingState !== null &&
    (placingState.kind === 'add' || scene.annotations.has(placingState.id))
      ? placingState
      : null;
  const detail = useEntityDetail(session, selection?.kind === 'entity' ? selection.id : null);

  const onModeChange = useCallback((m: ViewMode) => {
    setMode(m);
    setSwitching(false);
  }, []);
  const onRendererProblem = useCallback((message: string) => {
    setProblems((p) => [...p.slice(1 - MAX_PROBLEMS), message]);
  }, []);
  const onPlace = useCallback((point: { x_m: number; y_m: number } | null) => {
    setPlacing((p) => p && (point ? { ...p, point, missed: false } : { ...p, missed: true }));
  }, []);
  const requestMode = useCallback(
    (m: ViewMode) => {
      if (map?.setMode(m)) setSwitching(true);
    },
    [map],
  );
  const startAdd = useCallback(() => {
    setPlacing({ kind: 'add', point: null, label: DEFAULT_MARKER_LABEL, missed: false });
  }, []);
  const restartMap = useCallback(() => {
    setProblems([]);
    setMapKey((k) => k + 1);
  }, []);

  const run = async <T,>(action: () => Promise<T>): Promise<T> => {
    setBusy(true);
    try {
      return await action();
    } finally {
      setBusy(false);
    }
  };

  const confirmPlacement = async () => {
    const p = placing;
    if (!p?.point || !scene) return;
    const at = {
      placement: 'ground_plane' as const,
      position: { x_m: p.point.x_m, y_m: p.point.y_m, z_m: 0 },
    };
    if (p.kind === 'add') {
      const id = await run(() => session.addMarker(p.label.trim(), at));
      if (id !== null) {
        setPlacing(null);
        setSelection({ kind: 'annotation', id });
      }
    } else {
      const marker = scene.annotations.get(p.id);
      if (!marker) return;
      const ok = await run(() => session.editMarker(marker.id, marker.revision, { at }));
      if (ok) setPlacing(null);
    }
  };

  const { playing, play, pause, step, goLive } = session;
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.altKey || e.ctrlKey || e.metaKey) return;
      if (e.key === 'Escape') {
        if (placing) setPlacing(null);
        else setSelection(null);
        return;
      }
      if (e.repeat || typingIn(e.target)) return;
      const key = e.key.toLowerCase();
      if (key === ' ') {
        // Space on a focused button presses that button instead.
        if (e.target instanceof HTMLButtonElement || core.state !== 'ready') return;
        e.preventDefault();
        if (playing) pause();
        else play();
      } else if (core.state === 'ready' && key === '.') step();
      else if (core.state === 'ready' && key === 'l') goLive();
      else if (editable && key === 'm') startAdd();
      else if (controlsDisabled) return;
      else if (key === '2') requestMode('2d');
      else if (key === '3') requestMode('3d');
      else if (key === 'h') map.fitSite();
      else if (key === 'n') map.northUp();
    };
    window.addEventListener('keydown', onKey);
    return () => {
      window.removeEventListener('keydown', onKey);
    };
  }, [
    controlsDisabled,
    map,
    requestMode,
    placing,
    playing,
    play,
    pause,
    step,
    goLive,
    editable,
    startAdd,
    core.state,
  ]);

  let selectionDetails = null;
  if (selection && scene && core.state === 'ready') {
    const center = () => {
      map?.centerOn(selection);
    };
    if (selection.kind === 'entity') {
      const entity = scene.entities.get(selection.id);
      if (entity) {
        selectionDetails = (
          <EntityDetails
            entity={entity}
            detail={detail}
            session={core.session}
            now={scene.timeline.tick}
            editable={editable && !busy}
            onCenter={center}
            onRemove={() => {
              void run(() => session.removeEntity(entity.id, entity.revision));
            }}
          />
        );
      }
    } else {
      const marker = scene.annotations.get(selection.id);
      if (marker) {
        selectionDetails = (
          <MarkerDetails
            key={`${marker.id}@${marker.revision}`}
            marker={marker}
            editable={editable}
            busy={busy}
            onRename={(label) => {
              void run(() => session.editMarker(marker.id, marker.revision, { label }));
            }}
            onCenter={center}
            onMove={() => {
              setPlacing({
                kind: 'move',
                id: marker.id,
                label: marker.label,
                point: null,
                missed: false,
              });
            }}
            onDelete={() => {
              void run(() => session.deleteAnnotation(marker.id, marker.revision));
            }}
          />
        );
      }
    }
  }

  return (
    <div className="app">
      <main className="map-area">
        <MapView
          key={mapKey}
          site={site}
          scene={scene}
          selection={selection}
          tool={placing ? 'place' : 'navigate'}
          preview={placing?.point ?? null}
          onMap={setMap}
          onModeChange={onModeChange}
          onRendererProblem={onRendererProblem}
          onPick={setSelection}
          onPlace={onPlace}
        />
        <div className="overlay-top">
          <Toolbar
            mode={mode}
            switching={switching}
            disabled={controlsDisabled}
            editable={editable}
            placing={placing !== null}
            onMode={requestMode}
            onHome={() => map?.fitSite()}
            onNorthUp={() => map?.northUp()}
            onAddMarker={startAdd}
          />
          {session.notice && (
            <div className="notice" role="status">
              <p>{session.notice}</p>
              <button type="button" onClick={session.dismissNotice}>
                Dismiss
              </button>
            </div>
          )}
        </div>
        <div className="overlay-bottom" data-placing={placing !== null}>
          {placing && (
            <PlacementBar
              placing={placing}
              busy={busy}
              onLabel={(label) => {
                setPlacing((p) => (p?.kind === 'add' ? { ...p, label } : p));
              }}
              onConfirm={() => {
                void confirmPlacement();
              }}
              onCancel={() => {
                setPlacing(null);
              }}
            />
          )}
          <Timeline
            timeline={scene?.timeline ?? null}
            tickMs={core.state === 'ready' ? core.session.tick_ms : 100}
            playing={playing}
            speed={session.speed}
            onPlay={play}
            onPause={pause}
            onStep={step}
            onSpeed={session.setSpeed}
            onScrub={session.scrub}
            onLive={goLive}
          />
        </div>
      </main>
      <DetailsPanel
        session={session}
        selection={selection}
        selectionDetails={selectionDetails}
        mode={mode}
        switching={switching}
        problems={problems}
        onSelect={setSelection}
        onClearSelection={() => {
          setSelection(null);
        }}
        onRestartMap={restartMap}
      />
    </div>
  );
}
