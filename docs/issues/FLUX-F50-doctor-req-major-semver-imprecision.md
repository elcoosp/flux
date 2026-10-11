# FLUX-F50 — `flux doctor`'s dependency-drift check mis-models `>=` / `=` requirements

**Status:** OPEN (documented; low severity; not fixed this round — the
fix shape is a design call)
**Found:** round-94 bug hunt (`flux-cli/doctor/dependency_drift.rs`)
**Severity:** low (advisory check only; today's workspace uses only
`^MAJOR` requirements, so the imprecision never fires. It would
produce a false-positive drift for any future dependency declared with
`>=1.2` or `=1.2.3` whose resolved version happened to be in a higher
major — e.g. `>=1.2` resolving to `2.0.0`, which is exactly what the
user asked for.)

## Summary

`crates/flux-cli/src/doctor/dependency_drift.rs::req_major` extracts
the first run of digits from the requirement string and treats it as
the `^MAJOR` ceiling:

    fn req_major(req: &str) -> Option<u64> {
        let digits: String = req
            .chars()
            .skip_while(|c| !c.is_ascii_digit())
            .take_while(|c| c.is_ascii_digit())
            .collect();
        digits.parse().ok()
    }

That is correct for `^1`, `~1.0`, and a bare `1`, but wrong for:

* `>=1.2` — semver: `>=1.2.0` with **no upper bound**. The doctor treats
  it as `^1` and flags a resolved `2.x` as drift, though `2.x` is
  exactly what `>=1.2` allows.
* `=1.2.3` — semver: the **exact** version `1.2.3`. The doctor treats it
  as `^1` and silently accepts a resolved `1.9.0` — a real drift the
  check would miss.
* `<=1.2.3`, `>1.0`, `<2.0` — same class.

The module's own doc says "the common forms used in this workspace:
`^1`, `>=1.2`, `=1.2.3`, `~1.0`, and a bare `1`" — it names the two
forms it mis-models. The module's test `req_major_parses_common_forms`
even asserts `req_major(">=1.2") == Some(1)` and
`req_major("=1.2.3") == Some(1)`, pinning the buggy semantics.

## Impact today

None: the Flux workspace declares only `^MAJOR` requirements (see
`Cargo.toml`'s `[workspace.dependencies]`). The bug is latent, but the
doctor's purpose is to warn about drift. A future dependency added with
`>=` or `=` would produce a false positive (spurious `[ok]`) or a false
negative (missed drift) that the user learns not to trust.

## What a fix needs

Distinguish the operators:

* `^MAJOR` — ceiling `MAJOR + 1` (a resolved version with
  `major >= MAJOR + 1` is drift; `major == MAJOR` is fine).
* `~MAJOR.MINOR` — ceiling `MAJOR.MINOR + 1`.
* `=X.Y.Z` — exact; any other resolved version is drift.
* `>=`, `>`, `<=`, `<` — not a `^MAJOR` ceiling; skip the check for
  these forms (they have no natural ceiling).
* bare `X` — equivalent to `^X` in Cargo (`1` == `^1`).

A small `enum Ceiling { Major(u64), MajorMinor(u64, u64), Exact(u64, u64, u64), None }`
returned from a replacement for `req_major` would cover every form
Cargo accepts, and the test suite would flip the two wrong assertions.

The fix is purely mechanical but touches the API and the tests;
filing rather than fixing leaves the design to the next reader.

## Regression tests

`crates/flux-cli/src/doctor/dependency_drift.rs::tests::req_major_parses_common_forms`
currently pins the buggy semantics. When the fix lands, that test's
`>=1.2` and `=1.2.3` assertions change to reflect the correct
operator semantics.
