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

## Workspace clippy --all-targets results (2026-10-09, round 26)

`cargo clippy --workspace --all-targets --all-features -- -D warnings`
still fails on **pre-existing** findings in two crates even without the
new gate (they were masked earlier because the raw `cargo clippy` run
was not using `--all-targets`):

### `flux-differ` (lib + lib test)
- 6x `clippy::useless_conversion` — `u32::from(*id)` where `*id: u32`
  (`src/diff/algorithm.rs:37,134,180,216`, `src/diff/tree.rs:75,86`)
- 2x `clippy::unnecessary_map_or` — `.map_or(false, …)` -> `.is_some_and`
  (`src/diff/tree.rs:103,122`)
- 1x `clippy::module_inception` (lib test)
- 2x additional findings in lib test

### `flux-codegen-core`
- 1x `clippy::double_must_use`
- 5x `clippy::needless_borrow`

Both are outside the F10 scope (which named `flux-ir-serde` and
`flux-devserver`). They block the F10 lint gate for the workspace as a
whole and are part of the option-1/2 work tracked here.

## Concrete site enumeration (2026-10-09, round 26 follow-up)

After the workspace reached clippy-clean under `--all-targets
--all-features -- -D warnings`, applying the F10 gate exactly as the
plan specified exposes **25 pre-existing sites** in production code
(test code is exempted via the `#[cfg_attr(test, allow(...))]`).

### `crates/flux-ir-serde/src/frame.rs` (11 sites)

All are the well-known "this encoder assumes its own caller produced
well-formed data" family:

- L283 `bytes[0..4].try_into().unwrap()` — magic read in `from_bytes`
- L336 `try_encode_str(...).expect(...)` — inside an infallible `to_bytes`
- L796 `self.try_to_bytes().expect(...)` — `InitFrame::to_bytes` shim
- L873 `self.try_encode_into(buf).expect(...)` — `InitFrame::encode_into`
- L1011 `self.try_to_bytes().expect(...)` — `DeltaFrame::to_bytes` shim
- L1059 `self.try_encode_into(buf).expect(...)` — `DeltaFrame::encode_into`
- L1231 `payload[0..4].try_into().unwrap()` — heartbeat decode
- L1280 `u16::try_from(bytes.len()).expect(...)` — InternString length
- L1301 `u16::try_from(len).expect(...)` — InternString length (audit H14)
- L1420 `payload[0..4].try_into().unwrap()` — signal-meta decode
- (one more in the same file; see the count)

### `crates/flux-ir-serde/src/resume.rs` (1 site)

- L160 `self.try_to_bytes().expect(...)` — `ResumeFrame::to_bytes` shim

### `crates/flux-ir-serde/src/telemetry.rs` (>= 1 site)

- L219 `...expect(...)` in the telemetry frame encoders

### `crates/flux-devserver/src/*` (remaining)

Findings continue through the devserver's frame construction sites.
Full list available by running:

    cargo clippy -p flux-ir-serde -p flux-devserver --lib --bins -- \
      -D warnings
    # with the gate applied to both lib.rs files

### Why the gate was reverted rather than landed with `#[allow]`

The audit plan is explicit that the gate is meant to make regressions
*hard* failures. Landing it with site-by-site `#[allow]` would defeat
that: a future regression would just add one more allow. The
correct path is to fix the 25 sites (each is mechanical — convert the
`to_bytes`/`encode_into` shims into `try_*`, propagate `WireError` up
to the frame construction call sites in the devserver, and change the
`bytes[0..4].try_into().unwrap()` reads to `Reader::u32` calls) and
then land the gate unmodified.

Estimated effort: ~2 hours, mechanical. **Recommendation:** option 2
(crate-by-crate ratchet) starting with `flux-ir-serde`, per the
existing "Recommended next step" section.
