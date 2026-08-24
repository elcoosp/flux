#!/bin/bash
#
# check-ownership.sh — boundary-contract guard (FLUX-011, contract R1/R2/R3/R5).
#
# Best-effort CI guard that fails if a pushed commit touches any file outside
# the directories an agent is allowed to modify, or edits a frozen build
# manifest. It diffs the merge base against the current HEAD so it works for
# both push and pull_request events.
#
# The set of protected (frozen) paths and agent-owned roots is centralised here
# so the guard stays in one place as the contributor roster changes. Editing
# THIS script is the only sanctioned way to change what the guard protects.
#
# Usage:
#   scripts/check-ownership.sh [base-ref] [head-ref]
#   - base-ref defaults to the PR base / origin/main
#   - head-ref defaults to HEAD
#
# Exit code 0 = no violations; 1 = violations found.

set -euo pipefail

base_ref="${1:-origin/main}"
head_ref="${2:-HEAD}"

# --- Frozen build manifests (contract R2). Editing any of these is forbidden
#     for every agent, including the foundation agent after Phase 0. -----------
frozen_manifests=(
  "Cargo.toml"
  "rust-toolchain.toml"
  ".gitignore"
  "settings.gradle.kts"
  "gradle/libs.versions.toml"
  "runtimes/ios/project.yml"
  "adapters/ui-swift/Package.swift"
  "adapters/ui-kotlin/build.gradle.kts"
  "runtimes/android/app/build.gradle.kts"
)

# --- Directories that NO agent owns (read-mostly, owned by the orchestrator).
#     Agents must not create or modify files under these. ---------------------
protected_dirs=(
  "docs/"
  "tests/isa-vectors/"
)

# Resolve the merge base so we only inspect the delta introduced by this branch.
if git rev-parse --verify "$base_ref" >/dev/null 2>&1; then
  merge_base="$(git merge-base "$base_ref" "$head_ref")"
else
  # base-ref not fetchable (e.g. fresh fork): fall back to the first parent.
  merge_base="$(git rev-parse "${head_ref}~1" 2>/dev/null || echo "$head_ref")"
fi

changed="$(git diff --name-only "$merge_base" "$head_ref")"

violations=()

for file in $changed; do
  # Frozen manifest check (exact path match).
  for frozen in "${frozen_manifests[@]}"; do
    if [ "$file" = "$frozen" ]; then
      violations+=("frozen manifest modified: $file")
    fi
  done

  # Protected directory check.
  for dir in "${protected_dirs[@]}"; do
    if [[ "$file" == "$dir"* ]]; then
      violations+=("file under protected directory: $file")
    fi
  done
done

if [ "${#violations[@]}" -gt 0 ]; then
  echo "::error::Boundary contract violation(s) detected:"
  for v in "${violations[@]}"; do
    echo "  - $v"
  done
  echo
  echo "See AGENTS.md §1.6 (rules R1-R3, R5) and docs/agents-boundaries-contract.md §1.2."
  exit 1
fi

echo "Ownership check passed: no frozen manifests or protected directories modified."
exit 0
