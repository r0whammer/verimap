// Display formatting only; values arrive validated from the core.

/** Integer millimetres as metres, e.g. `-20 m`, `0.25 m`. */
export function metres(mm: number): string {
  return `${String(mm / 1000)} m`;
}

/** Degrees with 6 decimals (about 0.1 m), e.g. `-98.350000°`. */
export function degrees(deg: number): string {
  return `${deg.toFixed(6)}°`;
}
