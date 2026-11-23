#!/usr/bin/env python3
"""Fail unless every production Rust line is covered or explicitly allowed.

Production lines are ``crates/**/src/**/*.rs``, excluding ``tests/`` paths and
every line from the first ``#[cfg(test)]`` module to the end of the file.
``scripts/coverage-allowlist.txt`` lists the exceptions, one ``path:line`` per
line. An allowlist entry that is covered, or an uncovered line that is not
listed, fails the gate.
"""

from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ALLOW = ROOT / "scripts" / "coverage-allowlist.txt"


def production_files() -> dict[str, int]:
    """Map a repo-relative path to the first test-module line, or a huge number."""
    out: dict[str, int] = {}
    for path in (ROOT / "crates").glob("*/src/**/*.rs"):
        rel = path.relative_to(ROOT).as_posix()
        if "/tests/" in f"/{rel}":
            continue
        cutoff = 10**12
        for n, line in enumerate(path.read_text().splitlines(), start=1):
            if line.strip() == "#[cfg(test)]":
                cutoff = n
                break
        out[rel] = cutoff
    return out


def parse_lcov(text: str) -> dict[str, dict[int, int]]:
    files: dict[str, dict[int, int]] = {}
    current: dict[int, int] | None = None
    for raw in text.splitlines():
        if raw.startswith("SF:"):
            name = raw[3:]
            marker = "crates/"
            idx = name.find(marker)
            key = name[idx:] if idx >= 0 else name
            current = files.setdefault(key, {})
        elif raw.startswith("DA:") and current is not None:
            line_s, count_s = raw[3:].split(",", 1)
            line, count = int(line_s), int(count_s)
            current[line] = max(current.get(line, 0), count)
        elif raw == "end_of_record":
            current = None
    return files


def allowlist() -> dict[str, str]:
    entries: dict[str, str] = {}
    if not ALLOW.is_file():
        return entries
    for raw in ALLOW.read_text().splitlines():
        line = raw.split("#", 1)[0].strip()
        if not line:
            continue
        loc, _, reason = line.partition(" ")
        entries[loc] = reason.strip()
    return entries


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: coverage-gate.py <verimap.lcov>", file=sys.stderr)
        return 2
    sources = production_files()
    covered = parse_lcov(Path(sys.argv[1]).read_text())
    allowed = allowlist()

    uncovered: set[str] = set()
    total = 0
    hit = 0
    for rel, cutoff in sorted(sources.items()):
        lines = covered.get(rel, {})
        file_total = file_hit = 0
        for line, count in lines.items():
            if line >= cutoff:
                continue
            file_total += 1
            total += 1
            if count > 0:
                file_hit += 1
                hit += 1
            else:
                uncovered.add(f"{rel}:{line}")
        if file_total:
            pct = 100.0 * file_hit / file_total
            print(f"  {rel}: {file_hit}/{file_total} lines ({pct:.2f}%)")

    pct = 100.0 * hit / total if total else 0.0
    print(f"coverage: production Rust {hit}/{total} lines ({pct:.2f}%)")

    missing = sorted(uncovered - set(allowed))
    stale = sorted(set(allowed) - uncovered)
    status = 0
    if missing:
        status = 1
        print("coverage: uncovered production lines with no allowlist entry:", file=sys.stderr)
        for loc in missing:
            print(f"  {loc}", file=sys.stderr)
    if stale:
        status = 1
        print("coverage: allowlist entries that are covered; remove them:", file=sys.stderr)
        for loc in stale:
            print(f"  {loc} {allowed[loc]}", file=sys.stderr)
    if status == 0:
        if allowed:
            print(f"coverage: {len(allowed)} allowlisted line(s)")
        else:
            print("coverage: every production Rust line is covered")
    return status


if __name__ == "__main__":
    raise SystemExit(main())
