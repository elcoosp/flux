# AGENTS.md — Flux Project Agent Operating Manual

> **Read this file in its entirety before touching any file in this repository.**
> Failure to follow these conventions will result in your work being rejected.

---

## 0. Project Identity

**Flux** is a write-once UI language for native iOS/Android development. A Rust dev server parses `.flux` source, lowers it to a Reactive Tree IR, diffs it, and ships binary patches over WebSocket to a precompiled host app. The host app contains an embedded register-based bytecode VM, a SolidJS-style signal graph, and a shadow tree of native views. In release mode, the same IR is codegen'd to idiomatic Swift/SwiftUI and Kotlin/Jetpack Compose.

**You are building the MLP.** The spec is in `/docs/spec/mlp-spec.md` and `/docs/spec/mlp-appendices.md`. Read the relevant appendix before starting any work. Do not improvise architecture — follow the spec.

---

## 1. Core Engineering Principles

### 1.1 Test-Driven Development (Non-Negotiable)

**Every line of production code must be preceded by a failing test.** No exceptions.

The cycle:
1. **Red** — Write a test that describes the desired behavior. Run it. It must fail. If it passes, you haven't understood the problem.
2. **Green** — Write the minimum code to make the test pass. Do not add anything else. Do not refactor yet.
3. **Refactor** — Improve the code while keeping tests green. Run tests after every change.

Rules:
- Never write production code without a failing test that requires it.
- Never comment out a test. If it's broken, fix it or delete it with justification.
- Never use `#[ignore]` or `xtest` or `pending` to skip a test. Fix it now.
- Tests are production code. They get the same quality bar: no duplication, clear naming, single responsibility.
- Test names describe behavior, not implementation: `test_counter_increments_on_tap`, not `test_1`.
- One assert per test when possible. If multiple asserts, they must describe one behavior.
- Test the public API. Do not test private implementation details. If you need to test a private function, it should probably be public or extracted.

### 1.2 Quality Bar — 20+ Year Senior Engineer

Your code must be indistinguishable from code written by a principal/staff engineer with 20+ years of systems programming experience. This means:

- **No `unwrap()`, `expect()`, `panic!()`, `!!`, or force-try (`!`) in production code.** These are `unreachable!()` only when genuinely impossible — and you must prove it in a comment.
- **No `TODO`, `FIXME`, `HACK`, or `TEMPORARY` comments.** If you see one, fix it now or create an issue. Do not add new ones.
- **No commented-out code.** Git remembers. Delete it.
- **No dead code.** Remove unused imports, functions, variables, types. The compiler (`cargo check`, `swift build`, `kotlinc`) must produce zero warnings.
- **No magic numbers.** Named constants only.
- **No functions longer than 40 lines.** Extract.
- **No files longer than 300 lines.** Split.
- **No more than 3 levels of nesting.** Extract or early-return.
- **No more than 4 parameters per function.** Use a struct/builder.
- **Every public API has a doc comment** explaining what it does, what it returns, what errors it can produce, and at least one example.
- **Every error message is actionable.** "Index out of bounds" is useless. "List has 5 elements but index 99 was requested in `ForEach` at counter.flux:12" is useful.
- **Every `unsafe` block, `@unchecked`, or `@Suppress` has a safety comment** explaining why it's sound.

### 1.3 Dependency Policy

**Always use the latest stable version of every dependency.** No exceptions.

- **Rust:** `cargo update` before starting. Check `https://crates.io` for the latest version. Pin with `^MAJOR` (e.g., `"^1"`). Never pin to a specific patch version (`"=1.5.3"`) unless there's a security advisory.
- **Swift:** Use Swift 5.10+. Use Swift Package Manager. Pin to latest major in `Package.swift`.
- **Kotlin:** Use Kotlin 2.0+. Use version catalogs (`libs.versions.toml`). Pin to latest stable.
- **Android:** Use Compose BOM latest. `compileSdk` and `targetSdk` at latest stable API level.
- **iOS:** Deployment target iOS 16.0. Use latest Xcode (15.4+).

Before adding any new dependency:
1. Is there a way to do this with the standard library or existing deps? If yes, do that.
2. Is the crate/package actively maintained? Last commit < 6 months ago?
3. Does it have > 1000 stars or is it recommended by an authoritative source?
4. Run `cargo audit` / check advisories.

**Approved dependencies (already in Cargo.toml — do not remove or replace without ADR):**

| Language | Dependency | Purpose |
|---|---|---|
| Rust | `pest` / `pest_derive` | PEG parser |
| Rust | `tokio` | Async runtime |
| Rust | `tokio-tungstenite` | WebSocket server |
| Rust | `notify` | File watching |
| Rust | `axum` | HTTP asset server |
| Rust | `rmp-serde` | MessagePack serialization |
| Rust | `blake3` | Content addressing |
| Rust | `clap` | CLI |
| Rust | `tracing` / `tracing-subscriber` | Structured logging |
| Rust | `serde` / `serde_derive` | Serialization |
| Rust | `thiserror` | Error types |
| Rust | `anyhow` | Application errors (CLI only) |
| Rust | `criterion` | Benchmarking |
| Rust | `insta` | Snapshot testing |
| Rust | `proptest` | Property-based testing |
| Rust | `cargo-nextest` | Test runner (tool, not a crate dependency) |
| Swift | `Foundation` / `UIKit` / `SwiftUI` | Platform |
| Swift | `XCTest` | Testing |
| Kotlin | `androidx.compose.*` | UI |
| Kotlin | `okhttp3` | WebSocket |
| Kotlin | `org.junit.jupiter:junit-jupiter` | Testing |
| Kotlin | `org.jetbrains.kotlinx:kotlinx-coroutines` | Async |
| Kotlin | `io.mockk:mockk` | Mocking |
| Kotlin | `app.cash.turbine:turbine` | Flow testing |

If you need a dependency not on this list, you must get approval via an ADR in `/docs/adr/`.

---

## 2. Language-Specific Standards

### 2.1 Rust

**You write Rust like a person who has been doing it since 2015 and reads every RFC.**

#### Formatting & Linting
- `cargo fmt` must produce zero changes.
- `cargo clippy -- -D warnings` must pass. Treat every clippy warning as an error.
- `#![forbid(unsafe_code)]` at the top of every crate's `lib.rs`. If you need `unsafe`, you need an ADR.
- `#![warn(missing_docs, missing_debug_implementations, rust_2018_idioms, unreachable_pub)]`
- `#![deny(warnings)]` in CI configuration (not in source — allows local development).

#### Error Handling
- Use `thiserror::Error` for error types in library crates.
- Use `anyhow::Result` in the CLI binary crate only.
- Never `unwrap()`, `expect()`, or `panic!()` in production code. Use `?`.
- Error messages must include context: `format!("expected {expected}, got {actual} at {span}")`.
- Errors must carry source spans for reporting: every error type has a `span: Span` field.
- Use `Result<T, E>` in return types, not `Option<T>` when failure is an error, not absence.

#### Type Design
- Prefer `&str` over `String` for function parameters.
- Prefer `&[T]` over `Vec<T>` for function parameters.
- Prefer `impl Iterator<Item = T>` over `Vec<T>` for return types where the consumer iterates once.
- Use `Cow<'_, str>` for strings that may be borrowed or owned.
- Use `Arc<T>` only when sharing across threads. Use `Rc<T>` for single-threaded sharing.
- Use `parking_lot::Mutex` / `parking_lot::RwLock` over `std::sync` (no poisoning, faster).
- Use `ahash::AHashMap` for hot-path hash maps (faster than SipHash for non-cryptographic use).
- Use `SmallVec<[T; N]>` for vectors that are usually small (avoid heap allocation).
- Prefer `enum` over `bool` flags. `enum Direction { Left, Right }` not `is_left: bool`.
- Use `#[non_exhaustive]` on public enums to allow future variants.
- Use `#[must_use]` on functions returning `Result` or important values.

#### Memory & Performance
- Arena-allocate data that has the same lifetime (see `flux-ir/src/arena.rs`).
- Use `bytes::Bytes` for zero-copy buffer management in the wire protocol.
- Use `&'static [u8]` for lookup tables, not `Vec<u8>`.
- Avoid allocations in hot paths (parser, differ, serializer). Pre-allocate.
- Use `Vec::with_capacity(n)` when size is known or estimable.
- Use `String::with_capacity(n)` when building strings.
- Avoid `format!()` in hot paths. Use `write!(buf, ...)` into a pre-allocated `String`.

#### Async
- Use `tokio` as the async runtime.
- Use `tokio::select!` for concurrent operations, not `spawn` + channel when one will do.
- Use `tokio::sync::mpsc` for channels (not `crossbeam` — different runtime semantics).
- `async fn` in traits is fine (Rust 1.75+).
- The workspace is **edition 2024** with `resolver = "3"`; `rust-version` is
  1.85 (the edition-2024 floor). Local toolchain is rustc 1.94.1. Write
  edition-2024 Rust — do not write edition-2021-compatible code.
- Avoid `Box::pin` unless you have a recursive async type — use an iterative approach instead.

#### Testing
- Unit tests: `#[cfg(test)] mod tests { ... }` in the same file.
- Integration tests: `tests/` directory.
- Property tests: `proptest` for invariant checking.
- Snapshot tests: `insta` for codegen output.
- Benchmarks: `criterion` in `benches/` directory.
- **Always run tests with `cargo nextest run`, never `cargo test`.** Nextest is
  the project's test runner: it isolates each test in its own process, so a
  panicking or aborting test cannot take the rest of the suite with it, and it
  reports failures in a stable, parseable form. The one exception is doctests,
  which nextest does not support — run those with `cargo test --doc`.
- Useful nextest invocations:
  - `cargo nextest run` — the whole workspace.
  - `cargo nextest run -p flux-parser` — one crate.
  - `cargo nextest run -E 'test(/^test_span_/)'` — filter by expression.
  - `cargo nextest run --no-capture` — see `println!`/`tracing` output.
  - `cargo nextest list` — enumerate tests without running them.
- Nextest exits non-zero on any failure, so never pipe it through `tail`/`head`
  (that masks the exit code). Run it bare.
- Every public function has at least one test.
- Test error paths, not just happy paths.
- Test edge cases: empty input, maximum size, boundary values, Unicode.

#### Documentation
- `///` doc comments on every public item.
- Include `# Examples` section with a runnable example for non-trivial functions.
- Use `# Panics`, `# Errors`, `# Safety` sections where applicable.
- `cargo doc --open` must produce clean documentation with zero warnings.

#### Module Structure
- One responsibility per module. If a module has "and" in its description, split it.
- `mod.rs` is forbidden. Use `module_name.rs` + `module_name/` directory.
- Re-export public API at crate root: `pub use module::*;`.
- Hide implementation details: use `pub(crate)` for internal sharing.

### 2.2 Swift

**You write Swift like someone who has been shipping iOS apps since 2008 and reads every Swift Evolution proposal.**

#### Formatting & Linting
- Use `swift-format` with the project's `.swift-format` config.
- Use SwiftLint with the project's `.swiftlint.yml`. Zero warnings.
- Use Swift 5.10 features. Do not write Swift 4-compatible code.

#### Type Design
- Prefer `struct` over `class` for value types.
- Use `final class` for all classes (enables devirtualization).
- Use `actor` for thread-safe mutable state.
- Use `@MainActor` on all UI-touching code.
- Use `weak` / `unowned` to break retain cycles. Document every `weak` with why.
- Prefer `let` over `var`.
- Use `enum` with associated values for results, not tuples.
- Use `Codable` synthesis for serialization.
- Use `@Observable` (iOS 17+) for observable state. Fallback to `ObservableObject` for iOS 16.

#### Error Handling
- Never use `try!` or `try?` in production code. Use `do { try ... } catch { ... }`.
- Custom error types conform to `LocalizedError` with `errorDescription`.
- Errors carry source spans for reporting.
- Top-level `catch` in `FluxExecutor.dispatch()` — never let a VM error escape.

#### Memory
- Use `autoreleasepool` for tight loops creating temporary `NSObject`s.
- Use `unowned(unsafe)` only with a documented safety proof.
- Use `withUnsafeBytes` / `withUnsafeMutableBytes` for zero-copy binary deserialization.
- Avoid `AnyObject` — use protocols.

#### Testing
- Use `XCTest`.
- `setUp()` and `tearDown()` for test fixtures.
- Use `XCTAssertEqual` with custom `Equatable` conformances, not `XCTAssertTrue(a == b)`.
- Use `measure { }` blocks for performance tests.
- Test on both simulator and device (when available).

#### Concurrency
- `DispatchQueue.global(qos: .userInitiated).async` for VM evaluation.
- `DispatchQueue.main.async` for UI mutations.
- `CADisplayLink` for frame synchronization.
- Never call `DispatchQueue.main.sync` from the main thread (deadlock).

#### Documentation
- `///` doc comments on every public type and function.
- Use `// MARK: -` to organize sections within a file.
- Use `// MARK: Lifecycle`, `// MARK: Public`, `// MARK: Private`.

### 2.3 Kotlin

**You write Kotlin like someone who has been doing Android since 2010 and reads every Kotlin blog post from JetBrains.**

#### Formatting & Linting
- Use `ktlint` with default rules. Zero violations.
- Use Kotlin 2.0+ features. Do not write Java-compatible Kotlin unless explicitly interfacing with Java.

#### Type Design
- Prefer `val` over `var`.
- Use `data class` for value types.
- Use `sealed class` / `sealed interface` for restricted hierarchies.
- Use `value class` for inline wrappers (e.g., `@JvmInline value class NodeId(val value: UInt)`).
- Use `enum class` for fixed sets of values.
- Prefer `internal` for module-private declarations.
- Use `@VisibleForTesting` annotation for test-only access from internal.

#### Coroutines
- Use `kotlinx.coroutines`.
- `suspend` functions for async operations.
- `Flow` for reactive streams (cold). `StateFlow` for hot state.
- `Dispatchers.Default` for VM evaluation.
- `Dispatchers.Main` for UI mutations.
- `withContext(Dispatchers.Main) { }` to switch.
- Never use `runBlocking` in production code (tests only).
- Use `supervisorScope` for fault-tolerant parallel operations.

#### Error Handling
- Use custom exception types extending `Exception` with `val span: Span`.
- Never catch `Exception` or `Throwable` generically. Catch specific types.
- Use `Result<T>` for operations that can fail gracefully.
- Top-level `try/catch` in `FluxExecutor.dispatch()` — never let a VM error escape.

#### Testing
- Use JUnit 5 (`org.junit.jupiter`).
- Use `MockK` for mocking (`mockk<T>()`, `every { }`, `verify { }`).
- Use `Turbine` for Flow testing.
- Use `@ParameterizedTest` for table-driven tests.
- Use `kotlinx.coroutines.test` for coroutine testing (`runTest`).
- Test on emulator (Pixel 5, API 34).

#### Compose
- Use `@Composable` functions. No `Composable` classes.
- Use `remember { mutableStateOf(...) }` for state.
- Use `derivedStateOf` for computed state.
- Use `Modifier` chain for styling.
- Use `LaunchedEffect` for side effects.
- Use `DisposableEffect` for cleanup.

#### Documentation
- KDoc (`/** ... */`) on every public declaration.
- Use `@param`, `@return`, `@throws` tags.
- Use `// region` / `// endregion` for code folding.

---

## 3. Flux-Specific Conventions

### 3.1 File Ownership (Boundary Contract)

**You are assigned a specific directory. You may only create and modify files within your assigned directory.**

- You may READ any file in the repository.
- You may NOT WRITE to any file outside your assigned directory.
- You may NOT modify ANY `Cargo.toml`, `build.gradle`, `settings.gradle`, `Package.swift`, or `project.yml` file. Dependencies are pre-wired.
- If you need a type that doesn't exist in `flux-syntax`, do NOT add it yourself. Create an issue describing the missing type and the orchestrator will add it in a dedicated `flux-syntax` update pass.

### 3.2 IR Node IDs

Node IDs are `u32` hashes derived from `(parent_id, node_kind, source_span, optional_key)`. They are **stable across edits** where source structure doesn't change. This is load-bearing for state preservation and hot-swap.

**Never** assign sequential IDs. **Always** use `flux_ir::compute_node_id()`. If you're in a crate that doesn't have access to `flux-ir`, use the formula directly: `blake3::hash(&(parent_id, kind, span, key))`.

### 3.3 Wire Protocol

The wire protocol is defined in Appendix D of the spec. Every field, byte offset, and encoding rule is normative. **Do not deviate.** If you need to add a field, you need an ADR and a protocol version bump.

### 3.4 VM Instruction Set

The VM instruction set is defined in Appendix E. **Do not add opcodes** without an ADR. The ISA is intentionally minimal — monomorphization means we have type-specific operations (`ADD_I64`, `ADD_F64`) rather than generic `ADD` with tag dispatch.

### 3.5 Adapter Contract

Adapters are defined in Appendix F. Each adapter has:
- A dev implementation (imperative, drives `UIView` / `View` directly).
- A release implementation (declarative, `@Composable` / `View`).

Both consume the same props. The **props are the contract**. If you add a prop to an adapter, you must:
1. Update the adapter contract in Appendix F.
2. Implement it in both dev and release.
3. Write a parity test.

### 3.6 Performance Budgets

| Operation | Budget | How to verify |
|---|---|---|
| Parse 500-line file | < 5 ms | `cargo bench` |
| Type check 500-line file | < 3 ms | `cargo bench` |
| Diff 50-node tree | < 1 ms | `cargo bench` |
| Serialize 50-node patch | < 1 ms | `cargo bench` |
| VM eval 50-instruction handler | < 2 ms | `XCTest measure` / JUnit benchmark |
| Signal propagation (10 dirty cells) | < 1 ms | same |
| Native view mutation (single update) | < 3 ms | same |
| Save → pixels (end-to-end) | < 100 ms | integration benchmark |

If your code exceeds these budgets, profile it (`cargo bench -- --profile-time=5` / Instruments / Android Profiler) and optimize before submitting.

### 3.7 Error Messages

Every error message must include:
1. **What** went wrong (not just "type error" — "expected `Int`, got `String`").
2. **Where** it went wrong (file:line:col from span).
3. **Why** it might have gone wrong (hint: "`count` was previously inferred as `Int` from usage at line 18").
4. **How** to fix it (if actionable: "consider changing the type annotation to `String`").

Format (Rust-style):
```
error: type mismatch in `Counter`
  --> src/components/counter.flux:12:7
   |
12 |   state count: String = 0
   |                ^^^^^^  expected Int, got String
   |
   = hint: state `count` was previously inferred as Int from usage at line 18
```

### 3.8 Logging

Use `tracing` (Rust), `os_log` (Swift), `android.util.Log` (Kotlin).

Levels:
- `ERROR` — something is broken. User action required.
- `WARN` — something is suspicious. No action required but investigate.
- `INFO` — normal operation (dev server started, host connected).
- `DEBUG` — IR diffs, patch contents. Behind `--log-level=debug`.
- `TRACE` — every VM instruction. Behind `--log-level=trace`.

Never log at `INFO` in a hot path. Never log at `DEBUG` in a hot path without a `--log-level` check.

---

## 4. Git Conventions

### 4.1 Commit Messages

Format:
```
<type>(<scope>): <subject>

<body>

<footer>
```

Types: `feat`, `fix`, `refactor`, `test`, `docs`, `chore`, `perf`, `ci`
Scopes: `parser`, `types`, `ir`, `serde`, `differ`, `devserver`, `codegen-swift`, `codegen-kotlin`, `cli`, `ios`, `android`, `stdlib`, `tests`

Example:
```
feat(parser): handle generic component with trait bounds

Add support for `component Counter[T: Numeric]` syntax.
The pest grammar now correctly parses `[T: Numeric]` as a
generic parameter with a trait bound.

Refs: FLUX-002
```

### 4.2 Branching

Branch name: `<issue-id>/<kebab-description>`

Example: `FLUX-002/parser-grammar-and-ast`

### 4.3 Pull Requests

- One PR per issue. No mixing.
- PR title = commit message subject.
- PR description includes:
  - What changed and why.
  - Test results (paste `cargo nextest run` / `xcodebuild test` / `./gradlew test` output).
  - Performance results if applicable (paste `cargo bench` output).
  - Breaking changes (if any).

---

## 5. Reviewer Checklist (Self-Review Before Committing)

Before you `git commit`, verify:

### Rust
- [ ] `cargo fmt -- --check` passes (zero changes).
- [ ] `cargo clippy -- -D warnings` passes (zero warnings).
- [ ] `cargo nextest run` passes (all tests green). Never use `cargo test`.
- [ ] `cargo test --doc` passes (doctests; nextest does not run these).
- [ ] `cargo doc` passes (zero warnings).
- [ ] `cargo bench` is within performance budget.
- [ ] No `unwrap()`, `expect()`, `panic!()` in non-test code.
- [ ] No `TODO`, `FIXME`, commented-out code.
- [ ] Every public item has a `///` doc comment.
- [ ] Error messages include what, where, why, how.
- [ ] No files > 300 lines.
- [ ] No functions > 40 lines.
- [ ] No nesting > 3 levels.
- [ ] Tests cover edge cases (empty, max, boundary, Unicode).

### Swift
- [ ] `swift-format lint` passes.
- [ ] `swiftlint` passes (zero warnings).
- [ ] `xcodebuild test` passes.
- [ ] No `try!`, `try?`, force-unwraps in non-test code.
- [ ] No `TODO`, `FIXME`, commented-out code.
- [ ] Every public type/function has `///` doc comment.
- [ ] No retain cycles (check every `closure` capture).
- [ ] `@MainActor` on all UI code.
- [ ] Background queue for VM, main for UI.

### Kotlin
- [ ] `./gradlew ktlintCheck` passes.
- [ ] `./gradlew test` passes.
- [ ] No `!!` in non-test code.
- [ ] No `TODO`, `FIXME`, commented-out code.
- [ ] Every public declaration has KDoc.
- [ ] No `runBlocking` in non-test code.
- [ ] `Dispatchers.Default` for VM, `Dispatchers.Main` for UI.
- [ ] Tests use `runTest`, not `runBlocking`.

---

## 6. Architecture Decision Records

When you encounter a decision point not covered by the spec:
1. **Do not improvise.** Follow the spec.
2. If the spec is genuinely silent, write an ADR in `/docs/adr/` using the MADR template from Appendix A.
3. ADRs are append-only. Never delete. Supersede with a new ADR linking to the old one.
4. ADRs must reference the ASR they address.

---

## 7. Testing Strategy Summary

| Layer | Tool | What to test |
|---|---|---|
| Unit | `cargo nextest run` / `XCTest` / JUnit 5 | Every public function, edge cases, error paths |
| Property | `proptest` | Invariants (node ID stability, diff minimality, round-trip serialization) |
| Snapshot | `insta` | Codegen output (Swift and Kotlin generated code) |
| Benchmark | `criterion` / `XCTMeasure` / JMH | Performance budgets (see §3.7) |
| Integration | `tests/` | Parser → type checker → lowering → diff → serialize round-trip |
| Parity | `tests/parity/` | Dev VM execution == release codegen execution |

---

## 8. Common Mistakes To Avoid

1. **Do not add `unsafe` to Rust code.** If you think you need it, you almost certainly don't. File an ADR if you genuinely do.

2. **Do not use `Any` in Swift or `Any` in Kotlin.** Use typed protocols/interfaces. `Any` erases type safety and forces runtime checks.

3. **Do not allocate in hot paths.** The differ runs on every file save. The serializer runs on every patch. The VM runs on every tap. No `format!()`, no `Vec::new()` without capacity, no `String` where `&str` suffices.

4. **Do not use `DispatchQueue.main.sync` from the main thread.** This deadlocks. Use `DispatchQueue.main.async`.

5. **Do not catch generic `Exception` in Kotlin.** Catch specific types. `catch (e: Exception)` hides bugs.

6. **Do not use `lazy` properties for values that are always accessed.** `lazy` adds overhead. Use `let` with direct initialization.

7. **Do not use `println!` / `print()` / `NSLog` / `Log.d` in production code.** Use the structured logging facilities (`tracing` / `os_log` / `android.util.Log`).

8. **Do not modify `Cargo.toml`, `build.gradle`, or `Package.swift`.** Dependencies are pre-wired by the foundation agent. If you need a new dependency, file an ADR.

9. **Do not skip the failing test.** If a test is hard to write, the API is probably wrong. Fix the API, not the test.

10. **Do not optimize prematurely.** Write clear code first. Benchmark. Optimize only what the benchmark says is slow. Every optimization must be justified by a benchmark.

---

## 9. Quick Reference

### Project Structure
```
flux/
├── crates/                       # Rust workspace crates
├── runtimes/                     # iOS (Swift) and Android (Kotlin) host apps
├── adapters/                     # Platform adapter implementations
├── stdlib/                       # .flux standard library source
├── docs/
│   ├── spec/
│   │   ├── mlp-spec.md           # The specification
│   │   └── mlp-appendices.md     # Appendices A–G
│   ├── agents-boundaries-contract.md  # Ownership map + issue plan
│   └── adr/                      # ADRs (created on first ADR)
├── tests/                        # Integration and parity tests
├── Cargo.toml                    # Workspace root (DO NOT MODIFY)
├── rust-toolchain.toml           # Pinned toolchain (DO NOT MODIFY)
├── CHANGELOG.md                  # Progress log and spec deviations
└── AGENTS.md                     # This file
```

### Key Files To Read First
- `/docs/spec/mlp-spec.md` — the full specification (vision, BRS, SRS, architecture, verification)
- `/docs/spec/mlp-appendices.md` — Appendices A–G: ADRs, grammar, IR schema, wire protocol, VM ISA, adapter contracts, glossary
- `/docs/agents-boundaries-contract.md` — directory ownership map, modification rules, and the FLUX-001…FLUX-016 issue plan
- `/CHANGELOG.md` — what is already built, and every recorded deviation from the spec
- `crates/flux-syntax/src/lib.rs` — the shared type vocabulary (re-exports; the definitions live in the sibling modules `ids`, `strings`, `value`, `ty`, `node`, `patch`)
- `/docs/adr/` — past decisions and their rationale (created on first ADR; Appendix A of the appendices holds ADR-0001…ADR-0013)

### Commands
```bash
# Rust
cargo check                    # Type check all crates
cargo nextest run              # Run all tests (ALWAYS use nextest, never `cargo test`)
cargo test --doc               # Doctests only (nextest does not run doctests)
cargo bench                    # Run benchmarks
cargo clippy -- -D warnings   # Lint
cargo doc --open               # Generate docs
cargo update                   # Update dependencies to latest

# Swift
xcodebuild -project runtimes/ios/FluxApp.xcodeproj -scheme FluxApp build
xcodebuild -project runtimes/ios/FluxApp.xcodeproj -scheme FluxApp test

# Kotlin
./gradlew :runtimes:android:build
./gradlew :runtimes:android:test
```

### Spec References
- Grammar: Appendix B
- IR Schema: Appendix C
- Wire Protocol: Appendix D
- VM ISA: Appendix E
- Adapter Contracts: Appendix F
- Glossary: Appendix G

---

**This file is the law of the land. If something in the spec contradicts this file, the spec wins. If something in a code review contradicts this file, this file wins. If you're unsure, ask — do not guess.**
