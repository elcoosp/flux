# ADR-0027: Single-source `compute_node_id` in `flux-syntax`

- **Status:** Proposed (created by orchestration agent 2026-08-24; applies Gap G2
  from `docs/agents-boundaries-contract.md` §843)
- **Supersedes:** `docs/adr/types-node-id-hashing.md` (flux-types-local FNV-1a scheme)
- **Related:** AGENTS.md §3.2 (Node IDs), Appendix C §C.1, FLUX-018 (lowering)

## Context

FLUX-018 (lowering) keys a lowered IR node's `NodeId` against
`TypedAST.types: HashMap<NodeId, TypeKind>`. For that lookup to succeed, the
type checker and the IR must derive the **same** `NodeId` from the same source
structure. At the point of discovery there were two independent
`compute_node_id` implementations that did **not** agree:

| Aspect | `flux-ir/src/node_id.rs` | `flux-types/src/kind.rs` |
|---|---|---|
| Algorithm | BLAKE3 (truncated to u32) | FNV-1a (masked to u32) |
| Tuple hashed | `parent, kind.tag(), file_id, start, end, key?` | `parent, kind_tag, start, end, key` |
| `file_id` included? | yes | **no** |
| `key` type | `Option<Key>` (8×`0xFF` sentinel on `None`) | bare `u64` (`None` == `Some(0)`) |

Because the functions disagree, lowering cannot reliably map a produced IR node
back to the type the checker recorded for it — a correctness hole for state
preservation and hot-swap (AGENTS.md §3.2).

The fix (Gap G2) is to relocate the **single** canonical `compute_node_id`
into `flux-syntax` (the only crate every other crate depends on) and have both
existing call sites delegate to it. This makes the *algorithm and field set*
identical across crates; each crate still chooses *what* tag/key it passes,
which is its own concern.

> Note on AGENTS.md §3.2: the prose says `(parent_id, node_kind, source_span,
> optional_key)` and gives `blake3::hash(&(parent_id, kind, span, key))`, while
> the shipped `flux-ir` variant additionally folds in `file_id`. The canonical
> function below includes `file_id` because (a) it is already exercised by every
> existing `flux-ir` test and (b) `file_id` is part of a node's source identity.
> A follow-up orchestrator edit should align the §3.2 wording with the canonical
> implementation. **The implementation in `flux-syntax` is normative; prose
> follows it.**
>
> **FLUX-071 update:** the digest is now **FNV-1a-32**, not blake3. The field
> set and fold order are unchanged (parent, kind_tag, file_id, start, end, key);
> only the primitive swapped from a cryptographic hash to the same deterministic,
> dependency-free FNV-1a-32 used for wire prop indices. This is a perf
> optimization (see FLUX-071) with no wire/contract change — every toolchain
> member derives IDs through the single `flux_syntax::compute_node_id`, so they
> stay in sync by construction.

## Decision

1. Add one canonical function to `flux-syntax`:

   ```rust
   pub fn compute_node_id(
       parent: NodeId,
       kind_tag: u8,
       span: Span,
       key: Option<Key>,
   ) -> NodeId {
       let mut buf = [0u8; 21];
       buf[0..4].copy_from_slice(&parent.to_le_bytes());
       buf[4] = kind_tag;
       buf[5..9].copy_from_slice(&span.file_id.to_le_bytes());
       buf[9..13].copy_from_slice(&span.start.to_le_bytes());
       buf[13..17].copy_from_slice(&span.end.to_le_bytes());
       match key {
           Some(k) => buf[17..25].copy_from_slice(&k.to_le_bytes()),
           None => buf[17..25].copy_from_slice(&[0xFF; 8]),
       };
       fnv1a32(&buf)
   }
   ```

   It takes `kind_tag: u8` (not `NodeKind`) so the type checker can pass its own
   declaration tag without a cast, while the IR passes `kind.tag()`.

2. `flux-ir/src/node_id.rs` becomes `pub use flux_syntax::compute_node_id;`
   (its doctest/examples are updated to refer to `flux_syntax`).

3. `flux-types/src/kind.rs` deletes its FNV-1a `compute_node_id` and calls
   `flux_syntax::compute_node_id(...)`; its `checker.rs` call sites pass `None`
   for the no-key case instead of `0`.

4. A cross-crate property test asserts determinism, parent/kind/span/key
   sensitivity, and that `None` and `Some(0)` are distinct (the FNV bug).

## Consequences

- **Good:** one algorithm, one field set; lowering can key types by `NodeId`
  safely; removes the dead `types-node-id-hashing.md` FNV scheme.
- **Good:** `flux-types` no longer carries an `blake3`-free hash fork; both
  crates share the same digest as the wire protocol and prop hashes. The digest
  itself is FNV-1a-32 (FLUX-071), matching the prop-index convention.
- **Bad (acceptable):** `flux-types` gains a dependency on `flux-syntax`'s
  `compute_node_id` — but `flux-types` already depends on `flux-syntax`, so
  this adds nothing new to the manifest (R2-safe).
- **Action required by orchestrator:** apply the three-file change set below
  (R5 forbids agents from editing `flux-syntax`; R3 forbids editing
  `flux-types`). The change set is self-contained and keeps both crates
  compiling (the `flux-types` edit must land together with its in-progress
  `CalleeShape::Adt { single }` fix so the crate stays green).

## Migration checklist (paste-ready)

### `crates/flux-syntax/src/node_id.rs` (NEW — foundation applies)
```rust
//! Stable node-ID derivation (ADR-0027). Single source of truth for all crates.
use blake3::Hasher;
use crate::ids::{Key, NodeId, Span};

#[must_use]
pub fn compute_node_id(parent: NodeId, kind_tag: u8, span: Span, key: Option<Key>) -> NodeId {
    let mut h = Hasher::new();
    h.update(&parent.to_le_bytes());
    h.update(&[kind_tag]);
    h.update(&span.file_id.to_le_bytes());
    h.update(&span.start.to_le_bytes());
    h.update(&span.end.to_le_bytes());
    match key {
        Some(k) => h.update(&k.to_le_bytes()),
        None => h.update(&[0xFF; 8]),
    };
    let mut d = [0u8; 4];
    d.copy_from_slice(&h.finalize().as_bytes()[..4]);
    u32::from_le_bytes(d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::Span;
    #[test] fn determinism() {
        let s = Span::new(1, 0, 10);
        assert_eq!(compute_node_id(0, 0, s, None), compute_node_id(0, 0, s, None));
    }
    #[test] fn parent_matters() {
        let s = Span::new(1, 0, 10);
        assert_ne!(compute_node_id(0, 0, s, None), compute_node_id(1, 0, s, None));
    }
    #[test] fn kind_matters() {
        let s = Span::new(1, 0, 10);
        assert_ne!(compute_node_id(0, 0, s, None), compute_node_id(0, 1, s, None));
    }
    #[test] fn key_none_differs_from_some_zero() {
        let s = Span::new(1, 0, 10);
        assert_ne!(compute_node_id(0, 0, s, None), compute_node_id(0, 0, s, Some(0)));
    }
    #[test] fn key_value_matters() {
        let s = Span::new(1, 0, 10);
        assert_ne!(compute_node_id(0, 0, s, Some(1)), compute_node_id(0, 0, s, Some(2)));
    }
}
```

### `crates/flux-syntax/src/lib.rs` (foundation adds one re-export)
```rust
pub use node_id::compute_node_id;
mod node_id;
```

### `crates/flux-ir/src/node_id.rs` (ir-core applies — replace body)
```rust
//! Stable node-ID derivation (ADR-0027). Delegates to the canonical
//! `flux_syntax::compute_node_id`; kept as a thin re-export so existing
//! `use flux_ir::compute_node_id` sites keep working.
pub use flux_syntax::compute_node_id;
```

### `crates/flux-types/src/kind.rs` (typechecker applies — replace fn)
```rust
// remove the FNV-1a compute_node_id (lines 295-319) and the NodeTag change below:
use flux_syntax::compute_node_id;

// `NodeTag` stays; `compute_node_id` now resolves to the canonical one.
```

### `crates/flux-types/src/checker.rs` (typechecker applies — 4 call sites)
```rust
// line 86:
let id = compute_node_id(0, tag, span, None);
// lines 846, 917, 921, 925:
compute_node_id(0, crate::kind::NodeTag::decl_tag(decl), span, None),
```

### Remove stale ADR
Delete `docs/adr/types-node-id-hashing.md` (superseded by this ADR).

## Verification

- `cargo nextest run -p flux-syntax -p flux-ir -p flux-types` all green.
- Property test in `flux-syntax` covers the `None`/`Some(0)` distinction that
  the old FNV fork got wrong.
- FLUX-018 may now be dispatched: lowering computes each IR node's `NodeId`
  with `flux_syntax::compute_node_id` and looks it up in `TypedAST.types`.
