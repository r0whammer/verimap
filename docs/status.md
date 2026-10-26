# Implementation status

What exists and has been checked. CLAUDE.md is the specification; this file
is the only place that claims progress. Updated 2026-10-25.

## M0 — Foundation (done locally; CI not yet run)

| Item | Status | Evidence |
| --- | --- | --- |
| Pinned Nix dev shell (Rust nightly + WASM target, Charon, Aeneas, Lean via elan, Node, pnpm, Playwright browsers) | done | `flake.nix`, `flake.lock`, ADR 0001, `just toolchain-check` |
| Cargo workspace, safe-Rust lints | done | `Cargo.toml`, `clippy.toml`; clippy clean on native and wasm32 |
| Kernel guards (confidence, coordinate range, axis bounds) | done, tested | `crates/verimap-kernel`; boundary + property tests |
| Extract kernel and check Lean theorems | done | `just check-generated`, `just audit`; docs/verification.md rows 1a–1d |
| Domain site frame (anchor, scene bounds) using kernel guards | done, tested | `crates/verimap-domain` |
| Versioned worker protocol, bounded parsing, generated schema + TS types | done, tested | `crates/verimap-protocol`, docs/protocol.md, `just check-generated` |
| WASM worker round trip with protocol negotiation | done, tested | `crates/verimap-wasm`, `web/src/worker/`; Rust session tests, Vitest bridge tests, e2e handshake |
| Synthetic Cesium scene (local ground, bounds box, anchor), 2D/3D switch | done, tested | `web/src/map/`; e2e switch test |
| CI workflow | written, **not yet run** | `.github/workflows/verify.yml` (passes `actionlint`); runs the same recipes as `just verify` |

### What `just verify` checked

On 2026-10-25, on Linux 6.18 under WSL2, the whole suite passed:
toolchain pairs, formatting, lint, tests, generated output, `lake build`
plus the audit of 44 theorems, and e2e. Playwright 1.61.1 ran the Nix
Chromium with SwiftShader WebGL: 7 tests passed and 1 was skipped by design
(the bottom-sheet test runs only at mobile width). The e2e tests check:

- the handshake reaches *Ready* with matching protocol versions, the site
  name and size are shown, and there are no console errors and no requests
  to anything except the local server or `data:`/`blob:` URLs;
- 3D → 2D → 3D through the button and the keyboard shortcut;
- every visible button is at least 44 × 44 CSS px, at desktop size and with
  Pixel 7 emulation;
- the narrow-screen details panel is a bottom sheet that can collapse.

The dev server (`just dev`, with React StrictMode) also loads with no
console errors or warnings.

### Known limitations

- **No real devices tested.** Mobile coverage is Chromium's Pixel 7
  emulation, which checks layout and touch wiring only. Firefox, WebKit,
  Android, and iOS have not been run.
- No service worker, manifest, or offline cache yet (M3). The CSP meta tag
  is added only to production builds.
- Build IDs are exchanged and shown but not compared; the protocol version
  alone decides compatibility (docs/protocol.md).
- The main bundle is 4.1 MB (1.1 MB gzip), almost all of it Cesium, with no
  code splitting. This has not been measured or optimized.
- The Cesium credit area shows Cesium's bundled logo image. It is a local
  asset; no ion request is made.

## Not started

M1 vertical slice, M2 map changes, M3 resilient demo, M4 gateway, M5 real
data. Unavailable recipes: `just demo-lan`, `just test-wasm`, `just bench`.
