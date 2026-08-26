# ADR-0041 — `gpui` desktop DevTools dependency

- Status: Accepted
- Date: 2026-08-26
- Scope: new crate `flux-devtools-ui`
- Supersedes: none
- Superseded by: none
- Related: DevTools spec §5, §9 (ADR-0032 in the spec is renumbered here to
  0041 to avoid colliding with the already-accepted ADR-0032
  `devserver-build-accessor`)

## Context

The DevTools desktop app needs a UI framework. Options considered: `egui`
(immediate mode, Rust-native, simplest), `iced` (retained-mode, Elm-architecture),
and `gpui` (Zed's GPU-accelerated retained-mode framework). The spec names
`gpui`.

`gpui` is a heavy, fast-moving dependency (pulls `smol`, `mina`, `async-task`,
`blade`/`wgpu`, `skrifa`, etc.) and requires a `build.rs`/`asset` setup plus a
run loop (`gpui::App`). Vendoring it as a workspace member pulls those
transitives into the entire workspace build graph, and `gpui` is not on the
pre-wired dependency list in `Cargo.toml`.

## Decision

Approve `gpui` **only** for the new `flux-devtools-ui` crate. It is added to
`Cargo.toml`'s `[workspace.dependencies]` and to the workspace `members` list.
All other crates remain `gpui`-free. The crate is owned exclusively by the
DevTools agent and follows every `AGENTS.md` constraint (no `unwrap`, clippy
`-D warnings`, ≤300-line files, tests).

To keep the gpui surface verifiable without a GPU/display in CI, the
time-travel core (ring buffer + state reconstruction) and the wire client live
in non-gpui modules (`time_travel/`, `wire_client.rs`) that are unit-tested in
isolation; only the `app.rs`/`views/*` modules import `gpui`.

## Consequences

- A native, GPU-accelerated DevTools window.
- CI can still `cargo nextest`/`clippy` the whole workspace; gpui compiles headless
  on macOS/Linux build agents (it links system frameworks on macOS and builds
  on Linux without a display for library compilation, though runtime needs a GPU).
- The rest of the workspace is unaffected.

## Alternatives considered

- **`egui`**: rejected by the spec; also immediate-mode does not match the
  retained `DevToolsState`/`Entity` model the spec sketches.
- **Defer the desktop app, ship only the wire + server + core**: rejected — the
  spec's §5 is explicit; `gpui` is approved here.
