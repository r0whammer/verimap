// The synthetic reference ground drawn for a site: a metric grid aligned to
// the scene frame, generated locally so the default scene needs no imagery
// service. This is the "reference" layer, not sensor-derived data.
import type { SceneBoundsDto } from '../generated/protocol.ts';

/** Colour of the synthetic ground, inside and outside the site. */
export const GROUND_COLOR = '#3a4a3e';

/** A 1 × 1 PNG data URL of `color`, for a globe-covering base layer. */
export function solidColorImage(color: string): string {
  const canvas = document.createElement('canvas');
  canvas.width = 1;
  canvas.height = 1;
  const ctx = canvas.getContext('2d');
  if (!ctx) throw new Error('2D canvas unavailable for the base layer');
  ctx.fillStyle = color;
  ctx.fillRect(0, 0, 1, 1);
  return canvas.toDataURL('image/png');
}

/** Grid spacing, and the spacing of the heavier lines, in millimetres. */
const MINOR_MM = 10_000;
const MAJOR_MM = 50_000;
const MAX_PIXELS = 1024;

/**
 * Renders the ground for `bounds` (x east, y north) as a PNG data URL. Image
 * x runs east and image y runs south, matching an imagery rectangle. Lines
 * fall on multiples of the spacing in scene coordinates, so the anchor (0, 0)
 * lies on a major line crossing.
 */
export function siteGroundImage(bounds: SceneBoundsDto): string {
  const { x, y } = bounds;
  const widthMm = x.max_mm - x.min_mm;
  const heightMm = y.max_mm - y.min_mm;
  const scale = MAX_PIXELS / Math.max(widthMm, heightMm);
  const canvas = document.createElement('canvas');
  canvas.width = Math.max(1, Math.round(widthMm * scale));
  canvas.height = Math.max(1, Math.round(heightMm * scale));
  const ctx = canvas.getContext('2d');
  if (!ctx) throw new Error('2D canvas unavailable for the site ground');

  ctx.fillStyle = GROUND_COLOR;
  ctx.fillRect(0, 0, canvas.width, canvas.height);

  const px = (eMm: number) => (eMm - x.min_mm) * scale;
  const py = (nMm: number) => (y.max_mm - nMm) * scale;
  const lines = (from: number, to: number, step: number) => {
    const out: number[] = [];
    for (let v = Math.ceil(from / step) * step; v <= to; v += step) out.push(v);
    return out;
  };
  const stroke = (style: string, width: number, path: (c: CanvasRenderingContext2D) => void) => {
    ctx.strokeStyle = style;
    ctx.lineWidth = width;
    ctx.beginPath();
    path(ctx);
    ctx.stroke();
  };

  for (const [step, style, width] of [
    [MINOR_MM, 'rgba(255,255,255,0.16)', 1],
    [MAJOR_MM, 'rgba(255,255,255,0.38)', 2],
  ] as const) {
    stroke(style, width, (c) => {
      for (const e of lines(x.min_mm, x.max_mm, step)) {
        c.moveTo(px(e), 0);
        c.lineTo(px(e), canvas.height);
      }
      for (const n of lines(y.min_mm, y.max_mm, step)) {
        c.moveTo(0, py(n));
        c.lineTo(canvas.width, py(n));
      }
    });
  }

  // Site boundary.
  const border = 6;
  ctx.strokeStyle = '#e8c547';
  ctx.lineWidth = border;
  ctx.strokeRect(border / 2, border / 2, canvas.width - border, canvas.height - border);

  return canvas.toDataURL('image/png');
}
