# Flux — Parallel Agent Boundary Contract & Issue Plan

## Part 1: The Boundary Contract

This document defines the rules of engagement for parallel AI agents working on the Flux codebase. Every agent MUST adhere to these rules. Violations cause merge conflicts, broken builds, and wasted agent time.

### 1.1 The Core Principle

**Each agent owns a disjoint set of directories. No two agents' directories overlap. Agents never modify files outside their ownership.**

### 1.2 Directory Ownership Map

```
┌──────────────────────────────────────────────────────────────────────────┐
│  AGENT OWNERSHIP MAP                                                     │
├──────────────────┬───────────────────────────────────────────────────────┤
│  Agent           │  Owned directories (exclusive)                        │
├──────────────────┼───────────────────────────────────────────────────────┤
│  foundation      │  /Cargo.toml (workspace root)                          │
│                  │  /rust-toolchain.toml                                 │
│                  │  /.gitignore                                          │
│                  │  /crates/flux-syntax/**                                │
│                  │  /crates/*/Cargo.toml (ALL crate manifests)           │
│                  │  /crates/*/src/lib.rs (stubs only, replaced by owner)  │
├──────────────────┼───────────────────────────────────────────────────────┤
│  parser          │  /crates/flux-parser/src/**                            │
├──────────────────┼───────────────────────────────────────────────────────┤
│  typechecker     │  /crates/flux-types/src/**                             │
├──────────────────┼───────────────────────────────────────────────────────┤
│  ir-core         │  /crates/flux-ir/src/**                                │
├──────────────────┼───────────────────────────────────────────────────────┤
│  ir-serde        │  /crates/flux-ir-serde/src/**                          │
├──────────────────┼───────────────────────────────────────────────────────┤
│  differ          │  /crates/flux-differ/src/**                            │
├──────────────────┼───────────────────────────────────────────────────────┤
│  devserver       │  /crates/flux-devserver/src/**                         │
├──────────────────┼───────────────────────────────────────────────────────┤
│  codegen-swift   │  /crates/flux-codegen-swift/src/**                     │
├──────────────────┼───────────────────────────────────────────────────────┤
│  codegen-kotlin  │  /crates/flux-codegen-kotlin/src/**                     │
├──────────────────┼───────────────────────────────────────────────────────┤
│  cli             │  /crates/flux-cli/src/**                               │
├──────────────────┼───────────────────────────────────────────────────────┤
│  ios-runtime     │  /runtimes/ios/**                                      │
├──────────────────┼───────────────────────────────────────────────────────┤
│  android-runtime │  /runtimes/android/**                                 │
├──────────────────┼───────────────────────────────────────────────────────┤
│  swift-adapters  │  /adapters/ui-swift/**                                 │
├──────────────────┼───────────────────────────────────────────────────────┤
│  kotlin-adapters │  /adapters/ui-kotlin/**                                │
├──────────────────┼───────────────────────────────────────────────────────┤
│  stdlib          │  /stdlib/**                                            │
├──────────────────┼───────────────────────────────────────────────────────┤
│  parity-tests    │  /tests/parity/**                                      │
├──────────────────┼───────────────────────────────────────────────────────┤
│  docs            │  /docs/**                                              │
└──────────────────┴───────────────────────────────────────────────────────┘
```

### 1.3 Interface Contract Strategy

Agents communicate **only** through:

1. **Public types in `flux-syntax`** — the shared vocabulary. All cross-crate type definitions live here. No agent defines types that another agent needs; they all use `flux-syntax`.

2. **Trait/function signatures in `flux-syntax`** — interfaces are declared here. Implementations live in the owning crate.

3. **The spec appendices** — wire protocol (Appendix D), IR schema (Appendix C), VM instruction set (Appendix E), adapter contracts (Appendix F). Platform agents (iOS/Android) code against these specs directly, not against Rust types.

4. **Cargo.toml dependency declarations** — set up in Phase 0 by the foundation agent. Never modified by other agents.

### 1.4 What `flux-syntax` Contains (The Shared Foundation)

```rust
// crates/flux-syntax/src/lib.rs

// === IDs ===
pub type NodeId = u32;
pub type HandlerId = u32;
pub type SignalId = u32;
pub type ComponentId = u32;
pub type StringId = u32;
pub type FileId = u32;
pub type TypeId = u32;
pub type PropIdx = u16;
pub type InstanceId = u32;

// === Source spans ===
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
pub struct Span {
    pub file_id: FileId,
    pub start: u32,
    pub end: u32,
}

// === Values (shared between IR, VM, wire protocol) ===
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

// === Type representation ===
#[derive(Clone, Debug)]
pub enum TypeKind {
    Int, Float, Bool, String, Unit,
    List(Box<TypeKind>),
    Map(Box<TypeKind>, Box<TypeKind>),
    Option(Box<TypeKind>),
    Fn(Vec<TypeKind>, Box<TypeKind>),
    Record(Vec<(StringId, TypeKind)>),
    Variant(StringId, Vec<TypeKind>),
    Var(u32),
    Constrained(u32, Vec<StringId>),
}

// === String table ===
pub struct StringTable {
    pub strings: Vec<String>,
    pub lookup: HashMap<String, StringId>,
}

// === Node kinds (shared between IR, differ, codegen) ===
#[repr(u8)]
pub enum NodeKind {
    Component = 0,
    Primitive = 1,
    ForEach = 2,
    If = 3,
    Match = 4,
    Router = 5,
    Screen = 6,
}

// === Patch types (shared between differ, ir-serde, devserver) ===
pub enum Patch {
    Replace { id: NodeId, node: NodeRef },
    Update { id: NodeId, props_diff: PropDiff },
    Insert { parent: NodeId, index: u16, node: NodeRef },
    Remove { id: NodeId },
    Reorder { parent: NodeId, keys: Vec<NodeId> },
    Handler { id: HandlerId, closure: ClosureRef },
}

// === Node reference (lightweight, points to arena) ===
pub struct NodeRef {
    pub id: NodeId,
    pub kind: NodeKind,
    pub component_id: ComponentId,
    pub props: Props,
    pub children: Vec<Child>,
    pub handlers: Vec<HandlerId>,
    pub span: Span,
}

pub enum Child {
    Node(NodeId),
    Splice { items: Vec<(u64, NodeId)> },
}

pub struct Props {
    pub fields: Vec<(PropIdx, Value)>,
    pub hash: u64,
}

pub struct PropDiff {
    pub changes: Vec<(PropIdx, Value)>,
    pub removals: Vec<PropIdx>,
}

pub struct ClosureRef {
    pub hash: u64,
    pub bytecode_offset: u32,
    pub bytecode_len: u16,
    pub captured_signals: Vec<SignalId>,
    pub span: Span,
}
```

### 1.5 Modification Rules

| Rule | Detail |
|---|---|
| **R1** | An agent may ONLY create/modify files within their owned directories. |
| **R2** | An agent may NEVER modify `Cargo.toml` (any of them). All dependency declarations are fixed in Phase 0. |
| **R3** | An agent may NEVER modify files in another agent's directory. Cross-crate communication is via `flux-syntax` public types only. |
| **R4** | An agent may READ any file in the repo (for understanding interfaces), but may not WRITE outside their ownership. |
| **R5** | If an agent discovers that `flux-syntax` is missing a type they need, they MUST NOT add it themselves. They flag it to the orchestrator, who batches `flux-syntax` updates in a dedicated pass. |
| **R6** | Each crate's `lib.rs` starts as a stub (empty or `unimplemented!()`). The owning agent replaces the stub with real code. No other agent touches `lib.rs`. |
| **R7** | Platform agents (iOS/Android/adapters) work against the spec appendices, not against Rust types. They define their own platform-native equivalents. |
| **R8** | The `stdlib` agent writes `.flux` source files only. They may run `flux parse` (once available) to validate, but do not modify the parser or type checker. |

### 1.6 Phase Dependency Graph

```
Phase 0 (1 agent: foundation)
    │
    │  flux-syntax fully implemented
    │  all other crates have stubs + correct Cargo.toml
    │
    ▼
Phase 1 (up to 7 agents in parallel)
    ┌───────────────┬───────────────┬───────────────┬───────────────┐
    │ parser        │ ir-core       │ ios-runtime   │ android-runtime│
    │               │               │               │               │
    │ swift-adapters│ kotlin-adapters│ stdlib        │               │
    └───────────────┴───────────────┴───────────────┴───────────────┘
    │
    │  parser done, ir-core done
    │
    ▼
Phase 2 (up to 3 agents in parallel)
    ┌───────────────┬───────────────┬───────────────┐
    │ typechecker   │ ir-serde      │ differ        │
    └───────────────┴───────────────┴───────────────┘
    │
    │  all Rust core crates done
    │
    ▼
Phase 3 (up to 3 agents in parallel)
    ┌───────────────┬───────────────┬───────────────┐
    │ devserver     │ codegen-swift │ codegen-kotlin│
    └───────────────┴───────────────┴───────────────┘
    │
    ▼
Phase 4 (1 agent)
    ┌───────────────┐
    │ cli           │
    └───────────────┘
    │
    ▼
Phase 5 (1 agent)
    ┌───────────────┐
    │ parity-tests  │
    └───────────────┘
```

### 1.7 Conflict Prevention Checklist

Before spawning agents, the orchestrator verifies:

- [ ] **D1:** No two agents share a directory.
- [ ] **D2:** All `Cargo.toml` files are pre-created with correct dependencies.
- [ ] **D3:** `flux-syntax` is fully implemented and compiles.
- [ ] **D4:** All other crates have `lib.rs` stubs that compile (`pub fn _placeholder() {}`).
- [ ] **D5:** The workspace compiles end-to-end (`cargo check` passes).
- [ ] **D6:** Each agent's issue specifies the exact directory boundary.
- [ ] **D7:** Each agent's issue specifies which `flux-syntax` types they consume.
- [ ] **D8:** Platform agents' issues reference the spec appendices (C, D, E, F) for interface definitions.

---

## Part 2: The Issues

### Phase 0 — Foundation (Sequential, 1 Agent)

---

#### FLUX-001: Foundation skeleton and `flux-syntax` crate

**Agent:** foundation  
**Owns:** `/Cargo.toml`, `/rust-toolchain.toml`, `/.gitignore`, `/crates/flux-syntax/**`, all `/crates/*/Cargo.toml`, all `/crates/*/src/lib.rs` (stubs)  
**Depends on:** Nothing  
**Estimated effort:** 4 hours  

**Scope:**
1. Create the Cargo workspace root `Cargo.toml` with all 10 crates listed.
2. Create `rust-toolchain.toml` (Rust 1.75+, edition 2021).
3. Create `.gitignore` (standard Rust + Xcode + Gradle).
4. Implement `flux-syntax` crate fully:
   - All ID types (`NodeId`, `HandlerId`, `SignalId`, etc.)
   - `Span` type
   - `Value` enum
   - `TypeKind` enum
   - `StringTable` with `intern()` and `lookup()`
   - `NodeKind` enum
   - `Patch` enum
   - `NodeRef`, `Child`, `Props`, `PropDiff`, `ClosureRef` structs
5. Create all other crates with:
   - Correct `Cargo.toml` with dependencies declared
   - `src/lib.rs` with `pub fn _placeholder() {}` (compiles)
6. Verify `cargo check` passes for the entire workspace.

**Crate dependency declarations (pre-wired in Cargo.toml):**

```toml
# crates/flux-parser/Cargo.toml
[dependencies]
flux-syntax = { path = "../flux-syntax" }
pest = "2.7"
pest_derive = "2.7"

# crates/flux-types/Cargo.toml
[dependencies]
flux-syntax = { path = "../flux-syntax" }
flux-parser = { path = "../flux-parser" }

# crates/flux-ir/Cargo.toml
[dependencies]
flux-syntax = { path = "../flux-syntax" }

# crates/flux-ir-serde/Cargo.toml
[dependencies]
flux-syntax = { path = "../flux-syntax" }
flux-ir = { path = "../flux-ir" }
rmp-serde = "1.1"        # MessagePack
blake3 = "1.5"           # Content addressing

# crates/flux-differ/Cargo.toml
[dependencies]
flux-syntax = { path = "../flux-syntax" }
flux-ir = { path = "../flux-ir" }

# crates/flux-devserver/Cargo.toml
[dependencies]
flux-syntax = { path = "../flux-syntax" }
flux-parser = { path = "../flux-parser" }
flux-types = { path = "../flux-types" }
flux-ir = { path = "../flux-ir" }
flux-ir-serde = { path = "../flux-ir-serde" }
flux-differ = { path = "../flux-differ" }
tokio = { version = "1.36", features = ["full"] }
tokio-tungstenite = "0.21"
notify = "6.1"
axum = "0.7"             # Asset HTTP server

# crates/flux-codegen-swift/Cargo.toml
[dependencies]
flux-syntax = { path = "../flux-syntax" }
flux-ir = { path = "../flux-ir" }

# crates/flux-codegen-kotlin/Cargo.toml
[dependencies]
flux-syntax = { path = "../flux-syntax" }
flux-ir = { path = "../flux-ir" }

# crates/flux-cli/Cargo.toml
[dependencies]
flux-devserver = { path = "../flux-devserver" }
flux-codegen-swift = { path = "../flux-codegen-swift" }
flux-codegen-kotlin = { path = "../flux-codegen-kotlin" }
clap = { version = "4.5", features = ["derive"] }
```

**Acceptance criteria:**
- `cargo check` passes for the entire workspace.
- `flux-syntax` crate has all types from Appendix C §C.1 implemented.
- Every other crate has a `lib.rs` that compiles (stub).
- `cargo doc --open` for `flux-syntax` shows all types.

---

### Phase 1 — Independent Work (Up to 7 Agents in Parallel)

---

#### FLUX-002: Parser crate (`flux-parser`)

**Agent:** parser  
**Owns:** `/crates/flux-parser/src/**`  
**Depends on:** FLUX-001  
**Estimated effort:** 2 days  

**Scope:**
1. Write the pest grammar file (`src/flux.pest`) based on Appendix B.
2. Implement the parser that produces a typed AST from `.flux` source.
3. AST types live in this crate (parser-specific: `ASTNode`, `Expr`, `Decl`, etc.).
4. Implement `pub fn parse(source: &str, file_id: FileId) -> Result<AST, ParseError>`.
5. Implement error reporting with source spans (Rust-style diagnostics).

**Types consumed from `flux-syntax`:** `Span`, `FileId`, `StringId`, `StringTable`.

**Types produced (public API):**
```rust
pub struct AST {
    pub statements: Vec<Decl>,
    pub string_table: StringTable,
    pub file_id: FileId,
}

pub enum Decl {
    Component { name: StringId, generics: Vec<GenericParam>, props: Vec<PropDecl>, body: Block, span: Span },
    Function { name: StringId, generics: Vec<GenericParam>, params: Vec<Param>, return_type: Option<TypeAnnotation>, body: Block, span: Span },
    Type { name: StringId, generics: Vec<GenericParam>, variants: Vec<Variant>, span: Span },
    Trait { name: StringId, generics: Vec<GenericParam>, methods: Vec<MethodDecl>, span: Span },
    Capability { name: StringId, methods: Vec<MethodDecl>, span: Span },
    Import { name: StringId, path: String, span: Span },
    Use { path: Vec<StringId>, is_glob: bool, span: Span },
}
```

**Acceptance criteria:**
- Parses the 10 grammar examples from Appendix B.3.
- Error messages include file:line:col.
- `cargo test` passes for all grammar examples.
- `cargo bench`: < 5 ms for 500-line file.

---

#### FLUX-003: IR core crate (`flux-ir`)

**Agent:** ir-core  
**Owns:** `/crates/flux-ir/src/**`  
**Depends on:** FLUX-001  
**Estimated effort:** 2 days  

**Scope:**
1. Implement `IRArena` with struct-of-arrays layout (Appendix C §C.1).
2. Implement `pack()` and `get()` methods for arena.
3. Implement `ComponentInstance` and `InstanceRegistry` for tracking component instances.
4. Implement the lowering pass: `pub fn lower(ast: &AST) -> IRArena`.
   - Note: full lowering requires type info. For now, implement structural lowering (AST → IR without type resolution). Type annotations are carried through as metadata.
5. Implement `ClosureIR` struct (bytecode container).
6. Implement node ID derivation: `fn compute_node_id(parent: NodeId, kind: NodeKind, span: Span, key: Option<u64>) -> NodeId`.

**Types consumed from `flux-syntax`:** All types (this is the primary consumer).

**Types produced (public API):**
```rust
pub struct IRArena { /* ... Appendix C §C.1 ... */ }
pub struct ComponentInstance { /* ... */ }
pub struct InstanceRegistry { /* ... */ }
pub struct ClosureIR { /* ... */ }
pub fn lower(ast: &AST) -> IRArena;
pub fn compute_node_id(parent: NodeId, kind: NodeKind, span: Span, key: Option<u64>) -> NodeId;
```

**Acceptance criteria:**
- `IRArena` packs and unpacks 100 nodes correctly.
- `compute_node_id` produces stable IDs (same source → same ID).
- `lower()` produces valid arena from parsed AST.
- `cargo bench`: pack 100 nodes < 1 ms.

---

#### FLUX-004: iOS host app (`runtimes/ios`)

**Agent:** ios-runtime  
**Owns:** `/runtimes/ios/**`  
**Depends on:** FLUX-001 (spec only — no Rust deps)  
**Estimated effort:** 5 days  

**Scope:**
1. Set up Xcode project (`FluxApp.xcodeproj`).
2. Implement `FluxExecutor` class (background queue, main dispatch).
3. Implement `FluxBytecodeVM` — register-based interpreter (Appendix E).
4. Implement `SignalGraph` — SolidJS-style fine-grained reactivity.
5. Implement `ShadowTree` + `ShadowNode`.
6. Implement `ShadowTreeReconciler` — keyed reconciliation.
7. Implement `WebSocketClient` — connects to `ws://localhost:7331`.
8. Implement `FrameDeserializer` — MessagePack binary frame parser (Appendix D).
9. Implement `FluxRootView: UIViewRepresentable` — SwiftUI entry point.
10. Implement `LaunchScreen` — shown until first frame.
11. Implement background lifecycle (`applicationWillResignActive` / `applicationDidBecomeActive`).
12. Implement error overlay (red banner with source span).
13. Implement VM safety: gas meter (100k budget), memory cap (16 MB), top-level catch.

**Interfaces coded against:** Appendix C (IR schema), Appendix D (wire protocol), Appendix E (VM ISA), Appendix F (adapter contracts).

**Acceptance criteria:**
- App launches, connects to `ws://localhost:7331`, receives `Init` frame, renders first frame.
- Tap on a button calls `executor.dispatch(handlerId)`.
- VM evaluates a simple handler (`count = count + 1`) correctly.
- Signal propagation updates dependent derived cells.
- Reconciler applies `Update` patch (text change) to `UILabel` without recreating the view.
- Gas exhaustion shows red banner, doesn't crash.
- `applicationWillResignActive` shows "Dev paused" indicator.

---

#### FLUX-005: Android host app (`runtimes/android`)

**Agent:** android-runtime  
**Owns:** `/runtimes/android/**`  
**Depends on:** FLUX-001 (spec only — no Rust deps)  
**Estimated effort:** 5 days  

**Scope:** Same as FLUX-004 but in Kotlin, targeting Android API 24+.

1. Set up Gradle project (`build.gradle`, `settings.gradle`).
2. Implement `FluxExecutor` class (background coroutine, main thread dispatch).
3. Implement `FluxBytecodeVM` — register-based interpreter (Appendix E).
4. Implement `SignalGraph` — SolidJS-style fine-grained reactivity.
5. Implement `ShadowTree` + `ShadowNode`.
6. Implement `ShadowTreeReconciler` — keyed reconciliation.
7. Implement `WebSocketClient` — OkHttp WebSocket.
8. Implement `FrameDeserializer` — MessagePack binary frame parser (Appendix D).
9. Implement `FluxRoot` composable — `AndroidView` entry point.
10. Implement lifecycle handling (`onPause` / `onResume`).
11. Implement error overlay.
12. Implement VM safety.

**Acceptance criteria:** Same as FLUX-004, on Android emulator (Pixel 5, API 34).

---

#### FLUX-006: Swift adapters (`adapters/ui-swift`)

**Agent:** swift-adapters  
**Owns:** `/adapters/ui-swift/**`  
**Depends on:** FLUX-001 (spec only)  
**Estimated effort:** 3 days  

**Scope:**
Implement the 7 dev-mode adapters (Appendix F):
1. `FluxTextAdapter` → `UILabel`
2. `FluxButtonAdapter` → `UIButton`
3. `FluxColumnAdapter` → `UIStackView(axis: .vertical)`
4. `FluxRowAdapter` → `UIStackView(axis: .horizontal)`
5. `FluxTextFieldAdapter` → `UITextField`
6. `FluxRouterAdapter` → `UINavigationController`
7. `FluxScreenAdapter` → `UIViewController`

Each adapter implements:
```swift
protocol FluxAdapter: AnyObject {
    associatedtype NativeView: UIView
    func create() -> NativeView
    func update(_ view: NativeView, from old: Props, to new: Props)
    func setChildren(_ children: [UIView], on view: NativeView)
    func bindHandler(_ id: HandlerId, event: HandlerEvent, on view: NativeView)
    func destroy(_ view: NativeView)
}
```

**Props types** are defined locally in Swift (mirroring `flux_syntax::Value`):
```swift
enum FluxValue {
    case int(Int64)
    case float(Double)
    case bool(Bool)
    case str(String)  // resolved from string table
    case handlerRef(HandlerId)
    case null
    case list([FluxValue])
    case record([(UInt16, FluxValue)])
}
```

**Acceptance criteria:**
- Each adapter creates, updates, and destroys its native view.
- Button tap calls `executor.dispatch(boundHandlerId)`.
- Column manages `arrangedSubviews` with keyed diff.
- Router pushes/pops `UIViewController` with state preservation.

---

#### FLUX-007: Kotlin adapters (`adapters/ui-kotlin`)

**Agent:** kotlin-adapters  
**Owns:** `/adapters/ui-kotlin/**`  
**Depends on:** FLUX-001 (spec only)  
**Estimated effort:** 3 days  

**Scope:** Same as FLUX-006 but in Kotlin, for Android.

Implement the 7 dev-mode adapters:
1. `FluxTextAdapter` → `TextView`
2. `FluxButtonAdapter` → `android.widget.Button`
3. `FluxColumnAdapter` → `LinearLayout(orientation: VERTICAL)`
4. `FluxRowAdapter` → `LinearLayout(orientation: HORIZONTAL)`
5. `FluxTextFieldAdapter` → `EditText`
6. `FluxRouterAdapter` → `FrameLayout` stack
7. `FluxScreenAdapter` → `FrameLayout` child

Each adapter implements:
```kotlin
interface FluxAdapter {
    fun create(): View
    fun update(view: View, old: Props, new: Props)
    fun setChildren(children: List<View>, on view: View)
    fun bindHandler(id: HandlerId, event: HandlerEvent, on view: View)
    fun destroy(view: View)
}
```

**Acceptance criteria:** Same as FLUX-006, on Android emulator.

---

#### FLUX-008: Standard library (`stdlib`)

**Agent:** stdlib  
**Owns:** `/stdlib/**`  
**Depends on:** FLUX-001 (spec only)  
**Estimated effort:** 1 day  

**Scope:**
Write `.flux` source files for the stdlib:
1. `stdlib/prelude.flux` — default imports (types, traits, primitives).
2. `stdlib/text.flux` — `Text` component declaration.
3. `stdlib/button.flux` — `Button` component declaration.
4. `stdlib/column.flux` — `Column` component declaration.
5. `stdlib/row.flux` — `Row` component declaration.
6. `stdlib/text_field.flux` — `TextField` component declaration.
7. `stdlib/router.flux` — `Router` and `Screen` component declarations.
8. `stdlib/color.flux` — `Color` type and constants.
9. `stdlib/font.flux` — `Font` type and constants.
10. `stdlib/traits.flux` — `Numeric`, `Eq`, `Show` trait declarations.
11. `stdlib/capabilities.flux` — `Storage`, `Camera` (declarations only).
12. `stdlib/platform.flux` — `platform` built-in value.

**Acceptance criteria:**
- All files are valid `.flux` syntax (parseable once FLUX-002 is done).
- Every component declares its props with types.
- `Color` has constants: `red`, `green`, `blue`, `black`, `white`.
- `Font` has presets: `body`, `title`, `caption`.

---

### Phase 2 — Core Layer (Up to 3 Agents in Parallel)

---

#### FLUX-009: Type checker crate (`flux-types`)

**Agent:** typechecker  
**Owns:** `/crates/flux-types/src/**`  
**Depends on:** FLUX-001, FLUX-002  
**Estimated effort:** 3 days  

**Scope:**
1. Implement bidirectional type checking algorithm.
2. Implement let-polymorphism (generalize `let` bindings).
3. Implement type class resolution (find trait instances).
4. Implement ADT exhaustiveness checking.
5. Implement monomorphization tracking (record all generic instantiations).
6. Implement `pub fn type_check(ast: &AST) -> Result<TypedAST, TypeError>`.
7. Implement error reporting with source spans.

**Types consumed from `flux-syntax`:** `Span`, `TypeKind`, `StringId`, `StringTable`.
**Types consumed from `flux-parser`:** `AST`, `Decl`, `Expr`.
**Types produced:**
```rust
pub struct TypedAST {
    pub ast: AST,
    pub types: HashMap<NodeId, TypeKind>,
    pub instantiations: Vec<GenericInstantiation>,
}
pub struct GenericInstantiation {
    pub generic_id: StringId,
    pub type_args: Vec<TypeKind>,
    pub span: Span,
}
```

**Acceptance criteria:**
- Type-checks all 10 grammar examples from Appendix B.3.
- Catches type mismatches with precise error messages (file:line:col).
- Records all generic instantiations for monomorphization.
- `cargo bench`: < 3 ms for 500-line file.

---

#### FLUX-010: IR serialization crate (`flux-ir-serde`)

**Agent:** ir-serde  
**Owns:** `/crates/flux-ir-serde/src/**`  
**Depends on:** FLUX-001, FLUX-003  
**Estimated effort:** 2 days  

**Scope:**
1. Implement `pub fn serialize_patches(patches: &[Patch], string_table: &StringTable) -> Vec<u8>` using MessagePack.
2. Implement `pub fn deserialize_frame(data: &[u8]) -> Frame` on the host side (but this is Rust — platform deserializers are in Swift/Kotlin).
3. Implement content addressing: `pub fn hash_props(props: &Props) -> u64` (BLAKE3).
4. Implement content addressing: `pub fn hash_closure(closure: &ClosureIR) -> u64`.
5. Implement the `Frame` struct (Appendix D §D.1).
6. Implement `Hello` and `Init` frame construction.
7. Implement string table delta serialization.

**Types consumed from `flux-syntax`:** `Patch`, `NodeRef`, `Child`, `Props`, `ClosureRef`, `StringTable`, `Value`.
**Types consumed from `flux-ir`:** `IRArena`, `ClosureIR`.

**Acceptance criteria:**
- Round-trip: serialize → deserialize → equals original.
- BLAKE3 hashes are deterministic.
- Frame for 50-node tree is < 20 KB.
- `cargo bench`: serialize 50-node patch < 1 ms.

---

#### FLUX-011: Differ crate (`flux-differ`)

**Agent:** differ  
**Owns:** `/crates/flux-differ/src/**`  
**Depends on:** FLUX-001, FLUX-003  
**Estimated effort:** 2 days  

**Scope:**
1. Implement keyed reconciliation algorithm (udomdiff-style).
2. Implement `pub fn diff(old: &IRArena, new: &IRArena) -> Vec<Patch>`.
3. Implement stable node ID derivation (uses `flux_ir::compute_node_id`).
4. Implement handler-only diff optimization (if only handler bodies changed, produce only `Handler` patches).
5. Implement prop diff (compare old props vs new props, produce `PropDiff`).
6. Implement reorder detection (list reordering → `Reorder` patch).

**Types consumed from `flux-syntax`:** `Patch`, `PropDiff`, `NodeId`, `NodeRef`.
**Types consumed from `flux-ir`:** `IRArena`.

**Acceptance criteria:**
- Diff of identical trees produces empty `Patch[]`.
- Diff of handler-body change produces single `Handler` patch.
- Diff of inserted sibling produces single `Insert` patch.
- Diff of reordered list produces `Reorder` patch (not remove+insert).
- `cargo bench`: diff 50-node tree < 1 ms.

---

### Phase 3 — Backend Layer (Up to 3 Agents in Parallel)

---

#### FLUX-012: Dev server crate (`flux-devserver`)

**Agent:** devserver  
**Owns:** `/crates/flux-devserver/src/**`  
**Depends on:** FLUX-001, FLUX-002, FLUX-003, FLUX-009, FLUX-010, FLUX-011  
**Estimated effort:** 3 days  

**Scope:**
1. Implement WebSocket server (tokio-tungstenite) on port 7331.
2. Implement file watcher (notify crate) with 50 ms debounce.
3. Implement asset HTTP server (axum) on port 7332.
4. Implement the dev server pipeline: file change → parse → type_check → lower → diff → serialize → send frame.
5. Implement handshake protocol (`Hello` → `Init`).
6. Implement reconnect protocol (host disconnect → retry → resend `Init`).
7. Implement protocol versioning.
8. Implement capability versioning.
9. Implement logging (`--log-level`).
10. Implement profiling (`--profile`).
11. Implement error frame sending (parse/type errors → error frame to host).
12. Implement frame coalescing (multiple saves within 16 ms → one frame).

**Types consumed from all prior crates.**

**Acceptance criteria:**
- File save → frame sent in < 50 ms (excluding parse/typecheck/lower time).
- Handshake completes in < 10 ms.
- Reconnect works after host crash.
- `--log-level=debug` prints IR diffs.
- `--profile` prints phase timings.

---

#### FLUX-013: Swift codegen crate (`flux-codegen-swift`)

**Agent:** codegen-swift  
**Owns:** `/crates/flux-codegen-swift/src/**`  
**Depends on:** FLUX-001, FLUX-003  
**Estimated effort:** 3 days  

**Scope:**
1. Implement `pub fn codegen(arena: &IRArena) -> String` that produces Swift source code.
2. Map `.flux` constructs to SwiftUI (Appendix F, ADR-0003):
   - `component` → `struct ViewName: View`
   - `state` → `@State private var`
   - `Column(gap: N)` → `VStack(spacing: N)`
   - `Button(text: T, onClick: H)` → `Button(T) { H }`
   - `when/otherwise` → `if/else` inside `@ViewBuilder`
   - `ForEach` → `ForEach(items, id: key) { item in ... }`
   - `Router` → `NavigationStack(path: $path)`
   - `match` → `switch`
3. Implement generic specialization → Swift generics.
4. Implement `@pure` annotation → plain `struct` (no `@State`).
5. Format output with swift-format conventions (manual, no external dependency).

**Acceptance criteria:**
- Generates compilable Swift for all 10 grammar examples.
- Generated Swift is readable by a Swift developer unfamiliar with `.flux`.
- Generic component generates `struct Counter<T: Numeric>: View`.

---

#### FLUX-014: Kotlin codegen crate (`flux-codegen-kotlin`)

**Agent:** codegen-kotlin  
**Owns:** `/crates/flux-codegen-kotlin/src/**`  
**Depends on:** FLUX-001, FLUX-003  
**Estimated effort:** 3 days  

**Scope:** Same as FLUX-013 but targeting Kotlin/Jetpack Compose.

1. Implement `pub fn codegen(arena: &IRArena) -> String`.
2. Map `.flux` constructs to Compose (Appendix F):
   - `component` → `@Composable fun`
   - `state` → `var x by remember { mutableStateOf(...) }`
   - `Column(gap: N)` → `Column(spacing = N.dp)`
   - `Button(text: T, onClick: H)` → `Button(onClick = { H }) { Text(T) }`
   - `ForEach` → `items(list, key = { it.id }) { item -> ... }`
   - `Router` → `NavHost(navController, ...)`
3. Implement generic specialization → Kotlin generics (erased, `inline fun <reified T>` for hot paths).

**Acceptance criteria:**
- Generates compilable Kotlin for all 10 grammar examples.
- Generated Kotlin is readable.

---

### Phase 4 — Integration (1 Agent)

---

#### FLUX-015: CLI crate (`flux-cli`)

**Agent:** cli  
**Owns:** `/crates/flux-cli/src/**`  
**Depends on:** FLUX-012, FLUX-013, FLUX-014  
**Estimated effort:** 1 day  

**Scope:**
1. Implement `flux init <name>` — scaffold project (create `flux.toml`, `src/`, `platforms/`, etc.).
2. Implement `flux dev` — start dev server + file watcher.
3. Implement `flux build --platform ios` — codegen Swift + `xcodebuild`.
4. Implement `flux build --platform android` — codegen Kotlin + `gradle build`.
5. Implement `flux doc` — emit JSON schema of stdlib API.
6. CLI argument parsing with `clap`.

**Acceptance criteria:**
- `flux init myapp` creates a valid project.
- `flux dev` starts the dev server and prints "Listening on ws://localhost:7331".
- `flux build --platform ios` produces `.swift` files in `platforms/ios/Generated/`.
- `flux doc` produces valid JSON.

---

### Phase 5 — Testing (1 Agent)

---

#### FLUX-016: Parity test harness (`tests/parity`)

**Agent:** parity-tests  
**Owns:** `/tests/parity/**`  
**Depends on:** ALL prior issues  
**Estimated effort:** 3 days  

**Scope:**
1. Implement `simulate_dev(ir: &IRArena, actions: &[Action]) -> State` — simulate VM execution in Rust.
2. Implement `run_swift(swift_source: &str, actions: &[Action]) -> State` — compile and run generated Swift in a test harness.
3. Implement `run_kotlin(kotlin_source: &str, actions: &[Action]) -> State` — compile and run generated Kotlin.
4. Write parity tests for:
   - Counter (tap → state change)
   - Form (10 fields, 10 changes)
   - Navigation (push → edit → pop)
   - Generic component (`Counter[Int]` and `Counter[Float]`)
   - `@pure` component (skip optimization)
5. Write performance benchmarks:
   - Save-to-pixels latency (50 nodes)
   - Tap-to-state-change latency
   - Memory growth over 1000 edits

**Acceptance criteria:**
- All parity tests pass (dev state == release state).
- Performance benchmarks meet targets from §32.

---

## Part 3: Agent Spawning Plan

### Spawn Batch 1 (Phase 0, 1 agent)
```
delegate_task:
  - id: foundation
  - issue: FLUX-001
  - model: claude-sonnet-4-20250514
  - system_prompt: |
      You are building the foundation of the Flux project.
      Read /docs/spec.md and /docs/appendices.md for context.
      You OWN: /Cargo.toml, /rust-toolchain.toml, /.gitignore,
      /crates/flux-syntax/**, /crates/*/Cargo.toml,
      /crates/*/src/lib.rs (stubs only).
      You MUST NOT create files outside these directories.
      Create the workspace, implement flux-syntax fully,
      stub all other crates, verify cargo check passes.
```

### Spawn Batch 2 (Phase 1, up to 7 agents)
```
delegate_task:
  - id: parser
  - issue: FLUX-002
  - depends_on: foundation
    
  - id: ir-core
  - issue: FLUX-003
  - depends_on: foundation
    
  - id: ios-runtime
  - issue: FLUX-004
  - depends_on: foundation
  - system_prompt: |
      Code against Appendix C (IR schema), D (wire protocol),
      E (VM ISA), F (adapter contracts). No Rust dependencies.
    
  - id: android-runtime
  - issue: FLUX-005
  - depends_on: foundation
    
  - id: swift-adapters
  - issue: FLUX-006
  - depends_on: foundation
    
  - id: kotlin-adapters
  - issue: FLUX-007
  - depends_on: foundation
    
  - id: stdlib
  - issue: FLUX-008
  - depends_on: foundation
```

### Spawn Batch 3 (Phase 2, up to 3 agents)
```
delegate_task:
  - id: typechecker
  - issue: FLUX-009
  - depends_on: [foundation, parser]
    
  - id: ir-serde
  - issue: FLUX-010
  - depends_on: [foundation, ir-core]
    
  - id: differ
  - issue: FLUX-011
  - depends_on: [foundation, ir-core]
```

### Spawn Batch 4 (Phase 3, up to 3 agents)
```
delegate_task:
  - id: devserver
  - issue: FLUX-012
  - depends_on: [foundation, parser, ir-core, typechecker, ir-serde, differ]
    
  - id: codegen-swift
  - issue: FLUX-013
  - depends_on: [foundation, ir-core]
    
  - id: codegen-kotlin
  - issue: FLUX-014
  - depends_on: [foundation, ir-core]
```

### Spawn Batch 5 (Phase 4, 1 agent)
```
delegate_task:
  - id: cli
  - issue: FLUX-015
  - depends_on: [devserver, codegen-swift, codegen-kotlin]
```

### Spawn Batch 6 (Phase 5, 1 agent)
```
delegate_task:
  - id: parity-tests
  - issue: FLUX-016
  - depends_on: [all]
```

---

## Part 4: Conflict Prevention Verification

### Pre-spawn checklist (orchestrator runs this before each batch):

```
□ All prior-phase issues are marked DONE.
□ git status is clean (no uncommitted changes).
□ cargo check passes (for Rust crates).
□ No two agents in the batch share a directory.
□ Each agent's issue specifies exact directory boundary.
□ Each agent's system_prompt references the correct spec appendices.
□ flux-syntax has all types the agent needs (if not, update flux-syntax first).
□ Cargo.toml dependencies are pre-wired (agents don't modify Cargo.toml).
```

### Post-merge checklist (orchestrator runs this after each batch):

```
□ git merge succeeds with no conflicts.
□ cargo check passes.
□ No agent modified files outside their ownership.
□ No agent modified any Cargo.toml.
□ All stubs have been replaced with real code (check lib.rs files).
```

---

This plan gives you **16 issues** across **6 phases**, with **up to 7 agents in parallel** during Phase 1, **up to 3 in Phase 2 and 3**, and zero file-level conflicts at any point. Each agent owns a disjoint directory subtree, communicates only through `flux-syntax` types or spec appendices, and never touches another agent's files.
