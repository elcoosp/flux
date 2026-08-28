# Flux — MVP → Production: Parallel Agent Lane Map

> Grounded in the live tree on 2026-08-28. Every claim below was verified against
> source (git log, `cargo nextest`, `xcodebuild`, grep) — NOT against the stale
> 10-section review that was handed in. The review's central theses (6/10 B.3
> lowering fails, async host halves unimplemented, capabilities unbuilt) are
> **already done**: 10/10 parity green, AwaitSuspend/Resume wire merged,
> `CapabilityRegistry`/`CALL_CAP`/`HelloFrame` capability handshake present.

## 0. Current realized state (what is actually working)

| Subsystem | Status | Evidence |
|---|---|---|
| Rust pipeline (parse→type→lower→wire/codegen) | GREEN | `cargo nextest` 412/412; clippy `-D warnings` clean; `cargo fmt` clean |
| B.3 parity (10 examples, dev vs Swift vs Kotlin) | GREEN | `flux-parity` snapshots B3.1–B3.10 |
| VM reference oracle (`flux-vm-ref`) | GREEN | all ISA vectors pass incl. async suspend/resume |
| iOS runtime build + tests | GREEN | `xcodebuild -scheme FluxApp` BUILD SUCCEEDED; 7/7 tests pass (incl. router signal-97 nav) |
| Android runtime | SOURCE PRESENT; **BUILDABLE via `./gradlew`** | repo-root `./gradlew` + `settings.gradle.kts` (incl. `:runtimes:android:host`, `:app`) + JDK 21 + `ANDROID_HOME`. `./gradlew :host:test` runnable (first run downloads Gradle 9.7.1). |
| Wire protocol + handshake + version-mismatch Error frame | GREEN | `crates/flux-devserver/src/server/session.rs` |
| Capability *wiring* (registry, IDL, CALL_CAP, Hello advertise) | GREEN | `Registry.swift`, `CapabilityRegistry.kt`, `HelloFrame.*` |
| Capability *bodies* (Camera/Storage/Router) | STUBS | in-memory only; see Lane C |
| Async resolver on host | PASSTHROUGH | `PassthroughAsyncResolver` (iOS) / inline object (Android) — true native async unbuilt (Lane A) |
| Release codegen (`flux build`) | EMITS ONLY | does NOT invoke `xcodebuild`/`gradle` (best-effort, not invoked) — Lane F |
| Runtime packaging as importable engine | GAP | ADR-0036; iOS app-shell-coupled, Android no `:host` lib module / no wrapper — Lane G |
| Dev-only log compile-out | GREEN (now) | all `NSLog` gated `#if DEBUG` (fixed this session) |
| Android router navigation | **GREEN (fixed this session)** | `activeRouteFromSignal()` only read `RecordVal` signal 97 → real `Router.navigate` tap writes a raw `StrVal` and silently no-op'd ("go to settings does nothing"). Added the `.str` branch to mirror iOS `routerActiveChildId`. `./gradlew :runtimes:android:host:test` 136 pass, 0 fail. |

No agent is currently "in flight" on the core Rust crates (working tree shows only
committed + other-agents' WIP in adapters/runtimes). The dirs below are free to spawn
into unless `git status` shows a file modified in that dir at dispatch time — re-check.

## 1. Lane taxonomy (production dimensions)

A **Core hardening** — make the demonstrated-small-example path bulletproof at scale.
B **Device-only blind spots** — things green in parity/tests but unverified on real silicon.
C **Real capabilities** — replace in-memory stubs with native API backends.
D **Async host bridge** — real `AsyncResolver` for network/timer/camera.
E **Release packaging & build gate** — `flux build` actually compiles; host is importable.
F **DX & tooling** — fmt, LSP, incremental compile, diagnostics.
G **Runtime distribution** — AAR / xcframework, version handshake, integration guides.
H **Scale & perf** — large trees, allocation pressure, fuzzing/wire robustness.
I **Compliance & docs** — error hierarchy, security sandboxing, prod deployment guide.

## 2. Concrete lanes (each = one agent, dir-disjoint)

### LANE-A — Host Async Resolver (real platform async)  [in-flight-safe on runtime dirs]
- **Owned:** `runtimes/ios/Sources/**` (replace `PassthroughAsyncResolver` with a
  `URLSessionTaskResolver`/`TimerResolver`), `runtimes/android/host/src/main/**`
  (`suspend fun resolve` → coroutine bridge). SEE skill hazard: the VM
  suspend/resume dispatch sites in `FluxBytecodeVM.swift`/`StepResult.kt` are
  ADR-0044's lane — do NOT patch dispatch, only the resolver impl.
- **Why production-ready:** `AWAIT` parks but the cell is never resolved by real IO;
  apps needing network/camera/timer suspend forever.
- **Acceptance:** a handler that `AWAIT`s a `Camera.take`/timer resumes with the real
  value; signal graph state preserved across suspend; release build keeps no debug logs.

### LANE-B — Device-only blind-spot verification (router + CALL_CAP ids)  [parity + runtime]
- **Owned:** `crates/flux-parity/src/**` (strengthen `router_example_emits_route_prop...`
  to assert the *lowered* `route` prop index == `FNV-1a("route")` for BOTH named and
  positional `Screen` forms) + a real on-device router smoke in
  `runtimes/ios/Tests/**`,`runtimes/android/host/src/test/**`.
- **Why:** parity can be green while a `Screen("x")` positional arg lowers to PropIdx(0)
  and navigation silently never swaps (documented trap in flux-capabilities skill).
- **Acceptance:** `examples/router/main.flux` uses NAMED `Screen(route:)`; a real tap on
  both sims swaps the active screen; CALL_CAP id regression (`Router.navigate`→(3,1))
  holds against a `FluxBytecodeVM.run` of a capability handler.

### LANE-C — Real native capability bodies  [capability owner dirs]
- **Owned:** `Registry.swift` (iOS), `CapabilityRegistry.kt` (Android),
  `stdlib/capabilities.flux`, `HelloFrame.*` (advertise). SAFE per flux-capabilities
  skill (registry/Hello/stdlib are the capability owner's lane — NOT the VM dispatch
  lane).
- **Scope:** `Camera.take`→`UIImagePickerController`/`PHPhotoLibrary` (iOS) /
  `CameraX` (Android); `Storage.set/get/delete`→`UserDefaults`+file (iOS) /
  `DataStore`/file (Android) instead of in-memory; keep `Router.navigate` signal-97
  write. New caps (e.g. `Clipboard`, `Geolocation`) follow the same table-entry pattern.
- **Acceptance:** round-trip tests in `RuntimeGapTests.swift`/`RuntimeFixesTest.kt`
  call `registry.lookup(cap,method).call(args,signals)` and assert real side-effects;
  the `call_cap_basic` oracle vector (cap1,method1→sig99) stays green.

### LANE-D — Wire robustness & fuzzing  [ir-serde + ci]
- **Owned:** `crates/flux-ir-serde/src/**`, `/.github/workflows/**`.
- **Scope:** `cargo fuzz` target on `Frame::from_*_bytes`; validate jump/opcode bounds
  before VM execution; canonical-string-id enforcement (INV-1) hardening; untrusted
  `.flux` resource-exhaustion guard (gas already in place — extend to frame size cap).
- **Acceptance:** fuzz target runs in CI (linux); malformed frame with out-of-range
  jump target errors, never panics.

### LANE-E — Release build gate (`flux build` compiles)  [cli]
- **Owned:** `crates/flux-cli/src/**`.
- **Scope:** when `xcodebuild`/`gradle` present, actually invoke the generated-app
  build and FAIL the command on a compile error (today it prints "emitted only" and
  returns 0). Generate a minimal host-app wrapper around `Generated/` if one doesn't
  exist, or document the required consumer wiring.
- **Acceptance:** `flux build --platform ios` on a Mac produces a compiling app;
  `flux build --platform android` uses `./gradlew` (needs wrapper — see Lane G).

### LANE-F — Runtime packaging as importable engine (ADR-0036)  [ios-runtime + android-runtime]
- **Owned:** `runtimes/ios/**` (extract engine from `@main` app shell so a consumer
  `import FluxHost` + `import FluxUIKit` drives it; keep `FluxApp` as the demo host),
  `runtimes/android/**` (split `:host` library module so a consumer app can
  `implementation(project(":runtimes:android:host"))`). **NOTE:** a `./gradlew` wrapper
  and `settings.gradle.kts` already exist at the repo root (incl. `:runtimes:android:host`
  + `:app`), so `./gradlew :host:test` IS runnable — Lane F is the module split, not the
  wrapper bootstrap. **R2 (frozen manifests):** do not recreate `settings.gradle.kts`;
  the runtime-module split is the agent's task, coordinated with the orchestrator.
- **Acceptance:** a separate consumer app can `import FluxHost` and render; Android
  `./gradlew :host:test` runs in CI.

### LANE-G — Distribution, versioning & integration guides  [docs + ci]
- **Owned:** `/docs/**` (orchestrator carve-out), `/.github/workflows/**`.
- **Scope:** produce AAR (Android) + xcframework (iOS) from Lane F; pin
  `PROTOCOL_VERSION` to a runtime release; handshake-version fail-closed (exists
  server-side, add host-side check); write "Embed Flux in an existing app" guide.
- **Acceptance:** published artifacts build; host refuses to boot against a mismatched
  `PROTOCOL_VERSION` with an actionable error.

### LANE-H — Scale & perf budgets  [vm-ref + differ + devserver + parity benches]
- **Owned:** `crates/flux-vm-ref/src/**`, `crates/flux-differ/src/**`,
  `crates/flux-devserver/src/**`, `crates/flux-parity/benches/**`.
- **Scope:** benchmarks for 1k/10k-node trees against §3.10 budgets; allocation
  pressure in decoder/reconciler (object pooling); dirty-subset reconcile stays
  O(dirty) not O(tree).
- **Acceptance:** benches within budget committed and run in CI.

### LANE-I — Error hierarchy & security hardening  [types + devserver]
- **Owned:** `crates/flux-types/src/**`, `crates/flux-devserver/src/**`.
- **Scope:** unify VM-fault / compile-error / capability-failure error types with
  actionable spans (AGENTS.md §3.11); capability *permission* gate (camera/storage
  need OS permission before CALL_CAP resolves); path-traversal audit on asset server.
- **Acceptance:** each error carries what/where/why/how; a denied camera permission
  surfaces a red banner, not a crash.

## 3. Dispatch waves (stagger 3–4, per parallel-agent-orchestration)

- **Wave 1 (independent, dir-disjoint):** LANE-C (caps), LANE-D (fuzzing),
  LANE-H (perf), LANE-I (errors). No shared dirs; all can run now.
- **Wave 2 (after Wave 1, depends on runtime dirs being free):** LANE-A (async
  resolver), LANE-B (device blind spots), LANE-E (build gate). These touch
  `runtimes/*` — ensure no other agent holds them at dispatch.
- **Wave 3 (structural, coordinate with orchestrator re: R2):** LANE-F (packaging),
  LANE-G (distribution/guides). Android native suite is runnable NOW via `./gradlew`
  (root wrapper present) — Wave 2 Android lanes (A/B/E) can verify here, not only in CI.

## 4. Explicitly OUT OF SCOPE (already done or not blocking MVP→prod)
- Lowering 10/10 B.3 (DONE), async wire bridge (DONE), capability handshake (DONE),
  prop-thunk fallback (DONE), debug-log compile-out (DONE this session).
- Do NOT re-spawn "finish lowering" or "implement async wire" — those are closed.

## 5. Verification gates per lane (do NOT claim green without these)
- Rust lanes: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo nextest`, `cargo doc`.
- iOS lane: `xcodebuild -scheme FluxApp test` on iPhone 17 Pro sim (SwiftLint/treat-warnings).
- Android lane: `./gradlew :host:test` (requires wrapper from Lane F first).
- Capability lanes: round-trip test on BOTH hosts + `call_cap_basic` oracle vector intact.
