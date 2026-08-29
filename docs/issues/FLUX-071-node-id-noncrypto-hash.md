---
id: FLUX-071
status: done
lane: LANE-H
phase: "Phase 0/1"
blocked_by: []
labels:
  - perf
  - ir
  - node-id
  - dx
source: Two independent architecture roasts (2026-08-29). Roast A (perf) claimed blake3 node-ID hashing is a "crypto-blasphemy" wasting CPU on every keystroke. Roast B (correctness) claimed blake3-of-span makes IDs flip on every edit. Both resolved to the same action: swap the cryptographic hash for a deterministic non-crypto one.
related_adrs:
  - ADR-0034
  - ADR-0013
---

# FLUX-071: Replace blake3 with a deterministic non-crypto hash for NodeId derivation

## Problem Statement

`compute_node_id` in `crates/flux-syntax/src/ids.rs:221` folds five fields
(`parent_id`, `tag`, `span.file_id`, `span.start`, `span.end`, and an 8-byte
key sentinel) through `blake3::Hasher` and truncates to `u32`.

Blake3 is a cryptographic, SIMD-optimized hash. For a non-security content
address — node identity across edits — it is the wrong tool:

1. **Throughput.** Blake3 has fixed per-call setup cost (hasher construction +
   finalization) that dominates for the tiny ~21-byte inputs here. A non-crypto
   32-bit hash (FNV-1a-32) reduces `compute_node_id` from ~131 ns to the
   low-tens-of-ns range with identical collision-resistance for this key space.
2. **No security benefit.** Node IDs are not secret, not signed, and not
   untrusted-input-derived in a way that needs a cryptographic primitive. The
   hash only needs to be deterministic and well-distributed across the
   `(parent, tag, span, key)` tuple.
3. **Dependency hygiene.** Blake3 stays a `flux-syntax` dependency only because
   of this one call plus `Value::hash_into`/`hash_fields`. The prop-index wire
   convention already uses FNV-1a-32 (`flux_ir::lower::prop_index_for_name`),
   so unifying on FNV removes a conceptual split between how node IDs and prop
   indices are derived.

### Note on the correctness half of the roast

Roast B's claim that "blake3-of-span makes IDs flip on every edit, so hot-swap
loses state" is **NOT addressed by this issue** and is **not accepted as
framed**. The hash inputs are `(parent, tag, span, key)`; span is the *source
offset* of the node, which is intentionally stable for structure-preserving
edits (sibling insertion / handler-body edit do not shift a node's own span).
The roast's proposed "content-hash the subtree" fix is a lowering-architecture
change (incremental lowering, ADR-0056-area) and is explicitly out of scope
here. This issue is purely the perf/determinism hash swap. It preserves every
existing ID-determinism property (same inputs → same ID) which is the contract
all downstream code (differ, wire, codegen bridge) depends on.

## Solution

1. Add a small, private, dependency-free `fnv1a32(bytes: &[u8]) -> u32` helper in
   `crates/flux-syntax/src/ids.rs` (or a shared `hash.rs` module if other
   `flux-syntax` sites later need it). FNV-1a-32: `hash = 0x811c9dc5;
   for b in bytes { hash ^= b; hash = hash.wrapping_mul(0x01000193); }`.
2. Rewrite `compute_node_id` to feed the same five fields (little-endian, in the
   same order) into `fnv1a32` instead of `blake3::Hasher`, truncate to `u32`
   (already `u32`).
3. Keep `blake3` as a dependency **only** if still used elsewhere in
   `flux-syntax` (`Value::hash_into`/`hash_fields` in `node.rs` use it for
   `props_hash`/`children_hash`). If those remain, blake3 stays; this issue does
   not touch them. If they are the only other user and we choose to keep them,
   fine — the node-ID path is the hot one we are optimizing.
4. Update the doc comment on `compute_node_id` to say "FNV-1a-32" instead of
   "BLAKE3", and keep the structural description of the fold.

### Determinism requirement (load-bearing)

Do **NOT** use `ahash`'s `AHasher` (already in the workspace) — its default
key is randomized per process, so IDs would differ across dev-server restarts
and break the stable-ID contract. FNV-1a-32 is fully deterministic with a fixed
seed (`0x811c9dc5`), matching the prop-index convention. This is the reason the
fix is FNV and not ahasher.

## Implementation Decisions

- Wire safety: hosts never call `compute_node_id` (grep `runtimes/**` returns 0
  hits). Node IDs are produced only in `flux-ir`/`flux-types`/`flux-devserver`
  (server side) and consumed as opaque `u32` over the wire. Swapping the hash
  function is therefore transparent to both hosts — no protocol change, no
  version bump.
- Existing hashes stay "stable" in the only sense that matters: every call site
  that previously got ID X for inputs I now gets the FNV-derived ID X'. All
  toolchain members (differ, dev server, codegen bridge) derive IDs through the
  same single function (`flux_syntax::compute_node_id`), so they stay in sync
  by construction.

## Testing Decisions

- Unit test in `ids.rs` (or existing `flux-ir/src/node_id.rs` tests) asserting:
  - `compute_node_id(0, ExprTag(7), span, None) != compute_node_id(0, DeclTag(7), span, None)`
    (family bit still separates Expr/Decl).
  - identical inputs → identical output (determinism; run twice, assert equal).
  - `(parent, tag, span, key)` all four axes independently affect the output
    (cheap smoke: differing one field yields a different id).
- Existing `flux-ir` node-id tests in `crates/flux-ir/src/node_id.rs` and the
  `flux-parity` B.3 suite must stay green (they exercise the bridge, not
  hardcoded hashes — confirm no test hardcodes a blake3-derived literal; a grep
  found none).
- Re-run the `flux-syntax` `micro` bench `compute_node_id` after the swap and
  record the new ns/call; it should drop materially vs the ~131 ns blake3 baseline.

## Out of Scope

- Incremental / content-addressing of *subtrees* (the roast B "fix"): that is a
  lowering-architecture change, tracked separately if at all.
- `props_hash` / `children_hash` (blake3 in `node.rs`): separate concern; not
  on the hot per-node-ID path in the same way. Left as-is unless a later issue
  targets them.

## Acceptance

- `cargo fmt --check` + `cargo clippy -D warnings` clean in `flux-syntax`.
- `cargo nextest -p flux-syntax -p flux-ir` green.
- `compute_node_id` bench shows a measurable drop (target: < ~50 ns/call).
- AGENTS.md §0.3 dependency table and §3.2 prose updated (blake3 → FNV-1a-32 for
  node IDs) so docs match source.
- ADR-0034 note: the ID-bridge contract is unchanged (one function, same fold
  order); only the primitive differs. No ADR number bump required, but a one-line
  note in ADR-0034 is courteous.
