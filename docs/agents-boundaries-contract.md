# Flux — Parallel Agent Boundary Contract & Issue Plan (v2)

| Field | Value |
|-------|-------|
| Project | Flux — Native Cross-Platform UI Development System |
| Document | Parallel Agent Boundary Contract & Issue Plan |
| Version | 2.0 (replaces v1 in full) |
| Status | Canonical — supersedes all prior versions |
| Companion docs | `/docs/spec/mlp-spec.md`, `/docs/spec/mlp-appendices.md`, `/AGENTS.md` |

> **Grounding note.** This contract is the *v1→v2 planning ledger*: its phase counts,
> issue list, and "What Changed in v2" bullets describe the dispatch plan that was
> executed. As of the current tree those phases have landed — the referenced crates
> (`flux-devserver`, `flux-codegen-*`, `flux-cli`, `flux-parity`), the three-host
> capability bridge (ADR-0045), and the signal-dep / prop-thunk reconciliation ladder
> (ADR-0027) are all implemented and verified (see `/CHANGELOG.md`). The text below
> is kept as the historical issue plan; the *realized* state is the code + CHANGELOG,
> not the phase numbering. When the phase labels below say "Phase N", read them as
> "issue batch N in the original dispatch", not as a status of work still to do.

---

## What Changed in v2 (Read This First)

1. **The three-VM reality is now explicit.** Production VMs are native Swift (`runtimes/ios`) and native Kotlin (`runtimes/android`) — there is no Rust VM in production. A Rust **reference VM** (`flux-vm-ref`) exists only as a test oracle. All three are kept in sync by **golden ISA vectors** (`/tests/isa-vectors/`), which are the behavioral source of truth — not any implementation.
2. **Golden ISA vectors added as a Phase 0 deliverable.** Runtime agents can now write VM conformance tests against shared fixtures instead of each inventing test cases (which guaranteed drift).
3. **Lowering split out of `flux-ir` into its own issue/phase.** v1 had `flux-ir` implement `lower(ast)` in Phase 1, but real lowering requires the type checker's `TypedAST` (Phase 2). This was a hidden circular dependency. Now: Phase 1 = arena only; Phase 3 = lowering.
4. **Codegen moved after lowering.** Codegen consumes lowered, type-annotated IR. v1 scheduled it alongside work it secretly depended on.
5. **Platform build manifests (XcodeGen `project.yml`, SPM `Package.swift`, Gradle files) are created once by foundation and frozen** — same rule as `Cargo.toml`. v1 only froze Cargo manifests, leaving Xcode/Gradle files as conflict magnets.
6. **Adapter↔runtime integration split into its own Phase 2 issues.** v1 pretended runtimes and adapters (built concurrently) would just work together. They can't test together until both exist.
7. **ADRs get a create-only carve-out** so agents can write ADRs per AGENTS.md §6 without violating directory ownership.
8. **Standard Definition of Done block** referencing AGENTS.md gates (TDD, fmt/clippy/lint/test) — applies to every issue, stated once.
9. **Wire-frame fixtures strategy defined**: runtimes test with hand-built frames in Phase 1, consume generated fixtures in Phase 6 via an env var — no boundary violations.
10. **CI agent added** so quality gates are enforced from Phase 1, not retrofitted.
11. **Stdlib validation issue added** (stdlib can't be parse-checked until the parser exists, since they're concurrent).

Issue count: **23 issues, 7 phases, max 9 parallel agents** (within your tool's 10-agent cap).

---

# Part 1: The Boundary Contract

This document defines the rules of engagement for parallel AI agents working on the Flux codebase. Every agent MUST adhere to these rules and to `/AGENTS.md`. Violations cause merge conflicts, broken builds, and wasted agent time.

## 1.1 The Core Principle

**Each agent owns a disjoint set of directories. No two concurrently-running agents' directories overlap. Agents never modify files outside their ownership. Agents never modify build manifests.**

## 1.2 Directory Ownership Map

```
┌──────────────────────────────────────────────────────────────────────────┐
│  AGENT OWNERSHIP MAP (v2)                                                │
├──────────────────┬───────────────────────────────────────────────────────┤
│  Agent           │  Owned directories (exclusive)                        │
├──────────────────┼───────────────────────────────────────────────────────┤
│  foundation      │  /Cargo.toml (workspace root)                          │
│  (Phase 0 only)  │  /rust-toolchain.toml, /.gitignore                     │
│                  │  /crates/flux-syntax/**                                │
│                  │  /crates/*/Cargo.toml (ALL, incl. dev-deps)           │
│                  │  /crates/*/src/lib.rs (stubs only)                     │
│                  │  /adapters/ui-swift/Package.swift (create once)        │
│                  │  /adapters/ui-kotlin/build.gradle.kts (create once)    │
│                  │  /runtimes/ios/project.yml (XcodeGen, create once)     │
│                  │  /runtimes/android/** manifests (create once)          │
│                  │  All of the above are FROZEN after Phase 0            │
├──────────────────┼───────────────────────────────────────────────────────┤
│  isa-vectors     │  /tests/isa-vectors/**  (FROZEN after Phase 0;        │
│  (Phase 0 only)  │   corrections go through the orchestrator)             │
├──────────────────┼───────────────────────────────────────────────────────┤
│  parser          │  /crates/flux-parser/src/**                            │
├──────────────────┼───────────────────────────────────────────────────────┤
│  ir-core         │  /crates/flux-ir/src/**                                │
├──────────────────┼───────────────────────────────────────────────────────┤
│  vm-ref          │  /crates/flux-vm-ref/src/**                            │
├──────────────────┼───────────────────────────────────────────────────────┤
│  typechecker     │  /crates/flux-types/src/**                             │
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
│  ios-runtime     │  /runtimes/ios/Sources/**, /runtimes/ios/Tests/**      │
├──────────────────┼───────────────────────────────────────────────────────┤
│  android-runtime │  /runtimes/android/app/src/**                          │
├──────────────────┼───────────────────────────────────────────────────────┤
│  swift-adapters  │  /adapters/ui-swift/Sources/**, .../Tests/**           │
├──────────────────┼───────────────────────────────────────────────────────┤
│  kotlin-adapters │  /adapters/ui-kotlin/src/**                            │
├──────────────────┼───────────────────────────────────────────────────────┤
│  stdlib          │  /stdlib/**                                            │
├──────────────────┼───────────────────────────────────────────────────────┤
│  ci              │  /.github/workflows/**, /scripts/**                    │
├──────────────────┼───────────────────────────────────────────────────────┤
│  parity          │  /tests/parity/**, /tests/wire-fixtures/**             │
├──────────────────┼───────────────────────────────────────────────────────┤
│  (orchestrator)  │  /docs/** except /docs/adr/** (create-only carve-out)  │
│                  │  /AGENTS.md, all frozen manifests, /tests/isa-vectors  │
└──────────────────┴───────────────────────────────────────────────────────┘
```

**Note on `/docs/adr/`:** Agents may **create** new ADR files there, named `<scope>-<slug>.md` (e.g., `parser-error-recovery.md`). No agent may **edit or delete** an existing ADR. This is the only cross-boundary write permitted (AGENTS.md §6 requires it). File-level disjointness via the scope prefix makes this conflict-free.

## 1.3 Interface Contract Strategy

Agents communicate **only** through:

1. **Public types in `flux-syntax`** — the shared Rust vocabulary. No agent defines types another Rust agent needs.
2. **The spec appendices** — wire protocol (D), IR schema (C), VM ISA (E), adapter contracts (F). Platform agents (iOS/Android/adapters) code against these directly, defining platform-native equivalents. They never depend on Rust code.
3. **Golden ISA vectors in `/tests/isa-vectors/`** — read-only fixtures pinning VM behavior across all three implementations (see §1.4).
4. **Build manifests pre-wired in Phase 0** — Cargo, SPM, XcodeGen, Gradle. Never modified by anyone afterward.

## 1.4 The Three-VM Problem and the Golden Vector Solution

The VM's semantics exist in **three implementations**:

| Implementation | Language | Role | Owner |
|---|---|---|---|
| `FluxBytecodeVM` (in `runtimes/ios`) | Swift | Production, iOS | ios-runtime |
| `FluxBytecodeVM` (in `runtimes/android`) | Kotlin | Production, Android | android-runtime |
| `flux-vm-ref` | Rust | Test oracle only — **never ships, never executes user code in production** | vm-ref |

Three implementations of one semantics is a parity hazard. **The ISA vectors, not any implementation, are the behavioral source of truth.** Enforcement:

1. `/tests/isa-vectors/*.json` — each vector is `bytecode + initial signals + payload → expected signals/registers/error/gas`.
2. All three implementations run the **same vectors** in their test suites. Any divergence fails in that implementation's tests.
3. Vectors are authored in Phase 0 from Appendix E only (no code exists yet — they are pure data). They are frozen afterward; corrections go through the orchestrator with all three implementations re-run.
4. The parity harness (Phase 6) additionally runs a small set of scenarios on the **real** Swift/Kotlin VMs in simulator/emulator, because the Rust oracle alone can't validate the production VMs end-to-end.

This is the standard pattern for multi-platform interpreter backends: duplicate the small interpreter per platform, pin behavior with a shared conformance suite.

## 1.5 What `flux-syntax` Contains (The Shared Foundation)

Normative shape (foundation implements fully in Phase 0):

```rust
// crates/flux-syntax/src/lib.rs
use std::collections::HashMap;

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
#[derive(Clone, Debug, PartialEq)]
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
#[derive(Clone, Debug, PartialEq)]
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
#[derive(Debug, Default)]
pub struct StringTable {
    pub strings: Vec<String>,
    pub lookup: HashMap<String, StringId>,
}
impl StringTable {
    pub fn intern(&mut self, s: &str) -> StringId { /* ... */ }
    pub fn lookup(&self, id: StringId) -> &str { /* ... */ }
}

// === Node kinds ===
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NodeKind {
    Component = 0,
    Primitive = 1,
    ForEach = 2,
    If = 3,
    Match = 4,
    Router = 5,
    Screen = 6,
}

// === VM opcodes (single Rust source of truth; Swift/Kotlin define
//     their own constants from Appendix E — the ISA vectors pin them) ===
pub mod opcode {
    pub const READ_SIGNAL: u8 = 0x10;
    pub const WRITE_SIGNAL: u8 = 0x11;
    pub const ADD_I64: u8 = 0x20;
    // ... all opcodes from Appendix E
}

// === IR node reference, patches, props, closures ===
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
    pub hash: u64, // BLAKE3 content hash
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

pub enum Patch {
    Replace { id: NodeId, node: NodeRef },
    Update { id: NodeId, props_diff: PropDiff },
    Insert { parent: NodeId, index: u16, node: NodeRef },
    Remove { id: NodeId },
    Reorder { parent: NodeId, keys: Vec<NodeId> },
    Handler { id: HandlerId, closure: ClosureRef },
}
```

## 1.6 Modification Rules

| Rule | Detail |
|---|---|
| **R1** | An agent may ONLY create/modify files within their owned directories. |
| **R2** | An agent may NEVER modify a build manifest: any `Cargo.toml`, `Package.swift`, `project.yml`, `build.gradle.kts`, `settings.gradle.kts`. All are created in Phase 0 and frozen. **Dev-dependencies are pre-wired by foundation** — if one is missing, flag to the orchestrator. |
| **R3** | An agent may NEVER modify files in another agent's directory. Rust cross-crate communication is via `flux-syntax` public types only. Platform code communicates with Rust via the spec appendices only. |
| **R4** | An agent may READ any file (to understand interfaces) but not WRITE outside ownership. |
| **R5** | If `flux-syntax` is missing a type you need, do NOT add it yourself. Flag it to the orchestrator, who batches `flux-syntax` updates in a dedicated pass and re-runs affected crates' tests. |
| **R6** | Every crate's `lib.rs` starts as a compiling stub. The owning agent replaces it. Same for platform source skeletons. |
| **R7** | Platform agents code against Appendices C–F, not against Rust types. They define platform-native equivalents (e.g., Swift `FluxValue` mirroring `flux_syntax::Value`). The ISA vectors and wire fixtures pin cross-implementation agreement. |
| **R8** | `/tests/isa-vectors/**` is read-only for everyone after Phase 0. The `stdlib` agent writes `.flux` files only and never modifies parser/typechecker code. |
| **R9** | ADRs: agents may CREATE `/docs/adr/<scope>-<slug>.md` files. Nobody edits or deletes existing ADRs. |
| **R10** | Runtime test suites must load wire fixtures from the path in env var `FLUX_WIRE_FIXTURES` when present and skip gracefully when absent. This lets Phase 6 drop in real fixtures without touching runtime code. |

## 1.7 Phase Dependency Graph

```
Phase 0 (2 agents in parallel — disjoint: Rust workspace vs. pure-data vectors)
    ┌──────────────────┬──────────────────┐
    │ foundation       │ isa-vectors      │
    └────────┬─────────┴────────┬─────────┘
             │  workspace +     │  ISA vectors
             │  flux-syntax +   │  (frozen)
             │  frozen manifests│
             ▼                  ▼
Phase 1 (9 agents in parallel)
    ┌────────┬────────┬────────┬────────┬────────┐
    │ parser │ir-core │vm-ref  │ios-rt  │android-rt
    ├────────┼────────┼────────┼────────┼────────┤
    │swift-adj│kotlin-adj│stdlib│  ci    │
    └────────┴────────┴────────┴────────┴────────┘
             │ parser + ir-core + runtimes + adapters done
             ▼
Phase 2 (6 agents in parallel)
    ┌────────────┬────────────┬────────────┐
    │ typechecker│ ir-serde   │ differ     │
    ├────────────┼────────────┼────────────┤
    │ stdlib-    │ ios-       │ android-   │
    │ validation │ integration│ integration│
    └────────────┴────────────┴────────────┘
             │ typechecker done (lowering needs TypedAST)
             ▼
Phase 3 (1 agent)
    ┌────────────┐
    │ lowering   │  (flux-ir dir: TypedAST → IRArena + bytecode)
    └─────┬──────┘
          │ lowered IR exists
          ▼
Phase 4 (3 agents in parallel)
    ┌────────────┬──────────────┬───────────────┐
    │ devserver  │ codegen-swift│ codegen-kotlin│
    └────────────┴──────────────┴───────────────┘
          ▼
Phase 5 (1 agent)          Phase 6 (1 agent)
    ┌────────┐                 ┌────────────────────────────┐
    │  cli   │                 │ parity + wire fixtures +   │
    └────────┘                 │ on-device smoke + benches  │
                               └────────────────────────────┘
```

**Why lowering is its own phase:** `lower()` requires `TypedAST` from the type checker (Phase 2). v1 placed it in Phase 1 — a circular dependency that would have forced rework. Codegen and devserver both consume lowered IR, so they wait for Phase 3.

## 1.8 Standard Definition of Done (Applies to EVERY Issue)

An issue is not DONE until **all** of the following hold. Issues below reference this block as **[DoD]** instead of repeating it.

1. **TDD evidence.** Commit history shows red → green → refactor. No production code without a preceding failing test. (AGENTS.md §1.1)
2. **Rust:** `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `cargo doc` all pass with zero warnings. No `unwrap()/expect()/panic!()` outside tests.
3. **Swift:** `swift-format` clean, `swiftlint` zero warnings, `xcodebuild test` passes. No `try!`/force-unwraps outside tests.
4. **Kotlin:** `ktlintCheck` clean, `./gradlew test` passes. No `!!` or `runBlocking` outside tests.
5. **Ownership compliance.** Zero files modified outside your owned directories. Zero manifest modifications. Verified by the orchestrator post-merge.
6. **Performance budgets** (spec §32) met where the issue defines them; benchmarks included as tests.
7. **AGENTS.md §5 reviewer checklist** for your language passes.
8. **API docs** on all public items.
9. **ADRs** written for any decision deviating from the spec (create-only, per R9).

## 1.9 Conflict Prevention Checklist

Before spawning each batch, the orchestrator verifies:

- [ ] **D1:** All prior-phase issues are DONE and merged.
- [ ] **D2:** `git status` clean; `cargo check` passes; platform skeletons build.
- [ ] **D3:** No two concurrently-running agents share a directory.
- [ ] **D4:** Every agent's task specifies exact directory boundaries and consumed interfaces (`flux-syntax` types or spec appendices).
- [ ] **D5:** All manifests frozen and pre-wired, including dev-dependencies.
- [ ] **D6:** ISA vectors exist and are frozen (before any VM work).
- [ ] **D7:** `flux-syntax` contains every type the batch's Rust agents need (else update it first and re-run tests).

After each batch merges:

- [ ] No merge conflicts; `cargo check`, `xcodebuild build`, `./gradlew build` all pass.
- [ ] Diff review confirms no writes outside ownership, no manifest edits, no edits to `/tests/isa-vectors/**`.
- [ ] All stubs for completed issues replaced by real code.

---

# Part 2: The Issues

### Phase 0 — Foundation (2 Agents in Parallel)

---

#### FLUX-001: Foundation — workspace, `flux-syntax`, frozen manifests, skeletons

**Agent:** foundation (runs once; all its outputs frozen afterward)  
**Owns:** `/Cargo.toml`, `/rust-toolchain.toml`, `/.gitignore`, `/crates/flux-syntax/**`, all `/crates/*/Cargo.toml`, all `/crates/*/src/lib.rs` stubs, `/adapters/ui-swift/Package.swift`, `/adapters/ui-kotlin/build.gradle.kts`, `/runtimes/ios/project.yml`, `/runtimes/android/**` manifest skeletons  
**Depends on:** Nothing  
**Estimated effort:** 1 day

**Scope:**
1. Cargo workspace with 12 members: `flux-syntax`, `flux-parser`, `flux-types`, `flux-ir`, `flux-ir-serde`, `flux-differ`, `flux-vm-ref`, `flux-devserver`, `flux-codegen-swift`, `flux-codegen-kotlin`, `flux-cli`, `flux-parity`.
2. `rust-toolchain.toml` (Rust 1.75+, edition 2021); `.gitignore` (Rust + Xcode + Gradle).
3. **Implement `flux-syntax` fully** per §1.5 — this is the only "real" code in Phase 0. TDD applies: write tests for `StringTable`, `Value`, `Patch`, ID derivation helpers.
4. **Pre-wire all manifests, including dev-dependencies** (`proptest`, `criterion`, `insta`, `serde_json`, `tempfile`, cross-crate dev-deps like `flux-vm-ref` for `flux-ir`'s future lowering tests). Key wiring:
   - `flux-parser`: flux-syntax, pest, pest_derive
   - `flux-types`: flux-syntax, flux-parser
   - `flux-ir`: flux-syntax; dev-dep flux-vm-ref
   - `flux-ir-serde`: flux-syntax, flux-ir, rmp-serde, blake3
   - `flux-differ`: flux-syntax, flux-ir
   - `flux-vm-ref`: flux-syntax; dev-dep serde_json
   - `flux-devserver`: all above + tokio, tokio-tungstenite, notify, axum, tracing
   - `flux-codegen-swift` / `-kotlin`: flux-syntax, flux-ir, flux-parser, flux-types (full-pipeline in-crate tests); dev-dep insta
   - `flux-cli`: devserver + both codegens + clap, anyhow
   - `flux-parity`: parser, types, ir, ir-serde, differ, vm-ref, both codegens; dev-deps serde_json, criterion
5. Compiling `lib.rs` stub in every other crate.
6. **Platform skeletons** (files only — platform agents verify builds): XcodeGen `project.yml` for `runtimes/ios` globbing `Sources/**`, `Tests/**`, and referencing `../../adapters/ui-swift` as a local package; SPM `Package.swift` for `adapters/ui-swift`; Gradle multi-module skeleton (`settings.gradle.kts` including `:adapters:ui-kotlin` and `:runtimes:android:app`) with Compose BOM and OkHttp pre-wired; empty source dirs.
7. Verify `cargo check` passes workspace-wide. Note in README that XcodeGen is a prerequisite.

**Acceptance criteria:**
- `cargo check`, `cargo test` (flux-syntax tests), `cargo doc` all pass.
- `flux-syntax` implements every type in §1.5 with tests.
- All 12 crates compile as stubs; all manifests exist and are correct.
- Platform skeleton files exist; iOS/Android build verification is explicitly deferred to Phase 1 agents (documented in the commit).

---

#### FLUX-002: Golden ISA vectors

**Agent:** isa-vectors (runs once; output frozen)  
**Owns:** `/tests/isa-vectors/**`  
**Depends on:** Nothing (needs only `/docs/spec/mlp-appendices.md` — may run parallel with FLUX-001)  
**Estimated effort:** 1 day

**Scope:**
Author pure-data JSON fixtures derived **exclusively from Appendix E**. No code. One file per vector plus `README.md` documenting the format.

**Vector schema:**
```json
{
  "name": "add_i64_basic",
  "description": "count = count + 1 via READ/LOAD/ADD/WRITE",
  "bytecode_hex": "1000010000000 01b001010000000000000001 2000010 110100000000",
  "initial_signals": [ { "id": 1, "value": { "type": "Int", "value": 41 } } ],
  "payload": null,
  "expected_signals": [ { "id": 1, "value": { "type": "Int", "value": 42 } } ],
  "expected_registers": {},
  "expected_error": null,
  "expected_gas_used": 4
}
```

**Coverage matrix (required):**
- Every opcode in Appendix E: ≥1 happy-path vector; arithmetic opcodes get boundary cases (i64::MIN/MAX, f64 edge values).
- Error vectors: `LIST_GET` out of bounds, `GET_FIELD` on `Null`, `GasExhausted` (loop with budget), `MemoryExhausted` (one allocation-heavy case).
- Signal write/read round-trips; `CALL_CAP` callback registration; pattern-match jump behavior; register conventions (r0 payload in / return out, r15 gas).
- ≥60 vectors total.

**Acceptance criteria:**
- Every opcode covered per the matrix; `README.md` documents schema and coverage.
- Vectors are self-consistent with Appendix E byte encodings (reviewed by orchestrator against the spec, since no code exists to validate them yet).
- Directory frozen after merge; corrections thereafter go through the orchestrator, who re-runs all three VM conformance suites.

---

### Phase 1 — Independent Work (9 Agents in Parallel)

---

#### FLUX-003: Parser crate

**Agent:** parser  
**Owns:** `/crates/flux-parser/src/**`  
**Depends on:** FLUX-001  
**Estimated effort:** 2 days  
**[DoD]** applies.

**Scope:**
1. pest grammar (`src/flux.pest`) from Appendix B.
2. `pub fn parse(source: &str, file_id: FileId) -> Result<AST, ParseError>` producing a typed AST with spans; AST types live in this crate (shape as specified in the issue's public API section: `AST`, `Decl`, `Block`, `Expr`, generics, props, patterns, lifecycle exprs).
3. Rust-style diagnostics (what/where/hint) per AGENTS.md §3.7.

**Consumes:** `Span`, `FileId`, `StringId`, `StringTable` from `flux-syntax`.

**Acceptance criteria (beyond DoD):**
- Parses all 10 grammar examples from Appendix B.3 as tests.
- Error tests: unclosed brace, bad interpolation, invalid generic bound — each with file:line:col.
- Bench: < 5 ms for a 500-line file.

---

#### FLUX-004: IR core — arena, node IDs, instance registry

**Agent:** ir-core  
**Owns:** `/crates/flux-ir/src/**`  
**Depends on:** FLUX-001  
**Estimated effort:** 2 days  
**[DoD]** applies.

**Scope (arena only — lowering is FLUX-018, Phase 3):**
1. `IRArena` with struct-of-arrays layout (Appendix C §C.1): `pack()`, `get()`/`NodeView`, blob packing for props/children/handlers/spans.
2. `pub fn compute_node_id(parent: NodeId, kind: NodeKind, span: Span, key: Option<u64>) -> NodeId` — BLAKE3-based, stable across edits (ADR-0013).
3. `ComponentInstance`, `InstanceRegistry`.
4. `ClosureIR` container (bytecode + captured signal IDs + span).
5. **Arena builder API** usable by other crates' tests to hand-construct trees (needed by differ, codegen, parity).

**Acceptance criteria:**
- Property tests (proptest): pack/unpack round-trip; node ID stability (same inputs → same ID; sibling insert doesn't shift sibling IDs).
- Bench: pack 100 nodes < 1 ms.

---

#### FLUX-005: Rust reference VM (`flux-vm-ref`) — test oracle

**Agent:** vm-ref  
**Owns:** `/crates/flux-vm-ref/src/**`  
**Depends on:** FLUX-001, FLUX-002  
**Estimated effort:** 2 days  
**[DoD]** applies.

**Scope:**
1. Register-based interpreter for Appendix E over `flux-syntax` types (`Value`, opcodes, 16 registers, gas meter, memory accounting).
2. Public API for reuse by `flux-ir` (lowering tests) and `flux-parity`:
   ```rust
   pub struct VmState { /* signals, gas_remaining, memory_used */ }
   pub fn eval(closure: &ClosureIR, state: &mut VmState,
               payload: Option<&Value>) -> Result<EvalOutcome, VmError>;
   ```
   `VmError` carries span, kind (`GasExhausted`, `MemoryExhausted`, `IndexOutOfBounds`, `NullDereference`, …).
3. **Conformance: run every vector in `/tests/isa-vectors/`** (loaded via pre-wired `serde_json` dev-dep). All must pass. This crate is the first consumer of the vectors — divergences between vectors and spec are flagged to the orchestrator, never "fixed" locally by editing vectors.

**Explicitly out of scope:** any production use. This VM never ships in a host app (ADR-0002: production VMs are native Swift/Kotlin).

**Acceptance criteria:**
- 100% of ISA vectors pass.
- Determinism test: same inputs → identical outputs across runs.

---

#### FLUX-006: iOS host app runtime

**Agent:** ios-runtime  
**Owns:** `/runtimes/ios/Sources/**`, `/runtimes/ios/Tests/**`  
**Depends on:** FLUX-001, FLUX-002  
**Estimated effort:** 5 days  
**[DoD]** applies (Swift gates).

**Scope:**
1. Verify the frozen XcodeGen `project.yml` builds the skeleton; own all Sources/Tests beneath it.
2. `FluxExecutor` (background queue for VM/patches; main queue for view mutations).
3. `FluxBytecodeVM` — **native Swift implementation** of Appendix E, with gas meter (100k) and 16 MB memory cap.
4. `SignalGraph` — fine-grained reactivity, topological propagation, batching.
5. `ShadowTree`/`ShadowNode` + keyed reconciler (udomdiff-style).
6. `WebSocketClient` behind a `FluxTransport` protocol; unit tests use `MockTransport` (real socket tested in FLUX-023).
7. `FrameDeserializer` — binary frame parser per Appendix D.
8. `FluxRootView: UIViewRepresentable`, launch screen, resign/become-active handling, red error overlay with source span.
9. **ISA conformance test target** loading `/tests/isa-vectors/*.json` (read-only) and asserting expected signals/registers/errors — same vectors as `flux-vm-ref`.
10. Frame deserializer tests: hand-built byte arrays from Appendix D **plus** the `FLUX_WIRE_FIXTURES` env-var loader (R10) that skips when absent.
11. Runtime tests use **in-dir mock adapters** conforming to the adapter kit protocol from `adapters/ui-swift` (real adapters wired in FLUX-016).

**Acceptance criteria:**
- All ISA vectors pass on the Swift VM.
- E2E-without-sockets test: hand-built `Init` frame bytes → deserializer → shadow tree → mock adapters → view hierarchy assertions; `dispatch(handlerId)` → VM → signals → reconciler → view updated.
- Gas exhaustion → red banner, no crash.

---

#### FLUX-007: Android host app runtime

**Agent:** android-runtime  
**Owns:** `/runtimes/android/app/src/**`  
**Depends on:** FLUX-001, FLUX-002  
**Estimated effort:** 5 days  
**[DoD]** applies (Kotlin gates).

**Scope:** Mirror of FLUX-006 in Kotlin (API 24+, Compose): `FluxExecutor` (coroutines, `Dispatchers.Default` for VM, `Dispatchers.Main` for views), native Kotlin `FluxBytecodeVM` per Appendix E, `SignalGraph`, shadow tree + reconciler, OkHttp transport behind `FluxTransport` interface with `MockTransport` tests, `FrameDeserializer` per Appendix D, `FluxRoot` composable via `AndroidView`, lifecycle (`onPause`/`onResume`), error overlay, VM safety, ISA conformance target over the shared vectors, hand-built frame tests + `FLUX_WIRE_FIXTURES` loader, in-dir mock adapters.

**Acceptance criteria:** Mirror of FLUX-006, on emulator (Pixel 5, API 34). All ISA vectors pass on the Kotlin VM.

---

#### FLUX-008: Swift adapter kit + dev adapters

**Agent:** swift-adapters  
**Owns:** `/adapters/ui-swift/Sources/**`, `/adapters/ui-swift/Tests/**`  
**Depends on:** FLUX-001  
**Estimated effort:** 3 days  
**[DoD]** applies (Swift gates).

**Scope:**
1. The shared Swift "kit" types in this package (the contract the runtime consumes): `FluxAdapter` protocol (`create/update/setChildren/bindHandler/destroy`), `FluxValue` (mirroring `flux_syntax::Value` per Appendix C), `Props` accessor helpers, `HandlerEvent`.
2. The 7 dev-mode adapters per Appendix F: `Text`→`UILabel`, `Button`→`UIButton`, `Column`→`UIStackView(vertical)`, `Row`→`UIStackView(horizontal)`, `TextField`→`UITextField`, `Router`→`UINavigationController` (all screens' shadow trees persist), `Screen`→`UIViewController`.
3. Adapters call `executor.dispatch(handlerId)` via a weak executor reference; no retain cycles (document each `weak`).
4. Tests: create/update/destroy per adapter; keyed child diff on stacks; router push/pop preserving screen state.

**Acceptance criteria:** All adapters pass behavior tests against the kit's own test doubles (executor mock). The package compiles standalone via its frozen `Package.swift`.

---

#### FLUX-009: Kotlin adapter kit + dev adapters

**Agent:** kotlin-adapters  
**Owns:** `/adapters/ui-kotlin/src/**`  
**Depends on:** FLUX-001  
**Estimated effort:** 3 days  
**[DoD]** applies (Kotlin gates).

**Scope:** Mirror of FLUX-008 in Kotlin: kit types (`FluxAdapter` interface, `FluxValue`, `Props`), 7 adapters (`TextView`, `android.widget.Button`, `LinearLayout(VERTICAL/HORIZONTAL)`, `EditText`, `FrameLayout` router stack, `FrameLayout` screen child), weak executor references (`WeakReference`), keyed child diffing, router state preservation.

**Acceptance criteria:** Mirror of FLUX-008.

---

#### FLUX-010: Standard library (`.flux` sources)

**Agent:** stdlib  
**Owns:** `/stdlib/**`  
**Depends on:** FLUX-001  
**Estimated effort:** 1 day  
**[DoD]** applies (docs/tests where meaningful — this issue is mostly declarative source).

**Scope:** Author the 12 stdlib files per the spec (§18.3): `prelude.flux`, per-component declarations (`text`, `button`, `column`, `row`, `text_field`, `router` incl. `Screen`), `color.flux` (RGB type + `red/green/blue/black/white` constants), `font.flux` (+ `body/title/caption` presets), `traits.flux` (`Numeric`, `Eq`, `Show`), `capabilities.flux` (declarations only), `platform.flux`.

**Acceptance criteria:** Every component declares typed props per Appendix F. Syntax conforms to Appendix B by manual review (parse-validation happens in FLUX-015 — that's why that issue exists).

---

#### FLUX-011: CI pipelines

**Agent:** ci  
**Owns:** `/.github/workflows/**`, `/scripts/**`  
**Depends on:** FLUX-001  
**Estimated effort:** 1 day  
**[DoD]** applies (YAML lint; no production code).

**Scope:**
1. `rust-check` workflow (ubuntu): `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `cargo doc` on the workspace; caching.
2. `ios-check` workflow (macos-latest): `xcodegen generate` then `xcodebuild test` on `runtimes/ios`; SPM test for `adapters/ui-swift`.
3. `android-check` workflow (ubuntu): `./gradlew :adapters:ui-kotlin:test :runtimes:android:app:test`.
4. Workflows must be green against the Phase 0/1 skeletons at merge time and stay green as sources land (glob-based manifests make this automatic).
5. Helper scripts under `/scripts/` (e.g., `check-ownership.sh` verifying no tracked file under frozen manifests changed vs. `main` — best-effort guard).

**Acceptance criteria:** All three workflows green on the commit that lands this issue, and remain green for the rest of Phase 1.

---

### Phase 2 — Core Layer + Integrations (6 Agents in Parallel)

---

#### FLUX-012: Type checker crate

**Agent:** typechecker  
**Owns:** `/crates/flux-types/src/**`  
**Depends on:** FLUX-003  
**Estimated effort:** 3 days  
**[DoD]** applies.

**Scope:**
1. Bidirectional checking with let-polymorphism, type class resolution, ADT exhaustiveness, monomorphization tracking.
2. `pub fn type_check(ast: &AST) -> Result<TypedAST, TypeError>`; `TypedAST { ast, types: HashMap<NodeId, TypeKind>, instantiations: Vec<GenericInstantiation> }` — **this public shape is normative for FLUX-018** (lowering codes against it).
3. Diagnostics per AGENTS.md §3.7 (what/where/hint).

**Acceptance criteria:**
- Type-checks the 10 grammar examples; catches mismatches with precise spans.
- Records instantiations (`Counter[Int]`, `Counter[Float]` both detected).
- Bench: < 3 ms for 500-line file.

---

#### FLUX-013: IR serialization crate

**Agent:** ir-serde  
**Owns:** `/crates/flux-ir-serde/src/**`  
**Depends on:** FLUX-004  
**Estimated effort:** 2 days  
**[DoD]** applies.

**Scope:**
1. `serialize_patches(&[Patch], &StringTable) -> Vec<u8>` per Appendix D (custom little-endian binary wire layout, ADR-0025; content addressing via BLAKE3, string-table deltas).
2. `Frame` construction: `Hello`/`Init`/delta/error/heartbeat; protocol version field.
3. A Rust-side **deserializer for round-trip tests only** (production deserializers are Swift/Kotlin, coded from Appendix D).
4. `hash_props`, `hash_closure` content addressing.

**Acceptance criteria:** Round-trip equality on property-generated patch sets; deterministic hashes; 50-node `Init` frame < 20 KB; serialize 50-node patch < 1 ms.

---

#### FLUX-014: Differ crate

**Agent:** differ  
**Owns:** `/crates/flux-differ/src/**`  
**Depends on:** FLUX-004  
**Estimated effort:** 2 days  
**[DoD]** applies.

**Scope:**
1. `pub fn diff(old: &IRArena, new: &IRArena) -> Vec<Patch>` — keyed reconciliation (udomdiff-style) over stable node IDs from `flux_ir::compute_node_id`.
2. Handler-only fast path (handler-body change → single `Handler` patch).
3. Prop diffing (`PropDiff`), reorder detection (`Reorder`, not remove+insert).

**Acceptance criteria:** Identical trees → empty patch; the four canonical minimal-diff cases from the spec; proptest: diff-then-apply equals new tree (apply using a test-only applier in this crate); bench: 50-node tree < 1 ms.

---

#### FLUX-015: Stdlib validation against the parser

**Agent:** stdlib (second issue, same directory — sequential after FLUX-010)  
**Owns:** `/stdlib/**`  
**Depends on:** FLUX-003, FLUX-010  
**Estimated effort:** 0.5 day  
**[DoD]** applies.

**Scope:** Run the parser against every stdlib file; fix syntax discrepancies in the stdlib (never in the parser). If a construct is genuinely unparseable per Appendix B, write an ADR and flag to the orchestrator.

**Acceptance criteria:** All 12 stdlib files parse clean; discrepancies resolved or ADR'd.

---

#### FLUX-016: iOS adapter↔runtime integration

**Agent:** ios-runtime (second issue, same directory)  
**Owns:** `/runtimes/ios/Sources/**`, `/runtimes/ios/Tests/**`  
**Depends on:** FLUX-006, FLUX-008  
**Estimated effort:** 1 day  
**[DoD]** applies.

**Scope:**
1. `AdapterRegistry` mapping `ComponentId` → adapter instance (uses the string table from the `Init` frame).
2. Replace mock adapters in the E2E test with the real adapter kit: hand-built `Init` frame → real `UILabel`/`UIButton`/`UIStackView` hierarchy; tap → dispatch → VM → reconciler → label text updated **without view recreation** (assert view identity preserved).
3. Router E2E: push → edit state → pop → state preserved.

**Acceptance criteria:** E2E test with real adapters green; view-identity-preservation assertions pass.

---

#### FLUX-017: Android adapter↔runtime integration

**Agent:** android-runtime (second issue, same directory)  
**Owns:** `/runtimes/android/app/src/**`  
**Depends on:** FLUX-007, FLUX-009  
**Estimated effort:** 1 day  
**[DoD]** applies.

**Scope:** Mirror of FLUX-016 in Kotlin.

---

### Phase 3 — Lowering (1 Agent)

---

#### FLUX-018: Lowering pass (`flux-ir` extension)

**Agent:** ir-core (second issue, same directory — sequential after FLUX-004)  
**Owns:** `/crates/flux-ir/src/**`  
**Depends on:** FLUX-004, FLUX-012  
**Estimated effort:** 2 days  
**[DoD]** applies.

**Scope:**
1. `pub fn lower(typed: &TypedAST) -> LoweredIr` where `LoweredIr { arena: IRArena, closures: HashMap<HandlerId, ClosureIR>, instances: InstanceRegistry }`.
2. **Monomorphized bytecode emission**: one specialized `ClosureIR` per recorded generic instantiation (`ADD_I64` vs `ADD_F64`), capped at 100 specializations with type-erased fallback (ADR-0005).
3. Stable node IDs via `compute_node_id`; spans on every node and closure; string interning into the shared table.
4. **Test executor is `flux-vm-ref`** (pre-wired dev-dep): lowering tests assert that emitted bytecode, when evaluated by the reference VM, produces expected signal mutations.

**Acceptance criteria:**
- Lowering the 10 grammar examples yields arenas + closures; bytecode runs correctly under `flux-vm-ref`.
- `Counter[Int]` and `Counter[Float]` produce distinct specialized bytecode (asserted byte-different).
- Bench: lower 50-node tree < 1 ms.

---

### Phase 4 — Backend Layer (3 Agents in Parallel)

---

#### FLUX-019: Dev server crate

**Agent:** devserver  
**Owns:** `/crates/flux-devserver/src/**`  
**Depends on:** FLUX-003, FLUX-004, FLUX-012, FLUX-013, FLUX-014, FLUX-018  
**Estimated effort:** 3 days  
**[DoD]** applies.

**Scope:**
1. WebSocket server (tokio-tungstenite) on :7331; asset HTTP server (axum) on :7332; file watcher (notify) with 50 ms debounce; 16 ms frame coalescing.
2. Pipeline: change → parse → type_check → lower → diff → serialize → send.
3. Handshake (`Hello` → `Init` with full tree + state seed + source map + string table), reconnect (retry, resend `Init`), protocol/capability versioning, error frames.
4. `tracing` logging levels; `--profile` phase timings.

**Acceptance criteria:** Integration test with a WebSocket test client: save → frame received < 50 ms (excluding pipeline compute); handshake < 10 ms; reconnect resends `Init`; error frame on malformed source with previous-good-tree semantics (no frame sent).

---

#### FLUX-020: Swift codegen crate

**Agent:** codegen-swift  
**Owns:** `/crates/flux-codegen-swift/src/**`  
**Depends on:** FLUX-003, FLUX-004, FLUX-012, FLUX-018  
**Estimated effort:** 3 days  
**[DoD]** applies.

**Scope:**
1. `pub fn codegen(lowered: &LoweredIr, ast: &flux_parser::Ast) -> String` — idiomatic SwiftUI per Appendix F / ADR-0003 / ADR-0004. **The two-input signature is normative** (settled by `docs/adr/ADR-0030-codegen-input-contract.md`): `LoweredIr` supplies tree *structure*; the `Ast` (reached via the ADR-0027 node-ID bridge, recovering names/generics/`@pure`/prop+state types/string interpolations through a `bridge` module) supplies *semantics*. Mapping rules: components → `struct …: View`, `state` → `@State`, `Column/Row` → `VStack/HStack(spacing:)`, flat props → deterministic modifier chains, `when/otherwise` → `if/else` in `@ViewBuilder`, `ForEach` with keys, `Router` → `NavigationStack(path:)`, `match` → `switch`, generics → Swift generics, `@pure` → stateless struct.
2. Full-pipeline tests: parse → typecheck → lower → codegen over the 10 grammar examples; **snapshot tests via `insta`**.
3. Syntax check: `swiftc -parse` on generated output where a toolchain is present; full compile happens in CI/parity.

**Acceptance criteria:** Snapshots for all 10 examples; generated code passes `swiftc -parse`; readability review against BR-001 (a Swift dev unfamiliar with `.flux` can read it).

---

#### FLUX-021: Kotlin codegen crate

**Agent:** codegen-kotlin  
**Owns:** `/crates/flux-codegen-kotlin/src/**`  
**Depends on:** FLUX-003, FLUX-004, FLUX-012, FLUX-018  
**Estimated effort:** 3 days  
**[DoD]** applies.

**Scope:** Mirror of FLUX-020 targeting Compose, **with the same two-input signature** `codegen(lowered: &LoweredIr, ast: &flux_parser::Ast) -> String` and the same `bridge`/ADR-0027 approach (see `docs/adr/ADR-0030-codegen-input-contract.md` — FLUX-021 is NOT blocked by the `LoweredIr` name gap; codegen recovers names from the `Ast`). Compose mapping: `@Composable fun`, `remember { mutableStateOf }`, `Column(spacing = N.dp)`, `Button(onClick) { Text(…) }`, `items(list, key)`, `NavHost`. Snapshot tests via `insta`; `kotlinc` parse check where available.

**Acceptance criteria:** Mirror of FLUX-020 (BR-002 readability).

---

### Phase 5 — Integration (1 Agent)

---

#### FLUX-022: CLI crate

**Agent:** cli  
**Owns:** `/crates/flux-cli/src/**`  
**Depends on:** FLUX-019, FLUX-020, FLUX-021  
**Estimated effort:** 1 day  
**[DoD]** applies.

**Scope:** `flux init` (scaffold project), `flux dev` (start dev server + watcher), `flux build --platform ios|android` (codegen → write into `platforms/*/Generated/` → invoke `xcodebuild`/`gradle`), `flux doc` (JSON schema of stdlib API). `clap` derive; `anyhow` for CLI errors only.

**Acceptance criteria:** `flux init myapp` produces a valid project; `flux dev` prints `Listening on ws://localhost:7331` and serves an `Init` frame to a test client; `flux build --platform ios` writes generated Swift files; `flux doc` emits valid JSON.

---

### Phase 6 — Verification (1 Agent)

---

#### FLUX-023: Parity harness, wire fixtures, on-device smoke, benchmarks

**Agent:** parity  
**Owns:** `/tests/parity/**`, `/tests/wire-fixtures/**`  
**Depends on:** ALL prior issues  
**Estimated effort:** 3 days  
**[DoD]** applies.

**Scope:**
1. **Wire fixtures:** use `flux-ir-serde` to serialize canonical trees/patches into `/tests/wire-fixtures/*.frame` + `manifest.json`. CI then runs the iOS/Android test suites with `FLUX_WIRE_FIXTURES` set (R10 loaders already exist from Phase 1) — validating Rust serialization against Swift/Kotlin deserialization without touching runtime code.
2. **Rust parity harness** (`flux-parity` crate): for each scenario (counter taps, form changes, navigation push/edit/pop, `Counter[Int]` vs `Counter[Float]`, `@pure` skip), run the **reference VM** on lowered IR and record final state.
3. **Release-side execution:** compile generated Swift/Kotlin for the same scenarios in minimal harness apps; run scripted actions; record state; assert equal to the VM side.
4. **On-device smoke:** simulator + emulator, `flux dev`, real app, scripted taps, assert visible state.
5. **Benchmarks vs spec §32:** save-to-pixels (50 nodes), tap-to-state-change, 1000-edit memory growth.

**Acceptance criteria:**
- All parity scenarios: dev state == release state.
- All wire fixtures pass in Swift and Kotlin deserializer suites.
- Performance targets met and reported.

---

## Known Deviations & Pending Follow-ups (tracked for dispatch)

These were surfaced during the FLUX-013/FLUX-018 boundary review (2026-08-24).
They are **not** blockers for the already-merged Phase 1 work, but each must be
resolved before or during the listed downstream issue.

### Deviation D1 — wire format is custom binary, not MessagePack (RESOLVED by ADR-0025)
`flux-ir-serde` ships a custom little-endian binary frame format; `rmp-serde`
is declared but unused. ADR-0025 supersedes ADR-0008's MessagePack choice and
the spec/contract prose now reference it. `rmp-serde` remains in the frozen
workspace `Cargo.toml` (R2) — flag to foundation for pruning.

### Gap G1 — handler bytecode has no transport (schedule `flux-ir-serde` 2nd pass)
`InitFrame`/`DeltaFrame` carry patches + strings only; `wire.rs HandlerDef` is
`#[allow(dead_code)]` and `ClosureRef { bytecode_offset, bytecode_len }` points
into a bytecode blob no frame ships (spec §21.1 frame includes a handler
section). **Sequence after FLUX-018** defines `ClosureIR`'s final shape, then a
`flux-ir-serde` second pass (owned by ir-serde) adds a handler section +
bytecode blob to `Init`/`Delta`. FLUX-019 (devserver) must not hard-code a
handler wire shape until G1 lands.

### Gap G2 — node-ID derivation has no single source (orchestrator task; BLOCKS FLUX-018) — DONE (2026-08-24)
`flux-types/src/kind.rs:303` defined its own `compute_node_id` (u8 tag, u64 key,
FNV hash) and `flux-ir/src/node_id.rs:46` was the canonical one (`NodeKind`,
`Option<Key>`, BLAKE3) — they hashed **different bytes** (flux-types omitted
`span.file_id`). RESOLVED by `ADR-0027`: canonical `compute_node_id` now lives
in `flux-syntax` (BLAKE3, the flux-ir layout), `flux-ir` delegates to it (public
`NodeKind` API unchanged, output identical — committed `1c9705f`), and
`flux-types` now also delegates (signature `key: u64`→`Option<Key>`, call sites
`0`→`None`; edits in working tree, pending the in-flight FLUX-012 agent finishing
their unrelated `CalleeShape::Adt` fix so `flux-types` compiles). Bridge tests
assert both crates equal `flux_syntax::compute_node_id`. FLUX-018 may now be
dispatched once FLUX-012 lands.

### Verify-only (post-MLP unless explicitly pulled in)
- **FR-014** `Image` adapter + asset pipeline: referenced in `mlp-spec.md` but
  absent from the 7-adapter MLP set and the 12 stdlib files.
- **ADR-0016** on-wire hash-reference dedup ("90%+ payload reduction"):
  `hash_props`/`hash_closure` exist but frames ship props inline; the
  indirection is not yet on the wire.

---

# Part 3: Agent Spawning Plan

### Universal system-prompt preamble (prepend to every spawn)

```
Before writing any code:
1. Read /AGENTS.md in full. It is the law of the land. TDD is mandatory:
   every production change starts with a failing test.
2. Read /docs/spec/mlp-spec.md sections and /docs/spec/mlp-appendices.md appendices
   referenced by your issue. The spec is normative; do not improvise.
3. You OWN exactly these directories (and nothing else):
   {OWNED_DIRS}
   You may READ anything. You may WRITE only within your ownership.
4. You MUST NOT modify any build manifest (Cargo.toml, Package.swift,
   project.yml, build.gradle.kts, settings.gradle.kts) or anything under
   /tests/isa-vectors/. Dev-dependencies are pre-wired; if something is
   missing, report it instead of adding it.
5. If /crates/flux-syntax is missing a type you need, report it to the
   orchestrator. Never add it yourself.
6. ADRs: you may CREATE /docs/adr/{scope}-{slug}.md. Never edit existing ADRs.
7. Latest-dependency policy is already satisfied by pinned manifests —
   do not touch versions.
```

### Batch 1 — Phase 0 (2 parallel)

| id | issue | notes |
|---|---|---|
| foundation | FLUX-001 | The only agent allowed to write manifests. Include full `flux-syntax` TDD. |
| isa-vectors | FLUX-002 | Pure data authoring from Appendix E. Can start immediately — only needs the spec. |

### Batch 2 — Phase 1 (9 parallel)

| id | issue | depends_on |
|---|---|---|
| parser | FLUX-003 | foundation |
| ir-core | FLUX-004 | foundation |
| vm-ref | FLUX-005 | foundation, isa-vectors |
| ios-runtime | FLUX-006 | foundation, isa-vectors |
| android-runtime | FLUX-007 | foundation, isa-vectors |
| swift-adapters | FLUX-008 | foundation |
| kotlin-adapters | FLUX-009 | foundation |
| stdlib | FLUX-010 | foundation |
| ci | FLUX-011 | foundation |

Model guidance: pin your strongest model for `parser`, `ir-core`, `vm-ref`, `ios-runtime`, `android-runtime` (high-complexity TDD work); lighter models handle `stdlib` and `ci` fine.

### Batch 3 — Phase 2 (6 parallel)

| id | issue | depends_on |
|---|---|---|
| typechecker | FLUX-012 | parser |
| ir-serde | FLUX-013 | ir-core |
| differ | FLUX-014 | ir-core |
| stdlib (2nd) | FLUX-015 | parser, stdlib |
| ios-runtime (2nd) | FLUX-016 | ios-runtime, swift-adapters |
| android-runtime (2nd) | FLUX-017 | android-runtime, kotlin-adapters |

### Batch 4 — Phase 3 (1 agent)

| id | issue | depends_on |
|---|---|---|
| ir-core (2nd) | FLUX-018 | ir-core, typechecker |

### Batch 5 — Phase 4 (3 parallel)

| id | issue | depends_on |
|---|---|---|
| devserver | FLUX-019 | parser, ir-core, typechecker, ir-serde, differ, lowering |
| codegen-swift | FLUX-020 | parser, ir-core, typechecker, lowering |
| codegen-kotlin | FLUX-021 | parser, ir-core, typechecker, lowering |

### Batch 6 — Phase 5 (1 agent)

| id | issue | depends_on |
|---|---|---|
| cli | FLUX-022 | devserver, codegen-swift, codegen-kotlin |

### Batch 7 — Phase 6 (1 agent)

| id | issue | depends_on |
|---|---|---|
| parity | FLUX-023 | all |

---

# Part 4: Verification

### Pre-spawn checklist (orchestrator, every batch)

```
□ All prior-phase issues DONE and merged; git clean.
□ cargo check passes; iOS/Android skeletons build.
□ No two concurrent agents share a directory.
□ Each task lists exact owned dirs + consumed interfaces.
□ All manifests frozen; dev-deps present.
□ ISA vectors frozen and untouched (before any VM-related batch).
□ flux-syntax covers every type this batch's Rust agents need.
```

### Post-merge checklist (orchestrator, every batch)

```
□ Merge clean; cargo check / xcodebuild / gradlew all green.
□ git diff shows zero writes outside ownership.
□ Zero manifest modifications; zero isa-vector edits.
□ New ADRs follow {scope}-{slug}.md naming; none edited.
□ CI (FLUX-011 onward) is green on main.
```

---

# Part 5: Summary

| Metric | Value |
|---|---|
| Issues | 23 |
| Phases | 7 |
| Max concurrent agents | 9 (Phase 1) — within the 10-agent cap |
| Total estimated effort | ~44 agent-days |
| Approx. wall-clock (critical path) | ~15 days |
| File-level write conflicts | 0 by construction |
| Production VM | Swift + Kotlin (native); Rust `flux-vm-ref` is test-oracle only |
| Behavioral source of truth for VMs | `/tests/isa-vectors/**` (golden vectors, frozen after Phase 0) |

The three rules that make this work: **disjoint directories**, **frozen manifests pre-wired in Phase 0**, and **shared fixtures (ISA vectors, wire fixtures) as the only cross-implementation contracts**. Everything else — TDD discipline, quality gates, dependency policy — is enforced by `/AGENTS.md` and CI, not by trust.
