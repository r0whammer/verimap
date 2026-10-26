import { useId, useState } from 'react';
import type { AnnotationDto, EntityDto } from '../generated/protocol.ts';
import { CLASS_LABEL, type Pick } from '../map/CesiumMap.ts';
import { FRESHNESS_LABEL } from './labels.ts';

export interface ObjectListProps {
  entities: ReadonlyMap<string, EntityDto>;
  annotations: ReadonlyMap<string, AnnotationDto>;
  selection: Pick | null;
  onSelect: (pick: Pick) => void;
}

/** Most rows rendered per list; the search narrows the rest. */
const MAX_ROWS = 200;

const byId = new Intl.Collator('en', { numeric: true });

/** A searchable alternative to picking small symbols on the map. */
export function ObjectList({ entities, annotations, selection, onSelect }: ObjectListProps) {
  const [query, setQuery] = useState('');
  const searchId = useId();
  const q = query.trim().toLowerCase();
  const tracks = [...entities.values()]
    .filter((e) => !q || `${e.id} ${CLASS_LABEL[e.class]} ${e.freshness}`.toLowerCase().includes(q))
    .sort((a, b) => byId.compare(a.id, b.id));
  const markers = [...annotations.values()]
    .filter((a) => !q || `${a.id} ${a.label}`.toLowerCase().includes(q))
    .sort((a, b) => byId.compare(a.id, b.id));

  const isSelected = (kind: Pick['kind'], id: string) =>
    selection?.kind === kind && selection.id === id;

  return (
    <>
      <label htmlFor={searchId} className="visually-hidden">
        Search tracks and markers
      </label>
      <input
        id={searchId}
        type="search"
        className="search"
        placeholder="Search tracks and markers"
        value={query}
        onChange={(e) => {
          setQuery(e.target.value);
        }}
      />
      <h3>
        Tracks <span className="muted">({tracks.length})</span>
      </h3>
      {tracks.length === 0 ? (
        <p className="muted">
          {entities.size === 0
            ? 'No tracks in this view. Press Play to run the simulated detector.'
            : 'No matching tracks.'}
        </p>
      ) : (
        <ul className="object-list" aria-label="Tracks">
          {tracks.slice(0, MAX_ROWS).map((e) => (
            <li key={e.id}>
              <button
                type="button"
                aria-pressed={isSelected('entity', e.id)}
                data-freshness={e.freshness}
                onClick={() => {
                  onSelect({ kind: 'entity', id: e.id });
                }}
              >
                <span>
                  {CLASS_LABEL[e.class]} <span className="mono">{e.id}</span>
                </span>
                <span className="row-status">{FRESHNESS_LABEL[e.freshness]}</span>
              </button>
            </li>
          ))}
        </ul>
      )}
      {tracks.length > MAX_ROWS && (
        <p className="muted">{tracks.length - MAX_ROWS} more; refine the search.</p>
      )}
      <h3>
        Markers <span className="muted">({markers.length})</span>
      </h3>
      {markers.length === 0 ? (
        <p className="muted">
          {annotations.size === 0
            ? 'No markers. Use Add marker to place one.'
            : 'No matching markers.'}
        </p>
      ) : (
        <ul className="object-list" aria-label="Markers">
          {markers.slice(0, MAX_ROWS).map((a) => (
            <li key={a.id}>
              <button
                type="button"
                aria-pressed={isSelected('annotation', a.id)}
                onClick={() => {
                  onSelect({ kind: 'annotation', id: a.id });
                }}
              >
                <span>{a.label}</span>
                <span className="row-status mono">{a.id}</span>
              </button>
            </li>
          ))}
        </ul>
      )}
      {markers.length > MAX_ROWS && (
        <p className="muted">{markers.length - MAX_ROWS} more; refine the search.</p>
      )}
    </>
  );
}
