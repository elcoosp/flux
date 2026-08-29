---
id: FLUX-074
status: todo
lane: LANE-H
phase: "Phase 0/2"
blocked_by: []
labels:
  - perf
  - ir
  - lowering
  - hot-reload
  - architecture
source: Roast #2 (2026-08-29), section 1 + "If dev-path perf is actually the goal" #1/#2. The roast argues node IDs hash a span that changes on every edit, and that the dev server re-parses / re-checks / re-lowers the *whole file* every save, wasting the flagship differ on the cheapest millisecond. It proposes content-addressed subtree identity → stable IDs across edits AND incremental lowering (only re-lower dirty subtrees). This is the single biggest unimplemented perf win the roast surfaces.
related_adrs:
  - ADR-0034
  - ADR-0013
  - FLUX-071
---

# FLUX-074: Content-addressed subtree IDs + incremental lowering

## Problem Statement

Today the per-save pipeline is: **re-parse whole file → re-typecheck whole file →
re-lower whole file → diff against last tree → patch**. Two related defects:

1. **Node-ID stability is tied to source spans.** `compute_node_id`
   (`crates/flux-syntax/src/ids.rs`, now FNV-1a-32 after FLUX-071) folds
   `(parent, tag, span, key)`. A node's `span` is its *source offset*, so editing
   text *above* a node — or the node's own text — shifts its span and flips its ID.
   The "minimal edit script" then tears down and rebuilds subtrees that did not
   actually change structurally, which is exactly what kills scroll position,
   input focus, and animation state across a hot reload (the actual product of hot
   reload, per Roast #2 §1). IDs are stable for *structure-preserving* edits
   (sibling insertion, handler-body edit) but not for *text-above* edits.
2. **Whole-file re-lowering.** Because lowering is per-file and opaque to the differ,
   every save re-lowers everything and then diffs; the differ (FLUX-014) is
   optimizing a step that a content-addressed, incremental design would bypass.

Note: Roast #2's premise that IDs "flip on every keystroke" is overstated — IDs are
recomputed once per *save* (server-side), not per keystroke, and `flux-parity` +
the differ already keep re-lowering correct. The real, actionable win is making
IDs **content-addressed at the subtree level** so they survive text-above edits, and
**lowering incrementally** so a one-line change re-lowers one subtree, not the file.

## Solution

### A. Content-addressed subtree identity (fixes state preservation)
Replace the span-based node-ID input with a **canonicalized subtree hash** as the
stability key, while keeping `key` to disambiguate siblings (the roast's exact
suggestion). Concretely:

- Compute a deterministic digest of a node's *structural content*: its tag, its
  prop *values* (not source offsets), its children's IDs (recursively), and its
  `key`. This is already mostly available — `flux-differ` already stores
  `props_hash` and `children_hash` on the arena (`crates/flux-differ/src/diff.rs:
  53`); the node-ID should be derived from those, not from `span`.
- `span` remains useful for *diagnostics* (error messages, "where") but stops being
  the *identity* input. A node whose source moved but whose content is identical
  keeps its ID → its view instance survives the reload (scroll/focus/animation
  preserved).
- The `key` field (already in `compute_node_id`) continues to disambiguate
  positionally-equal siblings (ForEach rows, lists).

### B. Incremental lowering (the big perf win)
Once IDs are content-addressed, the lower passes can skip subtrees whose content
hash is unchanged since the last lower:

- The pipeline keeps the previous lower's per-node content hash (it already has
  `props_hash`/`children_hash` on the arena; persist them per `NodeId`).
- On re-lower, only nodes whose content hash changed (or whose parent changed) are
  re-lowered; unchanged subtrees are reused by ID. The differ then sees a
  near-identical tree and emits a near-empty patch — or the lower can emit the
  minimal patch directly, bypassing the full structural diff for the common case.
- This needs the source to be parsed into a stable per-subtree unit. The grammar
  currently re-parses the whole file; a subtree-keyed parse cache (keyed on the
  file's syntactic boundaries) lets unchanged top-level declarations reuse their
  prior AST. This is the "diff at the source-subtree level before lowering" the
  roast asks for.

## Implementation Decisions

- **Wire-safe.** Hosts never compute node IDs (grep `runtimes/**` for
  `compute_node_id` returns 0 hits); IDs are server/IR-side and consumed as opaque
  `u32` over the wire. Swapping the ID input from `span` to content hash changes
  only which `u32` each node gets — every toolchain member derives through the
  single `flux_syntax::compute_node_id`, so they stay in sync. No protocol bump.
- **Keep `span` for errors.** Do not remove span tracking; keep it on `NodeId`'s
  *metadata* (or a side table) for diagnostics. Only the *hash input* changes.
- **Incremental lowering is the riskier half.** Phase it: land A (content-addressed
  IDs + preserved state across text-above edits) first and measure; land B
  (incremental re-lower) only after A is green and the parity suite confirms no
  structural drift.
- **Determinism:** the content hash must be the same primitive and seed as the
  prop/children hashes already in `flux-differ` (FNV-1a-32 family, consistent with
  FLUX-071's node-ID choice) so the arena hashes and the ID hash agree.

## Testing Decisions

- A test that edits text *above* a `Text` node (shifting its span) and asserts the
  node's `NodeId` is **unchanged** (today it flips). This is the core regression
  guard for A.
- A hot-reload state-preservation test: a host with scroll position / input focus
  on a node survives a text-above edit with the same view instance (requires the
  host to expose instance identity — see ADR-0048 / iOS convergence).
- Incremental lowering: a fixture where editing one declaration re-lowers only that
  subtree — assert the lower's work count (or the emitted patch size) is bounded by
  the changed subtree, not the file.
- `flux-parity` B.3 suite must stay green (dev vs Swift vs Kotlin tree shape
  unchanged — only ID *values* shift, which parity does not compare).

## Out of Scope

- Killing the VM or the codegen path (Roast #2 §3 "kill the VM or the codegen") —
  rejected; the VM enables local, offline, capability-gated handler eval that
  codegen cannot replace for dev, and the differ-on-server "send view ops" idea
  loses the already-local tap latency. Local taps are already handled on-device
  (verified in FLUX-071 context).
- The iOS dev-tier convergence (ADR-0048) is a separate fidelity effort; this
  issue is about ID stability + lowering cost, not dev/prod pixel parity.

## Acceptance

- Editing text above a node no longer flips that node's `NodeId` (test in A).
- A full save on a 1k-node tree where one leaf changed re-lowers only the changed
  subtree (bounded work; measured against the current whole-file baseline).
- `cargo nextest` for `flux-syntax`/`flux-ir`/`flux-parity` green.
- `AGENTS.md` §3.2 updated to describe content-addressed (not span-based) IDs.
