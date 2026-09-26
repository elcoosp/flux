#!/usr/bin/env bash
# Verifies that every `.flux` file in `stdlib/` parses without error.
# Mirrors the `every_stdlib_file_parses` integration test in
# crates/flux-parser/tests/stdlib.rs — this script is the shell-callable gate
# cited by the release-rehearsal (T-605) and `flux doctor` (probe.rs).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
cargo test -p flux-parser --test stdlib -- every_stdlib_file_parses 2>&1
