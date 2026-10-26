import { useId, useState } from 'react';
import { metres1, percent, sceneTime } from '../format.ts';
import type {
  AnnotationDto,
  EntityDetailDto,
  EntityDto,
  PointMmDto,
  SessionDto,
  U64String,
} from '../generated/protocol.ts';
import { CLASS_LABEL } from '../map/CesiumMap.ts';
import { FRESHNESS_LABEL } from './labels.ts';

const position = (p: PointMmDto) =>
  `${metres1(p.x_mm)} E, ${metres1(p.y_mm)} N, ${metres1(p.z_mm)} up`;

function ticksAgo(now: U64String, then: U64String): string {
  const d = BigInt(now) - BigInt(then);
  return d === 0n ? 'this tick' : `${d.toString()} ticks ago`;
}

export interface EntityDetailsProps {
  entity: EntityDto;
  detail: EntityDetailDto | null;
  session: SessionDto;
  now: U64String;
  editable: boolean;
  onCenter: () => void;
  onRemove: () => void;
}

export function EntityDetails({
  entity,
  detail,
  session,
  now,
  editable,
  onCenter,
  onRemove,
}: EntityDetailsProps) {
  const tick = (t: U64String) => sceneTime(t, session.tick_ms);
  const freshnessNote = {
    active: '',
    stale: ` — not observed for over ${String((session.stale_after_ticks * session.tick_ms) / 1000)} s`,
    expired: ` — not observed for over ${String((session.expire_after_ticks * session.tick_ms) / 1000)} s`,
  }[entity.freshness];
  // Newest first.
  const history = detail ? [...detail.history].reverse() : null;
  return (
    <>
      <p className="selection-title" data-testid="selection-title">
        {CLASS_LABEL[entity.class]} <span className="mono">{entity.id}</span>
      </p>
      <dl>
        <dt>Status</dt>
        <dd data-testid="selection-freshness">
          {FRESHNESS_LABEL[entity.freshness]}
          {freshnessNote}
        </dd>
        <dt>Confidence</dt>
        <dd>{percent(entity.confidence_bp)} (detector score, not a probability of identity)</dd>
        <dt>Position</dt>
        <dd>{position(entity.position)}</dd>
        <dt>Uncertainty</dt>
        <dd>{metres1(entity.sigma_mm)} (1σ, horizontal; ring on the map)</dd>
        <dt>Last seen</dt>
        <dd>
          {tick(entity.last_observed_tick)} ({ticksAgo(now, entity.last_observed_tick)})
        </dd>
        <dt>Source</dt>
        <dd className="mono">
          {entity.source.source_id} #{entity.source.sequence}
        </dd>
        <dt>Revision</dt>
        <dd className="mono">{entity.revision}</dd>
      </dl>
      <p className="muted">
        Track ID from the baseline tracker: it follows one object between observations, not a
        verified real-world identity.
      </p>
      <div className="actions">
        <button type="button" onClick={onCenter}>
          Center on map
        </button>
        <button type="button" disabled={!editable} onClick={onRemove}>
          Remove track
        </button>
      </div>
      <h3>Observation history</h3>
      {history === null ? (
        <p className="muted">Loading…</p>
      ) : (
        <div className="table-scroll">
          <table className="history" aria-label="Observation history, newest first">
            <thead>
              <tr>
                <th scope="col">Acquired</th>
                <th scope="col">Position (E, N)</th>
                <th scope="col">1σ</th>
                <th scope="col">Conf.</th>
                <th scope="col">Obs.</th>
              </tr>
            </thead>
            <tbody>
              {history.map((h) => (
                <tr key={h.revision}>
                  <td>{tick(h.acquired_tick)}</td>
                  <td>
                    {metres1(h.position.x_mm)}, {metres1(h.position.y_mm)}
                  </td>
                  <td>{metres1(h.sigma_mm)}</td>
                  <td>{percent(h.confidence_bp)}</td>
                  <td className="mono">#{h.source.sequence}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </>
  );
}

export interface MarkerDetailsProps {
  marker: AnnotationDto;
  editable: boolean;
  busy: boolean;
  onRename: (label: string) => void;
  onCenter: () => void;
  onMove: () => void;
  onDelete: () => void;
}

export function MarkerDetails({
  marker,
  editable,
  busy,
  onRename,
  onCenter,
  onMove,
  onDelete,
}: MarkerDetailsProps) {
  // Starts from the committed label; the parent remounts this component
  // when the marker or its revision changes.
  const [label, setLabel] = useState(marker.label);
  const labelId = useId();
  const changed = label !== marker.label && label.trim() !== '';
  return (
    <>
      <p className="selection-title" data-testid="selection-title">
        Marker “{marker.label}”
      </p>
      <dl>
        <dt>Position</dt>
        <dd>
          {metres1(marker.position.x_mm)} E, {metres1(marker.position.y_mm)} N
        </dd>
        <dt>Height</dt>
        <dd>
          {metres1(marker.position.z_mm)} — placed on the ground plane, not a measured surface
        </dd>
        <dt>Source</dt>
        <dd>Operator annotation</dd>
        <dt>ID</dt>
        <dd className="mono">{marker.id}</dd>
        <dt>Revision</dt>
        <dd className="mono">{marker.revision}</dd>
      </dl>
      <form
        className="rename"
        onSubmit={(e) => {
          e.preventDefault();
          if (changed) onRename(label);
        }}
      >
        <label htmlFor={labelId}>Label</label>
        <input
          id={labelId}
          type="text"
          value={label}
          maxLength={128}
          disabled={!editable}
          onChange={(e) => {
            setLabel(e.target.value);
          }}
        />
        <button type="submit" disabled={!editable || busy || !changed}>
          Save label
        </button>
      </form>
      <div className="actions">
        <button type="button" onClick={onCenter}>
          Center on map
        </button>
        <button type="button" disabled={!editable || busy} onClick={onMove}>
          Move
        </button>
        <button type="button" className="danger" disabled={!editable || busy} onClick={onDelete}>
          Delete
        </button>
      </div>
    </>
  );
}
