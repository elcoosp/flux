# FLUX-F18 — F10 clippy lint gate blocked by pre-existing violations

**Status:** open (follow-up to the audit plan's F10)
**Filed:** 2026-10-09 (round 26 follow-up)

## Context

The audit plan (F10) asked for:

    #![warn(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    #![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

at the top of `crates/flux-ir-serde/src/lib.rs` and
`crates/flux-devserver/src/lib.rs`, to make the "panicking encoders on
production paths" class un-regressable.

## Why it is not landed yet

CI runs `cargo clippy --all-targets --all-features -- -D warnings`
(`.github/workflows/rust-check.yml:74`). The gate is `warn`, but
`-D warnings` promotes it to deny. That immediately fails on
**pre-existing** violations that predate this audit:

- `crates/flux-ir-serde/src/frame.rs` and `wire/*.rs` — around 30
  `unwrap()` / `expect()` / `panic!` sites in production code (decoder
  paths that assume well-formed input from the local encoder). None of
  these were introduced by the audit.
- `crates/flux-differ/src/diff/algorithm.rs` and `diff/tree.rs` —
  8 findings (useless_conversion `u32::from`, `map_or` becomes
  `is_some_and`).
- Other workspace crates are likely to follow; enumeration stopped
  after the first cascade.

Fixing all of them is a multi-crate effort and is not part of the F10
finding (which is about **regression protection**, not about the
existing site count).

## Options

1. **Fix every site** then land the gate. Correct but large.
2. **Land the gate crate-by-crate** as each crate reaches zero findings.
   Combines (1) with incremental risk.
3. **Downgrade CI to `-W warnings`** for these two crates only via a
   workspace-level `[lints]` override, then ratchet later. Cheapest.

## Recommended next step

Option 2, starting with `flux-ir-serde`: the plan already enumerated
the specific files (`frame.rs`, `wire/value.rs:48`, `wire/patch.rs:68`),
and the fixes are mechanical (return `WireError` variants instead of
panicking, replace `unreachable!` with typed errors).

## Pre-existing clippy fixes landed in the same round

`chore(clippy): clear pre-existing findings on main (F10 prerequisite)` —
see commit history. Those four fixes are prerequisites for option 2:

- `flux-types/checker/field.rs`   useless_format
- `flux-ir/arena/blob.rs`         manual_saturating_arithmetic
- `flux-ir/lower/bytecode.rs`     too_many_arguments (targeted allow)
- `flux-ir/lower/{bytecode,mod}`  useless_conversion (u16::from on u16)
