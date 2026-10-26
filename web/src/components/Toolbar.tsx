import type { ViewMode } from '../map/CesiumMap.ts';

export interface ToolbarProps {
  mode: ViewMode;
  switching: boolean;
  disabled: boolean;
  onMode: (mode: ViewMode) => void;
  onHome: () => void;
  onNorthUp: () => void;
}

/** View controls. Each is a button, so keyboard and touch work alike; App
 * also binds the shortcuts shown in each title. */
export function Toolbar({ mode, switching, disabled, onMode, onHome, onNorthUp }: ToolbarProps) {
  return (
    <div className="toolbar" role="toolbar" aria-label="Map view">
      <div className="segmented" role="group" aria-label="View mode">
        {(['2d', '3d'] as const).map((m) => (
          <button
            key={m}
            type="button"
            aria-pressed={mode === m}
            disabled={disabled || switching}
            onClick={() => {
              onMode(m);
            }}
            title={`${m.toUpperCase()} view (key ${m[0] ?? ''})`}
          >
            {m.toUpperCase()}
          </button>
        ))}
      </div>
      <button
        type="button"
        disabled={disabled || switching}
        onClick={onHome}
        title="Fit the site (key H)"
      >
        Home
      </button>
      <button
        type="button"
        disabled={disabled || switching || mode === '2d'}
        onClick={onNorthUp}
        title="Face north (key N); the 2D view is always north-up"
      >
        North up
      </button>
    </div>
  );
}
