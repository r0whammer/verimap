// Locally drawn map symbols (no icon fonts or remote images). Shapes differ
// as well as colours, so tracks and markers are distinguishable without
// colour vision.

/** A map-pin billboard image: a teardrop with a hole, tip at the bottom. */
export function markerPinImage(fill: string): HTMLCanvasElement {
  const w = 28;
  const h = 40;
  const canvas = document.createElement('canvas');
  canvas.width = w;
  canvas.height = h;
  const ctx = canvas.getContext('2d');
  if (!ctx) throw new Error('2D canvas unavailable for the marker icon');
  const r = 11;
  const cx = w / 2;
  const cy = r + 3;
  ctx.beginPath();
  ctx.moveTo(cx, h - 2);
  ctx.bezierCurveTo(cx - 4, h - 12, cx - r, cy + 8, cx - r, cy);
  ctx.arc(cx, cy, r, Math.PI, 0);
  ctx.bezierCurveTo(cx + r, cy + 8, cx + 4, h - 12, cx, h - 2);
  ctx.closePath();
  ctx.fillStyle = fill;
  ctx.strokeStyle = '#0d1117';
  ctx.lineWidth = 2.5;
  ctx.fill();
  ctx.stroke();
  ctx.beginPath();
  ctx.arc(cx, cy, 4, 0, Math.PI * 2);
  ctx.fillStyle = '#0d1117';
  ctx.fill();
  return canvas;
}
