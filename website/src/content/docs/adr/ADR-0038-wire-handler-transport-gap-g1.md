---
title: "ADR-0028 — Handler transport on the wire (Gap G1)"
---

# ADR-0028 — Handler transport on the wire (Gap G1)

- Status: Accepted
- Date: 2026-08-25
- Scope: `flux-ir-serde` (Rust wire codec)
- Supersedes: none
- Superseded by: none
- Related: Gap G1 in `docs/agents-boundaries-contract.md`, Appendix D §D.8 / §D.12

## Context

The Flux dev server ships a reactive tree to the host app as binary frames
(Appendix D). The tree (`NodeRef` / `Patch`) and the string table moved fine,
but **handler bodies had no transport**: a lowered `ClosureIR` (bytecode +
captured signals, per Appendix C §C.1 / ADR-0014) was never serialized into a
frame. The contract flagged this as "Gap G1" with an explicit instruction:

> `InitFrame`/`DeltaFrame` carry patches + strings only; … `ClosureRef {
> bytecode_offset, bytecode_len }` points into a bytecode blob no frame ships.
> Sequence after FLUX-018 defines `ClosureIR`'s final shape, then a
> `flux-ir-serde` second pass adds a handler section + bytecode blob to
> `Init`/`Delta`. FLUX-019 (devserver) must not hard-code a handler wire shape
> until G1 lands.

FLUX-018 has landed (the `ClosureIR` shape is final), so the second pass is now
safe to implement.

## Decision

Add a **handler section** to both `Init` and `Delta` frames (Appendix D §D.12),
immediately after the string stream:

1. **Shared bytecode blob** — a `u32` byte-length followed by the raw
   little-endian concatenation of every closure's bytecode in this frame. One
   blob per frame; all `ClosureRef`s index into it.
2. **`HandlerDef` stream** (Appendix D §D.8) — a `u16` count followed by
   `HandlerDef` entries, each `HandlerId` + `ClosureRef` (Appendix D §D.7).
   The `ClosureRef.bytecode_offset`/`bytecode_len` are resolved against the
   shared blob at encode time, so the produced `ClosureRef` is self-consistent
   with Appendix D and with what `flux-ir-serde::hash_closure` digests.

A frame with **no** handlers still writes a valid handler section: a
zero-length blob (`u32 = 0`). This keeps the decoder's blob read from
underflowing on every existing (handler-less) frame, so the change is
backward-compatible with the previously-shipped empty frames.

The `D.1` header already reserves `handler_count` at offset 12 (previously
always written as `0`); it now carries the true `HandlerDef` count.

### Public API surface

- `serialize_patches(patches, table, closures: &[ClosureIR]) -> Vec<u8>`
- `deserialize_patches(bytes) -> Result<(Vec<Patch>, Vec<ClosureRef? no — Vec<ClosureIR>), _>` — now returns the frame's handler section alongside the patches.
- `Frame::init(..., closures: &[ClosureIR])` and `Frame::delta(..., closures: &[ClosureIR])` — both gain a `closures` parameter.
- `InitFrame` / `DeltaFrame` gain a `closures: Vec<ClosureIR>` field.

This is a **non-breaking source change within the workspace**: the only
consumers at merge time are `flux-ir-serde`'s own tests/bench and
`serialize_patches` (used by the devserver stub, which does not yet ship
handlers). No production (Swift/Kotlin) deserializer is touched — the wire
layout is additive and matches the spec's reserved `handler_count`.

## Consequences

- Handlers finally have wire transport; the dev server (FLUX-019) can now ship
  `ClosureIR`s without inventing a one-off shape.
- The Swift/Kotlin host `VM`/`FrameDeserializer` already reserve a `HandlerDef`
  decode path per D.8; once FLUX-023 drops real fixtures, the host side can
  decode this section without a protocol-version bump (the `handler_count` slot
  was reserved since D.1).
- `serialize_patches`'s signature changed (added `closures`). That is the only
  API break, and it is contained to this crate; call sites updated in-tree.

## Alternatives considered

- **Per-closure bytecode blob** (one blob per `HandlerDef`): rejected — a shared
  blob lets the host dedup identical bytecode and matches the single-blob model
  the spec's `ClosureRef.bytecode_offset` implies.
- **Inline bytecode in `ClosureRef`** (extend D.7): rejected — D.7 is normative
  and shared with the production VMs; changing it would desync Swift/Kotlin
  without buying anything over a post-string blob.
- **Defer to FLUX-023 parity**: rejected — the contract explicitly gates
  FLUX-019 on G1 landing first, so the transport must exist before the
  devserver consumes it.
