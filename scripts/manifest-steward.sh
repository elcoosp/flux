#!/usr/bin/env bash
# manifest-steward.sh — fulfils dependency requests from MANIFEST_REQUESTS.md.
#
# Agents never edit frozen manifests. They append a row to the request table in
# MANIFEST_REQUESTS.md; this steward parses those rows, writes the dependency into
# the real manifest, commits the manifest change, and truncates the request file
# back to its header.
#
# Usage:
#   manifest-steward.sh --dry-run   # report pending requests, change nothing
#   manifest-steward.sh             # apply requests, commit, truncate
#
# Supported targets (by request `crate` column):
#   <crate-name>            -> crates/<crate-name>/Cargo.toml   [dependencies]
#   workspace               -> Cargo.toml   [workspace.dependencies]
#   ios | Package.swift     -> not auto-applied; reported for manual review
#   android | build.gradle  -> not auto-applied; reported for manual review
#
# Exit codes: 0 = success (including "no pending requests"), 1 = failure.

set -euo pipefail

readonly REQUEST_FILE="MANIFEST_REQUESTS.md"
readonly TABLE_HEADER='| crate | dependency | version | reason |'

DRY_RUN=0

usage() {
  printf 'usage: %s [--dry-run]\n' "${0##*/}" >&2
  exit 1
}

repo_root() {
  git rev-parse --show-toplevel 2>/dev/null || pwd
}

# Emits one TAB-separated `crate<TAB>dependency<TAB>version` line per request row.
parse_requests() {
  python3 - "$1" <<'PY'
import sys

path = sys.argv[1]
try:
    with open(path, encoding="utf-8") as handle:
        lines = handle.read().splitlines()
except OSError as exc:
    print(f"manifest-steward: cannot read {path}: {exc}", file=sys.stderr)
    sys.exit(1)

seen_header = False
for line in lines:
    stripped = line.strip()
    if not stripped.startswith("|"):
        continue
    cells = [cell.strip() for cell in stripped.strip("|").split("|")]
    if not seen_header:
        seen_header = cells[:1] == ["crate"]
        continue
    if all(set(cell) <= set("-: ") for cell in cells):
        continue
    if len(cells) < 3 or not cells[0] or not cells[1] or not cells[2]:
        print(f"manifest-steward: malformed request row: {stripped}", file=sys.stderr)
        sys.exit(1)
    print("\t".join(cells[:3]))
PY
}

manifest_for() {
  local crate="$1"
  case "$crate" in
    workspace) printf 'Cargo.toml' ;;
    ios | Package.swift | android | build.gradle | build.gradle.kts) printf '' ;;
    *) printf 'crates/%s/Cargo.toml' "$crate" ;;
  esac
}

apply_request() {
  local manifest="$1" crate="$2" dep="$3" version="$4"
  python3 - "$manifest" "$crate" "$dep" "$version" <<'PY'
import re
import sys

manifest, crate, dep, version = sys.argv[1:5]
section = "[workspace.dependencies]" if crate == "workspace" else "[dependencies]"
with open(manifest, encoding="utf-8") as handle:
    text = handle.read()

if re.search(rf'(?m)^\s*{re.escape(dep)}\s*=', text):
    print(f"manifest-steward: {dep} already present in {manifest}")
    sys.exit(0)

entry = f'{dep} = "{version}"\n'
index = text.find(section)
if index == -1:
    text = text.rstrip("\n") + f"\n\n{section}\n{entry}"
else:
    start = index + len(section)
    newline = text.find("\n", start)
    insert_at = len(text) if newline == -1 else newline + 1
    text = text[:insert_at] + entry + text[insert_at:]

with open(manifest, "w", encoding="utf-8") as handle:
    handle.write(text)
print(f"manifest-steward: added {dep} = \"{version}\" to {manifest}")
PY
}

truncate_requests() {
  local file="$1"
  python3 - "$file" <<'PY'
import sys

path = sys.argv[1]
with open(path, encoding="utf-8") as handle:
    lines = handle.read().splitlines()

out, seen_header = [], False
for line in lines:
    cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
    if line.strip().startswith("|"):
        if not seen_header:
            seen_header = cells[:1] == ["crate"]
            out.append(line)
            continue
        if all(set(cell) <= set("-: ") for cell in cells):
            out.append(line)
        continue
    out.append(line)

with open(path, "w", encoding="utf-8") as handle:
    handle.write("\n".join(out).rstrip("\n") + "\n")
PY
}

main() {
  case "${1:-}" in
    --dry-run) DRY_RUN=1 ;;
    "") ;;
    *) usage ;;
  esac

  cd "$(repo_root)"
  if [ ! -f "$REQUEST_FILE" ]; then
    printf 'manifest-steward: %s not found\n' "$REQUEST_FILE" >&2
    return 1
  fi
  if ! grep -Fq "$TABLE_HEADER" "$REQUEST_FILE"; then
    printf 'manifest-steward: %s is missing the request table header:\n  %s\n' \
      "$REQUEST_FILE" "$TABLE_HEADER" >&2
    return 1
  fi

  local requests
  requests="$(parse_requests "$REQUEST_FILE")"
  if [ -z "$requests" ]; then
    printf 'manifest-steward: no pending requests.\n'
    return 0
  fi

  local applied=""
  local crate dep version manifest
  while IFS=$'\t' read -r crate dep version; do
    [ -n "$crate" ] || continue
    manifest="$(manifest_for "$crate")"
    if [ -z "$manifest" ]; then
      printf 'manifest-steward: %s -> %s@%s needs manual review (non-Cargo manifest).\n' \
        "$crate" "$dep" "$version"
      continue
    fi
    if [ ! -f "$manifest" ]; then
      printf 'manifest-steward: no manifest %s for requested crate %s\n' "$manifest" "$crate" >&2
      return 1
    fi
    if [ "$DRY_RUN" -eq 1 ]; then
      printf 'manifest-steward: would add %s = "%s" to %s\n' "$dep" "$version" "$manifest"
      continue
    fi
    apply_request "$manifest" "$crate" "$dep" "$version"
    case " $applied " in
      *" $manifest "*) ;;
      *) applied="${applied:+$applied }$manifest" ;;
    esac
  done <<<"$requests"

  if [ "$DRY_RUN" -eq 1 ]; then
    return 0
  fi

  truncate_requests "$REQUEST_FILE"
  if [ -z "$applied" ]; then
    printf 'manifest-steward: nothing applied.\n'
    return 0
  fi

  # shellcheck disable=SC2086
  git commit --only $applied "$REQUEST_FILE" \
    -m "chore: apply pending manifest requests" >/dev/null
  printf 'manifest-steward: committed %s and truncated %s.\n' "$applied" "$REQUEST_FILE"
}

main "$@"
