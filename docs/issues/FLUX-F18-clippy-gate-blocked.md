# FLUX-F18 — F10 clippy lint gate blocked by pre-existing violations

**Status:** open (follow-up to the audit plan's F10)
**Filed:** 2026-10-09 (round 26 follow-up)

## Context

The audit plan (F10) asked for:

```rust
#![warn(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
