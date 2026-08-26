#!/usr/bin/env bash
# merge-guard-check.sh — directory-collision guard for the parallel-`main` flow.
#
# Usage:
#   merge-guard-check.sh <dirs>            # fail if <dirs> overlaps the recorded push
#   merge-guard-check.sh --write <dirs>    # record <dirs> as the last push
#
# <dirs> is a comma-separated list of top-level directories touched by a push.
# State lives in .github/dir-locks.json:
#   {"last_push": {"dirs": ["crates", "docs"]}}
#
# Exit codes: 0 = no collision (or record written), 1 = collision, 2 = usage error.

set -euo pipefail

readonly LOCKFILE_REL=".github/dir-locks.json"

repo_root() {
  git rev-parse --show-toplevel 2>/dev/null || pwd
}

usage() {
  printf 'usage: %s [--write] <comma-separated-dirs>\n' "${0##*/}" >&2
  exit 2
}

# Prints the recorded directories, one per line (empty output when unrecorded).
recorded_dirs() {
  local lockfile="$1"
  [ -f "$lockfile" ] || return 0
  python3 - "$lockfile" <<'PY'
import json
import sys

try:
    with open(sys.argv[1], encoding="utf-8") as handle:
        data = json.load(handle)
except (OSError, ValueError):
    sys.exit(0)
if not isinstance(data, dict):
    sys.exit(0)
last = data.get("last_push") or {}
for entry in last.get("dirs", []) if isinstance(last, dict) else []:
    print(entry)
PY
}

write_lock() {
  local lockfile="$1" dirs="$2"
  python3 - "$lockfile" "$dirs" <<'PY'
import json
import sys

lockfile, raw = sys.argv[1], sys.argv[2]
dirs = sorted({part for part in raw.split(",") if part})
with open(lockfile, "w", encoding="utf-8") as handle:
    json.dump({"last_push": {"dirs": dirs}}, handle, indent=2, sort_keys=True)
    handle.write("\n")
PY
}

main() {
  local mode="check"
  if [ "${1:-}" = "--write" ]; then
    mode="write"
    shift
  fi
  [ "$#" -eq 1 ] || usage

  local current="$1"
  local lockfile
  lockfile="$(repo_root)/${LOCKFILE_REL}"

  if [ "$mode" = "write" ]; then
    write_lock "$lockfile" "$current"
    printf 'merge-guard: recorded directories [%s]\n' "${current:-none}"
    return 0
  fi

  local collisions=""
  local previous
  previous="$(recorded_dirs "$lockfile")"
  local dir
  for dir in ${current//,/ }; do
    if printf '%s\n' "$previous" | grep -Fxq -- "$dir"; then
      collisions="${collisions:+$collisions, }$dir"
    fi
  done

  if [ -n "$collisions" ]; then
    printf 'merge-guard: directory %s collided with the previous push.\n' "$collisions" >&2
    printf '  why: two agents edited the same top-level directory in consecutive\n' >&2
    printf '       pushes to main, which is how parallel work overwrites itself.\n' >&2
    printf '  fix: pull main, re-apply your change on top of the previous push,\n' >&2
    printf '       and confirm the other agent is finished with %s.\n' "$collisions" >&2
    return 1
  fi

  printf 'merge-guard: no collision (touched: %s).\n' "${current:-none}"
}

main "$@"
