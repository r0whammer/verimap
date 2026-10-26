# VeriMap task runner. Run `just` to list recipes.
# Every recipe expects the `nix develop` shell (direnv can activate it on `cd`).
# Recipes appear as their components land; README.md lists what is not yet
# available.

set shell := ["bash", "-euo", "pipefail", "-c"]

# List available recipes
default:
    @just --list

# Format Rust, Nix (and web, once present) sources in place
fmt:
    cargo fmt --all
    nixfmt flake.nix

# Check formatting without changing files
fmt-check:
    cargo fmt --all -- --check
    nixfmt --check flake.nix

# Clippy with warnings as errors
lint:
    cargo clippy --workspace --all-targets --all-features -- -D warnings

# Deterministic Rust unit, property, and integration tests
test:
    cargo test --workspace --all-features

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
verify: fmt-check lint test check-generated audit

# Remove Cargo build output and LLBC (keeps committed generated files)
clean:
    cargo clean

[private]
_nix:
    @test -n "${AENEAS_SOURCE:-}" || { echo "run inside nix develop (or allow direnv)"; exit 1; }

[private]
_proofs-ready: _nix
    @test -f proofs/lakefile.toml || { echo "proofs/ not set up; run: just init-proofs"; exit 1; }
