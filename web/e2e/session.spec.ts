import { readFileSync } from 'node:fs';
import { expect, test, type Page } from '@playwright/test';
import { openReady, pressMap, shownTick, watch } from './support.ts';

interface Checkpoint {
  tick: number;
  last_sequence: number;
  state_hash: string;
}

// Native (Rust) state hashes of the same seeded session.
const fixture = JSON.parse(
  readFileSync(new URL('../../fixtures/replay/demo-seed-1.json', import.meta.url), 'utf8'),
) as { seed: number; checkpoints: Checkpoint[] };

const button = (page: Page, name: string) => page.getByRole('button', { name, exact: true });

async function playUntil(page: Page, tick: number) {
  await page.getByLabel('Playback speed').selectOption('10');
  await button(page, 'Play').click();
  await expect.poll(() => shownTick(page), { timeout: 60_000 }).toBeGreaterThanOrEqual(tick);
  await button(page, 'Pause').click();
}

test('the simulated detector reveals a track that can be selected from the list or the map', async ({
  page,
}, info) => {
  const seen = await watch(page);
  await openReady(page);
  await expect(page.getByRole('list', { name: 'Tracks' })).toHaveCount(0);

  await playUntil(page, 40);
  const tracks = page.getByRole('list', { name: 'Tracks' });
  const track = tracks.getByRole('button', { name: /Vehicle trk:1/ });
  await expect(track).toBeVisible();

  await track.click();
  await expect(page.getByTestId('selection-title')).toHaveText('Vehicle trk:1');
  await expect(track).toHaveAttribute('aria-pressed', 'true');
  await expect(page.getByRole('table', { name: /Observation history/ })).toBeVisible();
  await expect(
    page.getByRole('table', { name: /Observation history/ }).getByRole('row'),
  ).not.toHaveCount(1);

  // Picking on the map selects the same track.
  await button(page, 'Center on map').click();
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('selection-title')).toHaveCount(0);
  await pressMap(page, info);
  await expect(page.getByTestId('selection-title')).toHaveText('Vehicle trk:1');

  // The selection survives a 2D/3D switch.
  await button(page, '2D').click();
  await expect(page.getByTestId('view-mode')).toHaveText('2D');
  await expect(page.getByTestId('selection-title')).toHaveText('Vehicle trk:1');

  expect(seen.external).toEqual([]);
  expect(seen.errors).toEqual([]);
});

test('markers are placed, renamed, moved, and deleted with explicit confirmation', async ({
  page,
}, info) => {
  const seen = await watch(page);
  await openReady(page);

  await button(page, 'Add marker').click();
  const placement = page.getByRole('region', { name: 'Add marker' });
  await expect(placement).toContainText('tap or click the map');
  await expect(button(page, 'Confirm')).toBeDisabled();
  await pressMap(page, info, 30, 20);
  await expect(placement).toContainText('on the ground plane (height 0 m)');
  await placement.getByLabel('Marker label').fill('Gate A');
  await button(page, 'Confirm').click();
  await expect(placement).toHaveCount(0);

  const markers = page.getByRole('list', { name: 'Markers' });
  await expect(markers.getByRole('button', { name: /Gate A/ })).toBeVisible();
  const title = page.getByTestId('selection-title');
  await expect(title).toHaveText('Marker “Gate A”');

  await page.getByLabel('Label', { exact: true }).fill('Gate B');
  await button(page, 'Save label').click();
  await expect(title).toHaveText('Marker “Gate B”');
  await expect(markers.getByRole('button', { name: /Gate B/ })).toBeVisible();

  const position = page.getByRole('region', { name: 'Selected' }).getByText(/ E, .* N$/);
  const before = await position.innerText();
  await button(page, 'Move').click();
  await expect(page.getByRole('region', { name: 'Move “Gate B”' })).toBeVisible();
  // Cancel leaves the marker where it was.
  await button(page, 'Cancel').click();
  await expect(position).toHaveText(before);
  await button(page, 'Move').click();
  await pressMap(page, info, -40, -30);
  await button(page, 'Confirm').click();
  await expect(position).not.toHaveText(before);

  await button(page, 'Delete').click();
  await expect(markers).toHaveCount(0);
  await expect(page.getByText('No markers.', { exact: false })).toBeVisible();

  expect(seen.external).toEqual([]);
  expect(seen.errors).toEqual([]);
});

test('review replays the journal read-only and matches the native state hash', async ({ page }) => {
  const seen = await watch(page);
  const checkpoint = fixture.checkpoints.find((c) => c.tick === 300);
  expect(checkpoint).toBeDefined();
  await openReady(page);
  await playUntil(page, 320);

  const hash = page.getByTestId('state-hash');
  await button(page, 'Compute state hash').click();
  await expect(hash).toHaveText(/^[0-9a-f]{16}$/);
  const liveHash = await hash.innerText();
  const liveTick = await shownTick(page);

  await page.getByLabel('Scene time').fill('300');
  await expect(page.getByTestId('timeline-mode')).toHaveText('Reviewing history');
  await expect.poll(() => shownTick(page)).toBe(300);
  await button(page, 'Compute state hash').click();
  await expect(hash).toHaveText(checkpoint?.state_hash ?? '');
  await expect(
    page.getByText(`at tick 300, event ${String(checkpoint?.last_sequence)}`),
  ).toBeVisible();
  await expect(button(page, 'Add marker')).toBeDisabled();

  // Playing through history stops at the end of the journal.
  await button(page, 'Play').click();
  await expect.poll(() => shownTick(page)).toBe(liveTick);
  await expect(button(page, 'Play')).toBeDisabled();

  await button(page, 'Return to live').click();
  await expect(button(page, 'Return to live')).toHaveCount(0);
  await expect(page.getByTestId('timeline-mode')).toHaveText('Live');
  await button(page, 'Compute state hash').click();
  await expect(page.getByText(`at tick ${String(liveTick)}`)).toBeVisible();
  await expect(hash).toHaveText(liveHash);

  expect(seen.external).toEqual([]);
  expect(seen.errors).toEqual([]);
});
