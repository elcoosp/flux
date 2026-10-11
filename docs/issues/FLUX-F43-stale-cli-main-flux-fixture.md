# FLUX-F43 — Stale `crates/flux-cli/main.flux/` fixture uses old syntax

**Status:** CLOSED (removed in this commit)
**Found:** round-88 bug hunt (`flux-cli` sweep)
**Severity:** low (dead fixture; never referenced by any test; its mere
presence is confusing to a reader who `grep`s for `.flux` examples)

## Summary

The tracked directory `crates/flux-cli/main.flux/` contains three files
(`main.flux`, `flux.toml`, `.fluxignore`) added by commit `dc925341`
"chore(cli): add main.flux fixture". Nothing in the workspace
references the directory — not a test, not a script, not a build file.

Worse, the `main.flux` inside uses **obsolete syntax**:

* `component Hello { … }` — the modern keyword is `compo`.
* `Button(text: …, onClick: …)` — the handler prop is `onPress`.
* `Text(text: "tapped ${count} times")` — Flux interpolation is
  `{count}`, not Kotlin-style `${count}`.

The fixture does not parse under the current grammar, and nothing
currently notices because no test exercises it.

## Resolution

Removed the directory (all three files). If a canonical "here is what
`flux init` produces" fixture is wanted, the correct source is now
`crates/flux-cli/src/init.rs::SAMPLE_ENTRY` (which the F42 fix made
correct and which `crates/flux-cli/tests/init_scaffold_compiles.rs`
verifies).

## Related

* F42 — the same `${count}` bug existed in `SAMPLE_ENTRY`; fixed in the
  same round.
