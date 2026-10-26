# ADR 0001: Toolchain pins and the two nixpkgs

Date: 2026-10-25. Status: accepted.

## Context

VeriMap needs one Rust compiler for native code, `wasm32-unknown-unknown`,
and Charon extraction, plus Node/pnpm/Playwright for the web client and a
Lean toolchain that agrees with Aeneas. VeriPacket (baseline commit
`f549faa3`) pins Aeneas `74aabd52f77001cf9eaad51b4f4b867a7dc6b111`, whose lock
pins Charon `260ba2c82588291b1f9efaf4f42d7100adc9c6c2` and Rust
`nightly-2026-09-17`; its Lean project uses `leanprover/lean4:v4.35.0`.

The nixpkgs Aeneas locks (`b3d51a03`, 2025-11-02) is old for web work: it
offers wasm-bindgen-cli up to 0.2.104 and Playwright 1.54.

## Decision

- Lock `aeneas` to VeriPacket's revision. Charon, rust-overlay, and the
  Rust nightly follow from Aeneas's own lock unchanged, as does the Lean
  version (`scripts/init-proofs.sh` copies it from the Aeneas source).
  `proofs/lake-manifest.json` resolves to the same package revisions as
  VeriPacket's.
- Use Charon's exact toolchain (`aeneas.inputs.charon.packages.*.rustToolchain`)
  with `rust-analyzer` and the `wasm32-unknown-unknown` target added. It is
  the only application compiler.
- Add a root `nixpkgs` input for general and web tools, separate from
  `aeneas/nixpkgs` so a proof-toolchain upgrade does not force web tool
  changes and vice versa. It was locked at bootstrap to `643809054d65`
  (2026-08-02), the same nixpkgs commit Charon pins, so no extra nixpkgs
  generation was introduced. Versions it provides: Node 24.18.1, pnpm
  10.34.5, Playwright driver 1.61.1 (Chromium only), wasm-bindgen-cli 0.2.126,
  elan 4.2.3, just 1.57.0, nixfmt 1.4.0.
- Exact-match pairs, checked by `just toolchain-check` once the web client
  exists: `wasm-bindgen` crate ⇔ `wasm-bindgen-cli_0_2_126`;
  `@playwright/test` ⇔ `playwright-driver`.

## Consequences

- Upgrading Aeneas means: update the lock, `just init-proofs`, `lake update`
  in `proofs/`, `just translate`, replay all proofs, and re-check that the
  new Charon toolchain still builds the WASM target.
- Upgrading web tools means updating the root `nixpkgs` lock and moving the
  exact-match pairs together.
- Not fully Nix-built: elan downloads Lean, Lake clones Aeneas/Mathlib and
  downloads Mathlib's build cache, Cargo and pnpm download packages. All are
  pinned by lock files; see README.md for the first-run fetches.
