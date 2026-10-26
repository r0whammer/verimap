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

## M1 — Vertical slice (done locally; CI not yet run)

One simulated vehicle goes through the whole path: synthetic observation →
normalization and tracking → command → journal → Rust scene state → Cesium.

| Item | Status | Evidence |
| --- | --- | --- |
| Kernel revision rules: single-slot upsert/delete decisions, base-revision precondition, checked sequence step | done, proof checked | `crates/verimap-kernel`, `tests/slots.rs`; docs/verification.md rows 2–5, 7, 8 |
| Committed events, scene store (validate the whole event, then apply; every object write through the kernel rules), tombstones, scene clock and freshness | done, tested | `crates/verimap-domain` (`store.rs`, `event.rs`, `entity.rs`) |
| Authority: command validation, strictly increasing sequences, bounded journal (200,000 events), operator markers with revision preconditions | done, tested | `authority.rs`; `tests/scene.rs` |
| Full replay and canonical state hash (fixed-order encoding, FNV-1a) | done, tested | `canonical.rs`; property test: random journals replay to the same state |
| Observation envelopes, source deduplication, late-observation handling, baseline tracker | done, tested | `observation.rs`, `tracker.rs` |
| Seeded simulator: integer-only truth path with an occluder; detector with dropout, noise, and delivery delay | done, tested | `crates/verimap-sim`; journal is independent of how ticks are batched |
| Native replay fixture | done | `fixtures/replay/demo-seed-1.json` (state hashes at ticks 100, 300, 600, 1600) |
| Protocol 0.2: scene deltas, edits, review, state hash, `u64` as decimal strings | done, tested | docs/protocol.md; protocol and WASM session tests |
| Web: play/pause, step, speed, scrubber, return to live | done, e2e tested | `web/src/session/`, `Timeline.tsx` |
| Web: select a track from a searchable list or by clicking/tapping the map; details with confidence, 1σ uncertainty, freshness, provenance, history | done, e2e tested | `ObjectList.tsx`, `SelectionDetails.tsx`, `CesiumMap.ts` |
| Web: add, rename, move, delete a marker; placement shows a preview and needs Confirm; height is labelled as the ground plane | done, e2e tested | `PlacementBar.tsx`, `App.tsx` |
| Review is read-only and leaves the live session unchanged | done, e2e tested | live state hash is equal before and after a review |
| Native/WASM canonical replay equivalence | done for one checkpoint | e2e: the browser's replay to tick 300 gives the native fixture hash `445c9821cc99a621` (event 429) |

### What `just verify` checked

On 2026-10-25, on Linux 6.18 under WSL2, the whole suite passed:

- toolchain pairs, formatting, and lint (clippy native and wasm32, `tsc`,
  ESLint);
- 89 Rust tests, among them property tests and the fixture comparisons;
- 16 Vitest tests;
- the generated model, schema, and TypeScript types are up to date;
- `lake build` and the audit of 92 theorems, auxiliary lemmas included;
- the e2e suite. 13 Playwright tests passed and 1 was skipped by design
  (the bottom-sheet test runs only at mobile width). They ran on desktop
  Chromium and on Pixel 7 emulation, which uses touch taps for map picking
  and placement.

The M1 e2e tests check:

- playing at 10× reveals `trk:1`;
- selecting it from the list and by a map pick;
- the selection survives a 2D switch;
- a marker is placed (Confirm stays disabled until a point is picked), then
  renamed and moved, a move is cancelled, and the marker is deleted;
- review to tick 300 matches the native hash, edits are disabled, playback
  stops at the end of the journal, and returning to live gives the same
  live hash.

Every test also checks that there were no console errors and no requests
off the local server.

### Known limitations (M1)

- **Baseline tracker only.** It uses nearest-neighbour gating by distance,
  class, and time. Nothing stops two detections in one frame from matching
  the same track, so crossing objects can merge (a test records this). The
  demo has one simulated object. Track IDs are not real-world identities.
- **Review pauses the live session.** The local core runs one session.
  Review replays from the start of the journal, with no checkpoints yet;
  its cost grows with journal length and has not been measured.
- **No persistence.** The journal lives in memory, and a reload starts a
  new session (M3). When the journal reaches its bound, ingestion stops
  with a message.
- **Marker height is assumed.** Markers sit on the ground plane at z = 0.
  A tap is picked on the WGS84 ellipsoid (the anchor is at 0 m ellipsoidal
  height), then projected to the scene frame. The z = 0 plane is tangent at
  the anchor, so the two differ by up to about 2 mm at the site corners.
  There is no surface picking yet.
- **No undo.** Removing a track is permanent for that ID, and later
  detections of the object start a new track (tested in
  `crates/verimap-sim/tests/replay.rs`). Lines, polygons, and surface
  edits are not implemented (M2).
- **Untuned refreshes.** Playback sends one request per tick period and
  slows down instead of queueing when the core lags. The detail panel asks
  for history on every revision of the selected track, about 10 requests/s
  at 10×. Neither has been profiled.
- **WebGL context loss** is reported, and *Restart map* rebuilds the map
  from the session. No test simulates a context loss yet.
- The M0 limitations above still apply: no real devices, no offline cache,
  and no performance measurements.

## Not started

M2 map changes, M3 resilient demo, M4 gateway, M5 real data. Unavailable
recipes: `just demo-lan`, `just test-wasm`, `just bench`.
