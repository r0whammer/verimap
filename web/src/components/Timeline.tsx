import { sceneTime } from '../format.ts';
import type { TimelineDto, U64String } from '../generated/protocol.ts';
import { SPEEDS, type Speed } from '../session/useSession.ts';

export interface TimelineProps {
  timeline: TimelineDto | null;
  tickMs: number;
  playing: boolean;
  speed: Speed;
  onPlay: () => void;
  onPause: () => void;
  onStep: () => void;
  onSpeed: (speed: Speed) => void;
  onScrub: (tick: U64String) => void;
  onLive: () => void;
}

/** A tick as a slider position, or null beyond what a slider can hold. */
function sliderValue(tick: U64String): number | null {
  const n = Number(tick);
  return Number.isSafeInteger(n) ? n : null;
}

/** Play/pause, step, speed, scrubber, and return-to-live. Reviewing history
 * never changes the live session; the core refuses edits while reviewing. */
export function Timeline({
  timeline,
  tickMs,
  playing,
  speed,
  onPlay,
  onPause,
  onStep,
  onSpeed,
  onScrub,
  onLive,
}: TimelineProps) {
  const disabled = timeline === null;
  const reviewing = timeline?.mode === 'review';
  const atLive = timeline !== null && timeline.tick === timeline.live_tick;
  const value = timeline ? sliderValue(timeline.tick) : null;
  const max = timeline ? sliderValue(timeline.live_tick) : null;

  return (
    <div className="timeline" role="group" aria-label="Timeline">
      <div className="timeline-row">
        <button
          type="button"
          disabled={disabled || (reviewing && atLive)}
          onClick={playing ? onPause : onPlay}
          title={playing ? 'Pause (Space)' : 'Play (Space)'}
        >
          {playing ? 'Pause' : 'Play'}
        </button>
        <button
          type="button"
          disabled={disabled || playing || (reviewing && atLive)}
          onClick={onStep}
          title="Forward one tick (.)"
        >
          Step
        </button>
        <label className="speed">
          <span className="visually-hidden">Playback speed</span>
          <select
            value={speed}
            disabled={disabled}
            onChange={(e) => {
              const s = SPEEDS.find((v) => String(v) === e.target.value);
              if (s !== undefined) onSpeed(s);
            }}
          >
            {SPEEDS.map((s) => (
              <option key={s} value={s}>
                {s}×
              </option>
            ))}
          </select>
        </label>
        <span className={`mode-badge mode-${timeline?.mode ?? 'live'}`} data-testid="timeline-mode">
          {reviewing ? 'Reviewing history' : 'Live'}
        </span>
        {reviewing && (
          <button type="button" onClick={onLive} title="Return to the live session (L)">
            Return to live
          </button>
        )}
      </div>
      <div className="timeline-row">
        <input
          type="range"
          className="scrubber"
          aria-label="Scene time"
          aria-valuetext={
            timeline ? `${sceneTime(timeline.tick, tickMs)}, tick ${timeline.tick}` : ''
          }
          min={0}
          max={max ?? 0}
          step={1}
          value={value ?? 0}
          disabled={disabled || value === null || max === null || max === 0}
          onChange={(e) => {
            onScrub(e.target.value);
          }}
        />
        <output className="time" data-testid="scene-time" data-tick={timeline?.tick}>
          {timeline ? sceneTime(timeline.tick, tickMs) : '–'}
          {reviewing && <span className="muted"> / {sceneTime(timeline.live_tick, tickMs)}</span>}
        </output>
      </div>
      {reviewing && (
        <p className="muted timeline-note">
          Read-only replay of the journal. The live session is paused until you return to live.
        </p>
      )}
    </div>
  );
}
