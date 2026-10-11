# FLUX-F59 — `HostAnnounceFrame::from_bytes` ignores the frame's version byte

**Status:** OPEN (fix landed in same commit)
**Found:** round-105 bug hunt (`flux-ir-serde/telemetry.rs`)
**Severity:** medium (a v2 `HostAnnounce` frame — which the wire
protocol still accepts per `PROTOCOL_VERSION_MIN = 2` — fails to
decode because the decoder reads v3 string-length prefixes. The DevTools
session then never learns the host's identity, and the host-picker in
the UI shows no device. Same class as F57.)

## Summary

`crates/flux-ir-serde/src/telemetry.rs`:

* `HostAnnounceFrame::to_bytes` correctly writes the versioned string
  prefix:

      let mut w = Writer::new();
      w.set_version(self.version);        // <- honors the frame's version
      w.u32(MAGIC);
      w.u8(self.version);
      w.u8(FRAME_HOST_ANNOUNCE);
      encode_str(&mut w, &self.platform); // <- version-dispatched
      encode_str(&mut w, &self.device);
      // …

* `HostAnnounceFrame::from_bytes` does **not** honor it:

      let mut r = Reader::new(bytes);     // <- defaults to PROTOCOL_VERSION=3
      let magic = r.u32("host_announce.magic").ok()?;
      // …
      let version = r.u8("host_announce.version").ok()?;
      // …  (version is read, but the reader's own `version` field was set
      //     to PROTOCOL_VERSION in the constructor, and the decoder never
      //     re-sets it)
      let platform = decode_str(&mut r, "host_announce.platform").ok()?;
      //                 ^ reads via `r.count()`, which dispatches on
      //                   `r.version() == PROTOCOL_VERSION (=3)`, NOT on the
      //                   version byte read two lines above.

For a v2 frame, the encoder wrote u16 string-length prefixes and the
decoder reads u32 — the decoder over-reads by 2 bytes per string,
desyncs the reader, and `from_bytes` returns `None`.

Contrast with `EnrichedTelemetryFrame::from_bytes`, which the round-63
fix (F21) updated to build its reader with the frame's own version
byte via `Reader::with_version(bytes, version)`.

## Reproduction

`crates/flux-ir-serde/tests/probe_r105_host_announce.rs` (deleted
after the probe run):

    let mut bytes: Vec<u8> = Vec::new();
    bytes.extend_from_slice(&MAGIC.to_le_bytes());
    bytes.push(2);                              // version 2
    bytes.push(FRAME_HOST_ANNOUNCE);            // kind
    bytes.extend_from_slice(&3u16.to_le_bytes());
    bytes.extend_from_slice(b"ios");
    bytes.extend_from_slice(&10u16.to_le_bytes());
    bytes.extend_from_slice(b"iPhone17,1");
    bytes.extend_from_slice(&0u16.to_le_bytes()); // cap_count = 0

    HostAnnounceFrame::from_bytes(&bytes)  // -> None

## Impact

Every v2 dev server that sends a `HostAnnounce` frame is silently
ignored by the DevTools UI: the host-picker stays empty, and the
"awaiting host…" placeholder is shown indefinitely. v3 (the default)
is unaffected because the reader's default and the frame's version
happen to coincide.

## Fix

Build the reader with the frame's own version byte, matching the
pattern `EnrichedTelemetryFrame::from_bytes` already uses:

    pub fn from_bytes(bytes: &[u8]) -> Option<HostAnnounceFrame> {
        // ADR-0059: read the version byte from the header before
        // constructing the reader, so every `decode_str` inside
        // dispatches on the frame's actual version, not the process's
        // `PROTOCOL_VERSION`.
        let version = *bytes.get(4)?;
        let mut r = Reader::with_version(bytes, version);
        // … rest unchanged …
    }

## Regression tests

`crates/flux-ir-serde/tests/host_announce_v2.rs` (added in this
commit):

* `v2_host_announce_frame_decodes` — the reproduced case above.
* `v3_host_announce_frame_still_decodes` — the default path is
  preserved.

## Related

* **F57** — the same class of bug in `wire/span.rs::encode_str`.
* **F21** — the round-63 fix that introduced `Reader::with_version`
  for `TelemetryFrame`, but missed this sibling decoder.
