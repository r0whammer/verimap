// Display formatting only; values arrive validated from the core.
import type { U64String } from './generated/protocol.ts';

/** Integer millimetres as metres, e.g. `-20 m`, `0.25 m`. */
export function metres(mm: number): string {
  return `${String(mm / 1000)} m`;
}

/** Integer millimetres as metres with one decimal, e.g. `-12.3 m`. */
export function metres1(mm: number): string {
  return `${(mm / 1000).toFixed(1)} m`;
}

/** Degrees with 6 decimals (about 0.1 m), e.g. `-98.350000°`. */
export function degrees(deg: number): string {
  return `${deg.toFixed(6)}°`;
}

/** Basis points as a percentage, e.g. `87.5 %`. */
export function percent(basisPoints: number): string {
  return `${(basisPoints / 100).toFixed(1)} %`;
}

/** A scene tick count as elapsed scene time, `m:ss.t`, using exact integer
 * arithmetic (ticks are u64 strings and may exceed 2^53). */
export function sceneTime(tick: U64String, tickMs: number): string {
  const tenths = (BigInt(tick) * BigInt(tickMs)) / 100n;
  const minutes = tenths / 600n;
  const seconds = (tenths % 600n) / 10n;
  const t = tenths % 10n;
  return `${minutes.toString()}:${seconds.toString().padStart(2, '0')}.${t.toString()}`;
}

/** A u64 string as a Number for UI controls (sliders), which need one.
 * Values above 2^53 cannot be represented, so they are refused. */
export function smallNumber(value: U64String): number {
  const n = Number(value);
  if (!Number.isSafeInteger(n)) throw new RangeError(`${value} is too large for this control`);
  return n;
}
