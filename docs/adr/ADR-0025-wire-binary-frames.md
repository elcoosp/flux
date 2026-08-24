# ADR-0025: Custom binary wire frames (supersedes ADR-0008's MessagePack choice)

- **Status:** Accepted
- **Date:** 2026-08-24
- **Supersedes:** ADR-0008 (MessagePack for wire format)
- **Scope:** `flux-ir-serde` wire codec (`crates/flux-ir-serde/src/{wire,frame,encode}.rs`)

## Context

Appendix D originally specified the wire format as **MessagePack** (ADR-0008,
`mlp-appendices.md` §D). The `flux-ir-serde` implementation that landed as
FLUX-013 instead ships a **custom little-endian binary frame format**
(`Writer`/`Reader` over `to_le_bytes` in `wire.rs`; `InitFrame`/`DeltaFrame`
in `frame.rs`). `rmp-serde` is declared as a dependency in `Cargo.toml` but is
**never called** anywhere in the crate — the format is hand-rolled binary, not
MessagePack.

This deviation is sound for the MLP's localhost transport (1–3 ms round trip,
fixed schema, no need for self-describing encoding) and the implementation is
already complete and tested. However, at the time of writing it was
**undocumented**: `mlp-spec.md` §14.1/§21.1, `mlp-appendices.md` §D, and the
ADR-0008 body still assert MessagePack, violating the Definition of Done §9
(every deviation from the spec requires an ADR).

## Decision

Adopt the **custom little-endian binary frame format** as the normative wire
encoding for the MLP. This **supersedes ADR-0008's MessagePack choice** for the
frame body. The earlier MessagePack decision is recorded as historical context
only.

Corrections applied alongside this ADR:
- `mlp-spec.md` §14.1/§21.1/§18.x framing prose → "custom little-endian binary
  (ADR-0025)".
- `mlp-appendices.md` §D Option-D narrative + Wire-Protocol glossary → binary
  (ADR-0025).
- `docs/agents-boundaries-contract.md` FLUX-013 scope → "custom little-endian
  binary wire layout (ADR-0025)".

`rmp-serde` remains declared in the workspace `Cargo.toml` (frozen manifest,
R2) but is unused; flag to foundation for pruning in a later cleanup pass.

## Consequences

**Positive**
- No third-party MessagePack dependency shipped in host apps (smaller binary,
  no version drift).
- Deterministic, schema-explicit frames; trivial to hand-build for tests.
- Matches the existing `flux-ir-serde` implementation with zero code change.

**Negative**
- Frames are not self-describing; both ends must agree on the exact `to_le_bytes`
  layout (Appendix D remains normative).
- ADR-0008's body in `mlp-appendices.md` is now historically stale; it is left
  in place as a record and explicitly superseded here (existing ADRs are not
  edited per boundary-contract R9).

## Related pending work (not resolved by this ADR)

- **Gap 1 — handler bytecode transport.** `InitFrame`/`DeltaFrame` carry patches
  + strings only; `wire.rs:441 HandlerDef` is `#[allow(dead_code)]` and
  `ClosureRef { bytecode_offset, bytecode_len }` points into a bytecode blob no
  frame ships. The spec's §21.1 frame includes a handler section. → schedule a
  `flux-ir-serde` **second pass** (depends on FLUX-018 defining `ClosureIR`'s
  final shape) to add a handler section + bytecode blob to `Init`/`Delta`.
- **Gap 2 — node-ID derivation single source.** `flux-types` defines its own
  `compute_node_id` (`kind.rs:303`) and `flux-ir` the canonical one
  (`node_id.rs:46`); they differ in signature and live in separate crates.
  FLUX-018 lowering must look up `TypedAST.types[NodeId]` by ID, so the two must
  agree. → orchestrator pass to relocate derivation into `flux-syntax` (R5) with
  a cross-crate proptest; blocks FLUX-018.
- **Verify-only (post-MLP):** FR-014 `Image` adapter + asset pipeline, and
  ADR-0016's on-wire hash-reference dedup (90%+ payload reduction) — `hash_props`
  / `hash_closure` exist but frames ship props inline. Both are out of MLP scope
  unless explicitly pulled in.
