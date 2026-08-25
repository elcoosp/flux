# ADR runtime-packaging-gap: host runtimes are apps, not importable packages

- **Status:** Proposed (created 2026-08-25 by the `examples-e2e` agent, FLUX-022 / PE-E)
- **Related:** ADR-0027 (node-ID bridge), `devserver-build-accessor`,
  `codegen-input-contract`, spec §14.3, and the runtime agents (P1/P2) who own
  the fix.

## Context

A Flux consumer authors `.flux` under a `flux.toml` and runs the pipeline two
ways:

1. **Dev (`flux dev`):** the Rust `flux-devserver` boots, watches the project,
   and ships `Init`/`Delta` wire frames to a connected host over WebSocket.
2. **Release (`flux build --platform ios|android`):** the Rust pipeline lowers
   the sources and the codegen crates emit `Generated/*.swift` / `*.kt`.

The Rust pipeline is fully headless — a consumer can run the *entire*
parse → type-check → lower → (wire | codegen) flow with no native toolchain
(verified end-to-end by `crates/flux-devserver/tests/full_pipeline.rs` against
`examples/counter`).

The native side — the host engine that receives the wire frames and renders
native views, and the runtime that links the codegen output — is **not** a
library. As of this writing:

- `runtimes/ios` is an **app**: `FluxApp.xcodeproj` with `FluxAppMain.swift`
  carrying `@main`. There is no SwiftPM library target a consumer could
  `import FluxHost` into their own app.
- `runtimes/android` has only an `:app` module (no `:host` / `:ui` library
  module). A consumer cannot `implementation` the host engine into their own
  `:app`.

Consequences:

- A consumer **cannot** embed the Flux host engine inside their existing
  native application; they must ship the Flux-provided app wrapper as-is.
- There is **no headless integration test** that boots the native render
  without a simulator/emulator: the host engine is coupled to the app entry
  point, so it cannot be driven from a plain `xcodebuild test` / `./gradlew
  test` harness in isolation.

This is a **packaging** gap, not a protocol or pipeline gap. The wire protocol
(Appendix D) and the Rust pipeline are complete and independently verifiable.
The gap is that the native half ships as a monolithic app rather than as an
importable engine module.

## Decision Drivers

- **Vertical slice exists on the Rust side (PE-E):** `examples/counter`
  (`main.flux` + `flux.toml`) compiles and lowers cleanly via `flux dev` and
  produces non-empty `Generated/counter.swift`/`.kt` via `flux build`. The
  consumer-facing source contract is settled; only the native integration path
  is blocked.
- **Boundary contract forbids restructuring the runtimes here:** the
  `examples-e2e` agent owns only `/examples/**`, the one devserver test file,
  and new ADRs. `runtimes/ios` and `runtimes/android` are owned by the P1/P2
  runtime agents. Extracting a library module from each app is therefore
  explicitly **out of scope** for this task and is queued below.
- **No manifest edits:** the host engine's dependency surface (which crates the
  runtime app links) is pre-wired by the foundation agent; this ADR records the
  gap and the recommended integration path, and queues the fix for the runtime
  owners. It does not change any build graph.

## Recommended Integration Path Today

Until the host engine is extracted into a library, the supported consumer flow
is:

1. `flux dev` (or `flux build` to inspect generated sources).
2. Open the runtime app (`FluxApp` on iOS, the `:app` on Android).
3. The app connects to the dev server's WebSocket and renders the wire frames
   it receives.

`examples/counter` is the canonical vertical slice to validate this path:
run `flux dev --root examples/counter`, launch the runtime app, and confirm the
`Counter` renders and increments on tap. The headless Rust e2e
(`full_pipeline.rs`) proves the frames the app would receive are well-formed,
so a failure to render is isolated to the native host — independently testable
against the byte-exact `Init`/`Delta` frames.

## Decision Outcome

**Do not restructure the runtimes in this task.** Document the gap and queue the
fix as owned by the runtime agents (P1/P2).

The recommended near-term remedy (to be executed by P1/P2, not here) is to
extract the host engine from each app into a library module that the app wraps:

- **iOS:** a `FluxHost` SwiftPM library target (the WebSocket client, the wire
  decoder, the shadow tree, the VM, and the adapter views) that `FluxApp`
  depends on and calls from its `@main`. A consumer's own app would
  `import FluxHost` and embed `FluxHost.run(...)`.
- **Android:** a `:host` (engine: wire client, decoder, shadow tree, VM,
  adapters) and/or `:ui` library module that the `:app` depends on. A
  consumer's own `:app` would `implementation project(":host")` and start the
  host engine from their `Application`/`Activity`.

Once extracted, a headless native integration test becomes possible: the
library target can be exercised by a thin test harness (no simulator UI), and a
consumer can run the engine inside their own app rather than forking the Flux
app wrapper.

## Queued Work (owned by P1/P2)

- [ ] iOS: extract `FluxHost` SwiftPM library from `runtimes/ios`;
  `FluxAppMain.swift` becomes a thin wrapper (spec §14.3).
- [ ] Android: extract `:host`/`:ui` library modules from `runtimes/android`;
  `:app` depends on them.
- [ ] Add a headless native integration test that boots the host engine against
  a canned `Init` frame and asserts a native view is materialised, without a
  simulator.

## Consequences

- **Good:** the Rust pipeline remains fully headless and independently
  verifiable (proven by `full_pipeline.rs`); the native gap is now explicitly
  recorded rather than silently blocking consumers.
- **Bad (temporary):** until P1/P2 land the extraction, a consumer cannot embed
  the Flux engine in their own app and there is no simulator-free native render
  test. The only supported native integration is "run the Flux app wrapper and
  connect it to `flux dev`."
- **Neutral:** no protocol, pipeline, or codegen behavior changes; the ADR is a
  packaging/ownership record, not a behavioral change.
