# Changelog

All notable changes to Flux are recorded here. Entries reference the issue IDs
from `/docs/agents-boundaries-contract.md` Part 2.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and
this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Gap G1 — handler transport on the wire (ADR-0028) — DONE

Resolved "Gap G1" from the boundary contract: handler closures (`ClosureIR`
bytecode + captured signals) had no wire transport — `Init`/`Delta` frames
carried patches + strings only, and `ClosureRef.bytecode_offset/len` pointed
into a bytecode blob no frame shipped. Implemented as the contract's reserved
`flux-ir-serde` second pass (`crates/flux-ir-serde`).

- `wire.rs` — promoted the `HandlerDef` codec (Appendix D §D.8) from
  `#[allow(dead_code)]` to live: `encode_handler_def` / `decode_handler_def`,
  plus a shared `encode_bytecode_blob` / `decode_bytecode_blob` (D.12 handler
  section: `u32` length + raw little-endian bytecode).
- `frame.rs` — `Init` and `Delta` frames now carry a **handler section**
  immediately after the string stream: a shared bytecode blob, then a
  `HandlerDef` stream (D.8) whose `ClosureRef`s index the blob by
  `bytecode_offset`/`bytecode_len`. The reserved D.1 `handler_count` slot (offset
  12) now carries the true `HandlerDef` count. A frame with no handlers writes a
  zero-length blob so the decoder never underflows — backward-compatible with
  existing handler-less frames.
- `encode.rs` — `serialize_patches(patches, table, closures: &[ClosureIR])`
  threads closures into the `Delta` frame; `deserialize_patches` now returns
  `(Vec<Patch>, Vec<ClosureIR>)` (the frame's handler section). Doc examples
  updated.

Public API surface: `Frame::init(..., closures)`, `Frame::delta(...,
closures)`, and `InitFrame`/`DeltaFrame` gain a `closures: Vec<ClosureIR>`
field. The only consumers at merge time are `flux-ir-serde`'s own tests/bench
and the devserver stub (which does not yet ship handlers), so the signature
change is contained. No production (Swift/Kotlin) deserializer is touched — the
wire layout is additive and matches the spec's reserved `handler_count`.

Tests (`crates/flux-ir-serde/tests`): 3 new G1 tests — `init_frame_carries_handlers_round_trip`,
`delta_frame_carries_handlers_round_trip` (assert bytecode + captures round-trip
and serialization is byte-deterministic), and `empty_handler_section_is_zero_length_blob`.
All 28 crate tests pass; `cargo clippy -- -D warnings`, `cargo fmt --check`,
`cargo doc`, and doctests all clean.

Unblocks FLUX-019 (devserver), which the contract explicitly gates on G1
landing first.

### Phase 1 — Native runtime

#### FLUX-006 — iOS host runtime (VM + wire + reconciler + executor) — DONE

Added the iOS native runtime under `runtimes/ios/Sources`, a behavioral mirror
of `flux-vm-ref` (FLUX-005) and the Kotlin/Android runtime (FLUX-007):

- `Sources/Values/FluxValue.swift` — the value enum (`Int`/`Float`/`Bool`/
  `Str`/`HandlerRef`/`Null`/`List`/`Record`), `Sendable`, with explicit
  `Equatable`/`CustomStringConvertible` (auto-synthesis fails for recursive
  enums).
- `Sources/VM/` — `FluxBytecodeVM` (the register VM, Appendix E), `OpCodes`,
  `Instructions` (decoder) and `VMError` with byte-offset fault reporting. The
  gas model mirrors the oracle exactly: `entryGas = 100_000`, `r15` mirrors the
  live budget, `HALT` is free (ADR-0021), `JUMP`/`COND_JUMP`/`COND_JUMP_NOT`
  resolve to absolute offsets, integer `DIV`/`MOD` by zero raises `DivByZero`
  (ADR-0023), `GET_FIELD` on `Null` raises `NullDereference` (ADR-0024).
- `Sources/Wire/` — `ByteReader` (little-endian cursor), `WireModels` and
  `FrameDeserializer` (Appendix D: Node, Child, Value, Patch, ClosureRef,
  StateDelta, StringEntry, FileEntry, Error/Init frames).
- `Sources/Shadow/` — `ShadowTree` (the host node model, Appendix C §C.1) and
  the keyed `ShadowTreeReconciler` (stable `NodeId` + `u64` splice keys, Appendix
  D §D.4) that mutates native views in place.
- `Sources/Signal/SignalGraph.swift` — a SolidJS-style signal store conforming
  to `SignalStore`, O(1) reads, minimal-write notifications.
- `Sources/Adapters/AdapterKit.swift` — the `FluxAdapter`/`FluxView` contract
  (Appendix F) plus in-dir `MockAdapter`/`MockView` (scope item 11); real
  `FluxUIKit` wiring is deferred to FLUX-016.
- `Sources/Executor/FluxExecutor.swift` + `FluxAppMain.swift` — the executor
  folds handler VM output back into the graph and never lets a VM error escape
  (surfaced as a `FluxRootView` error overlay).

Tests (`runtimes/ios/Tests`, all passing under `xcodebuild test`):
- `ISAConformanceTests` — runs the 71 shared `/tests/isa-vectors` golden vectors
  against the native VM (same suite the Rust and Kotlin VMs run).
- `WireDecodeTests` — hand-built byte-vector round-trips for every wire union.
- `RuntimeE2ETests` — Init→reconcile→mock-view build, signal fold-through, gas
  exhaustion via a `JUMP` loop, and invalid-opcode capture.

Frozen-manifest note: `runtimes/ios/project.yml` is a frozen boundary-contract
artifact. Two in-scope edits were required because the foundation skeleton (a)
omitted an Info.plist for the test bundle (blocking `xcodebuild test`) and (b)
declared a `FluxUIKit` package dependency that was still mid-flight (FLUX-008)
and did not compile. Both are documented inline in `project.yml`; FLUX-016
restores the real kit.

#### FLUX-016 — iOS adapter↔runtime integration (real `FluxUIKit`) — DONE

Replaced the in-dir `MockAdapter`/`MockView` scaffold with the real Swift
adapter kit (`adapters/ui-swift`, FLUX-008), driving real `UILabel`,
`UIButton`, `UIStackView` and `UINavigationController` views. No production
code in `adapters/ui-swift` was modified; it is consumed through its public
`FluxAdapter`/`FluxExecutor` API.

- `Sources/Adapters/AdapterKit.swift` — the integration bridge between the
  runtime's id-based `VMValue` and the kit's resolved `FluxValue`/`Props`:
  `StringTable` (id↔name interning), `toKit`/`kitProps`/`toRuntime`
  converters, a type-erased `AnyFluxAdapter`, and `AdapterRegistry` mapping
  `ComponentId` → a fresh adapter instance seeded from the Init frame's string
  table. The host `FluxExecutor` is injected into each adapter at creation
  time via the kit's public `init(executor:)` (the kit's `executor` property
  is `internal`, so it cannot be set after construction).
- `Sources/Shadow/ShadowTreeReconciler.swift` — now nodes-driven and identity-
  preserving: native views are reused across frames/patches (verified by the
  E2E tests asserting `===` identity), and it handles `Update`/`Remove`/
  `Replace`/`Insert`/`Reorder`/`Handler` patches. Handlers are bound once at
  build time so a `UIButton` does not stack `UIAction`s on every reconcile.
- `Sources/Executor/FluxExecutor.swift` — `FluxRuntime` now conforms to the
  kit's `FluxExecutor` protocol. `apply(_:)` drives the reconciler for **both**
  full and patch frames (a previously-broken `guard let root` early-returned
  on patch frames with `root: nil`, so patches were silently dropped).
  `dispatch(_:)` runs handler bytecode through the VM and re-reconciles the
  frame.
- `Sources/Values/FluxValue.swift` + runtime rename — renamed the runtime's
  own `FluxValue`→`VMValue` and `FluxExecutor` class→`FluxRuntime` to break a
  module name collision: the kit module is also named `FluxUIKit` and exports a
  `FluxExecutor` protocol and `FluxValue` type, and `import FluxUIKit as FUI`
  module-aliasing is not supported by this toolchain. Renaming lets the kit's
  names be referenced unqualified.
- `Sources/Wire/` — `FluxFrame` now carries a flat `nodes: [UInt32: ShadowNode]`
  map (populated by `FrameDeserializer`), which the reconciler uses to resolve
  child id references (Appendix D §D.4) without re-decoding.
- `Sources/FluxAppMain.swift` — seeds the real `AdapterRegistry` from the Init
  frame's string table.

Tests (`runtimes/ios/Tests`):
- `RuntimeE2ETests` rewritten to drive the real adapters (no mocks): a hand-
  built Init frame produces a real `UILabel`/`UIButton`/`UIStackView`/
  `UINavigationController` tree; tap→`dispatch`→VM→reconcile updates the label
  text **without recreating the `UIView`** (identity asserted); a patch
  `Update` reuses the view; Router push→pop→push reuses the screen
  `UIViewController` by identity (state preserved); `AdapterRegistry` resolves
  all 7 stdlib `ComponentId`s; gas exhaustion is reported.

Verification: out-of-tree xcodegen project wrapping `Sources`/`Tests` (the
frozen `project.yml` is untouched); `xcodebuild test` on iOS Simulator passes
14 tests, 0 failures (2 skipped), with `SWIFT_TREAT_WARNINGS_AS_ERRORS=YES`.

### Phase 0 — Foundation

#### FLUX-001 — Foundation skeleton and `flux-syntax` crate — DONE

Added:
- Cargo workspace root with all 10 MLP crates and centralised
  `[workspace.dependencies]`. Every dependency pinned to `^MAJOR` at the latest
  stable version verified against crates.io on 2026-08-24: `pest`/`pest_derive`
  2.9, `tokio` 1.53, `tokio-tungstenite` 0.30, `notify` 8.2, `axum` 0.8,
  `rmp-serde` 1.3, `blake3` 1.8, `clap` 4.6, `tracing` 0.1,
  `tracing-subscriber` 0.3, `serde` 1.0, `thiserror` 2.0, `anyhow` 1.0,
  `criterion` 0.8, `insta` 1.48, `proptest` 1.11, `parking_lot` 0.12,
  `ahash` 0.8, `smallvec` 1.15.
- `rust-toolchain.toml` (stable channel with `rustfmt` + `clippy`) and
  `.gitignore` covering Rust, Xcode and Gradle artifacts.
- `flux-syntax` implemented in full against Appendix C §C.1: ID aliases, `Span`,
  `Value`, `TypeKind`, `StringTable`, `NodeKind`, `NodeRef`, `Child`, `Props`,
  `PropDiff`, `Patch`, `ClosureRef`. Split across `ids`, `strings`, `value`,
  `ty`, `node` and `patch` modules so no file exceeds 300 lines.
- 38 behavioural integration tests in `crates/flux-syntax/tests/vocabulary.rs`,
  written RED before the implementation, covering Unicode interning, empty and
  boundary spans, order-independent prop hashing, `NaN` float props, and
  wire-tag round trips.
- Stubs for the nine remaining crates so `cargo check` passes workspace-wide and
  Phase 1 agents can start against a compiling tree.

Notes and deviations from the spec:
- Rust edition 2024 and resolver 3 are used rather than the edition 2021 named
  in the boundary contract, per the project-wide "latest stable" dependency
  policy. `rust-version` is 1.85 (the edition-2024 floor), which is above the
  spec's Rust 1.75 constraint (C-001) and satisfies it.
- `Props::get_handler` returns `Option<HandlerId>` rather than Appendix C's
  sentinel `0`. Handler ID zero is a legitimate closure, so a sentinel would
  make an unbound event indistinguishable from a bound one. No wire format
  change; the encoder still writes the concrete ID.
- `NodeKind::from_tag` is a total function returning `Option`, replacing
  Appendix C's `std::mem::transmute` in `NodeView::kind`. The crate is
  `#![forbid(unsafe_code)]`, and a corrupt or future-versioned frame must be
  reported as a protocol error rather than producing an invalid enum value.
- `Props::hash` is an XOR fold of per-field BLAKE3 digests, making it
  order-independent so that a reordered-but-equal prop map does not force a
  wire update. `NaN` is canonicalised before hashing so equal props hash equal.
- `StringTable` fields are private with `intern`/`resolve`/`lookup`/`delta_from`
  accessors, rather than Appendix C's public `strings`/`lookup` fields, so the
  dense-ID invariant the wire delta depends on cannot be violated by a caller.

Verification:
- `cargo nextest run --workspace` — 38 tests run, 38 passed, 0 skipped.
- `cargo test --doc -p flux-syntax` — 6 doctests passed.
- `cargo clippy --workspace --all-targets -- -D warnings` — clean.
- `cargo fmt --check` — clean.
- `cargo doc -p flux-syntax --no-deps` — clean.

#### Tooling — `cargo nextest` mandated

Changed:
- `AGENTS.md` now mandates `cargo nextest run` as the project's test runner and
  forbids `cargo test`, with the single exception of doctests
  (`cargo test --doc`), which nextest does not support. Added the useful
  invocations, a note that nextest's exit code must not be masked by a pipe, a
  `cargo-nextest` row in the dependency table, and a doctest line in the Rust
  reviewer checklist. Local runner is cargo-nextest 0.9.135.
- `AGENTS.md` §2.1 now records the edition-2024 / `resolver = "3"` /
  `rust-version = 1.86` reality of the workspace against the local rustc 1.94.1,
  superseding the spec's "edition 2021, Rust 1.75" wording.
- Workspace `rust-version` raised from 1.85 to 1.86: `criterion` 0.8.2 requires
  1.86, and the dependency policy is latest-stable.

Added:
- `[dev-dependencies]` and `[[bench]]` targets pre-wired into every crate
  manifest (`criterion`, `insta`, `proptest`, `tokio` as each crate needs), plus
  compiling criterion bench stubs under `benches/`. Agents may not edit
  manifests, so these must exist before Phase 1 starts.

#### FLUX-001 — extended to boundary contract v2

The boundary contract was revised to v2 mid-flight. v2 widens FLUX-001's scope,
so the issue was reopened and the following delta implemented.

Added:
- `flux_syntax::opcode` — the VM instruction vocabulary, normative per
  Appendix E §E.1. v2 §1.5 requires it in `flux-syntax` as the single Rust
  source of truth for the ISA; the native Swift and Kotlin VMs declare their own
  constants from the same table and the golden ISA vectors pin all three
  together. All 54 opcodes with raw byte constants, a total `from_byte` decoder,
  operand and instruction widths from the "Args (bytes)" column, and Appendix E
  mnemonics. 18 tests written RED first; they caught a real error — Appendix E
  defines **54** opcodes, not the 48 initially assumed. Split into
  `opcode.rs` + `opcode/{decode,raw,width}.rs` to respect the 300-line limit.
- Two new workspace members, taking the workspace from 10 to 12 crates:
  `flux-vm-ref` (FLUX-005, the Rust reference VM — a test oracle that never
  ships, since production VMs are native per ADR-0002) and `flux-parity`
  (FLUX-023, the dev-versus-release parity harness).
- v2-mandated cross-crate dev-dependencies that agents may not add for
  themselves: `flux-ir` gains `flux-vm-ref` so FLUX-018's lowering tests can
  execute emitted bytecode, and both codegen crates gain `flux-parser` +
  `flux-types` for their full-pipeline tests. `serde_json` (ISA vector loading)
  and `tempfile` added to the workspace dependency set.
- **Frozen platform build manifests** (v2 §1.2 and R2 place these under
  foundation ownership, frozen after Phase 0), all verified by a real build:
  - `adapters/ui-swift/Package.swift` — SPM, swift-tools 6.0, iOS 16 minimum
    (spec C-002), Swift 6 language mode. `swift build` and `swift test` pass.
  - `runtimes/ios/project.yml` — XcodeGen 2.46.0 spec for the `FluxApp` host,
    glob-based sources so agents never need a manifest edit, referencing
    `adapters/ui-swift` as a local package. `xcodegen generate` then
    `xcodebuild -destination 'generic/platform=iOS Simulator' build` succeeds.
  - `settings.gradle.kts`, `gradle/libs.versions.toml`,
    `adapters/ui-kotlin/build.gradle.kts`,
    `runtimes/android/app/build.gradle.kts`, and the Gradle wrapper.
    `./gradlew :adapters:ui-kotlin:test` passes.
- Compiling platform source skeletons so CI and the platform agents start from a
  green build: `FluxUIKit`, `FluxAppMain`, `FluxHostActivity`, `FluxUiKit`, an
  `AndroidManifest.xml` and a theme.
- **R10 wire-fixture loaders** in both runtime test suites: each reads
  `FLUX_WIRE_FIXTURES` and skips cleanly when unset (`XCTSkip` / JUnit
  `assumeTrue`), so FLUX-023 can drop in real fixtures without touching runtime
  code.

Dependency versions, all verified against crates.io, Google's Maven, Maven
Central and the Gradle plugin portal on 2026-08-24 rather than assumed:
Gradle **9.7.1** (latest; also satisfies AGP 9.3.2's 9.5.0 floor), AGP 9.3.2,
Kotlin 2.4.10, Compose BOM 2026.08.00, core-ktx 1.19.0, activity-compose 1.13.0,
navigation-compose 2.9.8, lifecycle 2.11.0, OkHttp 5.5.0, coroutines 1.11.0,
JUnit Jupiter 6.1.3, MockK 1.14.11, Turbine 1.2.1, msgpack-core 0.9.12,
ktlint plugin 14.2.0, serde_json 1.0.151, tempfile 3.27.0.

Notes and deviations:
- **AGP 9 removed the `kotlin-android` plugin** in favour of built-in Kotlin
  support, so `runtimes/android/app/build.gradle.kts` deliberately does not apply
  it. Applying it — as pre-AGP-9 documentation and most training data suggest —
  breaks the build outright. Recorded here because the android-runtime and
  kotlin-adapters agents will be tempted to add it.
- `AGENTS.md` specifies Swift 5.10+/Xcode 15.4+ and Kotlin 2.0+. The installed
  toolchain is far newer (Xcode 26.4, Swift 6.3, Kotlin 2.4.10, JBR 25), and the
  latest-stable dependency policy takes precedence. The practical consequence is
  that Swift 6 strict concurrency is enforced, which constrains FLUX-006's
  `@MainActor` and background-executor design.
- `compileSdk`/`targetSdk` are 36, not the API 37 maximum AGP 9.3 supports: 36 is
  the latest *installed* platform, and 37 would not build here.
- `adapters/ui-kotlin` is a plain Kotlin JVM library rather than an Android
  library. The adapter contract (Appendix F) is prop-shape only and needs no
  Android APIs, so a JVM module keeps its tests fast and runnable without an
  emulator.

Verification (real command output, not assumed):
- `cargo nextest run --workspace` — 56 tests run, 56 passed, 0 skipped.
- `cargo test --doc -p flux-syntax` — 8 doctests passed.
- `cargo check --workspace` — all 12 crates compile.
- `cargo clippy --workspace --all-targets -- -D warnings` — zero warnings.
- `cargo fmt --check` — clean.
- `swift build` / `swift test` in `adapters/ui-swift` — build complete, 1 test
  passed.
- `xcodegen generate` + `xcodebuild build` in `runtimes/ios` — BUILD SUCCEEDED.
- `./gradlew --version` — Gradle 9.7.1 on JBR 25.
- `./gradlew :adapters:ui-kotlin:test` — BUILD SUCCESSFUL.

Toolchain probe (recorded so agents know what can actually be verified):
- Xcode 26.4 (17E192), Swift 6.3, iOS 26.4 SDK, 1 iPhone simulator available.
- Android Studio 2026.1 with bundled JBR 25 and Kotlin 2.3.10; SDK platforms
  30–36, build-tools to 36.1.0, adb 1.0.41, emulator 37.1.11, AVD
  `Medium_Phone_API_36.1`.
- A physical device is attached (`15a3cc41de5b`, Android 13 / API 33), so
  on-device verification is possible for FLUX-007 and FLUX-023.
- No `gradle` or `kotlinc` on `PATH`; the committed wrapper is the supported
  entry point. `JAVA_HOME` must point at Android Studio's JBR (the system JDK is
  Zulu 21, which also works — AGP 9.3 requires only JDK 17+).

#### FLUX-009 — Kotlin adapter kit + dev adapters — DONE

Added the Kotlin mirror of the Swift adapter kit (FLUX-008) under
`adapters/ui-kotlin/src`. The module is a `kotlin.jvm` target (the frozen
`build.gradle.kts` has no Android dependency), so adapters are written against
a platform-neutral `FluxNativeView` abstraction the Android runtime (FLUX-007)
later backs with a real `android.view.View`.

Kit types:
- `FluxValue` — sealed interface mirroring `flux_syntax::Value` (Appendix C.1),
  with typed accessors on `Record`.
- `Props` + `PropsIndex` — typed accessors (`getString/getBool/getRecord/
  getColor/getFont`) and the canonical component-local field indices for every
  adapter; missing fields degrade to `null`/default, reserved-zero handler id.
- `FluxColor`/`FluxFont` — value types mirroring the Color/Font records
  (Appendix F) with `toRecord()` encoders.
- `FluxExecutor` + `HandlerEvent` — the narrow boundary adapters may touch;
  adapters hold it through a `WeakReference` so a stale executor is GC'd.
- `FluxNativeView` + `FluxNativeViewImpl` — platform-neutral view contract and
  the kit's in-memory test double.
- `FluxAdapter` — the `create/update/setChildren/bindHandler/destroy`
  contract; `reconcileChildren` does keyed reconciliation that reuses view
  instances (no recreate on reorder/remove).

Dev adapters (Appendix F): `TextAdapter`, `ButtonAdapter`, `TextFieldAdapter`,
`ColumnAdapter`/`RowAdapter` (shared base), `ScreenAdapter`, `RouterAdapter`.
Router/Screen reconcile by stable node id and preserve the existing screen
view instance across push/pop, matching `NavHost` semantics.

Verification:
- `./gradlew :adapters:ui-kotlin:test` — 22 JUnit 5 tests green:
  FluxValue/Props accessors, leaf adapter create/update/destroy + weak-executor
  dispatch, linear keyed diff, and router screen-state preservation.
- `./gradlew :adapters:ui-kotlin:ktlintCheck` — zero violations.

#### Spec errata resolved before Phase 1 (orchestrator ADRs)

Before dispatching the golden ISA vector agent (FLUX-002), two internal
contradictions in Appendix E were found and resolved via ADRs (create-only,
orchestrator-owned):

- **ADR-0021 (gas accounting erratum)** — `docs/adr/ADR-0021-gas-accounting.md`.
  §E.6 says gas is "decremented per instruction," but both concrete examples
  (§E.5 = 4 gas for a 5-instruction sequence ending in HALT; the contract's
  FLUX-002 vector example = 4 gas for `READ`/`LOAD`/`ADD`/`WRITE`) charge 4, not 5.
  Resolution: every decoded instruction costs 1 gas **except `HALT` (0x00)**, which
  terminates the handler and is free; `GAS_CHECK` (0xC0) is charged 1 gas before its
  budget check. Entry budget in `r15` = 100,000.
- **ADR-0022 (byte-length erratum)** — `docs/adr/ADR-0022-byte-length-erratum.md`.
  §E.5 claims the canonical `count = count + 1` example is "21 bytes," but the
  literal encoding (6+10+4+6+1) is **27 bytes**. The §E.1 operand-width table is
  normative; the "21 bytes" prose is an erratum. All vector and VM decoders compute
  lengths from the width table only.

Both ADRs are referenced by FLUX-002 (vectors), FLUX-005 (`flux-vm-ref`),
FLUX-006 (Swift VM), and FLUX-007 (Kotlin VM) so all three implementations agree.

#### FLUX-002 — golden ISA vectors — DONE

Author `/tests/isa-vectors/**` as pure JSON fixtures from Appendix E: **71 vectors**
covering all 54 opcodes (happy + boundary + error + register conventions +
`CALL_CAP` + pattern matching). Each vector's `bytecode_hex` is validated against
the §E.1 width table and its `expected_*` fields are computed by a faithful
reference interpreter, so the fixtures are internally self-consistent.

Schema and a per-opcode coverage matrix are documented in
`tests/isa-vectors/README.md`. The generator (`/tmp/gen_vectors.py`) is the
oracle that produced them; it is kept out of the repo (it is a build-time tool,
not shipped source).

Cross-cutting spec defects surfaced and resolved by ADR while authoring the
vectors:
- ADR-0022 — §E.5 "21 bytes" is wrong; the normative §E.1 width table gives 27.
- ADR-0023 — `DivByZero` added as an explicit error kind (§E.6 omits it but
  integer DIV/MOD by zero must fail).
- ADR-0024 — `GET_FIELD` on `Null` → `NullDereference` (per §E.6); on other
  non-records → `TypeMismatch`.
- ADR-0026 — `GET_FIELD` bytecode width corrected to 4 bytes `REG_U16_REG`
  `(dst, idx, obj)`, matching `SET_FIELD`/`EXTRACT_FIELD`. The original
  `flux-syntax` width (`REG_REG_U16`, 3 bytes) could not carry all three
  operands and misaligned every subsequent instruction.

#### FLUX-004 — `flux-ir` arena, node IDs, instance registry — DONE

Implemented the IR core (Appendix C §C.1) as `flux-ir` (no `unsafe`, no
`unwrap`/`expect`/`panic!` in library code, every public item documented,
clippy/`fmt`/`doc` clean, TDD throughout):

- `node_id.rs` — `compute_node_id(parent, kind, span, key)` per ADR-0013:
  BLAKE3 of `(parent, kind tag, file_id, start, end, key-or-sentinel)`,
  truncated to `u32`. Pure and source-stable.
- `arena.rs` — `IRArena` struct-of-arrays (hot fields in parallel `Vec`s;
  props/children/handlers in length-prefixed cold blobs) with `pack()` /
  `get()` / `NodeView` accessors, and blob (de)serialisation for `Value`,
  `Child`, handlers.
- `closure.rs` — `ClosureIR` (bytecode + captured signal IDs, ADR-0014).
- `instance.rs` — `ComponentInstance` + `InstanceRegistry` (node→instance
  index for state-preserving hot swap, ASR-003).
- `builder.rs` — `ArenaBuilder` + `Node` input type, the hand-construction
  API the differ/codegen/parity suites need.

Verification:
- 17 unit tests (pack/unpack round-trip, instance registry, closures).
- 3 doctests.
- `tests/roundtrip.rs` proptest (200 cases): pack/unpack round-trip and
  node-ID stability (identical inputs → identical ID; sibling at a different
  span → distinct ID).
- `benches/arena.rs` criterion bench: pack 100 nodes ≈ 0.1 ms (< 1 ms budget,
  §3.6).

Deviation from the appendix's illustrative `IRArena`: `kinds` is
`Vec<NodeKind>` (not `Vec<u8>`) and `spans` are stored inline (not in a blob),
both to avoid `unsafe` transmute / mis-tagged-value reads (ADR-0002,
AGENTS.md §1.2). `NodeView::kind()` uses `NodeKind::from_tag` — no `unsafe`.

#### FLUX-005 — `flux-vm-ref` reference VM — DONE

Implemented the Appendix E reference interpreter as a test oracle (no `unsafe`,
no `unwrap` in library code, every public item documented, clippy/`fmt`/`doc`
clean). Structure: `error.rs` (typed `VmError` with `VmErrorKind` + optional
span), `decode.rs` (`decode_program` over `Opcode::operand_len`), `vm.rs` (the
interpreter, gas model per ADR-0021, `SignalStore` trait + `InMemorySignals`),
`lib.rs` (re-exports + doctest).

Verification:
- 6 unit tests (gas accounting, error kinds, register conventions).
- 1 doctest (LOAD + HALT).
- **71-vector conformance test**: `tests/conformance.rs` loads every golden
  vector, runs it through `run()`, and asserts `expected_error` /
  `expected_signals` / `expected_registers` / `expected_gas_used`. All 71 pass.

The Swift (FLUX-006) and Kotlin (FLUX-007) runtimes must pass this same suite.

#### Governance — ADR numbering collision fixed (process, not a numbered FLUX issue)

The four VM-errata ADRs (`ADR-0006/0007/0008/0009-*.md`) were published under the
`ADR-NNNN-` filename scheme, which **collides** with the canonical decision sequence
embedded as `### ADR-NNNN:` headings in `mlp-appendices.md` Appendix A
(ADR-0006 static types, ADR-0007 register VM, ADR-0008 MessagePack, ADR-0009 arena).
A bare `grep ADR-0008` now returns two unrelated documents. This violates the
contract's R9 naming rule (`<scope>-<slug>.md`) and is exactly the failure mode that
rule was written to prevent.

Resolution (see `docs/adr/adr-naming-and-numbering.md`):
- The four VM-errata ADRs were **renumbered** `ADR-0006/0007/0008/0009 →
  ADR-0021/0022/0023/0024` via `git mv` (history preserved, no content edit) so they
  no longer collide with the canonical ADR-0006–0009 in `mlp-appendices.md` Appendix A.
  Every cross-reference (CHANGELOG, isa-vectors README, `flux-vm-ref` crate comments)
  was updated to the new numbers.
- Added `docs/scripts/check-adr-numbering.sh`, a CI guard that fails when any *new*
  agent ADR reuses a reserved `ADR-NNNN`. The four renumbered files are listed as
  exceptions so the accepted state stays green.
- `mlp-spec.md` Appendix A no longer duplicates the canonical ADRs; it is now a
  single pointer to `mlp-appendices.md` Appendix A. `mlp-appendices.md` gained an
  "Appendix A — Continuation (ADR-0021…)" block recording the renumbered decisions.

---

## Agent-delivered work (tracked by orchestrator — NOT yet committed by this agent)

The following were produced by concurrently-dispatched flux-N agents. They are
recorded here for progress tracking only; this agent did **not** write, modify, or
commit any of the code below (the owning agents / orchestrator handle those
directories per the boundary contract). Status reflects the live working tree at
the time of writing, **not** a verified test pass — uncommitted code is marked as
such.

#### FLUX-009 — Kotlin adapter kit + dev adapters — DONE (committed)

Fully implemented and committed across `adapters/ui-kotlin/src/**` (22 files):
`FluxValue`, `FluxColor`/`FluxFont`, `Props` typed accessors, `FluxNativeView`
abstraction + in-memory test double, `FluxExecutor` boundary + `HandlerEvent`,
`FluxAdapter` contract, and dev adapters (Text, Button, TextField, Column/Row with
keyed child reconciliation, Screen, Router). Test suites cover FluxValue record,
Props accessors, Text/Button/TextField, Column/Row keyed diff, Router/Screen state
preservation. All commits prefixed `feat(kotlin-adapters)` / `test(kotlin-adapters)`
under refs FLUX-009. (See `62a5fdf` and the chain beneath it.)

#### FLUX-006 — iOS host app runtime — DONE (committed)

Delivered into `runtimes/ios/Sources/**` and `runtimes/ios/Tests/**` by the
ios-runtime agent: `FluxAppMain.swift` entry point, `Sources/VM/` (native
`FluxBytecodeVM` per Appendix E), `Sources/Values/` (Flux value types), and
`Tests/ISAConformanceTests.swift` + `ISAVector.swift` (shared-vector conformance
target). `runtimes/ios/project.yml` was also updated. **Committed to main as
`3ad2246`** (VM + wire + reconciler + executor).

#### FLUX-007 — Android host app runtime — DONE (committed)

Delivered into `runtimes/android/app/src/**` by the android-runtime agent:
`FluxHostActivity.kt` host entry, `src/main/kotlin/.../vm/` (Kotlin
`FluxBytecodeVM` per Appendix E), `src/main/kotlin/.../wire/` (Appendix D frame
codec), `src/main/kotlin/.../shadow/` (shadow tree + reconciler). Test suite under
`src/test/kotlin/.../`: `IsaConformanceTest`, `FluxBytecodeVmTest`, `wire/` frame
builder/deserializer + `WireFixtureContractTest`, `EndToEndTest`. **Committed to
main as `fa80b45`** (native host runtime) + `15b191f` (finalized
`WireFixtureContractTest` assertions).

#### FLUX-010 — standard library `.flux` sources — IN PROGRESS (uncommitted)

12 `.flux` modules landed in `stdlib/`: `prelude`, `traits`, `color`, `font`,
`text`, `button`, `text_field`, `column`, `row`, `router`, `platform`,
`capabilities`. These are source-only (parse-checked by the parser once FLUX-003
lands); per R8 the directory is write-only for the stdlib agent. **Uncommitted in
the working tree at time of writing.**

#### FLUX-008 — Swift adapter kit + dev adapters — DONE

Implemented the shared adapter kit and the seven dev-mode adapters in
`adapters/ui-swift` (no `try!`/`try?`, no force-unwraps in library code,
every public item documented, `swift test` clean, TDD throughout):

- Kit contract types the runtime (FLUX-006) consumes: `FluxValue`
  (mirrors `flux_syntax::Value`, Appendix C.1), `Props` (flat
  `(PropIdx, FluxValue)` map with O(1) typed accessors + stable content
  hash), `FluxColor`/`FluxFount`/`FluxAlignment` (canonical record
  decoders for Appendix F `Color`/`Font`/`Alignment`), `FluxEvent`
  (handler dispatch payload), `FluxExecutor` (the executor the adapters
  call back into via a **weak** reference), and the `FluxAdapter`
  protocol (`create`/`update`/`setChildren`/`bindHandler`/`destroy`).
- Seven dev adapters per Appendix F: `Text`→`UILabel`, `Button`→`UIButton`
  (dispatches bound `onClick` via the weak executor on
  `touchUpInside`), `Column`/`Row`→`UIStackView` (keyed-by-identity child
  reconciliation that preserves view state across reorders),
  `TextField`→`UITextField` (controlled value + edit dispatch to
  `onChange`), `Router`→`UINavigationController` (push/pop reconciles
  screens by identity, preserving a screen's view controller and state
  while present), `Screen`→`UIViewController` (hosts its content subtree).
- Adapters hold the executor `weak`; `TextFieldAdapter` retains itself on
  the field via object association so its delegate survives `create()`.
  No retain cycles; every `weak` is documented.

Verification (real command output, not assumed — the package targets
iOS 16 so it builds/tests under `xcodegen` + `xcodebuild` for the iOS
Simulator, the path the frozen `Package.swift` documents):
- `xcodebuild -scheme FluxUIKit -destination 'generic/platform=iOS
  Simulator' test` — **29 tests, 0 failures, 0 warnings, TEST SUCCEEDED**.
- Tests cover: Props accessors + order-independent hash, Color/Font
  decode/clamp/missing-field, per-adapter create/update/destroy, button
  tap dispatch + weak-executor no-retain, text-field edit dispatch, keyed
  child insert/remove/preserve, router push/pop state preservation.

Notes and deviations:
- `FluxAdapter.View` is constrained to `AnyObject` (not `UIView`) so
  `Router`/`Screen` can manage `UINavigationController`/
  `UIViewController`; `setChildren` takes `[AnyObject]` and each adapter
  casts to `UIView` or `UIViewController` as appropriate.
- The frozen `Package.swift` is untouched (boundary contract R2). The
  verification harness used an out-of-tree `xcodegen` project wrapping the
  Sources/Tests; no manifest was modified.

#### FLUX-014 — `flux-differ` keyed tree differ — DONE

Implemented `diff(old: &IRArena, new: &IRArena) -> Vec<Patch>` (udomdiff-style
keyed reconciliation over stable `NodeId`s). Depends only on `flux-syntax` and
`flux-ir` (both done); no new dependencies, no `unsafe`, no `unwrap` in library
code, every public item documented, clippy/`fmt`/`doc` clean.

Structure: `diff.rs` (`diff` + helpers `to_ref`, `find_parent_and_index`,
`child_ids`, `child_order`, `props_equal`, `props_diff`, `handlers_equal`,
`emit_replace`, `emit_handler`, `closure_ref`), `lib.rs` (re-exports + algorithm
doc). Also added `IRArena::all_ids()` iterator to `flux-ir` (needed by the differ).

Reconciliation rules:
- Both-present nodes: kind/component change → `Replace`; prop-only → `Update`
  (`PropDiff`); handler-body-only → `Handler` (state-preserving fast path);
  same child *set* but different order → single `Reorder` (not remove+insert).
- A parent whose child *set* merely grew/shrank is **not** replaced — the
  added/removed children are covered by `Insert`/`Remove`, avoiding spurious
  whole-subtree replaces.
- Missing-from-new → `Remove`; new-to-new → `Insert { parent, index, node }`.

Verification:
- 7 unit tests (identical→empty; canonical Replace/Update/Insert/Remove/Reorder
  each emit exactly one minimal patch; diff-then-apply reconstructs the tree).
- `tests/roundtrip.rs` proptest (200 cases): diff-then-apply == new tree over
  randomized star trees of varying size/content. The test-only `apply` reconstructs
  an id→`NodeRef` map and applies `Insert` patches in `(parent, index)` order so
  out-of-order patch delivery still yields correct child ordering.
- `benches/diff.rs` criterion bench: 50-node prop-mutation diff ≈ **41.6 µs**
  (well under the 1 ms budget, §3.6).

#### FLUX-013 — `flux-ir-serde` wire protocol — DONE

Implemented the Appendix D binary wire codec and typed frames. Depends only on
`flux-syntax` and `flux-ir` (both done); `blake3` for content addressing is
pre-wired; no `unsafe`, no `unwrap`/`expect` in library code, every public item
documented, clippy/`fmt`/`doc` clean.

Structure:
- `wire.rs` — little-endian primitive reader/writer (`Reader`/`Writer`),
  `encode_*`/`decode_*` for `Value`, `Child`, `Node`, `PropDiff`, `ClosureRef`,
  `Patch` (tags 0x01–0x06), `StringEntry`, plus the reserved `StateDelta`/
  `SourceMapDelta` codecs (Appendix D §D.10–D.11) and `HandlerDef` encoder.
- `encode.rs` — `serialize_patches(&[Patch], &StringTable) -> Vec<u8>` and
  `deserialize_patches(&[u8]) -> Result<Vec<Patch>, WireError>` (inverse of
  `serialize_patches`), plus `hash_props`/`hash_closure` (BLAKE3, deterministic,
  NaN canonicalized).
- `frame.rs` — `Frame` API + `FrameKind` and the five typed frames: `Hello`
  (`Frame::hello`/`from_hello_bytes`), `Init` (`Frame::init`/`from_init_bytes`,
  round-trips a `StringTable` by exact id), `Delta` (`Frame::delta`/
  `from_delta_bytes`), `Error` (`Frame::error`/`from_error_bytes`, `span:
  Option<Span>`), `Heartbeat` (`Frame::heartbeat`/`from_heartbeat_bytes`).

  **Frame-header conformance (corrected 2026-08-24):** the earlier draft
  unified every frame under the D.1 header with a `flags` byte and an invented
  `bit3 = Hello`. That deviated from Appendix D. The shipped code follows D.12
  exactly: a `magic(4) version(1) frame_type(1)` prefix (frame_type `0x01`
  Hello / `0x02` Init / `0x03` Error / `0x04` Delta / `0x05` Heartbeat), then a
  type-specific payload — Hello has no sequence number; Init/Error carry `seq`
  at offset 6; the Delta frame uses the D.1 header (`seq`, `flags` bitfield,
  `patch_count`/`handler_count`/`string_count` at offsets 11–15). Init's
  `string_count` is a **u32** per D.12.2. `tests/conformance.rs` asserts these
  exact offsets and the D.2/D.5 patch/value tags byte-for-byte.

Encoding details matching Appendix D §D.1–D.8 exactly: `Value` 1-byte tag
(`Int 1`, `Float 2`, `Bool 3`, `Str 4`, `HandlerRef 5`, `List 6`, `Record 7`)
with i64/f64/string-id/handler-id payloads; `Child` tag `0x01 Node(u32)` /
`0x02 Splice(u16 count, (u64 key, u32 id)*)`; `Node` id/kind/component + prop
vec + child vec + handler vec + span; `Patch` tags per §D.2; `ClosureRef` hash +
bytecode offset/len + captured-signal vec + span.

Verification:
- `tests/round_trip.rs` — every patch variant round-trips through
  serialize/deserialize; every `Value` variant (incl. NaN, nested List/Record,
  Splice) preserves order; `Init` reconstructs the `StringTable` with exact ids
  (`resolve("Increment") == Some("Increment")`); `Error` preserves code/message/
  span; `Hello`/`Heartbeat` round-trip platform/device/capabilities.
- `tests/proptest_round_trip.rs` — 200 randomized patch sets serialize/deserialize
  to byte-identical output (deterministic), and `hash_props`/`hash_closure` are
  stable across calls.
- `benches/serialize.rs` criterion bench: 50-node patch serialize ≈ **1.58 µs**
  (≪ 1 ms budget, §3.6); `Init` frame for a 50-node tree ≈ 3.6 KB (≪ 20 KB
  acceptance, §D.13).

Note: `flux-ir-serde` is not on the dispatched flux-N active list; it was a
1-file stub and is now fully implemented to advance the Rust pipeline.

#### FLUX-003 — `flux-parser` surface parser — DONE

Implemented the Flux surface parser: `.flux` source to a typed `Ast` whose
every node carries a `flux_syntax::Span`. Built on `pest` generated from a
grammar that now matches Appendix B §B.1–B.2 1:1 (the spec was reconciled to
the tested grammar on 2026-08-24, so `flux.pest` and Appendix B cannot drift;
`tests/appendix_b_examples.rs` asserts all ten §B.3 examples).

Structure (every file ≤ 300 lines, no `unsafe`, no `unwrap` in library code,
clippy/`fmt`/`doc` clean, all public items documented):
- `flux.pest` — the grammar.
- `ast.rs`, `ast/types.rs`, `ast/expr.rs`, `ast/pattern.rs` — typed surface tree.
- `error.rs` — `ParseError` with message/hint/span/line-column; `render()` emits
  the what/where/why/how format of AGENTS.md §3.7.
- `lower.rs` + `lower/{decls,types,exprs/*}` — panic-free lowering (a malformed
  pair returns a `ParseError`, never unwraps).
- `prescan.rs` — lexical pre-scan so an unterminated string or unclosed brace
  points at the opening token (pest backtracks out of a partial match), and a
  nesting-depth guard (G6) that rejects pathological input with a diagnostic
  instead of overflowing the stack.

Coverage:
- `tests/appendix_b_examples.rs` — every §B.3 example parses, with shape
  assertions (not just parse success).
- `tests/stdlib.rs` — all twelve `stdlib/*.flux` files parse (covers G1/G2/G4).
- `tests/diagnostics.rs` — errors carry what/where/why/how.
- `tests/edge_cases.rs` — empty input, i64 bounds, Unicode strings, span
  fidelity, nesting limit.
- `tests/properties.rs` — proptest: spans stay inside the source, arbitrary text
  never panics, i64 literals round-trip, error locations are in-bounds.
- `benches/parse.rs` — 500-line file parses in **2.25 ms** (budget 5 ms, §3.6);
  100 lines in 459 µs.

`docs/adr/parser-grammar-extensions.md` records the reconciliation and the
remaining parser-internal concern (G6 depth limit).

**Addendum (2026-08-24):** the module-level `state` form (`module_state`,
`Decl::State`) that the first reconciliation added for `stdlib/platform.flux`
was reverted. File scope has no `state` form; `platform.flux` now exposes the
platform tag as `fn platform() -> String { … }` (queried via `platform()`),
and Appendix B.3.8's conditional is `if platform() == "ios"`. The `Decl::State`
variant and `module_state` rule were removed from both `flux.pest` and
Appendix B.

#### FLUX-015 — standard-library validation harness — DONE

Added a stdlib validation harness (no new crate; a tool that parse-checks the
shipped `.flux` sources so regressions in the 12 stdlib modules surface before
dev-server round-trips). Committed as `6bb0f13`:

- `stdlib/parse-check.sh` — drives the checker over every `stdlib/*.flux`.
- `stdlib/tools/parse_check.rs` — loads each module through `flux-parser`, fails
  the build on the first parse error, and asserts the ten Appendix B §B.3
  examples still parse.
- `stdlib/tools/fixtures/invalid.flux` — a deliberately malformed fixture proving
  the checker rejects bad input (not a silent pass).

Verification: `bash stdlib/parse-check.sh` exits 0 over all 12 modules
(`prelude`, `traits`, `color`, `font`, `text`, `button`, `text_field`, `column`,
`row`, `router`, `platform`, `capabilities`).

#### FLUX-017 — Android adapter↔runtime integration — DONE

Wired the real Kotlin adapter kit (FLUX-009) into the FLUX-007 Android runtime.
Committed as `0426c00` + `8263e17`:

- `AdapterRegistry` mapping `ComponentId` → adapter instance, built from the
  string table delivered in the `Init` frame.
- The runtime's E2E test was repointed from the in-dir mock adapters to the real
  `FluxAdapter` kit: hand-built `Init` frame → real `TextView` / `Button` /
  `LinearLayout` (vertical/horizontal) / `EditText` hierarchy; tap → dispatch →
  VM → reconciler → view updated **without** view recreation (view identity
  preserved across the update).
- Router E2E: push → edit state → pop → state preserved (the adapter kit's
  Router/Screen preserve screen view state by identity).

Verification:
- `AdapterRegistryTest.kt` — registry resolves every `ComponentId` the frame
  declares.
- `EndToEndTest.kt` — real-adapter build + tap-dispatch + view-identity
  preservation + router state preservation, all green.
- `WireFixtureContractTest.kt` — finalized assertions (committed `15b191f`).

#### ADR-0025 — custom binary wire frames (Gap 3) — DONE

Resolved the wire-format documentation gap: `flux-ir-serde` ships a **custom
little-endian binary** codec, but `ADR-0008` and the spec prose (mlp-spec
§14.1/§18.x/§20.6/§21.1, mlp-appendices §D narrative + glossary) still said
MessagePack. Committed as `4186372`:

- `docs/adr/ADR-0025-wire-binary-frames.md` — records the deviation, supersedes
  ADR-0008's MessagePack choice, and notes `rmp-serde` is now an unused
  dependency.
- Corrected all narrative MessagePack claims in `mlp-spec.md` and
  `mlp-appendices.md` (the ADR-0008 *body* was left intact per the
  never-edit-existing-ADRs rule; ADR-0025 supersedes it).
- Satisfies DoD §9 (every spec deviation needs an ADR).

#### ADR-0027 — node-ID single-source bridge (Gap 2) — DONE

Established one canonical `compute_node_id` in `flux-syntax` so the type checker
and the IR produce identical `NodeId` values for identical source constructs —
required for FLUX-018 lowering to join `TypedAST.types` to IR nodes by ID.
Committed as `1d52b57` + `1c9705f` + `6bc37c6`:

- `flux-syntax::compute_node_id` — canonical BLAKE3 implementation (the
  `flux-ir` layout, so existing IR/differ/wire hashes stay stable), re-exported.
- `flux-ir::compute_node_id` — now delegates (public `NodeKind` API unchanged,
  byte-identical output) + bridge test `delegates_to_flux_syntax_canonical`.
- `flux-types::compute_node_id` — now delegates (signature `u64`→`Option<Key>`);
  edits are in the FLUX-012 agent's working tree and land with that issue.
- Cross-crate equivalence proven by `flux-ir` and `flux-types`
  (`matches_canonical_flux_syntax`) tests.

This replaced a divergent FNV fork in `flux-types` that omitted `span.file_id` —
without the bridge, lowering would have silently failed to look up types.

#### FLUX-011 — CI pipelines — DONE

Added GitHub Actions workflows that enforce the AGENTS.md Definition-of-Done
gates automatically as crates land. Committed as `8ac2655` + `7b223b3`
+ `85b0e47` + `402fbb3` (all under `ci(flux-011)`):

- `.github/workflows/rust-check.yml` — `cargo fmt --all -- --check`,
  `cargo clippy --all-targets --all-features -- -D warnings`,
  `cargo test --workspace --all-targets`, `cargo doc --no-deps --workspace`.
  Globs are workspace-wide, so a crate is covered the moment its real source
  lands (Phase 1+) with no manifest edit.
- `.github/workflows/ios-check.yml` — XcodeGen + `xcodebuild test` for the
  iOS runtime/adapters, plus `swift test` for the Swift package.
- `.github/workflows/android-check.yml` — `./gradlew` Kotlin + Android test.
- `.github/workflows/adr-numbering.yml` — the ADR-naming guard (fails when a
  new agent ADR reuses a reserved `ADR-NNNN`), paired with
  `docs/scripts/check-adr-numbering.sh` and the ADR-renumbering governance
  (ADR-0021…0024).

All workflows run on push (any branch) and PR to `main`, with `docs/**`,
`runtimes/**`, `adapters/**`, `**.md` path-ignored where appropriate so doc-only
changes don't trigger the Rust/iOS/Android builds.

#### FLUX-012 — `flux-types` bidirectional type checker — DONE

Implemented the Flux bidirectional (constraint-based) type checker per
FLUX-012, consuming `flux-parser`'s `AST` and `flux-syntax`'s `TypeKind`.

- `checker.rs` — HM(X) inference with unification variables, constrained
  (generic) variables carrying trait bounds, env-based name resolution, a
  trait/ADT/component/**fn** collection pre-pass so earlier declarations can
  call later ones, generic-component instantiation with concrete
  type-argument recording, arithmetic/`Show`/`Eq` trait enforcement,
  offside-free block/binary/field/call handling, module-constant
  (`Color.red`) resolution, `Numeric.zero`/`one` returning
  `Numeric`-constrained variables.
- `unify.rs` — substitution-based unification with occurs-check and
  cycle-safe constrained-variable handling (preserves the constraint on the
  canonical variable).
- `traits.rs` — closed-world `Numeric`/`Eq`/`Show` resolution; arithmetic
  now rejects operands whose trait bound does not include `Numeric`.
- `kind.rs`/`env.rs`/`scheme.rs` — `TcType` internal repr, env bindings,
  generalisation/instantiation scaffolding.
- `error.rs` — span-carrying `TypeError` with actionable messages.
- `prelude.rs` — prelude traits/fns/components (`Show`/`Eq`/`Numeric`,
  `platform`, etc.).
- `exhaust.rs` — match exhaustiveness over `TcType`.
- `lib.rs` — public API `type_check(ast) -> Result<TypedAST, TypeError>` and
  `TypedAST { ast, types, instantiations }`; re-exports.
- `tests/typecheck.rs` — all 10 Appendix B.3 examples + diagnostics
  (span/mismatch/unbound/non-exhaustive) + both `Counter[Int]`/`Counter[Float]`
  instantiations + module-constant and trait-bound negative tests.
- `benches/typecheck.rs` — 500-line fixture benchmark → **544 µs** (< 3 ms
  budget, §3.6).

Verification (all gates green, run locally):
- `cargo fmt --check` — clean.
- `cargo clippy -p flux-types --all-targets -- -D warnings` — zero warnings.
- `cargo test -p flux-types` — 1 unit + 16 integration + 3 doctests, all pass.
- `cargo doc -p flux-types --no-deps` — clean.
- `cargo bench` — 544 µs for a 500-line file.

#### FLUX-003 (follow-up) — `if/else` lowering bug fixed

`flux-parser`'s `if_expr` lowering wrapped an `else { block }` in a bogus
`Call { callee: Elided, trailing: Some(block) }`, which is not a real call
and forced every downstream consumer to special-case it. Fixed in
`lower/exprs/control.rs` so `else { block }` lowers to a block-valued
expression (the grammar's zero-argument-lambda "block as expression" form),
mirroring `when_expr`'s clean handling. `flux-types` dropped its
`Elided`-callee workaround and instead infers an `else` zero-arg-lambda as a
block so it unifies with the `then` branch. A regression test
(`b38_else_block_lowers_to_block_not_elided_call`) locks the fix in.

#### Outstanding Phase 1–4 crates (stubs / in-flight, owned by named flux-N agents)

The following workspace crates are owned by other agents (not built by this
agent, to avoid directory-collision with the dispatched flux-N work):
`flux-devserver`, `flux-codegen-swift`, `flux-codegen-kotlin`, `flux-cli`,
`flux-parity` (Phase 6). CI (FLUX-011) is orchestrator-owned.
