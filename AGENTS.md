# AGENTS.md — Flux Project Agent Operating Manual

> **Read this file in its entirety before touching any file in this repository.**
> Failure to follow these conventions will result in your work being rejected.

---

## 0. Project Identity

### 0.1 What Flux is

**Flux** is a write-once UI language for native iOS/Android development. A Rust
dev server parses `.flux` source, lowers it to a Reactive Tree IR, diffs it, and
ships binary patches (MessagePack, Appendix D) over WebSocket to a precompiled
host app. The host app contains an embedded register-based bytecode VM
(Appendix E), a SolidJS-style signal graph, and a shadow tree of **observable
prop nodes**. In release mode, the same IR is codegen'd to idiomatic
Swift/SwiftUI and Kotlin/Jetpack Compose — no VM, no runtime, no reconciler.

**You are building the MLP.** The spec is in `/docs/spec/mlp-spec.md` and
`/docs/spec/mlp-appendices.md`. Read the relevant appendix before starting any
work. Do not improvise architecture — follow the spec, and where this manual
and the spec disagree on the dev rendering model, **this manual is current**
(§0.2); a doc-reconciliation ADR is pending.

### 0.2 The unified component tier (load-bearing doctrine)

Dev and release render through **the same declarative components**:

* The shadow tree is **not** a tree of native views. Each `ShadowNode` holds its
  materialized props in a platform observable (Compose `MutableState` via
  `propsStateFactory` on Android; the SwiftUI equivalent on iOS).
* **Dev:** the VM re-materializes a node's props in place; the observable fires;
  only affected leaves recompose. Dirty-subset reconciliation
  (`reconcileDirty`) touches exactly `dependents[S]` of written signals — never
  the whole tree.
* **Release:** codegen emits the same component vocabulary with literals
  (`flux-codegen-swift` / `flux-codegen-kotlin`).
* There is **no imperative dev tier**. UIKit/`android.view`-based libraries
  interop through the platforms' own wrappers (`UIViewRepresentable` /
  `AndroidView`) and become Flux views that way.
* Old Appendix F text ("dev implementation: imperative, drives UIView/View
  directly") describes a **superseded** model. Write all new code to the
  unified model. One component, two feeders: dev is fed by the VM, release by
  codegen — the component is the same.

### 0.3 Repository map

| Path | Contents |
|---|---|
| `crates/flux-syntax` | `Span`, `NodeId`, `compute_node_id`, node tags |
| `crates/flux-parser` | Lexer (Indents/Dedents, keyword map) + parser, `Ast`, `Decl`, `Expr` |
| `crates/flux-types` | Type checker (`type_check`) |
| `crates/flux-ir` | Arena IR, `lower`, `prop_index_for_name`, `LoweredIr` |
| `crates/flux-ir-serde` | IR serialization for tests/fixtures |
| `crates/flux-devserver` | `Pipeline`, `DevServer`, frame/diff/patch, asset server |
| `crates/flux-codegen-swift` / `flux-codegen-kotlin` | Release codegen + ADR-0027 node-ID bridge |
| `crates/flux-cli` | `flux init/dev/build/doc` |
| `crates/flux-devtools-ui` | DevTools on gpui (**nightly toolchain required**) |
| `adapters/ui-kotlin`, `adapters/ui-swift` | Dev adapter kits (contract version 1) |
| `runtimes/android/host` | Pure-JVM reactive core: VM, signals, shadow tree, wire, `FluxExecutor` |
| `runtimes/android/app` | Android shell: `FluxHostActivity`, `FluxSession`, `FluxRoot`, `FluxTreeView` |
| `runtimes/ios` | XcodeGen project (`project.yml`, scheme `FluxApp`) |
| `stdlib/` | Stdlib `.flux` sources (reflected by `flux doc`) |
| `docs/spec`, `docs/adr`, `docs/scripts/check-adr-numbering.sh` | Spec, ADRs, numbering guard |
| `scripts/` | `merge-guard-check.sh`, `manifest-steward.sh` |

### 0.4 State of play

* **Grammar migration in flight (ADR-0029):** the lexer already emits
  `Indent`/`Dedent`/`Newline` layout tokens, but sample projects and some
  fixtures still use brace syntax. See §3.6 before writing any `.flux` test
  source.
* **CLI v0.1 shipped:** `init`, `dev`, `build`, `doc`. `flux build` emits
  generated sources under `platforms/<platform>/Generated/` and **does not yet
  invoke** `xcodebuild`/`gradle` — it only detects them.
* **ADRs referenced by code:** 0003/0004 (codegen), 0027 (R-graph threading,
  lifecycle, node-ID bridge), 0029 (Appendix B grammar repairs), 0044 (async
  futures / result cells), 0045 (unified sync/async capability bridge). Before
  filing a new ADR, run `bash docs/scripts/check-adr-numbering.sh` and take the
  next sequential number (currently ≥ 0046).
* **Issue prefixes in use:** FLUX-001 … FLUX-023 (foundation, hosts, adapter
  kits, CI, registry, hot-reload, codegen, CLI, wire fixtures).

---

## 1. Core Engineering Principles

### 1.1 Test-Driven Development (Non-Negotiable)

**Every line of production code must be preceded by a failing test.** No exceptions.

The cycle:
1. **Red** — Write a test that describes the desired behavior. Run it. It must fail.
2. **Green** — Write the minimum code to make the test pass. Do not add anything else.
3. **Refactor** — Improve the code while keeping tests green.

Rules:
- Never write production code without a failing test that requires it.
- Never comment out a test. Fix it or delete it with justification.
- Never use `#[ignore]`, `xtest`, or `pending` to skip a test.
- Tests are production code: no duplication, clear names, single responsibility.
- Test names describe behavior: `test_counter_increments_on_tap`, not `test_1`.
- One assert per test when possible; multiple asserts must describe one behavior.
- Test the public API, not private implementation details.
- Unit-test host/adapter Kotlin on the **plain JVM** wherever possible —
  `runtimes/android/host` and `adapters/ui-kotlin` deliberately have no Android
  dependency so their suites run without an emulator. Keep it that way.

### 1.2 Quality Bar — 20+ Year Senior Engineer

- **No `unwrap()`, `expect()`, `panic!()`, `!!`, or force-try in production
  code.** `unreachable!()` only when genuinely impossible, proven in a comment.
- **No `TODO`, `FIXME`, `HACK`, `TEMPORARY` comments.** Fix now or file an issue.
- **No commented-out code.** Delete it.
- **No dead code.** Zero compiler warnings (`cargo check`, `swift build`,
  `kotlinc`/`ktlint`).
- **No magic numbers.** Named constants only.
- **No functions longer than 40 lines. No files longer than 300 lines.**
- **No more than 3 levels of nesting. No more than 4 parameters** (use a
  struct/builder).
- **Every public API has a doc comment** (what / returns / errors / example).
- **Every error message is actionable** (what, where, why, how — see §3.11).
- **Every `unsafe`, `@unchecked`, or `@Suppress` has a safety comment.**

### 1.3 Dependency Policy

**Always use the latest stable version.** Pin with `^MAJOR`; never pin a patch
version except for a security advisory. Before adding a dependency: prefer
stdlib/existing deps; require active maintenance (< 6 months) and > 1000 stars
or authoritative recommendation; run `cargo audit`.

**Manifests are frozen** (`Cargo.toml`, `build.gradle.kts`, `settings.gradle.kts`,
`Package.swift`, `project.yml`). Agents do not edit them. Append dependency
requests to `MANIFEST_REQUESTS.md`; the weekly manifest-steward workflow
applies them.

Approved dependencies (do not remove or replace without an ADR):

| Language | Dependency | Purpose |
|---|---|---|
| Rust | `pest` / `pest_derive` | PEG parser |
| Rust | `tokio`, `tokio-tungstenite` | Async runtime, WebSocket server |
| Rust | `notify` | File watching |
| Rust | `axum` | HTTP asset server |
| Rust | `rmp-serde` | MessagePack wire serialization |
| Rust | `blake3` | Content addressing / node IDs |
| Rust | `clap`, `tracing`, `tracing-subscriber` | CLI, logging |
| Rust | `serde` / `serde_derive`, `thiserror`, `anyhow` (CLI only) | Serde, errors |
| Rust | `criterion`, `insta`, `proptest`, `cargo-nextest` | Bench, snapshots, props, runner |
| Swift | `Foundation` / `UIKit` / `SwiftUI` / `XCTest` | Platform, testing |
| Kotlin | `androidx.compose.*`, `okhttp3`, `kotlinx.coroutines` | UI, WebSocket, async |
| Kotlin | `junit-jupiter`, `mockk`, `turbine` | Testing |

New dependencies require an ADR (§1.3 vetting first).

---

## 2. Language-Specific Standards

### 2.1 Rust

- `cargo fmt` zero changes; `cargo clippy -- -D warnings` zero warnings.
- `#![forbid(unsafe_code)]` in every `lib.rs`. `#![warn(missing_docs,
  missing_debug_implementations, rust_2018_idioms, unreachable_pub)]`.
- **Errors:** `thiserror` in library crates; `anyhow` only in the CLI binary.
  Never `unwrap/expect/panic` — use `?`. Every error type carries a `Span`.
  `Result<T, E>` over `Option<T>` for failures.
- **Types:** `&str`/`&[T]` params; `Cow` for maybe-owned strings; `Arc` only
  across threads, `Rc` single-threaded; `parking_lot` locks; `ahash` for
  hot-path maps; `SmallVec` for usually-small vectors; enums over bool flags;
  `#[non_exhaustive]` on public enums; `#[must_use]` on `Result`-returning fns.
- **Memory:** arena-allocate same-lifetime data (see `flux-ir/src/arena.rs`);
  `Vec::with_capacity`; avoid `format!` in hot paths (use `write!` into a
  pre-allocated `String`).
- **Async:** `tokio` runtime; `tokio::select!` over spawn+channel when one will
  do; `tokio::sync::mpsc` for channels. **Edition 2024, `resolver = "3"`,
  `rust-version` 1.85, local toolchain 1.94.1** — write edition-2024 Rust.
- **Testing:** unit tests in-file; integration tests in `tests/`; `proptest`
  for invariants; `insta` for codegen snapshots; `criterion` in `benches/`.
  **Always `cargo nextest run`, never `cargo test`** — except doctests
  (`cargo test --doc`). Never pipe nextest through `tail`/`head`.
- **Docs:** `///` on every public item; `# Examples`, `# Errors`, `# Panics`
  sections where applicable; `cargo doc` clean.
- **Modules:** one responsibility per module; **`mod.rs` is forbidden**
  (`module_name.rs` + `module_name/`); re-export at crate root; `pub(crate)`
  for internals.

### 2.2 Swift

- `swift-format` + SwiftLint with project configs; zero warnings. Swift 5.10+;
  do not write Swift-4-compatible code.
- `struct` for values; `final class` for all classes; `actor` for shared
  mutable state; `@MainActor` on all UI-touching code; document every `weak`/
  `unowned`; `let` over `var`; enums with associated values over tuples;
  `Codable` synthesis; `@Observable` (iOS 17+) with `ObservableObject` fallback
  for iOS 16.
- Never `try!`/`try?` in production — `do/catch`. Custom errors conform to
  `LocalizedError`. Errors carry spans. Top-level `catch` in
  `FluxExecutor.dispatch()` — a VM error never escapes.
- `autoreleasepool` for tight `NSObject` loops; zero-copy binary reads via
  `withUnsafeBytes`; avoid `AnyObject`.
- `XCTest`; `setUp/tearDown`; custom `Equatable` + `XCTAssertEqual`;
  `measure {}` for perf; test on simulator and device when available.
- Concurrency: `DispatchQueue.global(qos: .userInitiated)` for VM evaluation,
  main queue for UI, `CADisplayLink` for frames; never
  `DispatchQueue.main.sync` from main.
- `///` docs on public items; `// MARK: -` organization.

### 2.3 Kotlin

- `ktlint` default rules; zero violations. Kotlin 2.0+; no Java-compat Kotlin
  except at Java boundaries.
- `val` over `var`; `data class` for values; `sealed class/interface` for
  restricted hierarchies; `value class` for inline wrappers
  (e.g. `@JvmInline value class NodeId(val value: UInt)`); `internal` for
  module-private; `@VisibleForTesting` for test access.
- Coroutines: `suspend` fns; cold `Flow`, hot `StateFlow`;
  `Dispatchers.Default` for VM evaluation, `Dispatchers.Main` for UI,
  `withContext` to switch; **never `runBlocking` in production** (tests only);
  `supervisorScope` for fault-tolerant parallel work.
- Catch specific exception types, never bare `Exception`/`Throwable`;
  exceptions carry `Span`s; top-level `try/catch` in `FluxExecutor.dispatch()`.
- JUnit 5, MockK, Turbine, `@ParameterizedTest`, `kotlinx.coroutines.test`
  (`runTest`). Emulator target: Pixel 5, API 34.
- Compose: `@Composable` functions only; `remember { mutableStateOf(…) }`;
  `derivedStateOf`; `Modifier` chains; `LaunchedEffect`/`DisposableEffect`.
- KDoc on every public declaration; `// region` folding.

---

## 3. Flux-Specific Conventions

### 3.1 File Ownership (Boundary Contract)

- You may **READ** any file; you may **WRITE** only inside your assigned
  directory.
- You may **not** modify any manifest (§1.3) — requests go to
  `MANIFEST_REQUESTS.md`.
- If you need a type that doesn't exist in `flux-syntax`, file an issue; the
  orchestrator adds it in a dedicated `flux-syntax` pass. Do not add it
  yourself.

### 3.2 Node IDs and Prop Indices (both load-bearing)

**Node IDs** are `u32` blake3 hashes from
`flux_syntax::compute_node_id(parent_id, tag, span, key)` — **never
sequential**. Tags matter: expressions lower under `ExprTag` (`EXPR_TAG = 10`),
declarations under `DeclTag` (`COMPONENT_TAG = 3`); the families occupy
disjoint byte ranges and using the wrong family silently produces an ID that
matches nothing (the codegen bridge in `flux-codegen-*/bridge.rs` depends on
getting this exactly right). IDs are stable across edits where structure
doesn't change; this drives state preservation and hot-swap.

**Prop indices** are FNV-1a-32 of the prop *name*, masked to `u16`
(`flux_ir::lower::prop_index_for_name`). Host kits **derive** them the same
way (`PropsIndex.propIndexForName` in `adapters/ui-kotlin`) — never hardcode a
positional index, never invent your own numbering. A hardcoded index desyncs
from the server and produces silently blank UI.

### 3.3 Wire Protocol

Appendix D is normative — every field, offset, encoding rule. Do not deviate;
additions require an ADR and a protocol version bump. MessagePack via
`rmp-serde`; string interning rules in §3.8.

### 3.4 VM and Capabilities

Appendix E is frozen — **no new opcodes without an ADR.** The ISA is
intentionally minimal; monomorphization means type-specific ops (`ADD_I64`,
`ADD_F64`).

The **capability system is the extension point**: a `CapabilityRegistry` maps
`(capId, methodId) → impl` and is threaded into the VM. `CALL_CAP` returns a
**result-cell signal id**; cells are `Ready` / `Pending` / `Error` (ADR-0044).
Synchronous capabilities settle the cell before returning; async capabilities
leave it `Pending` and an injected `AsyncResolver` settles it
(`FluxBytecodeVM.runResumable` + `FluxExecutor.dispatchAsync`, ADR-0045).
Capability ids must be **derived deterministically** (same rule on server and
both hosts), never hand-assigned. Lifecycle hooks (`onMount`/`onCleanup`)
register per-node closures that run through the same VM.

### 3.5 Adapter Contract (unified tier)

* **Props are the contract.** Adapters read props only through the typed
  accessors on `Props`/`PropsIndex` (§3.2). Missing/renamed fields degrade to
  `null`/default, never throw.
* One **declarative** implementation per platform per component. Dev renders
  the shadow tree through it (`FluxTreeView` on Android); release codegen emits
  the same vocabulary. Convergence direction (pending ADR): each component's
  platform lowering lives in **one place** shared by the dev renderer and the
  codegen template — do not add a fourth independent copy of a component
  mapping.
* Adapter kits are platform-neutral and JVM/Swift-testable: adapters declare
  intent through `FluxNativeView.setProperty`, hosts translate to real views.
* **Per-node adapter instances via factories** (`FluxAdapterFactory`) — shared
  singletons leak per-node state across siblings (FLUX-007 history). Executors
  are held through `WeakReference`.
* **Keyed reconciliation preserves identity** (`Reconcile.kt`): children match
  by `nodeId`; a node that already exists is reordered, **never recreated** —
  this is what preserves scroll position, text, and screen state across diffs
  and router push/pop.

### 3.6 Grammar Transition (ADR-0029)

The lexer emits `Indent`/`Dedent`/`Newline` layout tokens, but brace-syntax
sources still exist (sample projects, some codegen fixtures). Before writing or
editing a `.flux` test source or fixture, **check which surface that crate's
existing tests assume** and match it. New surface syntax (keywords, token
kinds) lands only via a syntax ADR; the lexer keyword map in
`crates/flux-parser/src/lexer.rs` is the choke point.

### 3.7 Threading — the R-graph (ADR-0027)

The reactive core — signal graph, string resolver, closure table, shadow-tree
mutations — is confined to a single injected `ReactiveDispatcher` (production:
main thread; tests: `StandardTestDispatcher`). Frame bytes are deserialized
**off** that dispatcher, then every stateful step runs `withContext` back onto
it. `dispatch`/`receiveFrame` are `@MainThread`-annotated, mirroring Swift's
`@MainActor`. **Never write signals or mutate the shadow tree from another
thread.** Cross-thread work (network, timers, capabilities) settles results
back onto the reactive dispatcher.

### 3.8 String Interning and Canonical IDs (INV-1)

The host **never synthesizes canonical string ids**. Strings missing from the
wire table are interned by the `InternString` → `StringInterned` RPC against
the dev server, cached in the O(1) reverse index (`StringInterning`). The
deterministic high-half local fallback exists **only** for an unreachable dev
server in dev mode and surfaces as a non-fatal error. Under any future
OTA/production path this fallback is forbidden — canonicality is absolute.

### 3.9 CLI Surface

`flux init <name>` · `flux dev [--root] [--ws-host] [--ws-port] [--http-port]`
(defaults 7331/7332; `--ws-host 0.0.0.0` exposes the server to physical devices
on the LAN) · `flux build --platform ios|android [--root]` (emits
`platforms/<platform>/Generated/`; native toolchains detected, not yet invoked)
· `flux doc` (stdlib JSON schema from `stdlib/`). Project config:
`flux.toml` (`[project] name/entry`, `[dev] ws_port/http_port`); ignore file:
`.fluxignore`.

### 3.10 Performance Budgets

| Operation | Budget | How to verify |
|---|---|---|
| Parse 500-line file | < 5 ms | `cargo bench` |
| Type check 500-line file | < 3 ms | `cargo bench` |
| Diff 50-node tree | < 1 ms | `cargo bench` |
| Serialize 50-node patch | < 1 ms | `cargo bench` |
| VM eval 50-instruction handler | < 2 ms | `XCTest measure` / JUnit benchmark |
| Signal propagation (10 dirty cells) | < 1 ms | same |
| Native view mutation (single update) | < 3 ms | same — under the unified tier this is measured as *observable props write → next composed frame* for a ~50-node subtree |
| Save → pixels (end-to-end) | < 100 ms | integration benchmark |

If you exceed a budget, profile (`cargo bench -- --profile-time=5` /
Instruments / Android Profiler) before submitting.

### 3.11 Error Messages

Every error includes **what** ("expected `Int`, got `String`"), **where**
(file:line:col from span), **why** (hint: "`count` was previously inferred as
`Int` at line 18"), and **how** to fix. Rust-style rendering with source
snippet and caret. VM/wire faults in the host show a red banner, never a crash
(Appendix E §E.6).

### 3.12 Logging

`tracing` (Rust), `os_log` (Swift), `android.util.Log` (Kotlin). Levels:
ERROR/WARN/INFO/DEBUG/TRACE; never INFO or DEBUG in hot paths without a level
check. Host trace emission is guarded by `BuildFlags.DEBUG` (a `const val`, so
R8 strips the call sites from release — keep it that way; runtime toggles
break the compile-out).

---

## 4. Git Conventions

### 4.1 Commit Messages

`<type>(<scope>): <subject>` — types: `feat fix refactor test docs chore perf
ci`; scopes: `parser types ir serde differ devserver codegen-swift
codegen-kotlin cli ios android stdlib tests`. Reference the FLUX issue.

### 4.2 Branching — NONE: commit directly to `main`

All agents work on `main` at once. No branches, no worktrees, no PRs.

- **Commit only your own affected files**, atomically, per logical change.
- **Shared-index hazard (critical):** the shared index usually contains other
  agents' staged files. Always commit with
  `git commit --only <your/files> -m "…"` — never `git add -A` +
  `git commit`, never `git commit -a`. Verify with
  `git diff --cached --name-only` before any commit.
- The `merge-guard` workflow records touched top-level directories in
  `.github/dir-locks.json` and fails overlapping consecutive pushes; the
  `manifest-steward` applies `MANIFEST_REQUESTS.md` weekly. Don't fight them.

---

## 5. Reviewer Checklist (Self-Review Before Committing)

**Rust:** fmt clean · clippy `-D warnings` clean · `cargo nextest run` green ·
`cargo test --doc` green · `cargo doc` clean · benchmarks within budget · no
unwrap/expect/panic in non-test code · no TODO/FIXME/dead code · public items
documented · errors carry what/where/why/how · files ≤ 300 lines, functions
≤ 40, nesting ≤ 3 · edge cases covered (empty, max, boundary, Unicode).

**Swift:** `swift-format lint` · SwiftLint zero warnings · XCTest green · no
`try!`/`try?`/force-unwrap in production · `@MainActor` respected · errors
localized with spans.

**Kotlin:** ktlint zero violations · JUnit 5 green · no `runBlocking` in
production · coroutines confined per §3.7 · MockK/Turbine used where
appropriate · JVM-only modules stay Android-free.

**Flux-specific:** prop indices derived, never hardcoded (§3.2) · node IDs via
`compute_node_id` with the correct tag family (§3.2) · per-node adapter
factories, no shared instances (§3.5) · keyed reconciliation never recreates
existing nodes (§3.5) · no signal writes or shadow-tree mutations off the
reactive dispatcher (§3.7) · no locally synthesized canonical string ids
(§3.8) · new `.flux` sources match their crate's grammar surface (§3.6) · new
opcodes/wire fields only via ADR (§3.3–3.4).
