# FLUX-F57 — `wire/span.rs::encode_str` writes u16, `decode_str` reads version-dispatched

**Status:** OPEN (fix landed in same commit)
**Found:** round-101 bug hunt (`flux-ir-serde/wire/span.rs` + closure_ref)
**Severity:** high (any frame carrying a `ClosureRef` whose
`SourceExcerpt.snippet` is `Some(_)` fails to decode: the encoder
writes a 2-byte length prefix, the decoder reads the 4-byte v3 prefix
at `PROTOCOL_VERSION = 3`. The decoder then interprets four bytes as
a length and immediately fails with a "truncated frame" error far past
the actual buffer.)

## Summary

`crates/flux-ir-serde/src/wire/span.rs` has an asymmetric pair:

    pub(crate) fn encode_str(w: &mut Writer, s: &str) -> Result<(), WireError> {
        w.u16_len_checked(s.len(), "span.len")?;    // <- ALWAYS u16
        w.bytes(s.as_bytes());
        Ok(())
    }

    pub(crate) fn decode_str(r: &mut Reader<'_>, ctx: &'static str) -> Result<String, WireError> {
        let len = r.count(ctx)? as usize;           // <- v3: u32, v2: u16
        let raw = r.bytes(len, ctx)?;
        // ...
    }

`Writer::count_prefix` (the version-dispatched write) and
`Writer::u16_len_checked` (the fixed-width write) are *different*
methods. The encoder picked the wrong one for a version-3 frame.

## Reproduction

The round-101 probe (deleted after the run):

    let excerpt = SourceExcerpt {
        file_id: FileId::from(1u32),
        byte_start: 10, byte_end: 20,
        line: 3, col: 5,
        snippet: "let x = 42;".to_string(),
    };
    let closure_ref = ClosureRef { /* … */ excerpt: Some(excerpt), /* … */ };
    let bytes = Frame::delta(0, 0, &[Patch::Handler { id: HandlerId::from(1), closure: closure_ref }], …)
        .try_to_bytes()
        .expect("encode");
    // 90 bytes, no error at encode time.
    Frame::from_delta_bytes(&bytes) // -> Err(Truncated { need: 1701576715, buffer_has: 13 })

The decode failure is deterministic: the 2-byte `0x000C` length (12
bytes for "let x = 42;") is read as the first two bytes of a 4-byte
u32, combined with the first two bytes of the snippet text (`"le"` =
0x6C65). `0x6C65_000C` = 1 701 576 715 — the number in the error.

## Why the existing round-trip tests missed it

`crates/flux-ir-serde/tests/round_trip.rs:314-329` asserts that a
`SourceExcerpt` on the **`ErrorFrame`** survives a round-trip. But
`ErrorFrame::to_bytes` uses `frame::encode_str` (the version-dispatched
helper), not `wire::span::encode_str`. The two helpers have the same
name but different implementations:

* `frame::encode_str` (private, in `frame.rs`):
  `w.count_prefix(s.len(), "frame.string_len")` — version-dispatched.
* `wire::span::encode_str` (private, in `wire/span.rs`):
  `w.u16_len_checked(s.len(), "span.len")` — fixed u16.

Only `wire::span::encode_str` feeds `ClosureRef.excerpt` (via
`wire::closure_ref::encode_closure_ref`), which is the field that
actually fails. No existing fixture builds a `ClosureRef` with an
excerpt.

## Fix

Change the encoder to use the version-dispatched write, matching the
decoder:

    pub(crate) fn encode_str(w: &mut Writer, s: &str) -> Result<(), WireError> {
        w.count_prefix(s.len(), "span.len")?;
        w.bytes(s.as_bytes());
        Ok(())
    }

## Regression tests

`crates/flux-ir-serde/tests/closure_ref_excerpt_roundtrip.rs` (added
in this commit):
* Round-trip a `Patch::Handler` with a `ClosureRef` carrying a
  `SourceExcerpt` through a `Delta` frame.
* Negative control: a v2 frame still decodes (the fix preserves v2
  behavior because both sides go through `count_prefix`).

## Related

* **F46** — the same *shape* of bug (`validate_bytecode` reading a
  jump target at the wrong offset).
* **F24** — the same *shape* of bug (release-side canonicalization
  mixing byte/char conventions).
* The two `encode_str` helpers should be unified; keeping two
  functions with the same name and different behavior is the real
  hazard this bug reveals.
