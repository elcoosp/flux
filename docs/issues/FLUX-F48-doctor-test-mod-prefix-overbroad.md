# FLUX-F48 — `flux doctor`'s `mod tests` detection is over-broad; suppresses legitimate findings

**Status:** OPEN (documented; not fixed — the fix shape is a design call)
**Found:** round-92 bug hunt (`flux-cli/doctor` sweep)
**Severity:** low (a false negative in an *advisory* health check; the
heuristic silently disables panic-API detection for any production
module whose name starts with `tests`, or for the rest of a file that
declares `mod tests;` as an external file)

## Summary

`crates/flux-cli/src/doctor/agents_rules.rs::scan_rust` decides
whether it is inside the file's test module with:

    if !is_test_file && in_test_mod.is_none() && line.starts_with("mod tests") {
        in_test_mod = Some(depth);
    }

Then, later in the same file, panic-API detection is gated on
`in_test_mod.is_none()`. Two ways this misfires:

1. **Prefix collision.** `mod tests_impl { … }` starts with `mod tests`,
   so the flag is set even though the module is production code. Any
   `.unwrap()` / `.expect()` inside `tests_impl` and any code *after*
   it in the file is silently exempted.

2. **External-module declaration.** `mod tests;` (no braces) is the
   normal Rust idiom for keeping tests in a sibling `tests.rs`. The
   flag is set at the current depth, but no `}` will ever close at that
   depth, so `in_test_mod` stays `Some(_)` for the rest of the file.
   Every `.unwrap()` in the file — including real production functions
   *after* the `mod tests;` line — is exempted.

The doctor's whole purpose is to be "exactly as strict as CI, not
stricter" (per its own doc comment). The over-broad match makes it
*slightly less* strict than CI in these two shapes, defeating that
contract.

## Reproduction

`crates/flux-cli/src/doctor/tests.rs` (added in this commit):
* `mod_tests_prefix_suppresses_panic_api_in_sibling_modules`
* `external_mod_tests_suppresses_panic_api_for_rest_of_file`

Both tests write a tiny fixture file and assert the current
(over-broad) behavior. When the fix lands, the assertions flip.

## What a fix needs

Make the match specific:

* Match `mod tests` exactly **followed by whitespace + `{`** on the
  same line (a real inline test module), not any prefix.
* Skip `mod tests;` entirely (the external form has no brace to
  close, and this file-local scanner cannot see into a sibling file).

A stricter alternative — a proper `syn`-based parser — is out of
scope for the doctor's "best-effort heuristic" design. A regex like
`^\s*(?:pub\s+)?mod\s+tests\s*\{` would close both cases.

## Related

`docs/issues/FLUX-F48-doctor-test-mod-prefix-overbroad.md` — this file.
