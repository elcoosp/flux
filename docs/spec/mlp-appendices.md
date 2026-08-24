# Flux — Appendices

| Field | Value |
|-------|-------|
| Project | Flux — Native Cross-Platform UI Development System |
| Document | Appendices (A through G) |
| Version | 0.1.0 (Draft) |
| Date | 2025-01-18 |
| Author | Architecture Team (assisted by AI) |
| Status | Draft — Pending Review |
| Scope | Reference material for the MLP specification suite |

---

## Table of Contents

- [Appendix A — Architecture Decision Records](#appendix-a--architecture-decision-records)
- [Appendix B — .flux Grammar Reference](#appendix-b--flux-grammar-reference)
- [Appendix C — IR Schema Reference](#appendix-c--ir-schema-reference)
- [Appendix D — Wire Protocol Reference](#appendix-d--wire-protocol-reference)
- [Appendix E — VM Instruction Set Reference](#appendix-e--vm-instruction-set-reference)
- [Appendix F — Adapter Contract Reference](#appendix-f--adapter-contract-reference)
- [Appendix G — Glossary](#appendix-g--glossary)

---

## Appendix A — Architecture Decision Records

### ADR-0001: Binary hot-swap over WebSocket

**Status:** Accepted  
**Date:** 2025-01-18  
**Decision Drivers:** ASR-001 (sub-100ms save-to-pixels)

#### Context and Problem Statement

The dev server must ship reactive tree patches to the host app with minimal latency. The transport must be bidirectional (host sends dispatch events back to the dev server for capabilities in dev mode). The host app is a real iOS simulator or Android emulator process.

#### Considered Options

**Option A — HTTP polling.** Host polls dev server every 50 ms for new patches.
- Pros: Simple, no persistent connection.
- Cons: 50 ms average latency floor. Wasted polls when no changes. No push for dispatch events.

**Option B — gRPC streaming.** Use gRPC bidirectional streaming.
- Pros: Typed messages, codegen for both platforms.
- Cons: Requires protobuf compilation. gRPC on iOS requires gRPC-Swift (heavy dependency). Overkill for localhost dev.

**Option C — WebSocket with JSON frames.** Persistent connection, JSON-encoded patches.
- Pros: Trivial to implement. Built-in support on both platforms (`URLSessionWebSocketTask` on iOS, OkHttp on Android).
- Cons: JSON is 5–20× larger than binary. Parsing cost on host. String keys everywhere.

**Option D — WebSocket with custom binary frames (MessagePack).**
- Pros: Persistent, bidirectional, minimal payload, zero-copy deserialization possible.
- Cons: Custom protocol must define its own versioning. No built-in schema validation.

#### Decision Outcome

**Chosen: Option D — WebSocket with custom binary frames (MessagePack).**

Locality is localhost (1–3 ms round trip). The wire format is MessagePack with content addressing. Protocol versioning is explicit in the handshake frame.

#### Consequences

**Positive:**
- Sub-millisecond wire time for typical patches.
- Bidirectional — host can send dispatch events back.
- No heavy dependencies on either platform.

**Negative:**
- Custom protocol must be versioned manually (see §21.4 of main spec).
- MessagePack encoder/decoder must be shipped in the host app (~5 KB Swift, ~8 KB Kotlin).

**Neutral:**
- The WebSocket connection is localhost-only in the primary case; device testing over LAN adds 2–5 ms.

---

### ADR-0002: Embedded VM in host app (host-authoritative state)

**Status:** Accepted  
**Date:** 2025-01-18  
**Decision Drivers:** ASR-001 (sub-100ms), ASR-002 (state preservation)

#### Context and Problem Statement

Handler evaluation can happen server-side (Design 1: dev server evaluates, sends patches back) or host-side (Design 2: host has embedded VM, evaluates locally). The choice affects tap latency, host complexity, and state ownership.

#### Considered Options

**Option A — Server-authoritative (Design 1).**
- All state cells live in the Rust dev server.
- Host is a dumb renderer: receives patches, dispatches events back to server.
- Tap → server evaluates → produces patches → ships back.
- Round trip on localhost: 2–4 ms. Acceptable for taps.
- Pros: Simple host (no VM, no signal graph). State persists across host crashes.
- Cons: Round trip on every tap. Bad for continuous gestures (60 Hz drag = 60 round trips/sec).

**Option B — Host-authoritative (Design 2).**
- State cells and closure table live in the host.
- Host has embedded register-based bytecode VM.
- Tap → local evaluation → local signal propagation → local native mutation.
- No round trip on tap.
- Pros: < 8 ms tap-to-state-change. Handles gestures well.
- Cons: Host has interpreter (2k LOC Swift/Kotlin). State lost on host crash (acceptable for dev).

#### Decision Outcome

**Chosen: Option B — Host-authoritative.**

For the MLP, tap latency must be imperceptible. < 8 ms tap-to-state-change is only achievable with local evaluation. Gestures (deferred beyond tap for MLP) would be impossible with server-authoritative design.

#### Consequences

**Positive:**
- < 8 ms tap-to-state-change on mid-range devices.
- No server round trip on tap. Server is free to parse/diff next edit.
- Scales to gestures without redesign.

**Negative:**
- Host app contains a VM (2k LOC Swift, 2k LOC Kotlin).
- State is lost on host crash. Mitigated by reconnect protocol (dev server re-sends `Init` frame with state seed from its own signal graph snapshot — but in host-authoritative mode, the dev server doesn't have state. So state IS lost on host crash. Acceptable for dev.)

**Neutral:**
- The dev server still maintains the IR (source of truth for the tree structure). The host maintains the signal graph (source of truth for state values). These are separate concerns.

---

### ADR-0003: Delegate layout to native platforms

**Status:** Accepted  
**Date:** 2025-01-18  
**Decision Drivers:** ASR-002 (native release output)

#### Context and Problem Statement

Layout can be computed by a custom engine (Flutter-style) or delegated to platform-native layout systems (SwiftUI's layout protocol, Compose's layout DSL). The choice affects native feel, dev/release parity, and codebase size.

#### Considered Options

**Option A — Own layout engine (Flexbox-like).**
- Rust computes positions. Host receives absolute positions.
- Pros: Perfect dev/release parity (one engine). Full control.
- Cons: Loses native feel (Material spacing, Cupertino margins, dynamic type, accessibility layout). Reinvents Flutter. ~10k LOC layout engine.

**Option B — Delegate to native.**
- Dev executor drives native `UIStackView`/`LinearLayout`. Release codegen emits `VStack`/`Column`.
- Pros: Native feel. Smaller codebase.
- Cons: Two reconciliation systems (dev executor + SwiftUI/Compose) may produce different layouts in edge cases.

**Option C — Hybrid.**
- Own engine for leaf layout (text, image). Native for structural layout.
- Pros: Best of both in theory.
- Cons: Worst of both in practice. Two layout models to maintain.

#### Decision Outcome

**Chosen: Option B — Delegate to native.**

The competitive wedge is "native feel in release." If we build our own layout engine, we've reinvented Flutter with extra steps. The cost is dev/release layout *may* diverge in edge cases; we mitigate by constraining the layout DSL to patterns that map cleanly to both platforms and instrumenting the dev executor to match native semantics.

#### Consequences

**Positive:**
- Native feel in release (Material, Cupertino, dynamic type).
- Smaller codebase (no layout engine).
- Platform accessibility features work out of the box.

**Negative:**
- Dev/release layout may diverge in edge cases (e.g., text wrapping at different breakpoints).
- Must constrain layout DSL to patterns that map to both platforms.

---

### ADR-0004: Individual styling props, not chained modifiers

**Status:** Accepted  
**Date:** 2025-01-18  
**Decision Drivers:** ASR-006 (dev/release parity)

#### Context and Problem Statement

SwiftUI uses chained view modifiers (`Text("hi").bold().italic().padding(8)`). Compose uses `Modifier` chains (`Modifier.bold().italic().padding(8.dp)`). Order matters differently on each platform. The IR must abstract this in a way that codegen produces correct output on both.

#### Considered Options

**Option A — Chained modifiers in `.flux`.**
- `Text("hi").bold().italic().padding(8)` in source.
- Pros: Familiar to SwiftUI developers. Expressive.
- Cons: Requires a modifier-application IR node type. Order-sensitivity creates dev/release parity bugs when platforms handle order differently. Parser is more complex.

**Option B — Individual styling props.**
- `Text("hi") { font: Font.bold, padding: 8 }` in source.
- Pros: Flat prop map per node. No order-sensitivity. Codegen translates flat props to platform-specific chains. Simpler IR.
- Cons: Less expressive. Can't express "padding then bold" vs "bold then padding" (but this is rarely needed and platform-dependent anyway).

**Option C — Modifier list prop.**
- `Text("hi") { modifiers: [Bold, Italic, Padding(8)] }` in source.
- Pros: Explicit ordering.
- Cons: Still requires order-sensitivity in codegen. More complex than Option B.

#### Decision Outcome

**Chosen: Option B — Individual styling props.**

Flat prop map per node. The codegen translates flat props to platform-specific modifier chains with a documented, deterministic order per adapter.

#### Consequences

**Positive:**
- No order-sensitivity bugs.
- Simpler IR (no modifier-application node type).
- Deterministic codegen.

**Negative:**
- Less expressive than chained modifiers (can't express modifier ordering).
- Some SwiftUI patterns (e.g., `.background(Color.red.opacity(0.5).blur(radius: 4))`) don't map cleanly. These are rare and handled via capabilities or custom adapters.

---

### ADR-0005: Monomorphization for dev bytecode

**Status:** Accepted  
**Date:** 2025-01-18  
**Decision Drivers:** ASR-007 (static types with generics)

#### Context and Problem Statement

Generics in source become either type-erased (slow, tag checks at runtime) or specialized (fast, code bloat) in bytecode.

#### Considered Options

**Option A — Type-erased bytecode.**
- All values are tagged unions. Generic functions use vtable dispatch.
- Pros: No code bloat. Simple VM.
- Cons: 5–10× slower arithmetic. Tag checks on every operation.

**Option B — Monomorphized bytecode.**
- Specialize per concrete type instantiation. `ADD_I64`, `ADD_F64` instead of generic `ADD`.
- Pros: No runtime type dispatch. Fast arithmetic.
- Cons: Code bloat. Must track all instantiations.

**Option C — JIT compilation.**
- Compile bytecode to native at runtime.
- Pros: Best of both worlds.
- Cons: Way too complex for MLP. Security concerns (W^X). Platform restrictions (iOS W^X).

#### Decision Outcome

**Chosen: Option B — Monomorphization.**

At type-check time, record every generic instantiation. Lowering pass emits specialized bytecode per instantiation. Cap at 100 specializations per generic; fall back to type-erased beyond.

#### Consequences

**Positive:**
- No runtime type dispatch. VM is as fast as native for arithmetic-heavy handlers.
- Codegen for release also benefits (Swift generics specialize at compile time; Kotlin can use `inline fun <reified T>`).

**Negative:**
- Code bloat: ~1 KB bytecode per specialization, ~50 KB typical total. Acceptable.
- Must track all instantiations during type checking. Adds complexity to the checker.

---

### ADR-0006: Static types with bidirectional checking

**Status:** Accepted  
**Date:** 2025-01-18  
**Decision Drivers:** ASR-005 (LLM-friendly syntax), ASR-007 (static types with generics)

#### Context and Problem Statement

The type system can be dynamic (no static types), full Hindley-Milner (global inference), or bidirectional (explicit where it matters, inferred where it doesn't).

#### Considered Options

**Option A — Dynamic typing.**
- `let x = 5` (x is `any`).
- Pros: Simple parser, simple VM.
- Cons: 5–10× slower VM (tag checks). Codegen emits reflection-heavy code. Poor LLM ergonomics (ambiguous).

**Option B — Full Hindley-Milner inference.**
- Global type inference. No annotations needed.
- Pros: Maximum expressiveness.
- Cons: Poor error messages ("cannot unify X with Y" three frames deep). Hard for LLMs to predict types. Complex checker.

**Option C — Bidirectional checking.**
- Explicit annotations on function/component signatures. Inference on local `let` bindings.
- Pros: Good error messages (at the annotation). LLM-friendly (explicit types are self-documenting). Familiar (TypeScript, Rust).
- Cons: Slightly more verbose than full inference.

#### Decision Outcome

**Chosen: Option C — Bidirectional checking.**

Explicit annotations required on component and function signatures. Local `let` bindings inferred. Generic instantiations inferred at call sites.

#### Consequences

**Positive:**
- LLM-friendly: explicit types are self-documenting and predictable.
- Good error messages at the annotation site.
- Maps cleanly to Swift and Kotlin codegen (both require explicit types on signatures).

**Negative:**
- Slightly more verbose than full inference.
- Users must write type annotations on every component and function.

---

### ADR-0007: Register-based bytecode VM (not stack-based)

**Status:** Accepted  
**Date:** 2025-01-18  
**Decision Drivers:** ASR-001 (sub-100ms)

#### Context and Problem Statement

The embedded VM can be stack-based (like JVM, CPython) or register-based (like LuaJIT, Dalvik). The choice affects instruction count, dispatch overhead, and VM code size.

#### Considered Options

**Option A — Stack-based VM.**
- Operands on a stack. `PUSH 1; PUSH 2; ADD`.
- Pros: Smaller bytecode (no register operands).
- Cons: 1.5–2× more instructions than register-based for the same computation. More dispatch overhead.

**Option B — Register-based VM.**
- Operands in named registers. `ADD r1, r2, r3`.
- Pros: Fewer instructions. Faster dispatch. Better for monomorphized arithmetic.
- Cons: Slightly larger bytecode (register operands per instruction).

#### Decision Outcome

**Chosen: Option B — Register-based VM.**

16 registers. 1-byte opcode + 1-byte register args + variable immediates. Average instruction = 3 bytes.

#### Consequences

**Positive:**
- 1.5–2× faster than stack-based for typical handler bodies.
- Fits 5 instructions per cache line (15 bytes vs 16-byte line).

**Negative:**
- Slightly larger bytecode than stack-based (offset by fewer instructions).

---

### ADR-0008: MessagePack for wire format

**Status:** Accepted  
**Date:** 2025-01-18  
**Decision Drivers:** ASR-001 (sub-100ms)

#### Context and Problem Statement

The wire format between dev server and host app must be compact, fast to serialize/deserialize, and supported on both iOS and Android.

#### Considered Options

**Option A — JSON.**
- Pros: Universal, trivial to debug.
- Cons: 5–20× larger than binary. String parsing cost. No content addressing.

**Option B — Protocol Buffers (protobuf).**
- Pros: Typed, codegen for both platforms.
- Cons: Requires `.proto` schema compilation. Overkill for localhost dev. Larger runtime dependency.

**Option C — FlatBuffers.**
- Pros: Zero-copy deserialization.
- Cons: Schema-driven (same issue as protobuf). More complex than needed.

**Option D — MessagePack.**
- Pros: Schemaless, compact, fast. Libraries available for Swift (MessagePack-Swift) and Kotlin (msgpack-core). Content addressing can be layered on top.
- Cons: Slightly less compact than protobuf for large schemas.

#### Decision Outcome

**Chosen: Option D — MessagePack.**

Empirical comparison shows MessagePack excels and beats FlatBuffers and NanoPB in performance tests. Content addressing is layered on top: props, closures, and IR nodes are interned by BLAKE3 hash. Wire protocol ships hashes for already-cached entries.

#### Consequences

**Positive:**
- 5–20× smaller than JSON.
- Schemaless — no `.proto` files to maintain.
- Content addressing reduces typical payloads by 90%+.

**Negative:**
- Custom content-addressing layer on top of MessagePack.
- MessagePack libraries must be shipped in host app (~5 KB Swift, ~8 KB Kotlin).

---

### ADR-0009: Arena allocation for dev server IR

**Status:** Accepted  
**Date:** 2025-01-18  
**Decision Drivers:** ASR-001 (sub-100ms)

#### Context and Problem Statement

The dev server's IR can be heap-allocated (each node is a `Box<Node>`) or arena-allocated (all nodes packed into a single `Vec<u8>` blob with offsets).

#### Considered Options

**Option A — Heap-allocated (array-of-structs).**
- `Vec<Box<Node>>`.
- Pros: Simple. Easy to modify in place.
- Cons: Cache-unfriendly (nodes scattered across heap). Serialization requires walking pointers. Diff scanning is slow (cache misses on every node access).

**Option B — Arena-allocated (struct-of-arrays).**
- `IRArena { ids: Vec<u32>, kinds: Vec<u8>, props_offsets: Vec<u32>, ... }`.
- Pros: Cache-linear diff scanning. Serialization is just writing the blob. 3–5× faster diffing.
- Cons: More complex to implement. Nodes are immutable once packed (must rebuild arena on every edit).

#### Decision Outcome

**Chosen: Option B — Arena-allocated (struct-of-arrays).**

The dev server rebuilds the arena on every parse. This is O(n) where n is the number of nodes — acceptable for 50–500 node screens.

#### Consequences

**Positive:**
- 3–5× faster diffing (cache-linear scans over `ids` and `kinds`).
- Serialization is trivial (write the blob).
- Memory locality benefits all passes (type checking, lowering, differ).

**Negative:**
- Arena is immutable once packed. Must rebuild on every edit. O(n) rebuild is acceptable.
- More complex implementation than `Vec<Box<Node>>`.

---

### ADR-0010: Signal graph topological propagation

**Status:** Accepted  
**Date:** 2025-01-18  
**Decision Drivers:** ASR-001 (sub-100ms)

#### Context and Problem Statement

When a signal changes, dependent derived cells must recompute. The propagation can be topological-order (precomputed order) or mark-sweep (mark dirty, walk all, recompute).

#### Considered Options

**Option A — Mark-sweep.**
- Mark all dirty cells. Walk all cells. Recompute dirty ones.
- Pros: Simple. Handles dynamic dependency changes.
- Cons: O(n) per propagation even if only 1 cell changed.

**Option B — Topological order.**
- Precompute topological order on graph construction. On signal change, walk dependents in topo order.
- Pros: O(diff) per propagation. Only affected cells recompute.
- Cons: Must recompute topo order when graph structure changes (component mount/unmount).

#### Decision Outcome

**Chosen: Option B — Topological order.**

Topological order is computed once when the component instance is created (its reactive graph is set up). On signal change, walk dependents in topo order. Each derived cell recomputes; if value unchanged, stop propagation (natural memoization).

#### Consequences

**Positive:**
- O(diff) per propagation. Only affected cells recompute.
- Natural memoization (if derived value unchanged, downstream doesn't update).

**Negative:**
- Must recompute topo order when graph structure changes (rare — only on component mount/unmount).

---

### ADR-0011: Keyed reconciliation (udomdiff-style)

**Status:** Accepted  
**Date:** 2025-01-18  
**Decision Drivers:** ASR-001 (sub-100ms), ASR-003 (state preservation)

#### Context and Problem Statement

When the IR tree changes, the reconciler must diff old children vs new children and produce minimal native view mutations. The algorithm can be general tree edit distance (Zhang-Shasha) or keyed reconciliation (React/Solid-style).

#### Considered Options

**Option A — Zhang-Shasha tree edit distance.**
- Academic algorithm. Computes minimum edit distance between two trees.
- Pros: Optimal diff.
- Cons: O(n³) or O(n²). Way too slow for 50+ node trees. Overkill for UI (we have stable IDs).

**Option B — Keyed reconciliation (React/Solid-style).**
- Each node has a stable ID. Diff by matching IDs. Insert/remove/move based on ID presence and position.
- Pros: O(n). Well-understood. Handles common UI patterns (list reorder, insert, remove) efficiently.
- Cons: Requires stable IDs (which we have — see ADR on stable node IDs).

#### Decision Outcome

**Chosen: Option B — Keyed reconciliation (udomdiff-style).**

Use a variation of the algorithm SolidJS borrowed from udomdiff. O(n) for common cases. Optimized for real-world list manipulations.

#### Consequences

**Positive:**
- O(n) diffing. Sub-millisecond for 50-node trees.
- Handles list reorders without destroying/recreating nodes (preserves state).

**Negative:**
- Requires stable node IDs (see ADR-0013 for ID derivation).

---

### ADR-0012: Callbacks for async (not async/await) in MLP

**Status:** Accepted  
**Date:** 2025-01-18  
**Decision Drivers:** MLP scope constraint

#### Context and Problem Statement

Capabilities like `Camera.capture()` are async (return a value later). The VM is synchronous. How do we bridge?

#### Considered Options

**Option A — Callbacks.**
- `Camera.capture(on_success: fn(Data) { ... })`.
- VM continues immediately. Callback runs later.
- Pros: Simple. No continuation machinery in VM.
- Cons: Nested callbacks (callback hell) for complex flows.

**Option B — Promises/futures.**
- `let photo = await Camera.capture()`.
- VM has a continuation stack.
- Pros: Linear code. No callback nesting.
- Cons: Requires continuation capture in VM. Complex. Real feature.

**Option C — Async/await with explicit marks.**
- Handlers can be `async fn`.
- Pros: Most ergonomic.
- Cons: Even more complex than Option B. Full async runtime.

#### Decision Outcome

**Chosen: Option A — Callbacks for MLP.**

Simplest. No VM changes needed. Async capabilities register a callback (a `HandlerId`); when the capability completes, the host calls `executor.dispatch(callback_id, payload)`.

**Defer async/await to MLP v2.** It requires continuation capture in the VM, which is a real feature with real complexity.

#### Consequences

**Positive:**
- No VM changes for async. VM stays synchronous.
- Simple mental model for MLP.

**Negative:**
- Callback nesting for complex async flows. Acceptable for MLP scope.
- LLMs may produce more verbose code with callbacks than with async/await. Mitigated by `resource` primitive for the common data-fetching pattern.

---

### ADR-0013: Stable node IDs derived from source structure

**Status:** Accepted  
**Date:** 2025-01-18  
**Decision Drivers:** ASR-003 (state preservation)

#### Context and Problem Statement

For keyed reconciliation and state preservation, nodes need stable IDs that persist across edits. Sequential IDs shift on every edit. How are IDs derived?

#### Considered Options

**Option A — Sequential IDs.**
- Assign IDs in tree construction order.
- Pros: Simple.
- Cons: Every edit after the change point shifts all IDs. Entire tree diffs. State lost.

**Option B — Source-span-derived IDs.**
- `id(node) = hash(parent_id, node_kind, source_span, optional_key)`.
- Pros: Editing a handler body changes no node IDs. Inserting a sibling doesn't shift sibling IDs. State preserved.
- Cons: Source spans must be stable across edits (file path + byte range). Renaming a file changes all spans (state lost — acceptable).

**Option C — Content-addressed IDs.**
- `id(node) = hash(node_content)`.
- Pros: Content-addressed. Naturally deduplicates.
- Cons: Two identical nodes get the same ID. Breaks parent-child relationships. Can't distinguish two `Text("hello")` in different positions.

#### Decision Outcome

**Chosen: Option B — Source-span-derived IDs.**

ID = hash of (parent_id, node_kind, source_span, optional_key). Source span is (file_id, start_byte, end_byte). For `ForEach` children, the key is the iteration key (React-style `key`).

#### Consequences

**Positive:**
- Handler body edits change no node IDs (handler is referenced by ID from the node; the handler's content changes, but the node containing it doesn't move).
- Inserting a sibling doesn't shift other siblings' IDs (keyed by source span, not sibling index).
- State preservation works: same component at same source position = same ID = same state.

**Negative:**
- Source spans must be tracked precisely. File renames lose state (acceptable).
- `ForEach` items need explicit keys for stability across reorders.

---

### ADR-0014: Handler closures capture signal IDs (not values)

**Status:** Accepted  
**Date:** 2025-01-18  
**Decision Drivers:** ASR-003 (state preservation)

#### Context and Problem Statement

A handler `on click { count = count + 1 }` references `count` from the enclosing component. How is `count` captured in the closure?

#### Considered Options

**Option A — By value.**
- Copy `count`'s value into the closure at capture time.
- Pros: Simple.
- Cons: Updates don't propagate back. Can't modify state. Broken for handlers.

**Option B — By reference (signal ID).**
- Capture a pointer to the signal cell. The closure holds `SignalId`.
- Pros: Updates propagate. Closure is tiny (just a list of signal IDs + bytecode).
- Cons: Closure depends on signal graph being alive. Must free closures when component is destroyed.

**Option C — By name (string lookup).**
- Closure holds the string name "count". VM looks up signal by name at dispatch time.
- Pros: Simple.
- Cons: String lookup on every dispatch. Slow. No interning benefit.

#### Decision Outcome

**Chosen: Option B — By reference (signal ID).**

Handlers don't close over values — they close over signal IDs. When the VM evaluates `count + 1`, it looks up the signal by ID, reads the current value, and writes back to the same signal ID.

Closures are tiny: a list of `SignalId`s they reference + the bytecode. No environment capture needed.

#### Consequences

**Positive:**
- Closures are tiny (a few `u32` IDs + bytecode).
- Hot-swapping a handler = replacing its closure entry in the table. No view mutation.
- No environment objects to manage.

**Negative:**
- Closure depends on signal graph being alive. Must free closures when component is destroyed (tracked by `ComponentInstance`).

---

### ADR-0015: Gas meter + memory cap for VM safety

**Status:** Accepted  
**Date:** 2025-01-18  
**Decision Drivers:** ASR-004 (VM must not crash host app)

#### Context and Problem Statement

A handler that loops forever (`while true {}`) freezes the app. A handler that builds a 10 GB list OOMs the device. The VM must protect the host app.

#### Considered Options

**Option A — No protection.**
- Pros: Simplest VM.
- Cons: A typo or bad edit can hang the app. Dev iteration speed dies.

**Option B — Gas meter (instruction budget).**
- Each instruction costs 1 gas. Dispatch has a budget (e.g., 100k). On exhaustion, raise error.
- Pros: Catches infinite loops.
- Cons: Doesn't catch memory exhaustion.

**Option C — Gas meter + memory cap.**
- Gas meter for instructions. Fixed memory pool for allocations.
- Pros: Catches both infinite loops and OOM.
- Cons: Slightly more complex VM.

#### Decision Outcome

**Chosen: Option C — Gas meter + memory cap.**

- Gas budget: 100,000 instructions per dispatch.
- Memory pool: 16 MB per component instance.
- On either exhaustion, raise `GasExhausted` or `MemoryExhausted` error.
- Errors caught by top-level `try/catch` in `executor.dispatch()`.
- Error reported as red banner on device. Previous good tree stays visible.

#### Consequences

**Positive:**
- Host app never hangs or OOMs due to a `.flux` source error.
- Dev iteration continues even after a bad edit.

**Negative:**
- Legitimate long-running handlers (e.g., processing a large list) may hit the gas limit. Gas budget is configurable and can be raised. Default is generous (100k instructions ≈ 1 ms of VM time).

---

### ADR-0016: Content addressing for props and closures

**Status:** Accepted  
**Date:** 2025-01-18  
**Decision Drivers:** ASR-001 (sub-100ms)

#### Context and Problem Statement

Props and closures are serialized on every patch. Most patches reuse the same props (e.g., `Text` with text "hello" appears in many places). Without interning, the wire protocol ships redundant data.

#### Considered Options

**Option A — No interning.**
- Ship full props on every patch.
- Pros: Simple.
- Cons: 5–20× larger payloads. Slower deserialization.

**Option B — Content addressing (BLAKE3 hash).**
- Props, closures, and IR nodes are interned by BLAKE3 hash. Wire protocol ships hashes for already-cached entries. Host caches `Hash → Value`.
- Pros: 90%+ smaller payloads after initial tree. Fast deserialization (hash lookup).
- Cons: Must maintain hash table on both sides. Slight complexity.

#### Decision Outcome

**Chosen: Option B — Content addressing (BLAKE3 hash).**

After the initial `Init` frame, typical patches are 90%+ hash references. A typical handler-body-change patch is < 500 bytes.

#### Consequences

**Positive:**
- 90%+ smaller payloads after initial tree.
- Hash compare is O(1) for equality checks.
- Natural deduplication.

**Negative:**
- Must maintain hash table on both sides.
- BLAKE3 must be shipped in host app (~10 KB Swift, ~15 KB Kotlin). Acceptable.

---

### ADR-0017: String interning with u32 IDs

**Status:** Accepted  
**Date:** 2025-01-18  
**Decision Drivers:** ASR-001 (sub-100ms)

#### Context and Problem Statement

Strings are everywhere in UI (text content, prop names, handler IDs, capability names). Without interning, every patch ships strings repeatedly.

#### Considered Options

**Option A — No interning.**
- Ship strings inline.
- Pros: Simple.
- Cons: 5–10× larger payloads. String comparison is O(n).

**Option B — String interning (dev server maintains table).**
- Dev server maintains `HashMap<String, u32>`. All IR references use string IDs (u32). Host has the same table (populated on `Init`, updated via `StringTable` patches).
- Pros: 5–10× smaller payloads. String comparison is `u32` compare.
- Cons: Must maintain string table on both sides.

#### Decision Outcome

**Chosen: Option B — String interning.**

Dev server maintains a string table. Host has a mirror. On `Init`, the full table is shipped. On subsequent frames, only new strings are shipped (as part of the frame's `string_table_delta`).

#### Consequences

**Positive:**
- 5–10× smaller payloads.
- String comparison is `u32` compare (used in prop name lookup, handler ID matching).

**Negative:**
- Must maintain string table on both sides.
- String table grows over a dev session (mitigated by: table is reset on `Init` frame / reconnect).

---

### ADR-0018: Prop field access by index (not name)

**Status:** Accepted  
**Date:** 2025-01-18  
**Decision Drivers:** ASR-001 (sub-100ms)

#### Context and Problem Statement

Records/objects in props can be accessed by name (hash lookup) or by index (direct array access). The choice affects VM speed.

#### Considered Options

**Option A — By name (hash lookup).**
- `GET_FIELD reg, "text"`.
- Pros: Readable bytecode.
- Cons: Hash lookup on every prop read. 10–50× slower than index.

**Option B — By index.**
- Dev server assigns each prop name an index (via string table). `GET_FIELD reg, obj, 3`.
- Pros: O(1) field access.
- Cons: Less readable bytecode (but bytecode is not human-readable anyway).

#### Decision Outcome

**Chosen: Option B — By index.**

Dev server assigns each prop name an index. VM's `GET_FIELD` instruction takes `(obj_reg, prop_idx) -> val_reg`. O(1) field access.

#### Consequences

**Positive:**
- O(1) prop field access. No hash lookups in the VM.

**Negative:**
- Prop indices must be stable across edits. If a new prop is added, existing indices don't change (new prop gets next index). If a prop is removed, its index is retired (not reused).

---

### ADR-0019: Persistent (immutable) lists

**Status:** Accepted  
**Date:** 2025-01-18  
**Decision Drivers:** ASR-003 (state preservation)

#### Context and Problem Statement

Lists in the VM can be mutable (in-place modification) or persistent (immutable, structurally shared). The choice affects signal tracking and code complexity.

#### Considered Options

**Option A — Mutable lists.**
- In-place modification.
- Pros: Less allocation.
- Cons: Complicates signal tracking (does mutation count as a write? Need explicit `notify()` calls). Hard to diff.

**Option B — Persistent lists (Clojure-style).**
- Immutable. Structurally shared. New list = new value.
- Pros: Signal tracking is simple (new list = new value = signal write). O(1) prepend. Clean diffing.
- Cons: More allocation for large updates. Must implement structural sharing.

#### Decision Outcome

**Chosen: Option B — Persistent lists.**

Immutable, structurally shared, O(1) prepend. Signal tracking works because a new list is a new value — signal write is unambiguous.

#### Consequences

**Positive:**
- Signal tracking is simple (new value = new reference = signal write).
- Clean diffing (structural sharing makes equality checks fast).
- Maps to Swift's value-type `Array` and Kotlin's `List` (immutable interface).

**Negative:**
- More allocation for large list updates. Mitigated by structural sharing (only changed elements are new).

---

### ADR-0020: Platform escape hatches via capabilities (not inline native code)

**Status:** Accepted  
**Date:** 2025-01-18  
**Decision Drivers:** ASR-005 (LLM-friendly syntax), ASR-006 (dev/release parity)

#### Context and Problem Statement

When `.flux` can't express something (e.g., ARKit integration, custom platform API), the user needs an escape hatch. How?

#### Considered Options

**Option A — Inline native code.**
- `native("swift", """ Button(action: handler) { Text("Hi") } """)`.
- Pros: Maximum flexibility.
- Cons: Breaks type safety. LLM-unfriendly (string interpolation of native code). Can't be hot-swapped. Parity impossible.

**Option B — Platform conditional.**
- `if platform == "ios" { CupertinoButton(...) } else { MaterialButton(...) }`.
- Pros: Still in `.flux`. Type-checked.
- Cons: Doesn't solve the "can't express at all" case (e.g., ARKit).

**Option C — Capabilities.**
- Declare capability in `.flux`: `capability AR { fn startSession() -> Unit }`. Bind per-platform in host app.
- Pros: Type-safe. LLM-friendly (API is discoverable). Hot-swappable in dev (RPC). Direct in release.
- Cons: Requires writing native binding code in host app. Not inline.

#### Decision Outcome

**Chosen: Option C — Capabilities.**

Capabilities are the escape hatch. The user declares the capability in `.flux` and binds it per-platform in the host app. In dev, capability calls are RPC'd over WS. In release, they're direct native calls.

#### Consequences

**Positive:**
- Type-safe (capability API is declared in `.flux`, type-checked).
- LLM-friendly (API is discoverable via `flux doc`).
- Hot-swappable in dev (RPC).
- Direct in release (no overhead).

**Negative:**
- Requires writing native binding code in host app for each capability.
- Not inline (can't write Swift code directly in `.flux` file). This is a feature, not a bug.

---

## Appendix B — .flux Grammar Reference

### B.1 Lexer Rules (pest)

```pest
// Whitespace and comments
WHITESPACE = _{ " " | "\t" | "\r" | "\n" }
COMMENT    = { "//" ~ (!"\n" ~ ANY)* }

// Identifiers
ident      = { @{ASCII_ALPHA} ~ (ASCII_ALPHANUMERIC | "_")* }
path       = { ident ~ ("::" ~ ident)* }

// Literals
int_lit    = { "-"? ~ ASCII_DIGIT+ }
float_lit  = { "-"? ~ ASCII_DIGIT+ ~ "." ~ ASCII_DIGIT+ }
bool_lit   = { "true" | "false" }
string_lit = { "\"" ~ (string_char | interp)* ~ "\"" }
string_char = { !("\"" | "{" | "\\") ~ ANY | "\\\"" | "\\{" | "\\\\" }
interp     = { "{" ~ ident ~ ("." ~ ident)* ~ "}" }
list_lit   = { "[" ~ (expr ~ ("," ~ expr)*)? ~ "]" }

// Keywords (reserved)
keyword    = { "component" | "fn" | "state" | "props" | "type" | "trait"
             | "import" | "use" | "let" | "if" | "else" | "when" | "otherwise"
             | "match" | "ForEach" | "provide" | "with" | "capability"
             | "onMount" | "onCleanup" | "effect" | "derived" | "batch"
             | "untrack" | "resource" | "createRef" | "useContext" | "pure" }
```

### B.2 Parser Rules (pest)

```pest
// ========================= Top-level =========================

file        = { SOI ~ statement* ~ EOI }
statement   = { import_decl | use_decl | component_decl | fn_decl
             | type_decl | trait_decl | capability_decl }

// ========================= Imports ==========================

import_decl = { "import" ~ ident ~ "from" ~ string_lit }
use_decl    = { "use" ~ path ~ ("::" ~ "*")? }

// ========================= Components =======================

component_decl
            = { annotations? ~ "component" ~ ident ~ generic_params?
                  ~ props_block? ~ block }

annotations = { "@" ~ ident ~ ("(" ~ args? ~ ")")? ~ whitespace* }

props_block = { "(" ~ prop_decl ~ ("," ~ prop_decl?)* ~ ")" }
prop_decl   = { ident ~ ":" ~ type }

// ========================= Functions ========================

fn_decl     = { "fn" ~ ident ~ generic_params? ~ "(" ~ params? ~ ")"
                  ~ ("->" ~ type)? ~ block }

params      = { param ~ ("," ~ param)* }
param       = { ident ~ ":" ~ type }

// ========================= Types ============================

type_decl   = { "type" ~ ident ~ generic_params? ~ "=" ~ variant+ }
variant     = { "|" ~ ident ~ ("(" ~ type_list? ~ ")")? }

trait_decl  = { "trait" ~ ident ~ generic_params? ~ "{" ~ method_decl* ~ "}" }
method_decl = { "fn" ~ ident ~ "(" ~ params? ~ ")" ~ ("->" ~ type)? }

// ========================= Capabilities =====================

capability_decl
            = { "capability" ~ ident ~ "{" ~ cap_method* ~ "}" }
cap_method  = { "fn" ~ ident ~ "(" ~ params? ~ ")" ~ ("->" ~ type)? }

// ========================= Generics =========================

generic_params
            = { "[" ~ type_param ~ ("," ~ type_param)* ~ "]" }
type_param  = { ident ~ (":" ~ ident)? }

generic_args = { "[" ~ type ~ ("," ~ type)* ~ "]" }

// ========================= Type Expressions =================

type        = { type_app | type_var | primitive | record_type | fn_type }
type_app    = { ident ~ generic_args? }
type_var    = { ident }
primitive   = { "Int" | "Float" | "Bool" | "String" | "Unit" }
record_type = { "{" ~ field_type ~ ("," ~ field_type)* ~ "}" }
field_type  = { ident ~ ":" ~ type }
fn_type     = { "Fn" ~ "(" ~ type_list? ~ ")" ~ "->" ~ type }
type_list   = { type ~ ("," ~ type)* }

// ========================= Blocks & Expressions =============

block       = { "{" ~ expr* ~ "}" }

expr        = { let_expr | assign_expr | if_expr | when_expr
             | match_expr | for_expr | call_expr | provide_expr
             | lifecycle_expr | literal | ident }

let_expr    = { "let" ~ ident ~ ("=" ~ expr)? }
assign_expr = { lvalue ~ "=" ~ expr }
lvalue      = { ident ~ ("." ~ ident)* }

if_expr     = { "if" ~ expr ~ block ~ ("else" ~ (if_expr | block))? }
when_expr   = { "when" ~ expr ~ block ~ ("otherwise" ~ block)? }

match_expr  = { "match" ~ expr ~ "{" ~ match_arm+ ~ "}" }
match_arm   = { pattern ~ "=>" ~ expr }
pattern     = { ident ~ ("(" ~ ident_list? ~ ")")? | "_" 
             | literal | guard_pattern }
guard_pattern = { ident ~ "if" ~ expr }

for_expr    = { "ForEach" ~ "(" ~ expr ~ "," ~ "key:" ~ expr ~ ")"
                  ~ block }

call_expr   = { ident ~ "(" ~ args? ~ ")" ~ block? }
args        = { named_arg ~ ("," ~ named_arg)* }
named_arg   = { ident ~ ":" ~ expr }

provide_expr = { "provide" ~ ident ~ "with" ~ expr }
useContext_expr = { "useContext" ~ "(" ~ ident ~ ")" }

// ========================= Lifecycle ========================

lifecycle_expr
            = { onMount_expr | onCleanup_expr | effect_expr
             | derived_expr | batch_expr | untrack_expr
             | resource_expr | createRef_expr }

onMount_expr   = { "onMount" ~ block }
onCleanup_expr = { "onCleanup" ~ block }
effect_expr    = { "effect" ~ block }
derived_expr   = { "derived" ~ block }
batch_expr     = { "batch" ~ block }
untrack_expr   = { "untrack" ~ block }
resource_expr  = { "resource" ~ "(" ~ expr ~ ")" }
createRef_expr = { "createRef" ~ generic_args? ~ "(" ~ ")" }

// ========================= Literals =========================

literal     = { int_lit | float_lit | bool_lit | string_lit | list_lit }
```

### B.3 Grammar Examples

#### B.3.1 Simple Component

```flux
component HelloWorld {
  state count: Int = 0
  
  Column(gap: 12) {
    Text("Count: {count}")
    Button(text: "Increment", onClick: {
      count = count + 1
    })
  }
}
```

#### B.3.2 Generic Component with Trait Bound

```flux
trait Numeric[T] {
  fn zero() -> T
  fn one() -> T
  fn +(a: T, b: T) -> T
  fn -(a: T, b: T) -> T
}

component Counter[T: Numeric] {
  state count: T = Numeric.zero()
  
  Column(gap: 8) {
    Text("Count: {count}")
    Button(text: "+", onClick: { count = count + Numeric.one() })
    Button(text: "−", onClick: { count = count - Numeric.one() })
  }
}
```

#### B.3.3 Algebraic Data Type and Pattern Matching

```flux
type Shape =
  | Circle(Float)
  | Rectangle(Float, Float)
  | Triangle(Float, Float, Float)

fn area(shape: Shape) -> Float {
  match shape {
    Circle(r) => 3.14159 * r * r
    Rectangle(w, h) => w * h
    Triangle(b, h, _) => 0.5 * b * h
  }
}

component ShapeDisplay {
  state shape: Shape = Circle(5.0)
  
  Column {
    Text("Area: {area(shape)}")
    Button(text: "Make Square", onClick: {
      shape = Rectangle(4.0, 4.0)
    })
  }
}
```

#### B.3.4 Lifecycle, Effects, and Cleanup

```flux
component Chat {
  state messages: List[String] = []
  let socket = createRef[WebSocket]()
  
  onMount {
    socket.set(WebSocket.connect("ws://localhost:8080"))
    socket.get().on_message = fn(msg: String) {
      batch {
        messages = messages + [msg]
      }
    }
  }
  
  onCleanup {
    socket.get().close()
  }
  
  Column {
    ForEach(messages, key: fn(m, i) { i }) { msg =>
      Text(msg)
    }
  }
}
```

#### B.3.5 Navigation with Router

```flux
component App {
  state route: String = "home"
  
  Router {
    Screen("home") { Home() }
    Screen("profile") { Profile() }
    Screen("settings") { Settings() }
  }
}

component Home {
  let router = useContext(RouterContext)
  
  Column(gap: 16) {
    Text("Home")
    Button(text: "Open Profile", onClick: {
      router.navigate("profile")
    })
    Button(text: "Settings", onClick: {
      router.navigate("settings")
    })
  }
}
```

#### B.3.6 Async with Resource

```flux
component UserList {
  let (users, { refetch }) = resource(fn {
    Api.fetch("/users")
  })
  
  Column {
    when users.is_loading {
      Text("Loading...")
    }
    otherwise {
      ForEach(users.value, key: fn(u) { u.id }) { user =>
        Text("{user.name}")
      }
    }
    Button(text: "Refresh", onClick: { refetch() })
  }
}
```

#### B.3.7 Pure Component

```flux
@pure
component Avatar(url: String, size: Float) {
  Image(url) {
    width: size,
    height: size,
    cornerRadius: size / 2
  }
}

component Profile {
  state avatarUrl: String = "https://example.com/me.png"
  
  Column {
    Avatar(url: avatarUrl, size: 80)
    Text("Profile")
  }
}
```

#### B.3.8 Platform Conditional

```flux
component PlatformButton {
  if platform == "ios" {
    CupertinoButton(text: "Tap", onClick: { ... })
  } else {
    MaterialButton(text: "Tap", onClick: { ... })
  }
}
```

#### B.3.9 Capability Declaration

```flux
capability Camera {
  fn capture() -> Data
  fn startPreview() -> Unit
  fn stopPreview() -> Unit
}

capability Storage {
  fn set(key: String, value: Data) -> Unit
  fn get(key: String) -> Option[Data]
  fn delete(key: String) -> Unit
}
```

#### B.3.10 Refs

```flux
component LoginForm {
  let emailRef = createRef[TextField]()
  let passwordRef = createRef[TextField]()
  
  onMount {
    emailRef.focus()
  }
  
  Column(gap: 12) {
    TextField(ref: emailRef, placeholder: "Email")
    TextField(ref: passwordRef, placeholder: "Password")
    Button(text: "Submit", onClick: {
      let email = emailRef.text()
      let password = passwordRef.text()
      Auth.login(email, password)
    })
  }
}
```

---

## Appendix C — IR Schema Reference

### C.1 Core Types

```rust
use std::collections::HashMap;

// ========================= IDs ==============================

pub type NodeId = u32;       // Stable across edits
pub type HandlerId = u32;   // Closure table index
pub type SignalId = u32;    // Signal graph cell index
pub type ComponentId = u32; // Interned component name
pub type StringId = u32;    // Interned string
pub type FileId = u32;      // Source file ID
pub type TypeId = u32;      // Interned type
pub type PropIdx = u16;     // Prop field index (per component)
pub type InstanceId = u32;  // Component instance ID

// ========================= Span ==============================

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
pub struct Span {
    pub file_id: FileId,
    pub start: u32,    // Byte offset in file
    pub end: u32,      // Byte offset in file
}

// ========================= Node ==============================

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
#[repr(u8)]
pub enum NodeKind {
    Component   = 0,
    Primitive   = 1,
    ForEach     = 2,
    If          = 3,
    Match       = 4,
    Router      = 5,
    Screen      = 6,
}

#[derive(Clone, Debug)]
pub struct Node {
    pub id: NodeId,
    pub kind: NodeKind,
    pub component_id: ComponentId,
    pub props: Props,
    pub children: Vec<Child>,
    pub handlers: Vec<HandlerId>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum Child {
    Node(NodeId),
    Splice {
        items: Vec<(Key, NodeId)>,
    },
}

pub type Key = u64; // Hash of the ForEach item's key

// ========================= Props =============================

#[derive(Clone, Debug, Default)]
pub struct Props {
    pub fields: Vec<(PropIdx, Value)>,
    pub hash: u64, // BLAKE3 hash for content addressing
}

impl Props {
    pub fn get(&self, idx: PropIdx) -> Option<&Value> {
        self.fields.iter()
            .find(|(i, _)| *i == idx)
            .map(|(_, v)| v)
    }
    
    pub fn get_str(&self, idx: PropIdx) -> Option<&str> {
        match self.get(idx)? {
            Value::Str(id) => Some(string_table.lookup(*id)),
            _ => None,
        }
    }
    
    pub fn get_bool(&self, idx: PropIdx, default: bool) -> bool {
        match self.get(idx) {
            Some(Value::Bool(b)) => *b,
            _ => default,
        }
    }
    
    pub fn get_handler(&self, idx: PropIdx) -> HandlerId {
        match self.get(idx) {
            Some(Value::HandlerRef(id)) => *id,
            _ => 0,
        }
    }
}

// ========================= Values ============================

#[derive(Clone, Debug)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(StringId),
    List(Vec<Value>),
    Record(Vec<(PropIdx, Value)>),
    HandlerRef(HandlerId),
    Null,
}

impl Value {
    pub fn type_id(&self) -> TypeId {
        match self {
            Value::Int(_) => TYPE_INT,
            Value::Float(_) => TYPE_FLOAT,
            Value::Bool(_) => TYPE_BOOL,
            Value::Str(_) => TYPE_STRING,
            Value::List(_) => TYPE_LIST_ANY, // Monomorphized at type-check
            Value::Record(_) => TYPE_RECORD_ANY,
            Value::HandlerRef(_) => TYPE_HANDLER,
            Value::Null => TYPE_UNIT,
        }
    }
}

// ========================= Patches ===========================

#[derive(Clone, Debug)]
pub enum Patch {
    Replace {
        id: NodeId,
        node: Node,
    },
    Update {
        id: NodeId,
        props_diff: PropDiff,
    },
    Insert {
        parent: NodeId,
        index: u16,
        node: Node,
    },
    Remove {
        id: NodeId,
    },
    Reorder {
        parent: NodeId,
        keys: Vec<NodeId>,
    },
    Handler {
        id: HandlerId,
        closure: ClosureRef,
    },
}

#[derive(Clone, Debug, Default)]
pub struct PropDiff {
    pub changes: Vec<(PropIdx, Value)>,
    pub removals: Vec<PropIdx>,
}

// ========================= Closures ==========================

#[derive(Clone, Debug)]
pub struct ClosureRef {
    pub hash: u64,          // Content hash for interning
    pub bytecode_offset: u32,
    pub bytecode_len: u16,
    pub captured_signals: Vec<SignalId>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct ClosureIR {
    pub id: HandlerId,
    pub bytecode: Vec<u8>,
    pub captured_signals: Vec<SignalId>,
    pub span: Span,
    pub param_types: Vec<TypeId>,
    pub return_type: TypeId,
}

// ========================= Types =============================

#[derive(Clone, Debug)]
pub struct Type {
    pub id: TypeId,
    pub kind: TypeKind,
}

#[derive(Clone, Debug)]
pub enum TypeKind {
    Int,
    Float,
    Bool,
    String,
    Unit,
    List(Box<Type>),
    Map(Box<Type>, Box<Type>),
    Option(Box<Type>),
    Fn(Vec<Type>, Box<Type>),
    Record(Vec<(StringId, Type)>),
    Variant(StringId, Vec<Type>),
    Var(u32),
    Constrained(u32, Vec<StringId>),
}

// ========================= Arena =============================

pub struct IRArena {
    // Struct-of-arrays for diff-hot fields
    pub ids: Vec<NodeId>,
    pub kinds: Vec<u8>,
    pub component_ids: Vec<ComponentId>,
    pub props_offsets: Vec<u32>,
    pub children_offsets: Vec<u32>,
    pub handler_offsets: Vec<u32>,
    pub span_offsets: Vec<u32>,
    
    // Blobs for cold data
    pub props_blob: Vec<u8>,
    pub children_blob: Vec<u8>,
    pub handlers_blob: Vec<u8>,
    pub spans_blob: Vec<u8>,
    
    // Index from NodeId → arena index
    pub node_index: HashMap<NodeId, usize>,
    
    // String table (shared across arena)
    pub string_table: StringTable,
    
    // Closure table
    pub closures: HashMap<HandlerId, ClosureIR>,
}

impl IRArena {
    pub fn get(&self, id: NodeId) -> Option<&NodeView> {
        let idx = *self.node_index.get(&id)?;
        Some(NodeView {
            arena: self,
            index: idx,
        })
    }
    
    pub fn pack(&mut self, node: Node) -> NodeId {
        let idx = self.ids.len();
        self.ids.push(node.id);
        self.kinds.push(node.kind as u8);
        self.component_ids.push(node.component_id);
        
        let props_offset = self.props_blob.len();
        self.pack_props(&node.props);
        self.props_offsets.push(props_offset as u32);
        
        // ... pack children, handlers, spans similarly
        
        self.node_index.insert(node.id, idx);
        node.id
    }
    
    fn pack_props(&mut self, props: &Props) {
        // Pack as: u16 field_count, then (u16 PropIdx, Value)* 
        self.props_blob.extend_from_slice(&(props.fields.len() as u16).to_le_bytes());
        for (idx, val) in &props.fields {
            self.props_blob.extend_from_slice(&idx.to_le_bytes());
            self.pack_value(val);
        }
    }
    
    fn pack_value(&mut self, val: &Value) {
        match val {
            Value::Int(i) => {
                self.props_blob.push(0x01); // type tag
                self.props_blob.extend_from_slice(&i.to_le_bytes());
            }
            Value::Float(f) => {
                self.props_blob.push(0x02);
                self.props_blob.extend_from_slice(&f.to_le_bytes());
            }
            Value::Bool(b) => {
                self.props_blob.push(0x03);
                self.props_blob.push(*b as u8);
            }
            Value::Str(id) => {
                self.props_blob.push(0x04);
                self.props_blob.extend_from_slice(&id.to_le_bytes());
            }
            Value::HandlerRef(id) => {
                self.props_blob.push(0x05);
                self.props_blob.extend_from_slice(&id.to_le_bytes());
            }
            Value::Null => {
                self.props_blob.push(0x00);
            }
            Value::List(items) => {
                self.props_blob.push(0x06);
                self.props_blob.extend_from_slice(&(items.len() as u16).to_le_bytes());
                for item in items {
                    self.pack_value(item);
                }
            }
            Value::Record(fields) => {
                self.props_blob.push(0x07);
                self.props_blob.extend_from_slice(&(fields.len() as u16).to_le_bytes());
                for (idx, val) in fields {
                    self.props_blob.extend_from_slice(&idx.to_le_bytes());
                    self.pack_value(val);
                }
            }
        }
    }
}

pub struct NodeView<'a> {
    arena: &'a IRArena,
    index: usize,
}

impl<'a> NodeView<'a> {
    pub fn id(&self) -> NodeId { self.arena.ids[self.index] }
    pub fn kind(&self) -> NodeKind { 
        unsafe { std::mem::transmute(self.arena.kinds[self.index]) }
    }
    // ... other accessors
}
```

### C.2 Component Instance Tracking

```rust
pub struct ComponentInstance {
    pub id: InstanceId,
    pub component_id: ComponentId,
    pub node_id: NodeId,
    pub signals: Vec<SignalId>,
    pub effects: Vec<EffectId>,
    pub closures: Vec<HandlerId>,
    pub children: Vec<InstanceId>,
    pub state: Vec<(StringId, Value)>, // Initial state values
}

pub struct InstanceRegistry {
    pub instances: HashMap<InstanceId, ComponentInstance>,
    pub node_to_instance: HashMap<NodeId, InstanceId>,
    pub next_id: InstanceId,
}
```

### C.3 String Table

```rust
pub struct StringTable {
    pub strings: Vec<String>,
    pub lookup: HashMap<String, StringId>,
}

impl StringTable {
    pub fn intern(&mut self, s: &str) -> StringId {
        if let Some(id) = self.lookup.get(s) {
            return *id;
        }
        let id = self.strings.len() as StringId;
        self.strings.push(s.to_string());
        self.lookup.insert(s.to_string(), id);
        id
    }
    
    pub fn lookup(&self, id: StringId) -> &str {
        &self.strings[id as usize]
    }
}
```

### C.4 Signal Graph

```rust
pub struct SignalGraph {
    pub cells: HashMap<SignalId, SignalCell>,
    pub dirty: Vec<SignalId>,
    pub topo_order: Vec<SignalId>, // Precomputed
}

pub struct SignalCell {
    pub value: Value,
    pub dependents: Vec<DerivedCell>,
    pub node_id: NodeId, // Which shadow node subscribes
}

pub struct DerivedCell {
    pub signal_id: SignalId,
    pub node_id: NodeId,
    pub compute: ClosureRef,
    pub cached_value: Option<Value>,
}

impl SignalGraph {
    pub fn read(&self, id: SignalId) -> &Value {
        &self.cells[&id].value
    }
    
    pub fn write(&mut self, id: SignalId, value: Value) {
        let cell = self.cells.get_mut(&id).unwrap();
        cell.value = value;
        if !self.dirty.contains(&id) {
            self.dirty.push(id);
        }
    }
    
    pub fn propagate(&mut self) -> Vec<NodeId> {
        let mut affected = Vec::new();
        let dirty = std::mem::take(&mut self.dirty);
        
        for id in &self.topo_order {
            if dirty.contains(id) {
                continue; // Source, not derived
            }
            let cell = self.cells.get(id);
            // Check if any dependency is dirty
            // If so, recompute
            // If value unchanged, skip
            // If changed, add to affected
        }
        
        affected
    }
}
```

---

## Appendix D — Wire Protocol Reference

### D.1 Frame Structure

All multi-byte integers are **little-endian**.

```
Offset  Size  Field           Description
------  ----  --------------  -----------
0       4     magic           0x465C5558 ("FLUX" in little-endian)
4       1     version         Protocol version (currently 1)
5       4     seq             Monotonic sequence number
9       1     flags           Bitfield:
                                  bit 0: full_tree (1) vs delta (0)
                                  bit 1: error frame
                                  bit 2: heartbeat
                                  bit 3: has_state_delta
                                  bit 4: has_src_map_delta
                                  bit 5: has_string_table_delta
10      2     patch_count     Number of Patch entries
12      2     handler_count   Number of HandlerDef entries
14      2     string_count    Number of new strings (0 if no delta)
16      ...   patches         [Patch; patch_count]
...     ...   handlers        [HandlerDef; handler_count]
...     ...   strings         [StringEntry; string_count] (if delta)
...     ...   state_delta     StateDelta (if flag set)
...     ...   src_map_delta   SourceMapDelta (if flag set)
```

### D.2 Patch Encoding

Each patch starts with a 1-byte tag:

```
Tag  Patch Type     Payload
---  -----------     -------
0x01 Replace         u32 id, Node node
0x02 Update          u32 id, PropDiff diff
0x03 Insert          u32 parent_id, u16 index, Node node
0x04 Remove          u32 id
0x05 Reorder         u32 parent_id, u16 key_count, [u32; key_count]
0x06 Handler         u32 id, ClosureRef closure
```

### D.3 Node Encoding

```
Offset  Size  Field           Description
------  ----  --------------  -----------
0       4     id              NodeId
4       1     kind            NodeKind (0=Component, 1=Primitive, ...)
5       4     component_id    Interned component name ID
9       2     prop_count      Number of props
11      ...   props           [(u16 prop_idx, Value); prop_count]
...     2     child_count     Number of children
...     ...   children        [Child; child_count]
...     2     handler_count   Number of handlers
...     ...   handlers        [u32 HandlerId; handler_count]
...     4     span_file       FileId
...     4     span_start      Byte offset
...     4     span_end        Byte offset
```

### D.4 Child Encoding

```
Tag  Child Type    Payload
---  ----------    -------
0x01 Node          u32 node_id
0x02 Splice        u16 item_count, [(u64 key, u32 node_id); item_count]
```

### D.5 Value Encoding

```
Tag  Value Type    Payload (after tag)
---  ----------    -------------------
0x00 Null          (none)
0x01 Int           i64 (8 bytes)
0x02 Float         f64 (8 bytes)
0x03 Bool           u8 (0 or 1)
0x04 Str           u32 string_id (interned)
0x05 HandlerRef    u32 handler_id
0x06 List          u16 count, [Value; count]
0x07 Record        u16 count, [(u16 prop_idx, Value); count]
```

### D.6 PropDiff Encoding

```
Offset  Size  Field           Description
------  ----  --------------  -----------
0       2     change_count    Number of changed props
2       ...   changes         [(u16 prop_idx, Value); change_count]
...     2     removal_count   Number of removed props
...     ...   removals        [u16 prop_idx; removal_count]
```

### D.7 ClosureRef Encoding

```
Offset  Size  Field              Description
------  ----  -----------------  -----------
0       8     hash               BLAKE3 hash (content address)
8       4     bytecode_offset    Offset into closure blob
12      2     bytecode_len       Length of bytecode
14      2     signal_count       Number of captured signals
16      ...   signals            [u32 SignalId; signal_count]
...     4     span_file          FileId
...     4     span_start         Byte offset
...     4     span_end           Byte offset
```

### D.8 HandlerDef Encoding

```
Offset  Size  Field           Description
------  ----  --------------  -----------
0       4     handler_id      HandlerId
4       ...   closure_ref      ClosureRef (see D.7)
```

### D.9 StringEntry Encoding

```
Offset  Size  Field           Description
------  ----  --------------  -----------
0       4     string_id       StringId (u32)
4       2     string_len      Length in bytes
6       ...   string_bytes    UTF-8 string data
```

### D.10 StateDelta Encoding

```
Offset  Size  Field           Description
------  ----  --------------  -----------
0       2     cell_count      Number of state cells
2       ...   cells           [(u32 signal_id, Value); cell_count]
```

### D.11 SourceMapDelta Encoding

```
Offset  Size  Field           Description
------  ----  --------------  -----------
0       2     file_count      Number of new/changed files
2       ...   files           [FileEntry; file_count]
```

**FileEntry:**
```
Offset  Size  Field           Description
------  ----  --------------  -----------
0       4     file_id         FileId
4       2     path_len        Length of file path
6       ...   path_bytes      UTF-8 file path
```

### D.12 Handshake Protocol

#### D.12.1 Hello Frame (Host → Server)

```
Offset  Size  Field              Description
------  ----  -----------------  -----------
0       4     magic              0x465C5558
4       1     version            Protocol version
5       1     frame_type         0x01 = Hello
6       2     platform_len       Length of platform string
8       ...   platform           "ios" or "android"
...     2     device_len         Length of device string
...     ...   device             Device model string
...     2     cap_count          Number of capabilities
...     ...   capabilities       [CapabilityEntry; cap_count]
```

**CapabilityEntry:**
```
Offset  Size  Field           Description
------  ----  --------------  -----------
0       4     name_id          Interned capability name ID
4       2     version           Capability version
6       2     method_count      Number of methods
8       ...   methods           [u32 method_name_id; method_count]
```

#### D.12.2 Init Frame (Server → Host)

```
Offset  Size  Field              Description
------  ----  -----------------  -----------
0       4     magic              0x465C5558
4       1     version            Protocol version
5       1     frame_type         0x02 = Init
6       4     seq                Sequence number (0)
10      ...   root_node          Node (root of the tree)
...     ...   state_seed         StateDelta (initial values)
...     ...   source_map         SourceMapDelta (file mappings)
...     4     string_count       Number of interned strings
...     ...   strings            [StringEntry; string_count]
```

#### D.12.3 Error Frame (Server → Host)

```
Offset  Size  Field              Description
------  ----  -----------------  -----------
0       4     magic              0x465C5558
4       1     version            Protocol version
5       1     frame_type         0x03 = Error
6       4     seq                Sequence number
10      2     message_len        Length of error message
12      ...   message            UTF-8 error message
...     4     span_file          FileId
...     4     span_start         Byte offset
...     4     span_end           Byte offset
```

### D.13 Reconnect Protocol

```
Host detects WS disconnect
    ↓
Host shows "Reconnecting..." banner
    ↓
Host retries connection every 1 second
    ↓
On connect:
    Host sends Hello frame
    Server validates protocol version
    Server sends Init frame (full tree + state seed + source map)
    Host rebuilds shadow tree from scratch
    Host hides "Reconnecting..." banner
```

### D.14 Content Addressing Protocol

Props, closures, and nodes are content-addressed by BLAKE3 hash.

```
Frame with content addressing:

For each props/closure/node in the frame:
    If hash is in host's cache:
        Send only the 8-byte hash (flag bit 0 = 0)
    Else:
        Send full data (flag bit 0 = 1)
        Host caches hash → data

Host cache: HashMap<u64, Props/ClosureIR/Node>
```

Typical patch after initial `Init`:
- 90%+ of props/closures are cached.
- Typical handler-body-change patch: < 500 bytes.
- Typical structural patch (insert one node): < 1 KB.

---

## Appendix E — VM Instruction Set Reference

### E.1 Opcode Table

```
Opcode  Mnemonic          Args (bytes)     Description
------  ----------------  --------------   -----------
0x00    HALT              0                Stop execution
0x01    NOP                0                No operation

// Signal operations
0x10    READ_SIGNAL       1+4              reg_dst(u8), signal_id(u32)
0x11    WRITE_SIGNAL      4+1              signal_id(u32), reg_src(u8)

// Integer arithmetic (monomorphized)
0x20    ADD_I64           3                dst(u8), a(u8), b(u8)
0x21    SUB_I64           3                dst(u8), a(u8), b(u8)
0x22    MUL_I64           3                dst(u8), a(u8), b(u8)
0x23    DIV_I64           3                dst(u8), a(u8), b(u8)
0x24    MOD_I64           3                dst(u8), a(u8), b(u8)
0x25    NEG_I64           2                dst(u8), src(u8)
0x26    EQ_I64            3                dst(u8), a(u8), b(u8)
0x27    LT_I64            3                dst(u8), a(u8), b(u8)
0x28    GT_I64            3                dst(u8), a(u8), b(u8)
0x29    LTE_I64           3                dst(u8), a(u8), b(u8)
0x2A    GTE_I64           3                dst(u8), a(u8), b(u8)

// Float arithmetic (monomorphized)
0x30    ADD_F64           3                dst(u8), a(u8), b(u8)
0x31    SUB_F64           3                dst(u8), a(u8), b(u8)
0x32    MUL_F64           3                dst(u8), a(u8), b(u8)
0x33    DIV_F64           3                dst(u8), a(u8), b(u8)
0x34    NEG_F64           2                dst(u8), src(u8)
0x35    EQ_F64            3                dst(u8), a(u8), b(u8)
0x36    LT_F64            3                dst(u8), a(u8), b(u8)
0x37    GT_F64            3                dst(u8), a(u8), b(u8)
0x38    I64_TO_F64        2                dst(u8), src(u8)
0x39    F64_TO_I64        2                dst(u8), src(u8)

// Bool operations
0x40    AND_BOOL          3                dst(u8), a(u8), b(u8)
0x41    OR_BOOL           3                dst(u8), a(u8), b(u8)
0x42    NOT_BOOL          2                dst(u8), src(u8)

// String operations
0x50    STR_CONCAT        3                dst(u8), a(u8), b(u8)
0x51    STR_INTERN        2+4              dst(u8), str_offset(u32)
0x52    STR_EQ            3                dst(u8), a(u8), b(u8)
0x53    STR_LEN           2                dst(u8), src(u8)

// Control flow
0x60    JUMP              4                offset(i32)
0x61    COND_JUMP         1+4              reg(u8), offset(i32)
0x62    COND_JUMP_NOT     1+4              reg(u8), offset(i32)

// Record operations
0x70    ALLOC_RECORD      1+2              dst(u8), field_count(u16)
0x71    GET_FIELD         3+2              dst(u8), obj(u8), field_idx(u16)
0x72    SET_FIELD         3+2              obj(u8), field_idx(u16), val(u8)
0x73    RECORD_EQ         3                dst(u8), a(u8), b(u8)

// List operations (persistent)
0x80    ALLOC_LIST        1+2              dst(u8), capacity(u16)
0x81    LIST_PUSH         2                list(u8), val(u8) → new list in list reg
0x82    LIST_GET          3                dst(u8), list(u8), idx(u8)
0x83    LIST_LEN          2                dst(u8), list(u8)
0x84    LIST_CONCAT       3                dst(u8), a(u8), b(u8)

// Capability calls (async via callback)
0x90    CALL_CAP          1+4+2+1          result_reg(u8), cap_id(u32),
                                             method_id(u16), args_reg(u8)
                                             Args is a record in args_reg.
                                             Result delivered via callback
                                             (dispatched as a new handler).

// Pattern matching
0xA0    MATCH_TAG         1+4+4            val(u8), tag_id(u32), offset(i32)
                                             Jump to offset if val's tag matches.
0xA1    EXTRACT_FIELD     1+2+1            val(u8), field_idx(u16), dst(u8)
                                             Extract field from variant.

// Register operations
0xB0    LOAD_INT_CONST    1+8              dst(u8), value(i64)
0xB1    LOAD_FLOAT_CONST  1+8              dst(u8), value(f64)
0xB2    LOAD_BOOL_CONST   1+1              dst(u8), value(u8)
0xB3    LOAD_STR_CONST    1+4              dst(u8), string_id(u32)
0xB4    LOAD_NULL         1                dst(u8)
0xB5    MOV               2                dst(u8), src(u8)

// Gas check
0xC0    GAS_CHECK         4                budget(u32) — raise error if gas < budget
```

### E.2 Register Conventions

```
Register  Name        Purpose
--------  ----------  -------
r0        return      Return value of the handler
r1–r14    general     General-purpose registers
r15       gas         Gas counter (decremented per instruction)
```

### E.3 Calling Convention

- Handler entry: `r0` = event payload (record), `r15` = gas budget (100,000).
- Handler exit: `r0` = return value (usually `Unit`).
- No stack. No nested calls (handlers are flat; sub-functions are inlined at bytecode level).
- Capability calls: `CALL_CAP` stores the callback `HandlerId` in the capability's internal table. When the capability completes, the host dispatches the callback handler with the result in `r0`.

### E.4 Bytecode Layout

```
ClosureIR {
    bytecode: [u8; N],
    captured_signals: [SignalId; M],
    entry_point: u32,  // 0 for simple handlers
    span: Span,
}
```

Bytecode is a flat `Vec<u8>`. Instructions are variable-length (1–9 bytes). The VM's IP (instruction pointer) is a `u32` offset into the bytecode.

### E.5 Example Bytecode

Source:
```flux
Button(text: "+", onClick: { count = count + 1 })
```

Bytecode (for `Counter[Int]` instantiation):
```
// count = count + 1
0x10  r0  0x00000001    // READ_SIGNAL r0, signal_id=1 (count)
0xB0  r1  0x0000000000000001  // LOAD_INT_CONST r1, 1
0x20  r0  r0  r1        // ADD_I64 r0, r0, r1
0x11  0x00000001  r0    // WRITE_SIGNAL signal_id=1, r0
0x00                    // HALT
```

Total: 21 bytes. At 1 gas per instruction = 4 gas. Well within budget.

### E.6 Error Conditions

| Error | Trigger | Behavior |
|---|---|---|
| `GasExhausted` | `r15` reaches 0 | Raise error. Top-level catch. Red banner. |
| `MemoryExhausted` | Allocation exceeds 16 MB pool | Raise error. Top-level catch. Red banner. |
| `IndexOutOfBounds` | `LIST_GET` with index >= list length | Raise error. |
| `NullDereference` | `GET_FIELD` on `Null` value | Raise error. |
| `InvalidDispatch` | Unknown opcode | Raise error. Should never happen (type checker caught it). |
| `TypeMismatch` | `ADD_I64` on non-Int value | Raise error. Should never happen (monomorphization guarantees types). |

All errors carry the current `Span` for source-map reporting.

---

## Appendix F — Adapter Contract Reference

### F.1 Text

```flux
Text("hello") {
  text: String,                    // required
  font: Option[Font] = None,       // optional
  size: Option[Float] = None,      // optional
  color: Option[Color] = None,     // optional
  alignment: Option[Alignment] = None,
  max_lines: Option[Int] = None,
  overflow: Option[Overflow] = None,
}
```

| Platform | Dev (imperative) | Release (declarative) |
|---|---|---|
| iOS | `UILabel` | `Text` |
| Android | `TextView` | `Text` |

**Dev (iOS):**
```swift
func update(_ view: UILabel, from old: Props, to new: Props) {
    view.text = new.getString("text")
    if let font = new.getRecord("font") {
        view.font = .systemFont(ofSize: font.get("size") ?? 14)
    }
    if let color = new.getColor("color") {
        view.textColor = color.toUIColor()
    }
}
```

**Release (Swift):**
```swift
Text("hello")
    .font(.system(size: 14))
    .foregroundColor(.black)
```

### F.2 Button

```flux
Button(text: "Tap", onClick: handler) {
  text: String,                    // required
  onClick: Handler,                // required
  enabled: Bool = true,            // optional
  color: Option[Color] = None,
}
```

| Platform | Dev | Release |
|---|---|---|
| iOS | `UIButton` | `Button` |
| Android | `android.widget.Button` | `Button` |

### F.3 Column

```flux
Column(gap: 12) {
  gap: Float = 0,
  alignment: Option[Alignment] = None,
}
```

| Platform | Dev | Release |
|---|---|---|
| iOS | `UIStackView(axis: .vertical)` | `VStack(spacing:)` |
| Android | `LinearLayout(orientation: VERTICAL)` | `Column(spacing:)` |

### F.4 Row

```flux
Row(gap: 8) {
  gap: Float = 0,
  alignment: Option[Alignment] = None,
}
```

| Platform | Dev | Release |
|---|---|---|
| iOS | `UIStackView(axis: .horizontal)` | `HStack(spacing:)` |
| Android | `LinearLayout(orientation: HORIZONTAL)` | `Row(spacing:)` |

### F.5 TextField

```flux
TextField(ref: myRef, text: state_text, onChange: handler) {
  text: String = "",               // controlled value
  onChange: Handler,               // fired on text change
  placeholder: Option[String] = None,
  ref: Option[Ref[TextField]] = None,
  enabled: Bool = true,
  secure: Bool = false,            // password field
  keyboard: Option[KeyboardType] = None,
}
```

| Platform | Dev | Release |
|---|---|---|
| iOS | `UITextField` | `TextField` |
| Android | `EditText` | `TextField` |

### F.6 Router

```flux
Router {
  // No props. Children are Screen components.
}
```

| Platform | Dev | Release |
|---|---|---|
| iOS | `UINavigationController` | `NavigationStack(path:)` |
| Android | `FrameLayout` stack | `NavHost` |

**Dev (iOS):**
```swift
func create() -> UINavigationController {
    return UINavigationController()
}

func setChildren(_ screens: [UIViewController], on nav: UINavigationController) {
    // Push/pop to match the new screen list
    // Preserve existing screens where possible
}
```

### F.7 Screen

```flux
Screen("home") {
  // Child is the screen's content
}
```

| Platform | Dev | Release |
|---|---|---|
| iOS | `UIViewController` | `navigationDestination` |
| Android | `FrameLayout` child | `composable` route |

### F.8 Image (deferred to MLP v2, but contract defined)

```flux
Image("assets/logo.png") {
  src: String,                      // asset path
  width: Option[Float] = None,
  height: Option[Float] = None,
  contentMode: Option[ContentMode] = None,
}
```

| Platform | Dev | Release |
|---|---|---|
| iOS | `UIImageView` (loads from HTTP) | `Image` (loads from asset catalog) |
| Android | `ImageView` (loads from HTTP) | `Image` / `painterResource` |

---

## Appendix G — Glossary

| Term | Definition |
|---|---|
| **ADT** | Algebraic Data Type. A type composed from other types via sum (variants) or product (records). E.g., `type Shape = Circle(Float) \| Rectangle(Float, Float)`. |
| **Adapter** | A platform-native class that bridges an IR node kind to a native view. Has two implementations: dev (imperative, drives `UIView`/`View` directly) and release (declarative, returns `@Composable`/`View`). Both consume the same props. |
| **Arena Allocation** | Memory layout where all IR nodes are packed into a single contiguous `Vec<u8>` blob with offsets, using struct-of-arrays layout for hot fields. Enables cache-linear diff scanning. |
| **ASR** | Architecturally Significant Requirement. A requirement that has a measurable effect on architecture — shapes structure, technology choices, and quality attributes. |
| **Bidirectional Type Checking** | Type checking algorithm that alternates between checking (pushing types down) and synthesis (pulling types up). Requires explicit annotations on signatures; infers locals. |
| **BLAKE3** | A cryptographic hash function used for content addressing of props, closures, and IR nodes. Chosen for speed (~6 GB/s on modern hardware). |
| **Capability** | A platform-specific API (camera, storage, navigation) declared in `.flux` and bound per-platform in the host app. In dev, calls are RPC'd over WS. In release, calls are direct native. |
| **ClosureIR** | The bytecode representation of a handler body. Contains bytecode (`Vec<u8>`), captured signal IDs, span, and type info. Shipped as data to the host VM. |
| **Content Addressing** | Optimization where props, closures, and IR nodes are interned by BLAKE3 hash. Wire protocol ships hashes for already-cached entries, reducing typical payloads by 90%+. |
| **Derived Cell** | A reactive cell computed from other signals. Only recomputes when dependencies change. If output is unchanged, downstream is not notified (natural memoization). |
| **Dev Server** | The Rust process that parses `.flux` files, type-checks, lowers to IR, diffs, serializes patches, and serves them over WebSocket to the host app. |
| **Dev/Release Parity** | The property that dev-mode (executor) and release-mode (codegen) produce identical visible behavior for the same `.flux` source. Verified by automated parity test harness. |
| **Effect** | A reactive computation that runs on signal changes and performs side effects (logging, analytics, syncing). Owned by a component instance. Cleaned up on destroy. |
| **Executor** | The host app's central coordinator. Receives binary frames, applies patches to the shadow tree, hot-swaps handler closures, evaluates handlers via the VM, and dispatches native view mutations to the main thread. |
| **Flux** | The name of the project: a write-once UI language, dev server, and host app system for native iOS/Android development. |
| **ForEach** | An IR node that materializes children from a list. Requires a `key` function for stable IDs. In dev, all items are materialized (no virtualization in MLP). |
| **Gas Meter** | VM safety mechanism. Each instruction costs 1 gas. Handler dispatch has a budget of 100,000 instructions. On exhaustion, raises `GasExhausted` error. Prevents infinite loops from freezing the app. |
| **Handler** | A reactive closure fired on user events (tap, change). Referenced by `HandlerId` in the IR. Hot-swappable without recompilation. Contains `ClosureIR` bytecode. |
| **Host App** | A precompiled iOS (Swift) or Android (Kotlin) app that contains the executor, VM, signal graph, shadow tree, reconciler, adapters, and WS client. Shipped once; never rebuilt during dev. |
| **Init Frame** | The frame sent by the dev server on first connection or reconnect. Contains the full IR tree, state seed, source map, and string table. |
| **IR (RT-IR)** | Reactive Tree Intermediate Representation. The canonical data structure representing a compiled `.flux` component tree. Arena-allocated in the dev server. Consumed by both the dev executor and release codegen. |
| **Keyed Reconciliation** | Diffing algorithm where nodes have stable IDs. Compare old vs new children by ID; produce insert/remove/move/update operations. O(n) for typical cases. Based on udomdiff algorithm. |
| **Memory Cap** | VM safety mechanism. VM allocates from a fixed pool of 16 MB per component instance. On exhaustion, raises `MemoryExhausted` error. |
| **MLP** | Minimum Lovable Product. The first end-to-end vertical slice that validates the architecture. Includes: static types with generics, iOS + Android, navigation, embedded interpreter hot-swap. |
| **Monomorphization** | The dev server's process of specializing generic code per concrete type instantiation. Produces type-specific bytecode (`ADD_I64`, `ADD_F64`) instead of generic `ADD` with tag dispatch. Capped at 100 specializations per generic. |
| **Node ID** | A `u32` derived from `hash(parent_id, node_kind, source_span, optional_key)`. Stable across edits where source structure doesn't change. Enables state preservation and minimal diffs. |
| **onCleanup** | A lifecycle hook that runs when a component instance is destroyed. Used to tear down resources (close WebSocket, unsubscribe). Runs in LIFO order. |
| **onMount** | A lifecycle hook that runs once when a component instance is created. Used for one-time setup (connect WebSocket, fetch data, start timer). |
| **Parity Test** | An automated test that runs the same actions in dev mode (via VM) and release mode (via compiled Swift/Kotlin) and asserts that state values are identical. The safety net for dev/release behavioral equivalence. |
| **Patch** | A minimal diff operation applied to the shadow tree. Types: `Replace`, `Update`, `Insert`, `Remove`, `Reorder`, `Handler`. |
| **Props** | A flat map of `(prop_idx, value)` pairs attached to an IR node. Content-addressed by BLAKE3 hash. Field access by index (O(1)). |
| **Pure Component** | A component annotated `@pure`. Has no internal state (only props). Reconciler skips its subtree if props are referentially equal (hash compare). |
| **Reconciler** | The host app's component that translates dirty signal nodes into native view mutations. Uses keyed reconciliation (udomdiff-style). Preserves native view instances when node IDs are stable. |
| **Ref** | A handle to a native view instance, used for imperative operations (focus, scroll, measure). Created via `createRef<T>()`. Adapter registers itself with the ref on creation. In release, maps to `@FocusState` (SwiftUI) / `FocusRequester` (Compose). |
| **Resource** | An async primitive for data fetching. `let (data, { refetch }) = resource(fn { Api.fetch(...) })`. Returns a value with loading/error states. Uses callbacks internally (no async/await in MLP). |
| **Router** | A navigation adapter that maintains a stack of screens. Pushing preserves the previous screen's shadow tree. Popping destroys it. Maps to `NavigationStack` (SwiftUI) / `NavHost` (Compose). |
| **Screen** | A navigation adapter child that declares a route. Contains the screen's content component. Maps to `navigationDestination` (SwiftUI) / `composable` (Compose). |
| **Shadow Tree** | The host app's in-memory mirror of the IR. Each IR node has a corresponding `ShadowNode` that owns a native view instance. The reconciler mutates the shadow tree on patches. |
| **Signal** | A fine-grained reactive cell holding a value (à la SolidJS). Mutations propagate to dependent derived cells and effects in topological order. Batched within a single dispatch. |
| **Signal Graph** | The host app's in-memory collection of all signal cells, derived cells, and effects for the current component tree. Owned by the executor. Propagation is topological-order, O(diff). |
| **Span** | A source location: `(file_id, start_byte, end_byte)`. Carried by every IR node and every bytecode instruction. Used for error reporting and click-to-jump. |
| **Stable Node ID** | A `NodeId` derived from source structure (parent + kind + source span + key). Preserved across edits where the source structure doesn't change. Enables state preservation and minimal diffs. |
| **String Interning** | Optimization where strings are stored in a shared table (`Vec<String>`) and referenced by `u32` ID. Reduces wire payload size 5–10×. String comparison becomes `u32` compare. |
| **Tombstone** | A record of a destroyed state cell, kept for a few seconds so rapid undo/re-edit can restore state. Prevents state loss on rapid edit cycles. |
| **Type Class** | A trait-like construct for ad-hoc polymorphism (Haskell-style). `trait Numeric[T] { fn zero() -> T; fn +(a: T, b: T) -> T }`. Resolved at type-check time. Enables generic components with trait bounds. |
| **VM (FluxBytecodeVM)** | The embedded register-based bytecode interpreter in the host app. Evaluates `ClosureIR` against the signal graph. 16 registers, 1-byte opcodes, gas meter, memory cap. ~2k LOC per platform. |
| **Wire Protocol** | The binary frame format between dev server and host app. MessagePack-encoded with content addressing. Frames: `Hello`, `Init`, delta patches, `Error`, heartbeat. |
| **WebSocket** | The transport protocol between dev server and host app. Default URL: `ws://localhost:7331`. Bidirectional: server pushes patches, host sends dispatch events (for capabilities in dev mode). |

---

**End of Appendices v0.1.0**

This document is the canonical reference for the Flux MLP's appendices. All implementation should trace back to the ADRs, grammar, schemas, and protocols documented herein.
