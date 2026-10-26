import { useEffect, useRef } from 'react';
import type { SiteDto } from '../generated/protocol.ts';
import type { SceneSnapshot } from '../scene/model.ts';
import { CesiumMap, type MapEvents, type Pick, type Tool } from './CesiumMap.ts';

export interface MapViewProps extends MapEvents {
  site: SiteDto | null;
  scene: SceneSnapshot | null;
  selection: Pick | null;
  tool: Tool;
  /** An uncommitted placement to preview, scene metres. */
  preview: { x_m: number; y_m: number } | null;
  /** Receives the adapter once created, and `null` when it is destroyed. */
  onMap: (map: CesiumMap | null) => void;
}

/** Hosts the Cesium adapter for the component's lifetime and pushes the
 * current props into it. A new adapter (after a remount) is rebuilt from
 * the props alone, so it always matches the authoritative scene. */
export function MapView({
  site,
  scene,
  selection,
  tool,
  preview,
  onMap,
  ...callbacks
}: MapViewProps) {
  const container = useRef<HTMLDivElement>(null);
  const map = useRef<CesiumMap | null>(null);
  const shownSite = useRef<SiteDto | null>(null);
  // The adapter lives longer than any one render; it calls the latest
  // callbacks through this ref.
  const events = useRef<MapEvents>(callbacks);
  useEffect(() => {
    events.current = callbacks;
  });

  useEffect(() => {
    const element = container.current;
    if (!element) return;
    let adapter: CesiumMap;
    try {
      adapter = new CesiumMap(element, {
        onModeChange: (mode) => {
          events.current.onModeChange(mode);
        },
        onRendererProblem: (message) => {
          events.current.onRendererProblem(message);
        },
        onPick: (pick) => {
          events.current.onPick(pick);
        },
        onPlace: (point) => {
          events.current.onPlace(point);
        },
      });
    } catch (e) {
      events.current.onRendererProblem(`the map could not start: ${String(e)}`);
      return;
    }
    map.current = adapter;
    shownSite.current = null;
    onMap(adapter);
    return () => {
      onMap(null);
      map.current = null;
      adapter.destroy();
    };
  }, [onMap]);

  useEffect(() => {
    const m = map.current;
    if (!m || !site) return;
    if (shownSite.current !== site) {
      m.setSite(site);
      m.fitSite();
      shownSite.current = site;
    }
    if (scene) m.sync(scene.entities, scene.annotations);
  }, [site, scene]);

  useEffect(() => {
    map.current?.setSelection(selection);
  }, [selection]);

  useEffect(() => {
    map.current?.setTool(tool);
  }, [tool]);

  useEffect(() => {
    map.current?.setPreview(preview);
  }, [preview, site]);

  return <div ref={container} className="map" aria-label="Map" role="application" />;
}
