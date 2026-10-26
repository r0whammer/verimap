# Implementation status

What exists and has been checked. CLAUDE.md is the specification; this file
is the only place that claims progress. Updated 2026-10-25.

## M0 — Foundation (in progress)

| Item | Status | Evidence |
| --- | --- | --- |
| Pinned Nix dev shell (Rust nightly + WASM target, Charon, Aeneas, Lean via elan, Node, pnpm, Playwright browsers) | done | `flake.nix`, `flake.lock`, ADR 0001 |
| Cargo workspace, safe-Rust lints | done | `Cargo.toml`, `clippy.toml` |
| Kernel guards (confidence, coordinate range, axis bounds) | done, tested | `crates/verimap-kernel`; boundary + property tests |
| Extract kernel and check Lean theorems | done | `just check-generated`, `just audit`; docs/verification.md rows 1a–1d |
| WASM worker round trip with protocol negotiation | not started | |
| Synthetic Cesium scene, 2D/3D switch | not started | |
| CI workflow | not started | |

## Not started

M1 vertical slice, M2 map changes, M3 resilient demo, M4 gateway, M5 real data.
