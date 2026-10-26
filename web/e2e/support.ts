import { expect, type Page, type TestInfo } from '@playwright/test';

/** Records anything that would break the "no external requests" rule or
 * indicate a runtime fault. External requests are also aborted. */
export async function watch(page: Page) {
  const external: string[] = [];
  const errors: string[] = [];
  await page.context().route('**/*', (route) => {
    const url = new URL(route.request().url());
    const local =
      url.protocol === 'data:' || url.protocol === 'blob:' || url.hostname === '127.0.0.1';
    if (local) return route.continue();
    external.push(url.href);
    return route.abort();
  });
  page.on('console', (msg) => {
    if (msg.type() === 'error') errors.push(msg.text());
  });
  page.on('pageerror', (e) => errors.push(e.message));
  return { external, errors };
}

/** Opens the app and waits for the core handshake. */
export async function openReady(page: Page) {
  await page.goto('/');
  await expect(page.getByTestId('core-status')).toHaveText('Ready', { timeout: 20_000 });
}

/** Clicks (desktop) or taps (touch project) the map at an offset from its
 * centre, in CSS pixels. */
export async function pressMap(page: Page, info: TestInfo, dx = 0, dy = 0) {
  const box = await page.locator('.map canvas').boundingBox();
  expect(box).not.toBeNull();
  const x = (box?.x ?? 0) + (box?.width ?? 0) / 2 + dx;
  const y = (box?.y ?? 0) + (box?.height ?? 0) / 2 + dy;
  if (info.project.name === 'mobile-chromium') await page.touchscreen.tap(x, y);
  else await page.mouse.click(x, y);
}

/** The scene tick currently shown. */
export async function shownTick(page: Page): Promise<number> {
  return Number(await page.getByTestId('scene-time').getAttribute('data-tick'));
}
