#!/usr/bin/env bash
# parse-check.sh — validate every stdlib .flux file against flux-parser (FLUX-015).
#
# Builds the workspace's flux-parser (and its dependencies), then compiles
# tools/parse_check.rs against the resulting rlibs and runs it over every
# `.flux` file in this directory. Exits non-zero on the first parse failure,
# printing the parser's rendered diagnostic.
#
# Why rustc and not a Cargo crate: adding a manifest here would change the
# frozen workspace membership, which docs/agents-boundaries-contract.md R2
# forbids for every agent. This keeps all writes inside /stdlib.
#
# Usage (from anywhere):  stdlib/parse-check.sh

set -euo pipefail

stdlib_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${stdlib_dir}/.." && pwd)"

cd "${repo_root}"

# Build flux-parser and ask cargo for the exact rlib it produced, rather than
# globbing target/debug/deps (which accumulates stale hashed artifacts).
build_log="$(mktemp)"
trap 'rm -f "${build_log}"' EXIT
cargo build -p flux-parser --message-format=json >"${build_log}"

parser_rlib="$(
  python3 - "${build_log}" <<'PY'
import json
import sys

rlib = None
with open(sys.argv[1], encoding="utf-8") as log:
    for line in log:
        try:
            message = json.loads(line)
        except json.JSONDecodeError:
            continue
        if message.get("reason") != "compiler-artifact":
            continue
        name = message.get("target", {}).get("name", "")
        if name.replace("-", "_") != "flux_parser":
            continue
        for filename in message.get("filenames", []):
            if filename.endswith(".rlib"):
                rlib = filename
print(rlib or "")
PY
)"

if [ -z "${parser_rlib}" ]; then
  echo "could not locate the flux-parser rlib in cargo's build output" >&2
  exit 1
fi

# The rlib sits either directly in the profile dir or in its `deps/`
# subdirectory; the dependency search path is always `<profile>/deps`.
rlib_dir="$(dirname "${parser_rlib}")"
if [ "$(basename "${rlib_dir}")" = "deps" ]; then
  target_dir="$(dirname "${rlib_dir}")"
else
  target_dir="${rlib_dir}"
fi
out_dir="$(mktemp -d)"
trap 'rm -rf "${out_dir}"; rm -f "${build_log}"' EXIT

rustc \
  --edition 2024 \
  -o "${out_dir}/parse_check" \
  --extern "flux_parser=${parser_rlib}" \
  -L "dependency=${target_dir}/deps" \
  "${stdlib_dir}/tools/parse_check.rs"

shopt -s nullglob
flux_files=("${stdlib_dir}"/*.flux)
shopt -u nullglob

if [ ${#flux_files[@]} -eq 0 ]; then
  echo "no .flux files found in ${stdlib_dir}" >&2
  exit 1
fi

# Self-test (the RED half of the harness): a known-invalid fixture must be
# rejected. Without this, a driver that reported success unconditionally would
# look identical to a genuinely clean stdlib.
invalid_fixture="${stdlib_dir}/tools/fixtures/invalid.flux"
if "${out_dir}/parse_check" "${invalid_fixture}" >/dev/null 2>&1; then
  echo "self-test failed: ${invalid_fixture} parsed, but it must not" >&2
  exit 1
fi
echo "self-test ok: invalid fixture is rejected"

"${out_dir}/parse_check" "${flux_files[@]}"
