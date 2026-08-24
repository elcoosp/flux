# Changelog

All notable changes to Flux are recorded here. Entries reference the issue IDs
from `/docs/agents-boundaries-contract.md` Part 2.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and
this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
