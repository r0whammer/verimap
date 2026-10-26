import { expect, test } from '@playwright/test';
import { PROTOCOL_VERSION } from '../src/generated/protocol.ts';
import { watch } from './support.ts';

test('core handshake reports the synthetic site', async ({ page }) => {
  const seen = await watch(page);
  await page.goto('/');
  await expect(page.getByTestId('core-status')).toHaveText('Ready', { timeout: 20_000 });
  const v = `${PROTOCOL_VERSION.major}.${PROTOCOL_VERSION.minor}`;
  await expect(page.getByTestId('protocol')).toHaveText(`UI ${v} · core ${v}`);
  await expect(page.getByTestId('site-name')).toHaveText('Synthetic Site Alpha');
  await expect(page.getByRole('region', { name: 'Site' })).toContainText(
    '200 m east × 200 m north',
  );
  expect(seen.external).toEqual([]);
  expect(seen.errors).toEqual([]);
});

test('switches between 3D and 2D and back', async ({ page }) => {
  const seen = await watch(page);
  await page.goto('/');
  await expect(page.getByTestId('core-status')).toHaveText('Ready', { timeout: 20_000 });
  const view = page.getByTestId('view-mode');
  const button = (name: string) => page.getByRole('button', { name, exact: true });
  await expect(view).toHaveText('3D');
  await expect(button('3D')).toHaveAttribute('aria-pressed', 'true');

  await button('2D').click();
  await expect(view).toHaveText('2D');
  await expect(button('2D')).toHaveAttribute('aria-pressed', 'true');
  await expect(button('North up')).toBeDisabled();

  // Keyboard equivalent.
  await page.keyboard.press('3');
  await expect(view).toHaveText('3D');
  await expect(button('North up')).toBeEnabled();
  await button('North up').click();
  await button('Home').click();

  expect(seen.external).toEqual([]);
  expect(seen.errors).toEqual([]);
});

test('controls meet the 44 px touch target', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByTestId('core-status')).toHaveText('Ready', { timeout: 20_000 });
  for (const button of await page.getByRole('button').all()) {
    if (!(await button.isVisible())) continue;
    const box = await button.boundingBox();
    expect(box, await button.innerText()).not.toBeNull();
    expect(box?.width ?? 0).toBeGreaterThanOrEqual(44);
    expect(box?.height ?? 0).toBeGreaterThanOrEqual(44);
  }
});

test('narrow screens show details as a collapsible bottom sheet', async ({ page }, info) => {
  test.skip(info.project.name !== 'mobile-chromium', 'layout for narrow screens');
  await page.goto('/');
  await expect(page.getByTestId('core-status')).toHaveText('Ready', { timeout: 20_000 });
  const panel = page.getByRole('complementary', { name: 'Scene details' });
  const viewport = page.viewportSize();
  const box = await panel.boundingBox();
  expect(viewport).not.toBeNull();
  // The sheet sits at the bottom and leaves most of the screen to the map.
  expect((box?.y ?? 0) + (box?.height ?? 0)).toBeCloseTo(viewport?.height ?? 0, 0);
  expect(box?.height ?? Infinity).toBeLessThanOrEqual((viewport?.height ?? 0) * 0.46);

  const toggle = page.getByRole('button', { name: 'Hide details' });
  await toggle.tap();
  await expect(page.getByRole('button', { name: 'Show details' })).toHaveAttribute(
    'aria-expanded',
    'false',
  );
  await expect(page.getByTestId('site-name')).toBeHidden();
});
