import { useEffect, useRef } from 'react';
import type { SiteDto } from '../generated/protocol.ts';
import { CesiumMap, type MapEvents } from './CesiumMap.ts';

export interface MapViewProps extends MapEvents {
  site: SiteDto | null;
  /** Receives the adapter once created, and `null` when it is destroyed. */
  onMap: (map: CesiumMap | null) => void;
}

/** Hosts the Cesium adapter for the component's lifetime. */
export function MapView({ site, onMap, onModeChange, onRendererProblem }: MapViewProps) {
  const container = useRef<HTMLDivElement>(null);
  const map = useRef<CesiumMap | null>(null);
  // The adapter lives longer than any one render; it calls the latest
  // callbacks through this ref.
  const events = useRef<MapEvents>({ onModeChange, onRendererProblem });
  useEffect(() => {
    events.current = { onModeChange, onRendererProblem };
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
      });
    } catch (e) {
      events.current.onRendererProblem(`the map could not start: ${String(e)}`);
      return;
    }
    map.current = adapter;
    onMap(adapter);
    return () => {
      onMap(null);
      map.current = null;
      adapter.destroy();
    };
  }, [onMap]);

  useEffect(() => {
    if (!site) return;
    map.current?.setSite(site);
    map.current?.fitSite();
  }, [site]);

  return <div ref={container} className="map" aria-label="Map" role="application" />;
}
