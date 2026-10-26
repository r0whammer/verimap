/// <reference types="vitest/config" />
import react from '@vitejs/plugin-react';
import { defineConfig, type Plugin } from 'vite';

// Set by the justfile from git; identifies the UI bundle in the handshake.
const uiBuild = process.env['VERIMAP_BUILD_ID'] ?? 'dev';

// Production pages may load only what they ship: no CDN, telemetry, map
// service, or other origin. 'wasm-unsafe-eval' permits compiling the core's
// WebAssembly; data:/blob: cover locally generated imagery and Cesium's
// workers. Not applied to the dev server, whose HMR client injects inline
// scripts.
const contentSecurityPolicy = [
  "default-src 'self'",
  "script-src 'self' 'wasm-unsafe-eval'",
  "worker-src 'self' blob:",
  "connect-src 'self' data: blob:",
  "img-src 'self' data: blob:",
  "style-src 'self' 'unsafe-inline'",
  "font-src 'self' data:",
  "object-src 'none'",
  "base-uri 'none'",
  "form-action 'none'",
].join('; ');

function contentSecurityPolicyMeta(): Plugin {
  return {
    name: 'verimap-csp',
    apply: 'build',
    transformIndexHtml: () => [
      {
        tag: 'meta',
        attrs: { 'http-equiv': 'Content-Security-Policy', content: contentSecurityPolicy },
        injectTo: 'head-prepend',
      },
    ],
  };
}

export default defineConfig({
  plugins: [react(), contentSecurityPolicyMeta()],
  define: {
    __VERIMAP_UI_BUILD__: JSON.stringify(uiBuild),
  },
  // Loopback only by default; LAN exposure is an explicit, documented step.
  server: { host: '127.0.0.1', port: 5173, strictPort: true },
  preview: { host: '127.0.0.1', port: 4173, strictPort: true },
  worker: { format: 'es' },
  build: {
    target: 'es2022',
    sourcemap: true,
    // Cesium is one large chunk by design; it is cached with the app shell.
    chunkSizeWarningLimit: 8000,
  },
  test: {
    include: ['src/**/*.test.ts'],
    environment: 'node',
  },
});
