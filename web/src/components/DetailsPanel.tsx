import { useId, useState, type ReactNode } from 'react';
import { UI_BUILD } from '../build-info.ts';
import { degrees, metres } from '../format.ts';
import { PROTOCOL_VERSION, type ProtocolVersion } from '../generated/protocol.ts';
import type { Pick, ViewMode } from '../map/CesiumMap.ts';
import type { Session } from '../session/useSession.ts';
import type { CoreStatus } from '../worker/bridge.ts';
import { ObjectList } from './ObjectList.tsx';

export interface DetailsPanelProps {
  session: Session;
  selection: Pick | null;
  /** The selected object's details, or null for none. */
  selectionDetails: ReactNode;
  mode: ViewMode;
  switching: boolean;
  problems: readonly string[];
  onSelect: (pick: Pick) => void;
  onClearSelection: () => void;
  onRestartMap: () => void;
}

const version = (v: ProtocolVersion) => `${v.major}.${v.minor}`;

const CORE_LABEL: Record<CoreStatus['state'], string> = {
  starting: 'Starting…',
  ready: 'Ready',
  incompatible: 'Incompatible — reload needed',
  failed: 'Failed',
};

/** Sidebar on wide screens, collapsible bottom sheet on narrow ones. All
 * text from the core (site name, labels, messages) is rendered as text. */
export function DetailsPanel({
  session,
  selection,
  selectionDetails,
  mode,
  switching,
  problems,
  onSelect,
  onClearSelection,
  onRestartMap,
}: DetailsPanelProps) {
  const [collapsed, setCollapsed] = useState(false);
  const bodyId = useId();
  const { core, scene } = session;
  const heading = (name: string) => `${bodyId}-${name}`;

  return (
    <aside className="panel" aria-label="Scene details" data-collapsed={collapsed}>
      <header className="panel-header">
        <div>
          <h1>VeriMap</h1>
          <p className="muted">Simulated sensors on a synthetic site — no live data</p>
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
        {selection !== null && (
          <section aria-labelledby={heading('selection')}>
            <div className="section-heading">
              <h2 id={heading('selection')}>Selected</h2>
              <button type="button" onClick={onClearSelection} title="Clear selection (Escape)">
                Clear
              </button>
            </div>
            {selectionDetails ?? (
              <p className="muted" data-testid="selection-title">
                <span className="mono">{selection.id}</span> is not in this view: it was removed, or
                does not exist yet at this time.
              </p>
            )}
          </section>
        )}

        {scene && (
          <section aria-labelledby={heading('objects')}>
            <h2 id={heading('objects')}>Objects</h2>
            <ObjectList
              entities={scene.entities}
              annotations={scene.annotations}
              selection={selection}
              onSelect={onSelect}
            />
          </section>
        )}

        {core.state === 'ready' && scene && (
          <section aria-labelledby={heading('session')}>
            <h2 id={heading('session')}>Session</h2>
            <dl>
              <dt>Simulator</dt>
              <dd>
                seed {core.session.seed}, version {core.session.simulator_version}
              </dd>
              <dt>Journal</dt>
              <dd data-testid="journal">
                {scene.timeline.journal_events} of {scene.timeline.journal_limit} events (not saved
                across reloads)
              </dd>
              <dt>Detections</dt>
              <dd>
                {scene.stats.received} received: {scene.stats.accepted} accepted,{' '}
                {scene.stats.duplicates} duplicate, {scene.stats.late} late, {scene.stats.rejected}{' '}
                rejected
              </dd>
              <dt>State hash</dt>
              <dd>
                {session.stateHash ? (
                  <span className="mono" data-testid="state-hash">
                    {session.stateHash.hash}
                  </span>
                ) : (
                  <span className="muted">not computed</span>
                )}
                {session.stateHash && (
                  <span className="muted">
                    {' '}
                    at tick {session.stateHash.tick}, event {session.stateHash.sequence}
                  </span>
                )}
              </dd>
            </dl>
            <div className="actions">
              <button type="button" onClick={session.computeStateHash}>
                Compute state hash
              </button>
            </div>
            <p className="muted">
              The hash covers the canonical scene state in this view; replaying the same journal
              gives the same hash.
            </p>
          </section>
        )}

        <section aria-labelledby={heading('core')}>
          <h2 id={heading('core')}>Core</h2>
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
          <section aria-labelledby={heading('site')}>
            <h2 id={heading('site')}>Site</h2>
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

        <section aria-labelledby={heading('view')}>
          <h2 id={heading('view')}>View</h2>
          <dl>
            <dt>Mode</dt>
            <dd data-testid="view-mode">{switching ? 'Switching…' : mode.toUpperCase()}</dd>
          </dl>
          {mode === '2d' && !switching && (
            <p className="muted">2D is a flat map: heights are not shown as geometry.</p>
          )}
          {problems.length > 0 && (
            <>
              <ul className="problem" role="alert" aria-label="Renderer problems">
                {problems.map((p, i) => (
                  <li key={i}>{p}</li>
                ))}
              </ul>
              <div className="actions">
                <button type="button" onClick={onRestartMap}>
                  Restart map
                </button>
              </div>
              <p className="muted">Restarting redraws the map from the session; nothing is lost.</p>
            </>
          )}
        </section>

        <section aria-labelledby={heading('legend')}>
          <h2 id={heading('legend')}>Legend</h2>
          <ul className="legend">
            <li>
              <span className="swatch swatch-track" aria-hidden="true" /> Track: detected object,
              labelled with class and ID. Colour shows class.
            </li>
            <li>
              <span className="swatch swatch-ring" aria-hidden="true" /> Ring: 1σ position
              uncertainty
            </li>
            <li>
              <span className="swatch swatch-stale" aria-hidden="true" /> Faded, with “· stale” or
              “· expired” in the label: no recent observation
            </li>
            <li>
              <span className="swatch swatch-marker" aria-hidden="true" /> Pin: operator marker
              (annotation, not sensor data)
            </li>
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
            Keys: Space plays or pauses, . steps, L returns to live, M adds a marker, Escape cancels
            or clears the selection; 2 / 3 switch view, H fits the site, N faces north. Drag to pan,
            scroll or pinch to zoom; in 3D, right-drag or two-finger drag to tilt.
          </p>
        </section>
      </div>
    </aside>
  );
}
