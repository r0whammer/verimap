#!/usr/bin/env bash
# Line coverage of production Rust, and line/branch coverage of the pure web
# modules. Run inside `nix develop` (the Rust sysroot must include llvm-tools).
#
# This toolchain's source-based coverage reports conditional coverage as
# regions. `llvm-cov report` shows a branch column of zero, so the Rust gate
# is line coverage of production sources. Web coverage uses V8 branch counts.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

sysroot="$(rustc --print sysroot)"
llvm_bin="$(find "$sysroot/lib/rustlib" -type d -name bin -print -quit)"
profdata="$llvm_bin/llvm-profdata"
cov="$llvm_bin/llvm-cov"
if [[ ! -x "$profdata" || ! -x "$cov" ]]; then
  echo "coverage: llvm-profdata/llvm-cov not in the Rust sysroot ($sysroot)." >&2
  echo "coverage: the dev shell's rustToolchain must include the llvm-tools component." >&2
  exit 1
fi

outdir="$root/target/coverage"
rm -rf "$outdir"
mkdir -p "$outdir/prof"
export CARGO_TARGET_DIR="$root/target/coverage-build"
export RUSTFLAGS="-C instrument-coverage ${RUSTFLAGS:-}"
export LLVM_PROFILE_FILE="$outdir/prof/%m_%p.profraw"

cargo test --workspace --all-features

"$profdata" merge -sparse "$outdir/prof"/*.profraw -o "$outdir/merged.profdata"

mapfile -t bins < <(
  find "$CARGO_TARGET_DIR" -type f -executable -path '*verimap*' \
    ! -name 'build-script-build' ! -name '*.so' | sort
)
if [[ ${#bins[@]} -eq 0 ]]; then
  echo "coverage: no instrumented verimap binaries under $CARGO_TARGET_DIR" >&2
  exit 1
fi
objs=()
for b in "${bins[@]}"; do
  objs+=(--object "$b")
done

"$cov" export "${objs[@]}" \
  --instr-profile="$outdir/merged.profdata" \
  --format=lcov \
  --ignore-filename-regex='(/rustc/|/cargo/registry/|/cargo/git/)' \
  > "$outdir/verimap.lcov"

python3 "$root/scripts/coverage-gate.py" "$outdir/verimap.lcov"

cd "$root/web"
pnpm exec vitest run --coverage
