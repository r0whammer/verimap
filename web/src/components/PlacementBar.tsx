import { useId } from 'react';
import { metres1 } from '../format.ts';

/** An uncommitted marker placement or move. */
export type Placing =
  | {
      readonly kind: 'add';
      readonly point: { x_m: number; y_m: number } | null;
      readonly label: string;
      readonly missed: boolean;
    }
  | {
      readonly kind: 'move';
      readonly id: string;
      readonly label: string;
      readonly point: { x_m: number; y_m: number } | null;
      readonly missed: boolean;
    };

export interface PlacementBarProps {
  placing: Placing;
  busy: boolean;
  onLabel: (label: string) => void;
  onConfirm: () => void;
  onCancel: () => void;
}

/** Instructions and explicit confirm/cancel while placing a marker. Taps on
 * the map choose the point; nothing is committed until Confirm. */
export function PlacementBar({ placing, busy, onLabel, onConfirm, onCancel }: PlacementBarProps) {
  const labelId = useId();
  const { point } = placing;
  const title = placing.kind === 'add' ? 'Add marker' : `Move “${placing.label}”`;
  return (
    <section className="placement" aria-label={title}>
      <p>
        <strong>{title}:</strong>{' '}
        {point
          ? `${metres1(point.x_m * 1000)} east, ${metres1(point.y_m * 1000)} north, on the ground plane (height 0 m). Tap again to adjust.`
          : 'tap or click the map to choose a point on the ground plane (height 0 m).'}
      </p>
      {placing.missed && (
        <p className="problem" role="alert">
          That point is not on the ground plane. Tap the ground inside the site.
        </p>
      )}
      <div className="placement-row">
        {placing.kind === 'add' && (
          <>
            <label htmlFor={labelId} className="visually-hidden">
              Marker label
            </label>
            <input
              id={labelId}
              type="text"
              value={placing.label}
              maxLength={128}
              placeholder="Marker label"
              onChange={(e) => {
                onLabel(e.target.value);
              }}
            />
          </>
        )}
        <button
          type="button"
          className="primary"
          disabled={busy || point === null || placing.label.trim() === ''}
          onClick={onConfirm}
        >
          Confirm
        </button>
        <button type="button" onClick={onCancel} title="Cancel (Escape)">
          Cancel
        </button>
      </div>
    </section>
  );
}
