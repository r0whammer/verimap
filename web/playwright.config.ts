import { defineConfig, devices } from '@playwright/test';

// Browsers come from Nix (PLAYWRIGHT_BROWSERS_PATH); @playwright/test must
// match its version exactly (just toolchain-check). Runs against the
// production build served by `vite preview` on loopback.
const port = 4173;

export default defineConfig({
  testDir: 'e2e',
  fullyParallel: true,
  forbidOnly: Boolean(process.env['CI']),
  retries: 0,
  reporter: process.env['CI'] ? [['list'], ['html', { open: 'never' }]] : 'list',
  use: {
    baseURL: `http://127.0.0.1:${port}`,
    trace: 'retain-on-failure',
    launchOptions: {
      // Headless runners have no GPU; render WebGL in software.
      args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'],
    },
  },
  projects: [
    { name: 'desktop-chromium', use: { ...devices['Desktop Chrome'] } },
    // Emulation only: it checks layout and touch input wiring, not real
    // mobile browsers or GPUs (see docs/status.md for device checks).
    { name: 'mobile-chromium', use: { ...devices['Pixel 7'] } },
  ],
  webServer: {
    command: `pnpm exec vite preview --host 127.0.0.1 --port ${port} --strictPort`,
    url: `http://127.0.0.1:${port}`,
    reuseExistingServer: !process.env['CI'],
    timeout: 30_000,
  },
});
