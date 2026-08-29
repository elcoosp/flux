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

![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)
![Rust](https://img.shields.io/badge/rust-nightly%20%7C%20edition%202024-orange.svg)
![Crates](https://img.shields.io/badge/workspace-15%20crates-9cf)
![Tests](https://img.shields.io/badge/runner-cargo%20nextest-brightgreen.svg)
![Source](https://img.shields.io/badge/rust%20LOC-42k-9cf)
![ADRs](https://img.shields.io/badge/ADRs-30%20%28MADR%29-blueviolet.svg)
![Platforms](https://img.shields.io/badge/platforms-iOS%2016%2B%20%7C%20Android-purple.svg)
![Rust CI](https://github.com/elcoosp/flux/actions/workflows/rust-check.yml/badge.svg)
![iOS CI](https://github.com/elcoosp/flux/actions/workflows/ios-check.yml/badge.svg)
![Android CI](https://github.com/elcoosp/flux/actions/workflows/android-check.yml/badge.svg)
![Merge Guard](https://github.com/elcoosp/flux/actions/workflows/merge-guard.yml/badge.svg)

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

```
 .flux source
      │  flux-parser  (hand-written lexer + recursive-descent parser)
      ▼
  flux-syntax  ──►  flux-types  (type check)
      │                    │
      ▼                    ▼
  flux-ir   (Reactive Tree IR, arena-allocated, stable u32 node ids)
      │
      ▼  flux-differ  (structural diff → minimal edit script)
  flux-ir-serde  (binary MessagePack patch, blake3 content-addressed)
      │
      ├──► dev:  flux-vm-ref  (register VM)  ──► host app (native shadow tree)
      └──► rel:  flux-codegen-swift / flux-codegen-kotlin  (native codegen)
```

The wire protocol (Appendix D), VM ISA (Appendix E), and adapter contracts
(Appendix F) are normative and versioned. Node IDs are `blake3` hashes of
`(parent_id, tag, span, key)` — **stable across edits** so state is preserved
through hot-swap.

---

## Monorepo layout

```
flux/
├── crates/                       # Rust workspace (15 crates)
│   ├── flux-syntax/              # Shared type vocabulary (ids, value, ty, node, patch)
│   ├── flux-parser/              # Hand-written lexer + recursive-descent parser
│   ├── flux-types/               # Type checker
│   ├── flux-ir/                  # Reactive Tree IR + arena
│   ├── flux-ir-serde/            # Binary patch (de)serialization
│   ├── flux-differ/              # Structural tree differ
│   ├── flux-vm-ref/              # Reference register-based VM
│   ├── flux-devserver/           # Hot-reload pipeline + WebSocket server
│   ├── flux-codegen-swift/       # SwiftUI codegen
│   ├── flux-codegen-kotlin/      # Jetpack Compose codegen
│   ├── flux-codegen-core/        # Shared data-driven emitter (Backend trait)
│   ├── flux-cli/                 # `flux` CLI binary
│   ├── flux-parity/              # Dev VM == release codegen parity tests
│   ├── flux-perf-harness/        # Render-perf benchmark harness + emit CLI
│   └── flux-devtools-ui/         # gpui DevTools desktop (ADR-0041)
├── runtimes/                     # Host apps (Swift / Kotlin)
│   ├── ios/                      # 43 Swift files
│   └── android/                  # 62 Kotlin files
├── adapters/                     # Platform adapter implementations
│   ├── ui-swift/
│   └── ui-kotlin/
├── stdlib/                       # 13 .flux standard-library components
├── docs/
│   ├── spec/                     # mlp-spec.md + mlp-appendices.md (A–G)
│   └── adr/                      # 30 Architecture Decision Records (MADR)
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

### Host apps

- **iOS:** open `runtimes/ios` in Xcode, run on a simulator or device (iOS 16+).
- **Android:** `./gradlew :runtimes:android:build` (or `:test`).

### Other CLI commands

`flux` also provides `fmt` (canonically format `.flux` sources), `doc` (emit the
stdlib JSON schema), `doctor` (diagnose the local toolchain), `lsp` (LSP-style
diagnostics for a `.flux` file), and `add` (install a community `.flux`
component). See `flux --help` for the full surface.

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
| `fmt-check.yml` | Dedicated formatting gate |
| `ios-check.yml` | Swift build + test of the iOS host |
| `android-check.yml` | Kotlin build + test of the Android host |
| `merge-guard.yml` | Blocks shared-index commit hazards on parallel `main` |
| `manifest-steward.yml` | Keeps the `Cargo.toml` workspace manifest consistent |
| `adr-numbering.yml` | Enforces ADR numbering discipline |
| `benchmarks.yml` | Runs the criterion benchmark suite |
| `perf-harness.yml` | Runs the `flux-perf-harness` render-perf suite |
| `wire-fuzz.yml` | Fuzzes the wire-protocol (de)serialization |
| `compat-matrix.yml` | Cross-version compatibility matrix |
| `artifact-publish.yml` | Publishes release artifacts |

---

## Documentation

- **Spec:** [`docs/spec/mlp-spec.md`](docs/spec/mlp-spec.md) and
  [`docs/spec/mlp-appendices.md`](docs/spec/mlp-appendices.md) (grammar, IR
  schema, wire protocol, VM ISA, adapter contracts, glossary — Appendices A–G).
- **Agent manual:** [`AGENTS.md`](AGENTS.md) — the law of the land for contributors.
- **Decisions:** [`docs/adr/`](docs/adr/) — 30 MADR records.
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

Licensed under **Apache-2.0**. See `Cargo.toml` (`workspace.package.license`).

> Note: a top-level `LICENSE` file is not yet present in the repo — only the
> `Cargo.toml` declaration. Add `LICENSE` before publishing to comply with the
> declared license.

---

<p align="center">
  <sub>Flux — write once, render native. 473 commits · 15 crates · 30 ADRs · built on <code>main</code>.</sub>
</p>
