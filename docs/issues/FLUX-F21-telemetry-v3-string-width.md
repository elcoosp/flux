# FLUX-F21 — telemetry strings are hard-coded u16, not widened by ADR-0059

**Status:** open
**Found:** round 49 (an attempt to widen was reverted because it broke round-trip tests)
**Severity:** low (telemetry strings are all host-bounded today; the gap matters only if a future capability payload crosses 64 KB)

## Summary

ADR-0059 lists the "String content" sites that widen from u16 to u32 at v3:

    StringEntry.text byte length
    frame::encode_str / try_encode_str byte length

It does **not** list `telemetry::encode_str` — the crate-local helper used by
the `Telemetry` / `EnrichedTelemetry` / `HostAnnounce` / `DebugCommand` frame
encoders. That helper (and its decoder partner `telemetry::decode_str`) is
hard-coded `u16` regardless of the frame's version byte:

    crates/flux-ir-serde/src/telemetry.rs:1343   fn encode_str(...) { w.u16_len(...) }
    crates/flux-ir-serde/src/telemetry.rs:1349   fn decode_str(...)  { ... u16::from_le_bytes ... }

By contrast, `frame::encode_str` / `frame::decode_str` **are** version-dispatched
(`frame.rs:352` branches on `version >= 3`).

## Why an attempted fix was reverted

Round 49 changed only the *encoder* side of `telemetry::encode_str` to use
`Writer::count_prefix` (which widens to u32 at v3), on the theory that all v3
strings should be widened for consistency. This broke
`perf_record_round_trips_on_raw_frame` / `_on_enriched_frame` because the
decoder side still reads a `u16` prefix — so the change must be made
**atomically** on both sides or not at all.

## What a fix needs

* Decide whether telemetry frame strings widen at v3, i.e. whether ADR-0059
  should be extended to include them. Today every telemetry string is host-
  authored and bounded by construction (`platform` / `device` come from
  `UIDevice` / `Build.MODEL`; capability names/features come from the IDL;
  `DebugCommand` payloads are dev-tool commands, not user data).
  * **If yes**: version-dispatch `telemetry::decode_str` (mirroring
    `frame::decode_str`) and widen `telemetry::encode_str`. Test with a >64 KB
    `DebugCommand` payload.
  * **If no**: document the deliberate u16 ceiling in ADR-0059's "Why not
    everything" section, and (ideally) add a compile-time or test-time check
    that the encoder rejects an oversized telemetry string rather than
    silently truncating.
* Whichever path is taken, keep both sides in one commit.

## Related

* `docs/adr/0059-wire-protocol-v3.md` — the ADR this issue extends.
* Round 49's reverted commit `9ef319b5` / `f38a32dc` — the encoder-only attempt.
