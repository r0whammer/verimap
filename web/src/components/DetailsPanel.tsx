import { useId, useState } from 'react';
import { UI_BUILD } from '../build-info.ts';
import { degrees, metres } from '../format.ts';
import { PROTOCOL_VERSION, type ProtocolVersion } from '../generated/protocol.ts';
import type { ViewMode } from '../map/CesiumMap.ts';
import type { CoreStatus } from '../worker/bridge.ts';

export interface DetailsPanelProps {
  core: CoreStatus;
  mode: ViewMode;
  switching: boolean;
  problems: readonly string[];
}

const version = (v: ProtocolVersion) => `${v.major}.${v.minor}`;

const CORE_LABEL: Record<CoreStatus['state'], string> = {
  starting: 'Starting…',
  ready: 'Ready',
  incompatible: 'Incompatible — reload needed',
  failed: 'Failed',
};

/** Sidebar on wide screens, collapsible bottom sheet on narrow ones. All
 * text from the core (site name, messages) is rendered as text. */
export function DetailsPanel({ core, mode, switching, problems }: DetailsPanelProps) {
  const [collapsed, setCollapsed] = useState(false);
  const bodyId = useId();

  return (
    <aside className="panel" aria-label="Scene details" data-collapsed={collapsed}>
      <header className="panel-header">
        <div>
          <h1>VeriMap</h1>
          <p className="muted">Synthetic demo site — no live sensor data</p>
        </div>
        <button
          type="button"
          className="sheet-toggle"
          aria-expanded={!collapsed}
          aria-controls={bodyId}
          onClick={() => {
            setCollapsed((c) => !c);
          }}
        >
          {collapsed ? 'Show details' : 'Hide details'}
        </button>
      </header>

      <div className="panel-body" id={bodyId}>
        <section aria-labelledby={`${bodyId}-core`}>
          <h2 id={`${bodyId}-core`}>Core</h2>
          <dl>
            <dt>Status</dt>
            <dd>
              <span className={`badge badge-${core.state}`} data-testid="core-status" role="status">
                {CORE_LABEL[core.state]}
              </span>
            </dd>
            <dt>Protocol</dt>
            <dd data-testid="protocol">
              UI {version(PROTOCOL_VERSION)}
              {core.state === 'ready' && ` · core ${version(core.protocol)}`}
              {core.state === 'incompatible' && ` · core ${version(core.coreProtocol)}`}
            </dd>
            <dt>Builds</dt>
            <dd className="mono">
              UI {UI_BUILD}
              {core.state === 'ready' && ` · core ${core.coreBuild}`}
            </dd>
          </dl>
          {core.state === 'failed' && (
            <p className="problem" role="alert">
              {core.reason}
            </p>
          )}
          {core.state === 'incompatible' && (
            <p className="problem" role="alert">
              This page and its cached core are different versions. Reload to load a matching set.{' '}
              <button
                type="button"
                onClick={() => {
                  window.location.reload();
                }}
              >
                Reload
              </button>
            </p>
          )}
        </section>

        {core.state === 'ready' && (
          <section aria-labelledby={`${bodyId}-site`}>
            <h2 id={`${bodyId}-site`}>Site</h2>
            <dl>
              <dt>Name</dt>
              <dd data-testid="site-name">{core.site.name}</dd>
              <dt>ID</dt>
              <dd className="mono">{core.site.site_id}</dd>
              <dt>Anchor</dt>
              <dd>
                {degrees(core.site.anchor.latitude_deg)} lat,{' '}
                {degrees(core.site.anchor.longitude_deg)} lon
                <br />
                {metres(core.site.anchor.ellipsoidal_height_m * 1000)} above the WGS84 ellipsoid
              </dd>
              <dt>Extent</dt>
              <dd>
                {metres(core.site.bounds.x.max_mm - core.site.bounds.x.min_mm)} east ×{' '}
                {metres(core.site.bounds.y.max_mm - core.site.bounds.y.min_mm)} north
                <br />
                up {metres(core.site.bounds.z.min_mm)} to {metres(core.site.bounds.z.max_mm)}
              </dd>
            </dl>
          </section>
        )}

        <section aria-labelledby={`${bodyId}-view`}>
          <h2 id={`${bodyId}-view`}>View</h2>
          <dl>
            <dt>Mode</dt>
            <dd data-testid="view-mode">{switching ? 'Switching…' : mode.toUpperCase()}</dd>
          </dl>
          {mode === '2d' && !switching && (
            <p className="muted">2D is a flat map: heights are not shown as geometry.</p>
          )}
          {problems.length > 0 && (
            <ul className="problem" role="alert" aria-label="Renderer problems">
              {problems.map((p, i) => (
                <li key={i}>{p}</li>
              ))}
            </ul>
          )}
        </section>

        <section aria-labelledby={`${bodyId}-legend`}>
          <h2 id={`${bodyId}-legend`}>Legend</h2>
          <ul className="legend">
            <li>
              <span className="swatch swatch-grid" aria-hidden="true" /> Reference ground
              (synthetic): 10 m grid, heavier line every 50 m
            </li>
            <li>
              <span className="swatch swatch-bounds" aria-hidden="true" /> Site boundary and scene
              volume
            </li>
            <li>
              <span className="swatch swatch-anchor" aria-hidden="true" /> Anchor: scene origin (x
              east, y north, z up)
            </li>
          </ul>
          <p className="muted">
            Keys: 2 / 3 switch view, H fits the site, N faces north. Drag to pan, scroll or pinch to
            zoom; in 3D, right-drag or two-finger drag to tilt.
          </p>
        </section>
      </div>
    </aside>
  );
}
