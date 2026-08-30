#!/usr/bin/env bash
#
# ci-size-gate.sh — FLUX-087 CI gate (LANE-CILINT, Phase 2 structural).
#
# Enforces AGENTS.md §1.2 structural rules so they cannot silently regress:
#   1. No tracked/production source file exceeds 300 lines (allowlist escape hatch).
#   2. No function exceeds 40 lines (best-effort awk heuristic, Rust/Swift/Kotlin).
#   3. No `unwrap` / `expect` / `panic!` in non-test Rust code (AGENTS.md §2.1).
#   4. No `try!` / force-unwrap in non-test Swift/Kotlin (AGENTS.md §2.2/§2.3).
#
# DESIGN — DELTA GATE (default mode, as wired into CI):
#   It only fails on NEW violations introduced by the lines you TOUCH in this
#   branch (diffed vs the merge base).
#     - Rule 1 (file length) is checked on the whole changed file: a file that is
#       now >300 lines and is NOT on the allowlist fails. Today's oversized files
#       are seeded into scripts/ci-size-gate.allowlist and will be split under
#       FLUX-088; each landed split removes its line so the rule re-arms.
#     - Rule 3/4 (forbidden calls) is a REGRESSION check on the lines you
#       add/modify: an offending call is only flagged when it sits on a newly-added
#       diff line. Pre-existing occurrences in untouched code are not punished.
#     - Rule 2 (function length) is STRICT only in --all mode. In delta mode it is
#       reported as a non-blocking [info] line: the current tree carries widespread
#       pre-existing function debt (no per-function allowlist), so blocking on it
#       would make the gate red immediately. Promote to blocking in delta mode once
#       function-split work (tracked alongside FLUX-088) removes the debt.
#   `--all` mode scans the entire tracked tree and fails on ANY violation (used by
#   FLUX-091's local check and by --selftest; NOT the default CI mode because the
#   repo currently carries known-debt files).
#
# Dependencies: bash, git, awk, wc. No tokei / language toolchain required.
#
# Usage:
#   scripts/ci-size-gate.sh [--base REF] [--head REF] [--all] [--selftest] [-v]
#   --base REF   merge-base default: origin/main (falls back to HEAD~1 off-git).
#   --head REF   default: HEAD.
#   --all        scan every tracked source file (strict).
#   --selftest   run internal tests in a throwaway git repo, then exit.
#   -v           verbose: print every check as it runs.
# Exit: 0 = pass, 1 = violations found, 2 = usage / environment error.

set -uo pipefail

# --- Configurable limits (single source of truth) --------------------------
MAX_FILE_LINES=300
MAX_FUNC_LINES=40

# --- Paths ------------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# Repo root = the git top-level that contains this script (robust to copies/runs
# from elsewhere); fall back to the script's parent dir when not in a work tree.
if REPO_ROOT="$(git -C "$SCRIPT_DIR" rev-parse --show-toplevel 2>/dev/null)"; then
  :
else
  REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
fi
ALLOWLIST="${ALLOWLIST:-$SCRIPT_DIR/ci-size-gate.allowlist}"

# --- CLI --------------------------------------------------------------------
BASE_REF="origin/main"
HEAD_REF="HEAD"
MODE="delta"
SELFTEST=0
VERBOSE=0
while [ $# -gt 0 ]; do
  case "$1" in
    --base)     BASE_REF="${2:-}"; shift 2 || { echo "::error::--base needs an arg" >&2; exit 2; } ;;
    --head)     HEAD_REF="${2:-}"; shift 2 || { echo "::error::--head needs an arg" >&2; exit 2; } ;;
    --all)      MODE="all"; shift ;;
    --selftest) SELFTEST=1; shift ;;
    -v)         VERBOSE=1; shift ;;
    -h|--help)  sed -n '1,45p' "$0"; exit 0 ;;
    *)          echo "::error::unknown arg: $1" >&2; exit 2 ;;
  esac
done

# --- Helpers ----------------------------------------------------------------
log()  { [ "$VERBOSE" -eq 1 ] && printf '  [check] %s\n' "$1" >&2; }
# Annotations go to stderr so they never contaminate a captured violation count.
warn() { printf '::error file=%s,line=%s::%s\n' "$1" "$2" "$3" >&2; }
info() { printf '[info] %s\n' "$1" >&2; }

# Resolve the merge base (or a sane fallback) once.
MERGE_BASE=""
if [ "$MODE" = "delta" ]; then
  if git -C "$REPO_ROOT" rev-parse --verify "$BASE_REF" >/dev/null 2>&1; then
    MERGE_BASE="$(git -C "$REPO_ROOT" merge-base "$BASE_REF" "$HEAD_REF")"
  else
    MERGE_BASE="$(git -C "$REPO_ROOT" rev-parse "${HEAD_REF}~1" 2>/dev/null || echo "$HEAD_REF")"
  fi
fi

# Collect repo-relative source paths (filtered: source ext, exclude build/test/Generated).
# Prints one path per line to stdout.
collect_files() {
  if [ "$MODE" = "all" ]; then
    git -C "$REPO_ROOT" ls-files \
      | grep -E '\.(rs|swift|kt|kts)$' \
      | grep -vE '(^|/)(build|target|Generated|\.build|platforms)/' \
      | grep -vE '(^|/)(Tests?|tests?|androidTest)/' \
      | grep -vE 'Tests?/'
    return
  fi
  git -C "$REPO_ROOT" diff --name-only "$MERGE_BASE" "$HEAD_REF" \
    | grep -E '\.(rs|swift|kt|kts)$' \
    | grep -vE '(^|/)(build|target|Generated|\.build|platforms)/' \
    | grep -vE '(^|/)(Tests?|tests?|androidTest)/' \
    | grep -vE 'Tests?/'
}

# True if $1 is an exact allowlisted path.
is_allowlisted() {
  [ -f "$ALLOWLIST" ] || return 1
  local p
  while IFS= read -r p; do
    case "$p" in
      ''|\#*) continue ;;
    esac
    [ "$p" = "$1" ] && return 0
  done < "$ALLOWLIST"
  return 1
}

# True if a path is a test file (exempt from the unwrap/func regression rules).
is_test_file() {
  case "$1" in
    *Tests*/*|*Tests/*|*/Tests/*|*tests/*|*/tests/*|*androidTest/*|\
    *Spec*/*|*Spec.swift|*Spec.kt|*Spec.kts) return 0 ;;
  esac
  return 1
}

# --- Check 1: file line count (whole changed file) --------------------------
check_file_lines() {
  local violations=0 rel f n
  while IFS= read -r rel; do
    [ -z "$rel" ] && continue
    f="$REPO_ROOT/$rel"; [ -f "$f" ] || continue
    n="$(wc -l < "$f" | tr -d ' ')"
    if [ "$n" -gt "$MAX_FILE_LINES" ]; then
      if is_allowlisted "$rel"; then
        log "allowlisted (> $MAX_FILE_LINES): $rel ($n)"
        continue
      fi
      warn "$rel" "$n" "file is $n lines, exceeds AGENTS.md §1.2 limit of $MAX_FILE_LINES"
      violations=$((violations+1))
    fi
  done
  echo "$violations"
}

# --- Check 2: function length ----------------------------------------------
# awk emits "rel:startline:funcname:length" for funcs longer than the limit.
emit_long_funcs() {
  local rel f
  while IFS= read -r rel; do
    [ -z "$rel" ] && continue
    f="$REPO_ROOT/$rel"; [ -f "$f" ] || continue
    [ "$MODE" = "delta" ] && is_test_file "$rel" && continue
    awk -v limit="$MAX_FUNC_LINES" -v rel="$rel" '
      BEGIN { depth=0; in_func=0; start=0; name="" }
      {
        line=$0
        if (in_func==0) {
          if (match(line, /(fn|func|def)[ \t]+[A-Za-z_][A-Za-z0-9_]*[ \t]*\(/)) {
            nm=substr(line, RSTART, RLENGTH)
            sub(/^[ \t]*(fn|func|def)[ \t]+/, "", nm)
            sub(/[ \t]*\(.*/, "", nm)
            in_func=1; start=NR; name=nm; depth=0
          } else { next }
        }
        n=split(line, ch, "")
        for (i=1;i<=n;i++) {
          c=ch[i]
          if (c=="{") depth++
          else if (c=="}") depth--
        }
        if (in_func==1 && depth<=0 && NR>start) {
          len=NR-start+1
          if (len>limit) print rel":"start":"name":"len
          in_func=0
        }
      }
    ' "$f"
  done
}

# Function-length is a STRICT (blocking) check only in --all mode. In delta mode
# it is reported as informational only: the current tree carries widespread
# pre-existing function debt (no per-function allowlist exists), so blocking on
# touched pre-existing long functions would make the gate red on day one. As
# FLUX-088/function-split work lands, switch this to blocking in delta mode too.
check_func_lines() {
  local violations=0 rel rest sl fn len added
  while IFS= read -r row; do
    [ -z "$row" ] && continue
    rel="${row%%:*}"; rest="${row#*:}"; sl="${rest%%:*}"
    r2="${rest#*:}"; fn="${r2%%:*}"; len="${r2##*:}"
    if [ "$MODE" = "all" ]; then
      warn "$rel" "$sl" "function '$fn' is $len lines, exceeds §1.2 limit of $MAX_FUNC_LINES"
      violations=$((violations+1))
    else
      info "func-length (non-blocking, pre-existing debt): $rel:$sl '$fn' $len lines"
    fi
  done < <(emit_long_funcs)
  echo "$violations"
}

# --- Check 3+4: forbidden calls in non-test source --------------------------
# Patterns per language. Delta mode restricts to added lines only.
check_forbidden() {
  local violations=0 rel f added
  while IFS= read -r rel; do
    [ -z "$rel" ] && continue
    f="$REPO_ROOT/$rel"; [ -f "$f" ] || continue
    if is_test_file "$rel"; then log "test file skipped (forbidden-call): $rel"; continue; fi
    case "$rel" in
      *.rs)
        if [ "$MODE" = "all" ]; then
          while IFS= read -r m; do
            [ -z "$m" ] && continue
            warn "$rel" "${m%%:*}" "forbidden in non-test Rust (§2.1): ${m#*:}"
            violations=$((violations+1))
          done < <(grep -nE '\b(unwrap|expect|panic!)\b' "$f")
        else
          added="$(git -C "$REPO_ROOT" diff -U0 "$MERGE_BASE" "$HEAD_REF" -- "$f" \
            | grep -E '^\+[^+]' | sed 's/^\+//' \
            | grep -cE '\b(unwrap|expect|panic!)\b')"
          if [ "$added" -gt 0 ]; then
            warn "$rel" "new" "forbidden in non-test Rust (§2.1): $added added line(s) with unwrap/expect/panic!"
            violations=$((violations+added))
          fi
        fi
        ;;
      *.swift|*.kt|*.kts)
        if [ "$MODE" = "all" ]; then
          while IFS= read -r m; do
            [ -z "$m" ] && continue
            warn "$rel" "${m%%:*}" "forbidden in non-test code (§2.2/§2.3): ${m#*:}"
            violations=$((violations+1))
          done < <(grep -nE '(try!|[A-Za-z0-9_)\]]\s*!(=|\?|;|,|\)|\s|$))' "$f" \
                     | grep -vE '//.*(!|\?)' | grep -E '(!|\?)')
        else
          added="$(git -C "$REPO_ROOT" diff -U0 "$MERGE_BASE" "$HEAD_REF" -- "$f" \
            | grep -E '^\+[^+]' | sed 's/^\+//' \
            | grep -cE '(try!|[A-Za-z0-9_)\]]\s*!(=|\?|;|,|\)|\s|$))')"
          if [ "$added" -gt 0 ]; then
            warn "$rel" "new" "forbidden in non-test code (§2.2/§2.3): $added added line(s) with try!/force-unwrap"
            violations=$((violations+added))
          fi
        fi
        ;;
    esac
  done
  echo "$violations"
}

# --- Main -------------------------------------------------------------------
run_gate() {
  echo "::group::FLUX-087 size gate ($MODE mode)"
  local files v1 v2 v3 total
  files="$(collect_files)"
  if [ -z "$files" ] && [ "$MODE" = "delta" ]; then
    echo "No source files changed in this delta — gate passes vacuously."
    echo "::endgroup::"
    return 0
  fi
  v1="$(printf '%s\n' "$files" | check_file_lines)"
  v2="$(printf '%s\n' "$files" | check_func_lines)"
  v3="$(printf '%s\n' "$files" | check_forbidden)"
  log "summary: file-lines=$v1 func=$v2 forbidden=$v3"
  echo "::endgroup::"
  total=$(( v1 + v2 + v3 ))
  if [ "$total" -gt 0 ]; then
    echo "::error::FLUX-087 gate FAILED: $v1 file-length, $v2 function-length, $v3 forbidden-call violation(s)."
    return 1
  fi
  echo "FLUX-087 gate passed (mode=$MODE)."
  return 0
}

# --- Selftest (own throwaway git repo) --------------------------------------
run_selftest() {
  local tmpd ec=0 script_copy allow_copy
  tmpd="$(mktemp -d)"
  echo "::group::FLUX-087 selftest"
  cp "$SCRIPT_DIR/ci-size-gate.sh" "$tmpd/ci-size-gate.sh"
  cp "$ALLOWLIST" "$tmpd/ci-size-gate.allowlist"
  chmod +x "$tmpd/ci-size-gate.sh"
  git -C "$tmpd" init -q
  git -C "$tmpd" config user.email t@t; git -C "$tmpd" config user.name t
  git -C "$tmpd" add -A && git -C "$tmpd" commit -qm base

  # A) oversized untracked file fails in --all (tracked via add, but not allowlisted)
  printf '%0.sX\n' $(seq 1 301) > "$tmpd/big.rs"
  git -C "$tmpd" add big.rs
  if ALLOWLIST="$tmpd/ci-size-gate.allowlist" bash "$tmpd/ci-size-gate.sh" --all --base HEAD --head HEAD >/dev/null 2>&1; then
    echo "::error::selftest A: expected failure on oversized file"; ec=1
  else echo "ok A: oversized tracked file fails"; fi

  # B) small clean file passes
  git -C "$tmpd" rm -q --cached big.rs; rm -f "$tmpd/big.rs"
  printf 'fn main() {}\n' > "$tmpd/small.rs"; git -C "$tmpd" add small.rs
  if ALLOWLIST="$tmpd/ci-size-gate.allowlist" bash "$tmpd/ci-size-gate.sh" --all --base HEAD --head HEAD >/dev/null 2>&1; then
    echo "ok B: small clean file passes"
  else echo "::error::selftest B: expected pass on small file"; ec=1; fi

  # C) unwrap in non-test rust fails (flagged line is added)
  git -C "$tmpd" rm -q --cached small.rs; rm -f "$tmpd/small.rs"
  printf 'fn f() { let x = None.unwrap(); }\n' > "$tmpd/u.rs"; git -C "$tmpd" add u.rs
  if ALLOWLIST="$tmpd/ci-size-gate.allowlist" bash "$tmpd/ci-size-gate.sh" --all --base HEAD --head HEAD >/dev/null 2>&1; then
    echo "::error::selftest C: expected failure on unwrap"; ec=1
  else echo "ok C: unwrap in non-test rust fails"; fi

  # D) allowlisted oversized file passes
  git -C "$tmpd" rm -q --cached u.rs; rm -f "$tmpd/u.rs"
  printf '%0.sX\n' $(seq 1 301) > "$tmpd/dummy.rs"; git -C "$tmpd" add dummy.rs
  printf 'dummy.rs\n' > "$tmpd/ci-size-gate.allowlist"
  if ALLOWLIST="$tmpd/ci-size-gate.allowlist" bash "$tmpd/ci-size-gate.sh" --all --base HEAD --head HEAD >/dev/null 2>&1; then
    echo "ok D: allowlisted oversized file passes"
  else echo "::error::selftest D: allowlisted file should pass"; ec=1; fi

  echo "::endgroup::"
  rm -rf "$tmpd"
  if [ "$ec" -eq 0 ]; then echo "selftest: ALL PASSED"; else echo "selftest: FAILED"; fi
  return "$ec"
}

if [ "$SELFTEST" -eq 1 ]; then
  run_selftest
  exit $?
fi

run_gate
exit $?
