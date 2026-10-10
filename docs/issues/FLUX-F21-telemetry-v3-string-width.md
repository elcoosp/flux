# FLUX-F21 — telemetry strings are hard-coded u16, not widened by ADR-0059

**Status:** CLOSED (round 63, 2026-10-10) — ADR-0059 amended; both sides widened atomically
**Found:** round 49 (an attempt to widen was reverted because it broke round-trip tests)

## Resolution (round 63)

Design decision: **extend ADR-0059's "String content" rule to telemetry
strings**. Rationale: `NetworkResponse.body` and `PerfRecord.json` are
exactly the "user-plausible input over 64 KB" ADR-0059 was written for — a
real HTTP body over the ceiling was silently truncated before this fix. The
ADR-0059 amendment adds `telemetry::encode_str`/`decode_str` to the String
content list.

Implementation (both sides atomic):

1. `Writer::set_version(self.version)` is called at the top of every
   telemetry frame's `to_bytes` so the emitted prefix width matches the
   frame's own version byte. `Writer::count_prefix` already did the
   version dispatch internally.
2. `Reader::with_version(bytes, version)` is used at every telemetry
   `from_bytes` — the version byte is read from `bytes[4]` before the
   reader is constructed, so the frame's own version threads through the
   entire decode tree.
3. `telemetry::encode_str` now uses `Writer::count_prefix` (u32 at v3,
   u16 at v2) instead of `Writer::u16_len`.
4. `telemetry::decode_str` now uses `Reader::count` (version-dispatched)
   instead of `Reader::u16`.
5. Nested inner-readers inside `TelemetryEvent::decode_from`,
   `EnrichedTelemetryEvent::decode_from`, and `DebugCommandFrame::from_bytes`
   inherit the outer reader's version via the new `Reader::version()`
   accessor, so nested strings read at the same width the encoder wrote.
6. A `> u16::MAX` string regression test
   (`oversize_telemetry_string_round_trips_at_v3`) confirms the whole path.

The prior revert (`9ef319b5` / `f38a32dc`) is superseded by this commit.

---

## Original filing — for context
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
