<div align="center">
  <img src="docs/logo.png" alt="Flux Logo" width="200"/>
  <p>
    <strong>Write once, render native.</strong><br/>
    A write-once UI language for native iOS &amp; Android. Edit <code>.flux</code> source and watch it
    hot-reload on-device in milliseconds as binary patches over WebSocket — then compile the exact
    same code to idiomatic <strong>SwiftUI</strong> and <strong>Jetpack Compose</strong> for release.
    No webview. No JS bridge. No interpreter in production.
  </p>
  <p>
    <img src="https://img.shields.io/badge/Rust-nightly%20%7C%20edition%202024-000000?style=flat-square&logo=rust" alt="Rust"/>
    <img src="https://img.shields.io/badge/License-Apache--2.0-blue?style=flat-square" alt="License"/>
    <img src="https://img.shields.io/badge/Platforms-iOS%2016%2B%20%7C%20Android-9D4EDD?style=flat-square" alt="Platforms"/>
    <img src="https://img.shields.io/badge/Codegen-SwiftUI%20%2B%20Compose-007ACC?style=flat-square" alt="Codegen"/>
    <img src="https://img.shields.io/badge/Hot%20Reload-Binary%20Patches-FF4500?style=flat-square" alt="Hot Reload"/>
    <img src="https://img.shields.io/badge/Crates-16-6F4E37?style=flat-square" alt="Crates"/>
    <img src="https://img.shields.io/badge/Code-99k%20LOC-9CF?style=flat-square" alt="Lines of code"/>
    <img src="https://img.shields.io/badge/Stdlib-28%20Components-228B22?style=flat-square" alt="Stdlib"/>
    <img src="https://img.shields.io/badge/ISA%20Vectors-92%20Golden-00BFFF?style=flat-square" alt="ISA vectors"/>
    <img src="https://img.shields.io/badge/ADRs-39%20MADR-blueviolet?style=flat-square" alt="ADRs"/>
    <img src="https://img.shields.io/badge/Testing-nextest%20%2B%20proptest%20%2B%20insta-brightgreen?style=flat-square" alt="Testing"/>
    <br/>
    <img src="https://github.com/elcoosp/flux/actions/workflows/rust-check.yml/badge.svg?style=flat-square" alt="Rust CI"/>
    <img src="https://github.com/elcoosp/flux/actions/workflows/ios-check.yml/badge.svg?style=flat-square" alt="iOS CI"/>
    <img src="https://github.com/elcoosp/flux/actions/workflows/android-check.yml/badge.svg?style=flat-square" alt="Android CI"/>
  </p>
</div>

---

# Flux

> [!NOTE]
> Flux is under active development. The Rust workspace — 16 crates, ~63k lines — is end-to-end
> functional: parse → typecheck → lower → diff → patch → VM, plus native codegen for both
> platforms. On Android, dev renders through the same declarative components the release
> codegen emits; the iOS dev tier is still imperative (UIKit) and convergence is gated on
> measurement (ADR-0048).

---

## Table of Contents

- [Why Flux](#why-flux)
- [Features](#features)
- [Architecture](#architecture)
- [Getting Started](#getting-started)
- [Usage](#usage)
- [Language Tour](#language-tour)
- [Development](#development)
- [Testing](#testing)
- [Continuous Integration](#continuous-integration)
- [Documentation](#documentation)
- [Acknowledgements](#acknowledgements)

---

## Why Flux

| | Traditional cross-platform | Flux |
|---|---|---|
| Render target | WebView / canvas | Real `UIView` / Compose `View` |
| Hot reload | Full rebuild or JS eval | Binary IR patch over WebSocket |
| Release build | Interpreter shipped in prod | SwiftUI / Compose codegen |
| State | Framework-locked | SolidJS-style signal graph over one shared IR |

The dev loop and the release artifact come from the **same Reactive Tree IR**. You iterate
against a fast bytecode VM host during development and ship platform-native code in release —
no fork in the road, no "dev looks different from prod." A parity harness
(`flux-parity`) continuously proves that dev-VM execution and release codegen stay equivalent,
so the two paths are verified, not assumed.

---

## Features

### Language

- **`compo` components** with typed props, `record` data types, local `state` signals, closures
  (`|| { ... }`, `fn(v) { ... }`), and `ForEach` with key functions.
- **SolidJS-style reactivity** — assignments to state signals propagate through a fine-grained
  dependency graph; `$binding` two-way bindings for text inputs.
- **Navigation** built in: `Router` / `Screen` / `RouterNav` with named routes.
- **28-component standard library** — `Text`, `Button`, `Column`, `Row`, `Stack`, `Grid`,
  `ScrollView`, `TextField`, `TextInput`, `Toggle`, `Switch`, `Slider`, `Picker`,
  `DatePicker`, `Image`, `Modal`, `Sheet`, `Dialog`, `SafeArea`, `Spacer`, and more — plus
  `prelude`, `traits`, and capability modules.
- **Platform capabilities** for storage, network, and device services through a versioned
  capability contract, resolved identically on server and both hosts.

### Pipeline

- **Hand-written lexer + recursive-descent parser** with error recovery; every error carries a
  source `Span`.
- **Type checker** (`flux-types`) over the parse tree.
- **Reactive Tree IR** (`flux-ir`) — arena-allocated, stable `u32` node IDs, a `ClosureIR`
  bytecode table, and an `InstanceRegistry` that lets hosts preserve component state across
  hot swaps.
- **Structural differ** (`flux-differ`) → minimal edit scripts, serialized as MessagePack
  binary patches (`flux-ir-serde`) with blake3 content-addressed interning.
- **Register-based bytecode VM** — the `flux-vm-ref` crate is the dependency-light behavioral
  oracle for the ISA; the shipping Swift (`FluxBytecodeVM`) and Kotlin VMs are validated
  against **92 golden ISA vectors** under `tests/isa-vectors/`.
- **Capability calls** (`CALL_CAP`) return result-cell signals (`Ready` / `Pending` / `Error`);
  async capabilities settle via an injected `AsyncResolver` (ADR-0044/0045).
- **Native codegen** — a data-driven shared emitter (`flux-codegen-core` with a `Backend`
  trait and primitive registry) drives the SwiftUI and Jetpack Compose backends, with a
  node-ID bridge so generated views keep the exact identity the VM used in dev.

### Tooling

- **`flux` CLI** — scaffold, dev server, native builds, formatter, diagnostics emitter,
  stdlib schema, environment doctor.
- **Hot-reload dev server** (`flux-devserver`) — file watching, lower + diff on every save,
  WebSocket patch channel on `:7331`, HTTP asset server on `:7332`, optional LAN pairing
  tokens.
- **Language Server** (`flux-lsp`, built on `async-lsp`) plus a **VS Code extension** with
  syntax highlighting, diagnostics, hot-reload status, and "Run on device".
- **DevTools desktop app** (`flux-devtools-ui`) — a gpui-based GUI rendering live telemetry
  over the same wire protocol (ADR-0041).
- **Parity harness** (`flux-parity`) — trace capture, reduction, and equivalence checking
  between dev-VM execution and release codegen output.
- **Perf harness** (`flux-perf-harness`) — render-performance measurement against the
  project's latency budgets.

---

## Architecture

```
 main.flux
     │
     ▼
┌─────────────┐   ┌─────────────┐   ┌──────────────────────┐
│ flux-parser │──▶│ flux-types  │──▶│       flux-ir        │
│ (lex/parse) │   │ (typecheck) │   │ (Reactive Tree IR)   │
└─────────────┘   └─────────────┘   └──────────┬───────────┘
                                               │
                    ┌──────────────────────────┴──────────────┐
                    ▼ dev                                     ▼ release
        ┌──────────────┐   ┌───────────────┐      ┌─────────────────────────┐
        │ flux-differ  │──▶│ flux-ir-serde │      │    flux-codegen-core    │
        │ (tree diff)  │   │ (MsgPack patch│      │ (data-driven emitter)   │
        └──────────────┘   └───────┬───────┘      └───────────┬─────────────┘
                                   │ WebSocket :7331           │
                                   ▼                           ├─▶ flux-codegen-swift  → SwiftUI
                        ┌──────────────────┐                   └─▶ flux-codegen-kotlin → Compose
                        │  Host app VM +   │
                        │  signal graph +  │   ◀── validated against
                        │  shadow tree     │        92 golden ISA vectors
                        │ (Swift / Kotlin) │        (tests/isa-vectors/)
                        └──────────────────┘
```

### Node identity

Node IDs are stable, content-derived FNV-1a-32 hashes — never sequential. Two tag families keep
expression and declaration IDs disjoint, and the codegen backends thread the exact same family
tags through their node-ID bridges. IDs are **stable across edits where structure doesn't
change**, which is what lets a hot swap preserve component state.

### Prop indexing

Prop indices are FNV-1a-32 of the prop *name* masked to `u16`, derived identically by the host
kits (`PropsIndex.propIndexForName` on Android). Indices are **never hardcoded** — a hardcoded
index desyncs from the server and renders silently blank UI.

### The reactive core and the shadow tree

- The **shadow tree** is not a tree of raw native views. Each `ShadowNode` holds its
  materialized props in a platform observable: on Android, props live in a Compose
  `MutableState` (injected via `propsStateFactory`, because a plain `var` is invisible to
  snapshot tracking); on iOS the tree is observed natively.
- **Keyed reconciliation** matches children by `nodeId` — an existing node is reordered, never
  recreated — preserving scroll position, text field contents, and screen state across diffs
  and router push/pop.
- Three contracts are normative and versioned: the **wire protocol** (Appendix D), the **VM
  ISA** (Appendix E), and the **adapter contracts** (Appendix F) in
  `docs/spec/mlp-appendices.md`.

---

## Getting Started

### Prerequisites

- **Rust** — the toolchain is pinned to an exact nightly (`nightly-2026-08-28`) via
  `rust-toolchain.toml`; `rustup` will provision it automatically on your first build.
  A pinned nightly is required because the gpui-based DevTools crate uses unstable std
  features; the declared `rust-version = "1.86"` is the edition-2024 language floor.
- **cargo-nextest** — the project's test runner.
- **Xcode** (iOS 16+ deployment target) and/or the **Android SDK** — only for the host apps.

> [!TIP]
> Everything on the Rust side — dev server, parser, differ, VM oracle, codegen, parity harness,
> benchmarks — builds and runs without Xcode or the Android SDK.

### Build & test the workspace

```bash
git clone https://github.com/elcoosp/flux
cd flux

cargo build --release          # the CLI lands in target/release/flux

# Format + lint must be clean
cargo fmt -- --check
cargo clippy -- -D warnings

# Full test suite (nextest, never plain `cargo test`)
cargo nextest run
cargo test --doc               # doctests (nextest does not run these)
```

### Run the todo example

```bash
cargo build -p flux-cli
flux dev --root ./examples/todo
```

Open `runtimes/ios` in Xcode or build the Android host with
`./gradlew :runtimes:android:build`, point it at `ws://<host>:7331`, and edit
`examples/todo/main.flux` — patches land on-device in milliseconds.

---

## Usage

### CLI

| Command | Purpose |
|---|---|
| `flux init <name>` | Scaffold a new Flux project at `<name>/`. |
| `flux dev [--root] [--ws-host] [--ws-port] [--http-port] [--token]` | Start the hot-reload dev server (WS `:7331`, HTTP `:7332` by default). |
| `flux build ios\|android [--root]` | Codegen the project to `platforms/<platform>/Generated/`, then invoke the native toolchain when present. |
| `flux fmt [<files>...] [--check]` | Format `.flux` sources to canonical style; `--check` exits non-zero on drift (CI gating). |
| `flux lsp <file> [--types]` | Emit parse + type-check diagnostics as JSON. |
| `flux doc` | Emit a JSON schema of the stdlib API to stdout. |
| `flux doctor [--strict]` | Environment health check: toolchain, stdlib parse, wire protocol version, dependency drift; `--strict` exits non-zero on findings. |

### Dev server flags

```bash
flux dev --root ./my-app                       # serve a project (defaults to cwd)
flux dev --ws-host 0.0.0.0 --token <secret>    # expose on the LAN, require pairing
```

Hosts authenticate with the token read from `FLUX_DEV_TOKEN` or `[dev] token` in `flux.toml`
during the `Hello` handshake. Without `--token`, localhost keeps an open dev loop.

### Project manifest (`flux.toml`)

```toml
[project]
name = "todo"
entry = "main.flux"

[dev]
ws_port = 7331
http_port = 7332
```

---

## Language Tour

A real component from [`examples/todo/main.flux`](examples/todo/main.flux):

```flux
record Task { label: String, done: Bool, }

compo TaskRow(task: Task, tasks: List[Task])
    Row gap: 8.0
        Text text: task.label
        Button text: "Remove", onPress: || { tasks.remove(task) }

compo TodoApp
    state tasks: List[Task] = [
        Task(label: "Buy milk", done: false),
        Task(label: "Walk dog", done: false),
    ]
    state newTask: String = ""
    Router initialRouteName: "tasks"
        Screen route: "tasks"
            Column gap: 12.0
                Text text: "Flux To-Do"
                Row gap: 8.0
                    TextInput text: $newTask, onChangeText: fn(v) { newTask = v }
                    Button text: "Add", onPress: || {
                        tasks.append(Task(label: newTask, done: false))
                        newTask = ""
                    }
                Column gap: 8.0
                    ForEach(tasks, key: fn(t) { t.label }) { item => TaskRow(task: item, tasks: tasks) }
        Screen route: "about"
            Column gap: 12.0
                Button text: "Back", onPress: || { RouterNav.navigate("tasks") }
```

More examples live in [`examples/`](examples/): `counter` (minimal state + patch flow) and
`router` (multi-screen navigation).

---

## Development

### Workspace layout

```
flux/
├── crates/                       # Rust workspace (16 crates)
│   ├── flux-syntax/              # Spans, NodeId, FNV-1a node-id hashing, value/patch vocab
│   ├── flux-parser/              # Hand-written lexer + recursive-descent parser
│   ├── flux-types/               # Type checker
│   ├── flux-ir/                  # Reactive Tree IR: arena, ClosureIR, InstanceRegistry
│   ├── flux-ir-serde/            # Binary patch (de)serialization (MessagePack, blake3)
│   ├── flux-differ/              # Structural tree differ
│   ├── flux-vm-ref/              # Reference register VM (test oracle, not the shipping VM)
│   ├── flux-devserver/           # Hot-reload pipeline + WebSocket + HTTP asset server
│   ├── flux-codegen-core/        # Shared data-driven emitter (Backend trait, primitives)
│   ├── flux-codegen-swift/       # SwiftUI codegen (node-ID bridge)
│   ├── flux-codegen-kotlin/      # Jetpack Compose codegen (node-ID bridge)
│   ├── flux-cli/                 # The `flux` binary
│   ├── flux-parity/              # Dev VM == release codegen parity harness
│   ├── flux-lsp/                 # Language server (async-lsp)
│   ├── flux-devtools-ui/         # gpui DevTools desktop app
│   └── flux-perf-harness/        # Render-perf benchmark harness
├── runtimes/                     # Host apps: ios/ (Swift 6, XcodeGen) · android/ (Gradle)
├── adapters/                     # Platform adapter implementations (contract version 1)
├── stdlib/                       # 28 .flux stdlib components + prelude/traits/capabilities
├── examples/                     # counter · router · todo
├── editors/vscode/               # VS Code extension
├── tests/isa-vectors/            # 92 golden ISA vectors shared by Rust + Swift + Kotlin VMs
├── docs/spec/                    # mlp-spec.md + mlp-appendices.md (A–G)
├── docs/adr/                     # 39 Architecture Decision Records (MADR)
└── website/                      # Astro documentation site
```

### Conventions

- `#![forbid(unsafe_code)]` in every library crate; no `unwrap` / `expect` / `panic!` in
  production code; every error carries a source `Span`.
- `cargo fmt` and `cargo clippy -- -D warnings` must be clean before commit.
- Dependencies are vetted through ADRs — `flux doctor` reports unapproved drift.
- Release profile ships with `lto = "thin"` and `codegen-units = 1`.

---

## Testing

Every public function has a test. The suite spans five layers, run by `cargo nextest`:

| Layer | Tool | What it proves |
|---|---|---|
| Unit | `cargo nextest run` | Every function, edge cases, error paths |
| Property | `proptest` | Node-ID stability, diff minimality, round-trips |
| Snapshot | `insta` | Swift & Kotlin codegen output |
| Benchmark | `criterion` | Per-stage latency budgets |
| Parity | `tests/` | Dev VM execution == release codegen |

### Performance budgets

Enforced by `cargo bench` — the headline metric is the **Save → pixels** budget, the
wall-clock a developer feels during hot reload, measured end-to-end by the LANE-H harness
(a real `DevServer` plus a headless loopback WebSocket client, ≥ 50 edits per tree size,
p50/p99):

| Stage | Budget |
|---|---|
| Parse a 500-line file | < 5 ms |
| Diff a 50-node tree | < 1 ms |
| Serialize a 50-node patch | < 1 ms |
| VM eval per frame | < 2 ms |

---

## Continuous Integration

19 GitHub Actions workflows guard `main`:

| Workflow | Purpose |
|---|---|
| `rust-check.yml` | `cargo fmt` / `clippy` / `nextest` |
| `ios-check.yml` | Swift build + test of the iOS host |
| `android-check.yml` | Kotlin build + test of the Android host |
| `parity-check.yml` | Dev VM == release codegen equivalence |
| `codegen-compile.yml` | Generated Swift/Kotlin compiles |
| `stdlib-contract.yml` | Stdlib component contract checks |
| `wire-fuzz.yml` | Fuzzes the wire-protocol (de)serialization |
| `compat-matrix.yml` | Cross-version compatibility matrix |
| `mutation-testing.yml` | Mutation testing |
| `benchmarks.yml` | Criterion benchmark suite |
| `perf-harness.yml` | Render-perf suite |
| `size-gate.yml` | Artifact size budgets |
| `release-gate.yml` | Pre-release gate |
| `artifact-publish.yml` | Publishes release artifacts |
| `devtools-nightly-canary.yml` | Probes new nightlies against the pinned gpui |
| `vscode-check.yml` | Builds/checks the VS Code extension |
| `website-check.yml` | Builds/checks the docs site |
| `adr-numbering.yml` | Enforces ADR numbering discipline |
| `manifest-steward.yml` | Keeps the workspace manifest consistent |

---

## Documentation

- **Specification:** [`docs/spec/mlp-spec.md`](docs/spec/mlp-spec.md) and
  [`docs/spec/mlp-appendices.md`](docs/spec/mlp-appendices.md) — grammar, IR schema, wire
  protocol (Appendix D), VM ISA (Appendix E), adapter contracts (Appendix F), glossary.
- **Architecture decisions:** [`docs/adr/`](docs/adr/) — 39 MADR records (through ADR-0058).
- **Contributor manual:** [`AGENTS.md`](AGENTS.md) — workflow, quality bar, and house rules.
- **Docs site:** [`website/`](website/) — Astro source (`pnpm install && pnpm dev`).
- **Benchmarks:** [`benches/`](benches/) and [`crates/flux-perf-harness/`](crates/flux-perf-harness/).

---

## Acknowledgements

Flux builds on excellent open-source foundations:

- [Zed's gpui](https://github.com/zed-industries/zed) and
  [gpui-component](https://github.com/longbridge/gpui-component) — the DevTools desktop UI.
- [Tokio](https://tokio.rs), [axum](https://github.com/tokio-rs/axum), and
  [tokio-tungstenite](https://github.com/snapview/tokio-tungstenite) — async runtime, HTTP,
  and WebSocket layers.
- [clap](https://github.com/clap-rs/clap) — CLI surface.
- [notify](https://github.com/notify-rs/notify) — file watching.
- [blake3](https://github.com/BLAKE3-team/BLAKE3) and
  [rmp-serde](https://github.com/3Hren/msgpack-rust) — content addressing and MessagePack.
- [insta](https://insta.rs/), [proptest](https://github.com/proptest-rs/proptest), and
  [criterion](https://github.com/bheisler/criterion.rs) — snapshot, property, and benchmark
  tooling.
- [async-lsp](https://github.com/oxalica/async-lsp) and
  [lsp-types](https://github.com/gluon-lang/lsp-types) — language server foundation.

---

<p align="center">
  <em>Flux — write once, render native.</em>
</p>
