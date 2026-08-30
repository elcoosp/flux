# ⚡ Flux

> Write-once UI language for native iOS & Android. Edit in `.flux`, see it hot-reload on-device in milliseconds.

Flux is a write-once UI language for native mobile development. A Rust dev server
parses `.flux` source, lowers it to a **Reactive Tree IR**, diffs it against the
previous tree, and ships **binary patches** over WebSocket to a precompiled host
app. The host app runs an embedded register-based bytecode VM on a SolidJS-style
signal graph over a shadow tree of real native views. In release mode the same IR
is codegen'd to idiomatic **SwiftUI** and **Jetpack Compose**.

No webview. No JS bridge. No runtime interpreter tax in production — just native
views driven by a minimal VM and a reactive signal graph.

---

## Badges

![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg?style=for-the-badge)
![Rust](https://img.shields.io/badge/rust-nightly%20%7C%20edition%202024-orange.svg?style=for-the-badge)
![Crates](https://img.shields.io/badge/workspace-16%20crates-9cf?style=for-the-badge)
![Tests](https://img.shields.io/badge/runner-cargo%20nextest-brightgreen.svg?style=for-the-badge)
![Source](https://img.shields.io/badge/rust%20LOC-51k-9cf?style=for-the-badge)
![ADRs](https://img.shields.io/badge/ADRs-38%20%28MADR%29-blueviolet.svg?style=for-the-badge)
![Platforms](https://img.shields.io/badge/platforms-iOS%2016%2B%20%7C%20Android-purple.svg?style=for-the-badge)
![Rust CI](https://github.com/elcoosp/flux/actions/workflows/rust-check.yml/badge.svg?style=for-the-badge)
![iOS CI](https://github.com/elcoosp/flux/actions/workflows/ios-check.yml/badge.svg?style=for-the-badge)
![Android CI](https://github.com/elcoosp/flux/actions/workflows/android-check.yml/badge.svg?style=for-the-badge)

---

## Why Flux

| | Traditional cross-platform | Flux |
|---|---|---|
| Render target | WebView / canvas | Real `UIView` / `View` |
| Hot reload | Full rebuild or JS eval | Binary IR patch over WebSocket |
| Release build | Interpreter in production | SwiftUI / Compose codegen |
| State | Framework-locked | SolidJS-style signal graph, shared IR |

The dev loop and the release artifact come from the **same IR**. You iterate against
a fast VM host during development and ship platform-native code in release — no
fork in the road, no "dev looks different from prod."

---

## Architecture

### Pipeline overview

```
 .flux source
      │  flux-parser  (hand-written lexer + recursive-descent parser)
      ▼
  flux-syntax  ──►  flux-types  (type check)
      │                    │
      ▼                    ▼
  flux-ir   (Reactive Tree IR: arena-allocated, stable u32 node ids, codegen-ready)
      │
      ▼  flux-differ  (structural diff → minimal edit script)
  flux-ir-serde  (binary MessagePack patch; blake3 content-addressed interning)
      │
      ├──► dev:  flux-vm-ref (register VM, test oracle)  ──► host app (native shadow tree)
      └──► rel:  flux-codegen-core ─► flux-codegen-swift / flux-codegen-kotlin (native codegen)
```

The three normative, versioned contracts are the **wire protocol** (Appendix D of
`docs/spec/mlp-appendices.md`), the **VM ISA** (Appendix E), and the **adapter
contracts** (Appendix F). The `flux-devtools-ui` crate (a gpui desktop app,
ADR-0041) renders live DevTools telemetry over the same wire.

### Node identity

Node IDs are stable, content-derived FNV-1a-32 hashes (never sequential) computed
by `flux_syntax::compute_node_id`. Two tag families keep expression and
declaration IDs disjoint so the codegen node-ID bridge
(`flux-codegen-*/bridge.rs`) threads the exact same family tag. IDs are **stable
across edits where structure doesn't change**, which is what lets hot-swap
preserve component state.

### Prop indexing

Prop indices are FNV-1a-32 of the prop *name* masked to `u16`
(`flux_ir::lower::prop_index_for_name`). Host kits derive them identically
(`PropsIndex.propIndexForName` on Android); indices are **never hardcoded** —
a hardcoded index desyncs from the server and renders silently blank UI.

### The reactive core and the shadow tree

- **`flux-ir`** owns the in-memory lowered shape: the packed `IRArena`, the
  `ClosureIR` bytecode table, and the `InstanceRegistry` that lets the host
  preserve state across hot swaps (Appendix C §C.1, FLUX-004).
- **`flux-vm-ref`** is the *reference* VM — the behavioral oracle for the ISA in
  Appendix E. It decodes Appendix E bytecode and interprets it, and both the
  production runtimes (Swift `FluxBytecodeVM`, Kotlin `FluxBytecodeVM`) and
  `flux-vm-ref` itself are validated against the golden ISA vectors under
  `/tests/isa-vectors/`. It is intentionally dependency-light and is **not** the
  VM that ships in a host app.
- The **capability system** is the extension point (`CALL_CAP` in the ISA): a
  `CapabilityRegistry` maps `(capId, methodId) → impl`, threaded into the VM.
  `CALL_CAP` returns a **result-cell signal id** that is `Ready` / `Pending` /
  `Error` (ADR-0044); synchronous capabilities settle the cell before returning,
  async capabilities leave it `Pending` and an injected `AsyncResolver` settles
  it (ADR-0045). Capability ids are derived deterministically on server and both
  hosts.
- The **shadow tree** is not a tree of raw native views. Each `ShadowNode` holds
  its materialized props in a platform observable. On Android, props live in a
  Compose `MutableState` injected via `propsStateFactory` (a plain `var` is
  invisible to snapshot tracking); on iOS the tree is observed natively. Dev
  renders through the same declarative components the release codegen emits
  (§0.2 of `AGENTS.md`) — there is no separate imperative dev tier on Android;
  the iOS dev tier is still imperative (UIKit) and convergence is gated on
  measurement (ADR-0048).
- **Keyed reconciliation** matches children by `nodeId`; an existing node is
  reordered, never recreated, which preserves scroll position, text, and screen
  state across diffs and router push/pop.

---

## Monorepo layout

```
flux/
├── crates/                       # Rust workspace (16 crates)
│   ├── flux-syntax/              # Span, NodeId, FNV-1a node-id hashing, value/ty/node/patch vocab
│   ├── flux-parser/              # Hand-written lexer + recursive-descent parser
│   ├── flux-types/               # Type checker
│   ├── flux-ir/                  # Reactive Tree IR: arena, ClosureIR, InstanceRegistry, lowering
│   ├── flux-ir-serde/            # Binary patch (de)serialization (MessagePack, blake3 interning)
│   ├── flux-differ/              # Structural tree differ
│   ├── flux-vm-ref/              # Reference register-based VM (test oracle, not the shipping VM)
│   ├── flux-devserver/           # Hot-reload pipeline + WebSocket + HTTP asset server
│   ├── flux-codegen-swift/       # SwiftUI codegen (node-ID bridge)
│   ├── flux-codegen-kotlin/      # Jetpack Compose codegen (node-ID bridge)
│   ├── flux-codegen-core/        # Shared data-driven emitter (Backend trait, primitive registry)
│   ├── flux-cli/                 # `flux` CLI binary
│   ├── flux-parity/              # Dev VM == release codegen parity harness + trace diffing
│   ├── flux-lsp/                 # Language server (async-lsp), FLUX-024/FLUX-029
│   ├── flux-perf-harness/        # Render-perf benchmark harness (PRD-J, ADR-0048)
│   └── flux-devtools-ui/         # gpui DevTools desktop (ADR-0041)
├── runtimes/                     # Host apps (Swift / Kotlin)
│   ├── ios/                      # SwiftUI/UIKit host (XcodeGen manifest, iOS 16.0 / Swift 6.0)
│   └── android/                  # Jetpack Compose host (host = pure-JVM reactive core, app = shell)
├── adapters/                     # Platform adapter implementations (contract version 1)
│   ├── ui-swift/
│   └── ui-kotlin/
├── stdlib/                       # 28 .flux standard-library components
├── docs/
│   ├── spec/                     # mlp-spec.md + mlp-appendices.md (A–G)
│   └── adr/                      # 38 Architecture Decision Records (MADR), highest ADR-0057
├── tests/                        # Integration + parity + ISA vectors
└── website/                      # Astro documentation site
```

---

## Quick start

### Prerequisites

- **Rust nightly.** The toolchain is pinned via `rust-toolchain.toml`
  (`channel = "nightly"`, with `rustfmt` + `clippy`). A nightly compiler is
  required because `flux-devtools-ui` (the gpui DevTools crate, ADR-0041) pins
  `gpui` to a `main` commit that uses unstable std features (e.g.
  `std::hint::cold_path`); the full workspace cannot build on stable. The
  declared `rust-version = "1.86"` in `Cargo.toml` is the edition-2024 language
  floor, not the active toolchain.
- `cargo-nextest` — the project's test runner.
- Xcode (iOS 16+ deployment target) and/or the Android SDK for the host apps.

### Build & test the workspace

```bash
# Format + lint (must be clean)
cargo fmt -- --check
cargo clippy -- -D warnings

# Run the full test suite (nextest, never `cargo test`)
cargo nextest run
cargo test --doc          # doctests (nextest does not run these)

# Run a single crate's tests
cargo nextest run -p flux-parser
```

### Start the dev server

```bash
# Build the CLI
cargo build -p flux-cli

# Start the hot-reload dev server (defaults to the current dir; WS :7331, HTTP :7332)
flux dev --root ./my-app
```

The server watches `.flux` files, lowers + diffs on every save, and streams
binary patches to the connected host app over `ws://`. Use `--ws-host 0.0.0.0`
to expose the server on the LAN so physical devices and simulators can reach it.
Optionally pass `--token <secret>` to require hosts to present a matching pairing
token during the `Hello` handshake (Appendix D §D.12.1) when exposed on a LAN.

### Host apps

- **iOS:** open `runtimes/ios` in Xcode (regenerate with `xcodegen generate`), run
  on a simulator or device (iOS 16+). The frozen `project.yml` sets
  `SWIFT_VERSION: "6.0"` and `SWIFT_STRICT_CONCURRENCY: complete`.
- **Android:** `./gradlew :runtimes:android:build` (or `:test`). The reactive core
  in `runtimes/android/host` is pure-JVM and has no Android dependency, so its
  suite runs without an emulator.

### CLI commands

`flux` exposes the following subcommands (see `flux --help` for the full surface):

| Command | Purpose |
|---|---|
| `flux init <name>` | Scaffold a new Flux project at `<name>/`. |
| `flux dev [--root] [--ws-host] [--ws-port] [--http-port] [--token]` | Start the hot-reload dev server (WS `:7331`, HTTP `:7332` by default). |
| `flux build --platform ios\|android [--root]` | Codegen the project to `platforms/<platform>/Generated/`, then invoke the native toolchain when present (FLUX-068); logs the manual build command in emit-only fallback. |
| `flux fmt [<files>...] [--check]` | Format `.flux` sources to canonical style (FLUX-078). `--check` verifies without writing. |
| `flux lsp <file> [--types]` | Emit parse + type-check diagnostics for a `.flux` file as JSON (FLUX-025). |
| `flux doc` | Emit a JSON schema of the stdlib API to stdout. |
| `flux doctor` | Environment health check: toolchain, stdlib parse, wire protocol version, best-effort connected devices/simulators. |

---

## Testing & quality standards

This repo follows a strict TDD + quality standard (see `AGENTS.md`):

- **Every public function has a test.** The suite spans unit, `proptest`
  property, `insta` snapshot, `criterion` benchmark, and dev/release parity tests
  (run with `cargo nextest run`).
- `cargo fmt` and `cargo clippy -- -D warnings` must be clean.
- `forbid(unsafe_code)` in every library crate; no `unwrap`/`expect`/`panic` in
  production code; every error carries a source `Span`.
- Performance budgets are enforced by `cargo bench` (parse 500 lines < 5 ms,
  diff 50 nodes < 1 ms, serialize 50-node patch < 1 ms, VM eval < 2 ms).

| Layer | Tool | What it proves |
|---|---|---|
| Unit | `cargo nextest run` | Every function, edge cases, error paths |
| Property | `proptest` | Node-ID stability, diff minimality, round-trip |
| Snapshot | `insta` | Swift & Kotlin codegen output |
| Benchmark | `criterion` | Performance budgets |
| Parity | `tests/` | Dev VM execution == release codegen |

---

## Continuous integration

GitHub Actions guard `main` (12 workflows under `.github/workflows`):

| Workflow | Purpose |
|---|---|
| `rust-check.yml` | `cargo fmt` / `clippy` / `nextest` on every push |
| `ios-check.yml` | Swift build + test of the iOS host |
| `android-check.yml` | Kotlin build + test of the Android host |
| `manifest-steward.yml` | Keeps the `Cargo.toml` workspace manifest consistent |
| `adr-numbering.yml` | Enforces ADR numbering discipline |
| `benchmarks.yml` | Runs the criterion benchmark suite |
| `perf-harness.yml` | Runs the `flux-perf-harness` render-perf suite |
| `wire-fuzz.yml` | Fuzzes the wire-protocol (de)serialization |
| `compat-matrix.yml` | Cross-version compatibility matrix |
| `mutation-testing.yml` | Mutation testing / compat matrix |
| `artifact-publish.yml` | Publishes release artifacts |
| `vscode-check.yml` | Builds/checks the VS Code extension |

---

## Documentation

- **Spec:** [`docs/spec/mlp-spec.md`](docs/spec/mlp-spec.md) and
  [`docs/spec/mlp-appendices.md`](docs/spec/mlp-appendices.md) (grammar, IR
  schema, wire protocol, VM ISA, adapter contracts, glossary — Appendices A–G).
- **Agent manual:** [`AGENTS.md`](AGENTS.md) — the law of the land for contributors.
- **Decisions:** [`docs/adr/`](docs/adr/) — 38 MADR records (highest ADR-0057).
- **Docs site:** [`website/`](website/) — Astro source (`pnpm install && pnpm dev`).

---

## Contributing

Flux is built by **many parallel agents committing directly to `main`** — there
are no branches and no pull requests (see `AGENTS.md` §4). To contribute:

1. Read `AGENTS.md` and the relevant spec appendix **in full** before touching code.
2. Write a failing test first (RED), make it pass (GREEN), then refactor.
3. Stay on `main`; commit with `git commit --only <your/files>` so you never
   sweep another agent's in-flight work into your commit.
4. One focused, atomic commit per logical change with a scoped message
   (`feat(parser): ...`, `fix(differ): ...`, …).

---

## License

Licensed under **Apache-2.0** (see the top-level `LICENSE` file and
`workspace.package.license` in `Cargo.toml`).

---

<p align="center">
  <sub>Flux — write once, render native. 633 commits · 16 crates · 38 ADRs · built on <code>main</code>.</sub>
</p>
