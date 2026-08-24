#!/usr/bin/env bash
# check-adr-numbering.sh — enforce ADR filename discipline (ADR-0025).
#
# Failure mode prevented: an agent-authored ADR file named `ADR-NNNN-*.md` reuses a
# number already claimed by the canonical decision sequence embedded as
# `### ADR-NNNN:` headings in `mlp-appendices.md` Appendix A. Two documents then share
# one ADR number and a bare `grep ADR-00NN` becomes ambiguous.
#
# Rules enforced:
#   1. (HARD FAIL) No file in docs/adr/ may use the `ADR-NNNN-` prefix with a number
#      that appears in the appendices' `### ADR-NNNN:` sequence (ADR-0001 … ADR-0020).
#      The continuation block (Appendix A — Continuation) deliberately uses `#### ADR-NNNN —`
#      (dash) headings so it is NOT parsed as reserved.
#   2. (ADVISORY)  Files not matching the `<scope>-<slug>.md` convention are listed so
#      the renumbered VM-errata files (ADR-0021–0024) stay visible but do not fail.
#
# Usage: docs/scripts/check-adr-numbering.sh [repo-root]
# Exit 0 = clean; exit 1 = collision (CI must fail).

set -euo pipefail

ROOT="${1:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
APPENDICES="$ROOT/docs/spec/mlp-appendices.md"
ADR_DIR="$ROOT/docs/adr"

if [[ ! -f "$APPENDICES" ]]; then
  echo "error: cannot find $APPENDICES" >&2
  exit 1
fi
if [[ ! -d "$ADR_DIR" ]]; then
  echo "error: cannot find $ADR_DIR" >&2
  exit 1
fi

# Reserved numbers: every NNNN used by a `### ADR-NNNN:` heading in the appendices.
RESERVED=()
while IFS= read -r n; do
  RESERVED+=("$n")
done < <(grep -oE '^### ADR-[0-9]{4}:' "$APPENDICES" | grep -oE '[0-9]{4}' | sort -u)

is_reserved() {
  local n="$1" r
  for r in "${RESERVED[@]:-}"; do
    [[ "$r" == "$n" ]] && return 0
  done
  return 1
}

declare -a COLLISIONS=()
declare -a NONCONFORMING=()

# Renumbered VM-errata files (ADR-0021–0024). They no longer collide with the
# reserved canonical sequence (ADR-0001–0020), so they pass Rule 1 naturally. They
# are listed here so Rule 2's advisory still names them as known non-<scope>-<slug>
# files. Documented in docs/adr/ADR-0025-adr-naming-and-numbering.md.
EXCEPTIONS=(
  "ADR-0021-gas-accounting.md"
  "ADR-0022-byte-length-erratum.md"
  "ADR-0023-div-by-zero-error.md"
  "ADR-0024-getfield-null.md"
)
is_exception() {
  local b="$1" e
  for e in "${EXCEPTIONS[@]:-}"; do
    [[ "$e" == "$b" ]] && return 0
  done
  return 1
}

shopt -s nullglob
for f in "$ADR_DIR"/*.md; do
  base="$(basename "$f")"
  # Known exceptions are recorded; they do not fail the build.
  if is_exception "$base"; then
    continue
  fi
  # Rule 1: reserved-number collision.
  if [[ "$base" =~ ^ADR-([0-9]{4})- ]]; then
    num="${BASH_REMATCH[1]}"
    if is_reserved "$num"; then
      COLLISIONS+=("$base (collides with canonical ADR-$num in mlp-appendices.md)")
    fi
  fi
  # Rule 2: convention advisory (scope-slug, lowercase, digits/hyphens only).
  if [[ ! "$base" =~ ^[a-z0-9]+-[a-z0-9-]+\.md$ ]]; then
    NONCONFORMING+=("$base")
  fi
done
shopt -u nullglob

if ((${#COLLISIONS[@]} > 0)); then
  echo "FAIL: ADR filename collides with the canonical ADR-NNNN sequence:" >&2
  for c in "${COLLISIONS[@]}"; do
    echo "  - $c" >&2
  done
  echo "Fix: rename to <scope>-<slug>.md (see docs/adr/adr-naming-and-numbering.md)." >&2
  exit 1
fi

if ((${#NONCONFORMING[@]} > 0)); then
  echo "ADVISORY: filenames not matching <scope>-<slug>.md (recorded exceptions):" >&2
  for n in "${NONCONFORMING[@]}"; do
    echo "  - $n" >&2
  done
fi

echo "OK: no ADR-NNNN filename collides with the canonical sequence (reserved: ${RESERVED[*]:-none})."
exit 0
