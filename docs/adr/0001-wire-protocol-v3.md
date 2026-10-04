# ADR-0001 — Wire Protocol v3: u32 Length Prefixes for User-Authored Collections

- **Status:** Accepted
- **Date:** 2026-10-04
- **Deciders:** Flux maintainers
- **Supersedes:** none
- **Superseded by:** none

## Context

The v2 wire protocol (Appendix D) prefixes every collection and string with a
`u16` length field. Any value the user authors that exceeds 65 535 elements or
bytes therefore cannot be represented. Before the audit that produced this
ADR, the encoder's `u16_len` helper would **panic** the pipeline thread on
such input (audit finding H14). The audit's encoder-hardening pass converted
the panic into a fallible `WireError::LengthExceedsU16`, so the failure is
now a diagnostic rather than a crash — but the underlying limitation remains:
a `.flux` program with a 70 000-element `List` prop, a 70 000-field record, a
70 000-entry state seed, or a single >64 KB string cannot ship to the host.

The audit explicitly called these "user-*plausible*" inputs, not hostile
ones. The MLP's target is a UI language where lists of tens of thousands of
items are normal (a contact list, a chat history, a dashboard feed). This
protocol version lifts the limit for the fields users actually author.

## Decision

Bump `PROTOCOL_VERSION` from `0x02` to `0x03`.

In v3, the following length prefixes are written and read as **u32**
(little-endian) instead of u16. Everything else stays u16.

**Frame-level counts:**
- `InitFrame::state_seed` count
- `InitFrame::source_map` count
- `InitFrame::component_names` count
- `DeltaFrame::patches` count
- `DeltaFrame::closures` count
- `DeltaFrame::strings` count

**Node-level counts:**
- `NodeRef::props` field count
- `NodeRef::children` count
- `NodeRef::handlers` count

**Prop-level counts:**
- `PropDiff::changes` count
- `PropDiff::removals` count
- `Patch::Reorder.keys` count

**Value-level counts:**
- `Value::List` item count
- `Value::Record` field count

**String content:**
- `StringEntry.text` byte length
- `frame::encode_str` / `try_encode_str` byte length

**Why not everything:** spans, closure captures, signal-meta
`deps`/`layout`/`metas`, capability lists, telemetry payloads, and
capability-feature lists are bounded by the source structure or by the
server's own output. 65 535 is far above any realistic ceiling for these, and
widening them would add wire bytes with no semantic benefit.

## Migration and compatibility

- The **encoder** always writes v3. There is no runtime switch; the version
  byte in every frame this binary produces is `0x03`.
- The **decoder** is version-aware. It dispatches on the version byte in the
  header:
  - `0x03`: read `u32` at each of the changed prefix sites.
  - `0x02`: read `u16` and widen to `u32` (the pre-v3 layout).
  - Anything else: fail closed with
    `WireError::InvalidTag { context: "frame.version", .. }`.
- A v2 host pointed at a v3 server fails closed on the first frame (the
  version byte mismatch surfaces before any field decode). This is the
  correct behavior for a dev tool, matching the project's "no v1 host ships
  to users" policy from ADR-0056.
- **Fixtures:** `fixtures/wire/init_v2.bin` and `fixtures/wire/delta_v2.bin`
  remain in tree as regression fixtures for the decoder's v2 fallback path.
  New `fixtures/wire/init_v3.bin` and `fixtures/wire/delta_v3.bin` are the
  current-version goldens. The existing `unsupported-version.bin` (currently
  byte `0x03`) is bumped to `0x04`, since `0x03` is now supported.

## Consequences

**Benefits:**
- A `.flux` program with a 70 000-item list, or any other user-authored
  collection exceeding u16, now encodes without error. The audit's
  "user-plausible input" concern is closed.
- The encoder's `LengthExceedsU16` variant still exists and remains reachable
  — a malicious input can still exceed `u32` — but the threshold has moved
  from "65 k items" to "4 billion items", a value no real program reaches.

**Costs:**
- Wire size: 2 extra bytes per changed prefix write. A frame with 100 strings
  and 100 patches adds ~400 bytes; negligible against the payloads that ship.
- Complexity: the decoder dispatches on version at each changed prefix site.
  This is contained in `flux-ir-serde::wire` and mirrored in the Swift and
  Kotlin decoders. The pattern is mechanical.
- Every host app must be rebuilt. This is a development tool; there are no
  user-shipped binaries at v2.

## Alternatives considered

1. **Keep v2, switch the offending fields to varint.** Rejected: introduces
   a new encoding primitive to maintain across three languages for a saving
   that does not matter (the frames carrying these fields are already large;
   a few extra bytes are noise).

2. **Keep v2, add a per-field "extended length" flag.** Rejected: doubles
   the decision points in the codec for no gain over a clean version bump.

3. **Keep v2, make the encoder drop oversized collections.** Rejected:
   silently loses user data. The current fallible-encoder path permits a
   typed error; sending a truncated list would be worse.

4. **Skip to v4 and reserve v3 for a future incompatible change.** Rejected:
   there is no other pending incompatible change to bundle; version numbers
   are scarce only if we invent future uses for them. Bumping to 3 and
   following the standard migration path is cleaner.

## References

- Codebase deep-dive audit (this repo): §3.3, §3.4 (u16_len panics on encode).
- Appendix D §D.1–§D.12 (wire format layout).
- ADR-0056 (fail-closed protocol version negotiation).
