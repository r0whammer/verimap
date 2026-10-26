# VeriMap task runner. Run `just` to list recipes.
# Every recipe expects the `nix develop` shell (direnv can activate it on `cd`).
# Recipes appear as their components land; README.md lists what is not yet
# available.

set shell := ["bash", "-euo", "pipefail", "-c"]

# Identifies the UI and WASM bundle in the worker handshake.
export VERIMAP_BUILD_ID := `git describe --always --dirty --abbrev=12 2>/dev/null || echo dev`

# List available recipes
default:
    @just --list

# Format Rust, Nix, and web sources in place
fmt: web-install
    cargo fmt --all
    nixfmt flake.nix
    cd web && pnpm run fmt

# Check formatting without changing files
fmt-check: web-install
    cargo fmt --all -- --check
    nixfmt --check flake.nix
    cd web && pnpm run fmt-check

# Clippy (native and wasm32) and TypeScript type checks plus ESLint
lint: wasm
    cargo clippy --workspace --all-targets --all-features -- -D warnings
    cargo clippy -p verimap-wasm --target wasm32-unknown-unknown -- -D warnings
    cd web && pnpm run typecheck && pnpm run lint

# Deterministic Rust and web unit/integration tests
test: web-install
    cargo test --workspace --all-features
    cd web && pnpm run test

# Install web dependencies exactly as locked (pnpm-lock.yaml)
web-install:
    cd web && pnpm install --frozen-lockfile --prefer-offline --reporter=silent

# Build the core for the browser: web/src/wasm/verimap_core{.js,_bg.wasm}
wasm:
    cargo build -p verimap-wasm --target wasm32-unknown-unknown --release
    wasm-bindgen --target web --out-dir web/src/wasm --out-name verimap_core \
      target/wasm32-unknown-unknown/release/verimap_wasm.wasm

# Regenerate protocol schemas (schemas/) and TypeScript types (web/src/generated/)
gen: web-install
    cargo run -q -p verimap-protocol --bin verimap-schemas -- schemas
    cd web && node scripts/gen-types.mjs ../schemas src/generated

# Local web demo on http://127.0.0.1:5173 (loopback only); rebuilds WASM on Rust changes
dev: web-install wasm
    #!/usr/bin/env bash
    set -euo pipefail
    watchexec --watch crates --exts rs,toml --postpone --debounce 500ms -- just wasm &
    watcher=$!
    trap 'kill $watcher 2>/dev/null || true' EXIT
    cd web && pnpm run dev

# Production build of the web client into web/dist
build: web-install wasm
    cd web && pnpm run build

# Browser end-to-end tests (desktop and mobile emulation) against a production build
e2e: build
    cd web && pnpm run e2e

# Check that exact-match tool pairs agree (wasm-bindgen, Playwright, Lean/Aeneas)
toolchain-check: _nix
    scripts/toolchain-check.sh

# Pin proofs/ to the Aeneas revision in flake.lock (only after an Aeneas upgrade)
init-proofs: _nix
    scripts/init-proofs.sh

# Extract verimap-kernel to LLBC with Charon (target/verimap_kernel.llbc)
extract: _nix
    charon cargo --preset=aeneas --dest-file=target/verimap_kernel.llbc -- -p verimap-kernel

# Regenerate the Lean model (proofs/VeriMap/Code) from fresh LLBC
translate: _nix
    scripts/translate.sh

# Build the Lean project, checking every proof imported by proofs/VeriMap.lean
proofs: _proofs-ready
    cd proofs && lake build

# Fetch Mathlib's prebuilt cache (first setup, or after a lake-manifest change)
proofs-cache: _proofs-ready
    cd proofs && lake exe cache get

# Lake build, then fail on unimported proof modules, placeholders, or
# theorems that depend on axioms beyond propext/Classical.choice/Quot.sound
audit: proofs
    scripts/audit.sh

# Fail if committed generated output is stale, modified, missing, or untracked
check-generated: _nix
    scripts/check-generated.sh

# All available required checks
verify: toolchain-check fmt-check lint test check-generated audit e2e

# Remove build output (keeps committed generated files and node_modules)
clean:
    cargo clean
    rm -rf web/dist web/src/wasm web/public/cesium web/test-results web/playwright-report

[private]
_nix:
    @test -n "${AENEAS_SOURCE:-}" || { echo "run inside nix develop (or allow direnv)"; exit 1; }

[private]
_proofs-ready: _nix
    @test -f proofs/lakefile.toml || { echo "proofs/ not set up; run: just init-proofs"; exit 1; }
