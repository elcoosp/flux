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
