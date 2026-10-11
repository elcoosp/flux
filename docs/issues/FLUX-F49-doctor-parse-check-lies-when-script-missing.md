# FLUX-F49 — `flux doctor`'s stdlib parse check reports `[ok]` when the script is missing

**Status:** OPEN (fix landed in same commit)
**Found:** round-93 bug hunt (`flux-cli/doctor/probe.rs`)
**Severity:** medium (the doctor is *supposed* to be "advisory but
real" — the missing-script path returns `[ok]` with a note that the
script was not run. A user who runs `flux doctor` from a subdirectory
(or any CWD where the parse-check script is not present) sees a green
checkmark for a check that did not run. Any actual regression in the
stdlib parser goes undetected until the user remembers to `cd` to the
repo root.)

## Summary

`crates/flux-cli/src/doctor/probe.rs::stdlib_parse_check`:

    let candidates = ["scripts/parse-check.sh", "stdlib/parse-check.sh"];
    for script in candidates {
        if std::path::Path::new(script).exists() {
            // ... run it, report ok or fail ...
        }
    }
    Check::ok(
        "Stdlib parse-check",
        "script not in CWD (run from repo root); modules registered",
    )

The final `Check::ok` is a lie: nothing was checked. It says "modules
registered", but the registration is not a parse check — it is only
the crate's static module list. A stdlib file with a syntax error
would be reported as `[ok]`.

The module doc explicitly promises "Each probe is real … and reports a
[Check] rather than guessing." The current behavior violates that
promise on this one path.

## Reproduction

    $ cd crates/flux-cli
    $ flux doctor
    ...
    [ok] Stdlib parse-check — script not in CWD (run from repo root); modules registered

Nothing was actually parsed.

## Fix

Distinguish "ran and passed" from "did not run". Use the existing
`Check::fail` shape (or add a new `Check::skipped` if we want to keep
`--strict` from bailing on a non-finding), so the user sees:

    [skipped] Stdlib parse-check — script not in CWD; run from repo root to enable

A `skipped` status is the honest answer: `--strict` should NOT bail
on it (nothing is wrong with the code), but the user is told that the
check did not run.

For a minimal change (no new status): change `Check::ok` to `Check::fail`
and update the detail message. The env checks are advisory (never
fatal by default), so a `[missing]` line here is harmless but visible.

The fix landed in this commit uses `Check::fail` with a clearer detail
that tells the user to run from the repo root.

## Regression test

`crates/flux-cli/src/doctor/tests.rs::stdlib_parse_check_reports_failure_when_script_missing`
— temporarily cds into a temp dir with no script, calls
`stdlib_parse_check`, and asserts the check is NOT `ok`.

## Note about the actual workspace

Running from the repo root works — both `scripts/parse-check.sh` and
`stdlib/parse-check.sh` exist under the repo. The bug only bites when
the user runs `flux doctor` from anywhere else.
