# Flux → 1.0: Roadmap to a Production-Grade, 10x-DX Native UI Framework

*Based on a full read of the current monorepo dump (13 Rust crates / ~33k LOC,
399 tests, Android + iOS host runtimes, gpui DevTools shell, 16 stdlib `.flux`
files, Astro docs site, ADR-0001→0048+).*

---

## 0. Where Flux actually is today (honest baseline)

Before planning forward, here's what the dump shows, stripped of aspiration:

**Genuinely strong / differentiated:**
- The core thesis is sound and *already built end-to-end*: `.flux` → parse
  (pest PEG) → type-check → arena IR → structural diff → binary MessagePack
  patch → dev VM execution *or* release SwiftUI/Compose codegen, from the
  *same* IR (`flux-codegen-core`, ADR-0047). No competitor (RN, Flutter, KMP
  Compose Multiplatform) ships a real "same IR, two backends" story — this is
  Flux's moat if it survives contact with real apps.
- Content-addressed `NodeId` (blake3 of parent/kind/span/key) giving
  state-preserving hot patches (`Patch::Reattach`, ADR-0027) is a genuinely
  better hot-reload primitive than Metro's JS-eval or Flutter's full-widget
  rebuild.
- First-class async over the wire (`AwaitSuspend`/`Resume`, ADR-0044/0045) and
  a unified sync/async capability bridge (`CALL_CAP`) is a clean model — most
  frameworks bolt this on later.
- Engineering discipline is unusually high for pre-1.0: TDD-enforced,
  zero-warnings, ADR-numbered decisions, a merge-guard that prevents directory
  collisions across parallel agents, a manifest-freeze process. This lowers
  the risk of the "rewrite everything at 1.0" tax most frameworks pay.

**Thin / pre-alpha (this is most of the gap to 1.0):**
- **Stdlib is 8 primitives** (`text`, `button`, `column`, `row`, `text_field`,
  `screen`, `router`, plus `color`/`font`/`platform`/`traits`/`capabilities`
  declarations). No `Image`, no scrollable list, no `Modal`/`Sheet`, no
  gestures, no animation primitive, no `Stack`/`Grid` layout beyond
  row/column, no forms beyond a single text field. This is nowhere near "90%
  of use cases."
- **Two example apps** (`counter`, `router`). No realistic app has been built
  end-to-end, so real-world friction is still unknown.
- **iOS has not converged to the declarative tier** (AGENTS.md §0.2, Axis 2):
  `adapters/ui-swift/FluxUIKit` is still an imperative UIKit reconciler while
  Android is declarative Compose. ADR-0048 gates the port on measurement that
  hasn't happened yet — **there is currently no render-perf test on either
  platform**, so the §3.10 "native mutation < 3ms" budget is unverified
  everywhere.
- **Only 5 platform capabilities** (Camera, Storage, Router, Clipboard,
  Geolocation) — no push notifications, biometrics, background tasks, file
  system, sensors, deep linking, or a native-module escape hatch for anything
  not on this list.
- **DevTools (`flux-devtools-ui`) exists as a skeleton**: `time_travel`,
  `views/{component_tree,signal_graph,timeline,vm_inspector}`, `wire_client`.
  Structure is there; there's no evidence of a shipped, polished experience —
  no error overlay, no network inspector, no perf flamegraphs, no on-device
  hookup validated against a real app.
- **Error handling is architecturally started, not finished**: `FluxError`
  hierarchy + permission gate just landed (LANE-I) and the umbrella was
  "half-disabled" until a follow-up fix — i.e., this is days old, not hardened.
  There is no on-device error overlay, no source-mapped stack traces from
  `.flux` spans through the VM/wire back to the editor.
- **`flux build` doesn't actually invoke the native toolchain in the general
  case** — it detects `xcodebuild`/`gradle` and logs a manual command if
  absent; that's a CI/DX gap for anyone without both toolchains installed.
- **Grammar is mid-migration** (ADR-0029): brace syntax still lives in some
  fixtures alongside the new indentation-based lexer. Any DX work (syntax
  highlighting, LSP) built against the old grammar will need rework.
- **No LSP, no editor extension, no linter, no formatter for `.flux` itself**
  — the single highest-leverage DX investment for "10x DX" is completely
  unstarted.
- **Website is docs + one interactive trace player**, in two locales (en/es)
  with an i18n-drift checker — a real base to build on, but thin on guides,
  cookbooks, and migration content.

**Conclusion:** Flux has an unusually solid *engine* and an unusually thin
*product surface*. The roadmap below is ordered so that surface work happens
on top of a measured, hardened engine rather than in parallel with an
unresolved architectural question (iOS convergence) or an unverified
performance budget — both of which would invalidate downstream DX/perf work
if left open.

---

## 1. Definition of "1.0"

Ship when all five are true, not just the feature list:

1. **90% use-case coverage** — a developer can build a typical CRUD /
   social / e-commerce mobile app (forms, lists, media, navigation, auth,
   networking, push, offline cache, animations) without dropping to native
   code, *except* for genuinely exotic integrations (AR, custom camera
   pipelines, etc.), which are explicitly out of scope and documented as such.
2. **10x DX** — time-to-first-error and time-to-fix are both faster than RN/
   Flutter, measured, not asserted (see §7 metrics). Concretely: inline
   `.flux`-source diagnostics in the editor, an on-device error overlay with
   a real stack trace back to source, and hot-reload state preservation that
   survives structural edits (already partially true via `Patch::Reattach`).
3. **DevTools parity+** vs Flutter DevTools / React Native DevTools /
   Flipper: component tree, signal graph, timeline/flamegraph, network
   inspector, time-travel debugging — and *ship it*, not scaffold it.
4. **Verified performance budget** — §3.10 budgets (e.g. native mutation
   < 3ms) measured on both platforms with a CI perf gate, not aspirational
   text in a spec doc.
5. **iOS/Android architectural parity** — the render-tier question (ADR-0048)
   is *closed*, one way or the other, with data, before 1.0. Shipping 1.0
   with one platform declarative and one imperative is a documented and
   permanent asymmetry, not a "temporary" one — it can't stay "pending."

---

## 2. Phase 0 — Foundation Hardening (prep work, before new features)

*This is the section you explicitly asked for: a high-quality base before
piling on scope. Nothing in Phase 1+ should start until the items marked
**(blocking)** are done — they invalidate downstream work if skipped.*

### 0.1 Close the iOS convergence question **(blocking)**
- Implement ADR-0048 Phase 0/1 measurement: build the render-perf harness
  (below) and run it against the current imperative `FluxUIKit` reconciler.
- Prototype a minimal declarative SwiftUI dev-tier for one primitive (e.g.
  `Text`) behind a feature flag; measure it against the same harness.
- Make the call, write the ADR conclusion, and either port `FluxUIKit` to
  match Android's `ShadowTreeRenderer`/`DirtyReconciler` model, or formally
  ratify the imperative UIKit tier as permanent with a documented rationale.
  Do not enter Phase 2 with this open — every DevTools/perf feature below
  assumes one consistent rendering model to instrument.

### 0.2 Render performance harness **(blocking)**
- Build the large-tree benchmark suite (LANE-H landed some of this) into a
  **repeatable, CI-gated** perf test on both Android and iOS hosts, plus the
  dev VM and release codegen paths separately.
- Instrument: node mutation latency, dirty-subset reconciliation size vs
  full-tree size, WebSocket patch round-trip time, VM dispatch latency,
  cold start (dev-session attach → first frame), release app cold start.
- Publish the numbers. This is the evidence base for every performance claim
  in your 1.0 marketing ("incredible performance") — without it you're
  guessing, and RN/Flutter reviewers will ask for it.

### 0.3 Finish and harden the `FluxError` hierarchy **(blocking)**
- Complete the unified `FluxError` hierarchy across Rust (`flux-types`,
  `flux-devserver`), Kotlin, and Swift with one shared taxonomy: `Parse`,
  `Type`, `Permission`/`Capability`, `Wire`, `Vm`, `Codegen`, `Runtime`.
- Every error carries a `Span` (already the Rust convention per AGENTS.md
  §2.1) — extend that guarantee through the wire protocol so a VM-level
  runtime error on-device can be traced back to a `.flux` source location.
- Add property tests (`proptest`) asserting no error path panics, and a
  clippy/lint rule banning new `unwrap`/`expect`/`!!`/`try!` outside tests.

### 0.4 Grammar freeze
- Finish the ADR-0029 indentation-based grammar migration; delete brace-
  syntax fixtures; make the new grammar the *only* one CI accepts.
- Only after this freeze should you build the LSP/syntax-highlighter (§4) —
  building it against a moving grammar is wasted work.

### 0.5 CI/build hardening
- Make `flux build` actually invoke `xcodebuild`/`gradle` when present in
  CI (it already detects them; wire the invocation into the release-gate
  path used by CI, keeping the "log manual command" fallback for local envs
  without toolchains installed).
- Add a full compatibility matrix job: min/max supported Xcode, Android
  Gradle Plugin, and Kotlin versions declared and tested, not assumed.
- Add mutation testing (`cargo-mutants` or similar) on `flux-differ` and
  `flux-vm-ref` — these are the correctness-critical crates; snapshot tests
  alone won't catch every regression class.
- Wire protocol: add an explicit version-compatibility test matrix (old dev
  server ↔ new host app and vice versa) now, before real users have devices
  running old host binaries against a wire format that changed underneath
  them.

### 0.6 Security pass on the capability system
- Formal threat model for `CALL_CAP`: can a malicious `.flux` patch (e.g.
  from a compromised dev server, or a supply-chain-poisoned stdlib package
  once packages exist, §6) escalate to a capability the manifest didn't
  declare? Fuzz the capability dispatch path the same way LANE-D fuzzed the
  wire (`flux-ir-serde`).
- Decide and document the production update-integrity story: since release
  builds are native codegen (no interpreter), there is no "JS bundle OTA"
  attack surface RN has — make this a documented security *advantage* once
  verified, not an assumption.

### 0.7 Documentation reconciliation
- AGENTS.md itself flags open drift between the spec and the current code
  (§0.2 unified-tier doctrine vs iOS reality). Finish reconciling
  `docs/spec/mlp-spec.md`/`mlp-appendices.md` to actual code before onboarding
  any new contributor or writing public docs — public docs built on a stale
  spec will need to be rewritten twice.

**Exit criterion for Phase 0:** iOS/Android render-tier question closed with
data; perf harness live in CI with published numbers; `FluxError` taxonomy
complete and fuzzed; grammar frozen; wire protocol has a versioning test.

---

## 3. Phase 1 — Compiler & Language Maturity

- **Diagnostics quality bar = rustc.** Every parse/type error: precise span,
  a one-line "what," a "why," and (where mechanical) a suggested fix. This
  is foundational to "10x DX" — it's cheaper to build now than to retrofit
  once the LSP and devtools already assume a diagnostic shape.
- **Close remaining stdlib grammar gaps as new needs surface** — G1–G4 are
  closed (ADR-0035/0037), but expanding the stdlib (Phase 2) will surface new
  constructs (e.g. list comprehension/iteration syntax for rendering lists,
  slot/children composition for containers like `Modal`). Track these the
  same way: ADR the gap, land the grammar production, close it.
- **Type system gaps for real apps:** generics are monomorphized (ADR-0047
  handles this for codegen), but confirm story for: nullable/optional
  chaining ergonomics, structural vs nominal typing for props, and a
  `Result`/error-propagation story in `.flux` itself for capability calls
  that can fail (per capabilities.flux's "denied grant returns a Capability
  error, never a crash" contract — make the language ergonomics around
  handling that error as good as the runtime contract already is).
- **Formatter (`flux fmt`)** — non-negotiable for a language with an
  indentation-sensitive grammar; ship before external contributors touch
  `.flux` files, or style debates will fragment the ecosystem immediately.

---

## 4. Phase 2 — Stdlib to 90% Coverage

Current: `text`, `button`, `column`, `row`, `text_field`, `screen`, `router`
+ `color`/`font`/`platform`/`traits`. Target additions, roughly ordered by
how often they appear in a typical CRUD/social app:

**Layout & scrolling**
- `ScrollView` / virtualized `List` (the single most-missing primitive —
  no app with more than a handful of items works without this)
- `Stack` (z-order overlay), `Grid`, `Spacer`, `SafeArea`
- `Modal` / `Sheet` / `Dialog` with a real transition/animation contract

**Media & input**
- `Image` (local + remote with caching), `Icon` (vector, themed)
- Form primitives: `Switch`, `Checkbox`, `Slider`, `Picker`/`Select`,
  `DatePicker`, multi-line `TextArea`, form validation composition
- `Gesture` primitives (tap already exists via `onClick`; add long-press,
  swipe, drag, pinch — these are core to "feels native")

**Motion**
- An animation primitive tied into the signal graph (spring/timing curves
  driving signals, not just discrete patches) — this is where "10x DX
  and incredible performance" gets judged hardest against SwiftUI/Compose
  native animation APIs, since your codegen targets those exact APIs.

**Data & networking**
- An HTTP capability (fetch/JSON) and a local persistence capability beyond
  raw `Storage.set/get` (structured, queryable — even a thin wrapper)
- WebView escape hatch capability, explicitly scoped as the "when Flux
  doesn't cover it" release valve — every framework needs one and pretending
  otherwise creates 20% of your GitHub issues.

**Theming & accessibility**
- A design-token system (spacing/color/typography scales) codegen'd into
  both SwiftUI and Compose theme mechanisms natively, not just hardcoded
  literals per component.
- Accessibility props (labels, roles, focus order) threaded through the
  adapter contract from day one of each new primitive — retrofitting a11y
  after 40 components ship is far more expensive than building it in.

For each new primitive: extend the adapter contract (Appendix F) on both
`adapters/ui-kotlin` and `adapters/ui-swift`, add it to the dev VM stdlib
registry, add the codegen mapping in `flux-codegen-core`'s primitive
registry, and — given the iOS/Android convergence work in Phase 0 — build it
against *one* rendering model on both platforms, not two.

---

## 5. Phase 3 — DX: Editor, LSP, CLI

This is the highest-leverage, currently completely unstarted investment for
"10x DX":

- **Language Server (`flux-lsp`, new crate)**: diagnostics-as-you-type
  (reuse Phase 1's diagnostic quality bar), go-to-definition, hover types,
  autocomplete for props/capabilities, rename-refactor.
- **VS Code extension** (the majority of RN/Flutter devs live here):
  syntax highlighting matched to the *frozen* grammar (Phase 0.4), LSP
  client, inline hot-reload status, "run on device" command.
- **`flux doctor`**: one command that checks toolchain versions, device/
  simulator availability, wire-protocol version match between dev server
  and any connected host apps, and stdlib parse-check status — RN's
  `react-native doctor` and Flutter's `flutter doctor` are both beloved for
  exactly this, and it's cheap to build.
- **`flux build` toolchain invocation** (carried from Phase 0.5) surfaced
  as a good CLI experience: progress, clear failure diagnostics distinguishing
  "your `.flux` code is wrong" from "your Xcode/Gradle setup is wrong."
- **Error overlay on-device** — when a VM-level runtime error occurs in dev
  mode, render a native (not webview) error screen with: the error message,
  the `.flux` source span highlighted, and a formatted stack through
  handler dispatch. This is the single most-loved Metro/Flutter DX feature
  and currently has zero equivalent in the dump.

---

## 6. Phase 4 — DevTools: Ship What's Scaffolded

`flux-devtools-ui` (gpui) already has the right module skeleton
(`time_travel`, `component_tree`, `signal_graph`, `timeline`, `vm_inspector`,
`wire_client`). Take each from scaffold to shipped:

- **Component tree**: click a node → jump to its `.flux` source line
  (needs the span-threading from Phase 0.3/3).
- **Signal graph**: live visualization of the SolidJS-style dependency
  graph, with "what wrote this signal" and "what reads it" — this is a
  feature neither RN nor Flutter has natively (Flutter's Riverpod/Provider
  devtools come closest), and it's a genuine differentiator given Flux's
  signal-graph-native architecture.
- **Timeline/flamegraph**: patch dispatch latency, VM instruction timing,
  dirty-reconciliation size per frame — feed it from the Phase 0.2 perf
  harness instrumentation so devtools and CI perf gates share one source of
  truth.
- **Time-travel**: scrub through signal-graph history and replay — the
  `time_travel/{buffer,reconstruct}` modules suggest this is designed but
  needs an end-to-end demo against a real running app before calling it done.
- **New: network inspector** (once Phase 2's HTTP capability exists) and a
  **structured log viewer** (tie into `tracing`, already a workspace dep).
- **Multi-device**: connect devtools to more than one running host
  simultaneously — needed the moment someone tests iOS+Android side by side.

---

## 7. Phase 5 — Performance: From "Should Be Fast" to "Proven Fast"

Building on the Phase 0.2 harness:

- Publish a public, reproducible benchmark comparing: cold start, hot-reload
  latency, large-list scroll performance (post `ScrollView`, Phase 2), and
  release-app binary size against equivalent RN and Flutter apps doing the
  same task. Native codegen with no runtime interpreter is your structural
  advantage on the last two — prove it with numbers, not the README table.
- VM dispatch hot-path audit in `flux-vm-ref`/host executors: this is the
  dev-mode critical path; even though release ships codegen, a slow dev VM
  directly degrades "10x DX" (nobody enjoys a laggy dev loop).
- Memory: arena/IR allocation patterns are already good Rust practice
  (AGENTS.md §2.1); extend the same audit to the Kotlin/Swift host runtimes,
  which don't get Rust's ownership guarantees for free.
- Add a perf regression bot on PRs (comment with before/after numbers from
  the harness) so performance work isn't a one-time push before 1.0 but a
  standing CI gate.

---

## 8. Phase 6 — Platform Capabilities: Beyond the MLP Five

Current: Camera, Storage, Router, Clipboard, Geolocation. For 90% coverage,
add (each following the existing `CALL_CAP` sync/async pattern and the
"denied grant → typed error, never a crash" contract):

- Push notifications (register/receive/handle-tap)
- Biometric auth (Face ID / fingerprint)
- Background tasks / app lifecycle hooks
- File system (beyond key-value `Storage`)
- Deep linking / universal links
- Device sensors as needed by common apps (accelerometer at minimum)
- **Native module escape hatch**: a documented, first-class way to wrap an
  arbitrary native SDK as a capability without waiting on the framework
  team — this is what actually gets you to "90%" rather than "the 20
  capabilities we thought of," and it's what RN's native modules and
  Flutter's platform channels both rely on for real-world adoption.

---

## 9. Phase 7 — Ecosystem & Production Concerns

- **Package manager for `.flux` components**: even a minimal registry +
  `flux add <pkg>` unlocks a community stdlib the core team doesn't have to
  build alone.
- **Testing framework for `.flux` apps**: component-level tests that run
  against the dev VM headlessly (mirrors `flux-parity`'s dev/release
  parity testing, but user-facing).
- **Crash reporting / error tracking integration** for release builds
  (Sentry-equivalent): since release is native codegen, this is "just" a
  Swift/Kotlin crash reporter integration, but it needs a story before 1.0
  — production apps without crash visibility don't get shipped by serious
  teams.
- **State management guidance**: the signal graph is your primitive; publish
  opinionated patterns (global stores, derived signals, async data-fetching
  patterns) so teams don't reinvent Redux-equivalents badly on top of it.
- **i18n for apps** (not just the docs site, which already has this):
  string externalization and locale-aware formatting as a stdlib/capability
  concern.

---

## 10. Phase 8 — Docs, Website, Community

- Convert the current concept docs (`dev-vs-release`, `host-authoritative-
  state`, `the-wire`) into a full guide set: getting started, cookbook per
  new stdlib primitive (Phase 2), migration guides *from* RN and Flutter
  (name the differences honestly — this is a credibility move, not just
  marketing), and a troubleshooting guide keyed to the `FluxError` taxonomy
  from Phase 0.3.
- Keep the i18n-drift checker in the loop as content grows — don't let es/
  docs silently rot behind en/.
- Build 2–3 substantial showcase apps (beyond `counter`/`router`) that
  exercise the Phase 2 stdlib end-to-end and double as living integration
  tests plus marketing assets.

---

## 11. Phase 9 — Beta, Hardening, 1.0 Cut

- Dogfood: the core team ships one real app on Flux before inviting
  outside users.
- Closed beta with a small set of external teams building real apps;
  track every DX friction point against the "10x" claim in §1 with
  actual time-to-fix data, not vibes.
- Freeze the wire protocol and adapter contract versions for 1.0 with an
  explicit backward-compatibility policy (semver already adopted per
  CHANGELOG.md — extend it to the wire/adapter contracts specifically,
  since those version independently of the crate versions today).
- Bug bash against the full stdlib + capability surface.
- Ship 1.0 only when the five criteria in §1 are met with evidence, not
  just when the feature checklist is full.

---

## 12. Suggested Lane Structure & Sequencing

Given the existing `LANE-A..I` parallel-agent convention and the
directory-collision merge guard, a natural mapping:

| Lane | Scope | Depends on |
|---|---|---|
| LANE-J | iOS/Android convergence decision + perf harness (Phase 0.1–0.2) | — (do first) |
| LANE-K | `FluxError` hardening + span-threading through wire (0.3) | — (parallel to J) |
| LANE-L | Grammar freeze + `flux fmt` (0.4, Phase 1) | — (parallel) |
| LANE-M | CI/build/security hardening (0.5–0.6) | — (parallel) |
| LANE-N | Stdlib expansion, primitive-by-primitive (Phase 2) | J (rendering model must be settled) |
| LANE-O | LSP + VS Code extension (Phase 3) | L (grammar frozen) |
| LANE-P | DevTools shipping pass (Phase 4) | K (spans), J (perf instrumentation) |
| LANE-Q | Capabilities expansion + native escape hatch (Phase 6) | K (error contract) |
| LANE-R | Docs/website/ecosystem (Phase 7–8) | rolling, behind N/Q |

Rough sequencing: **Phase 0 first and mostly serial** (it's genuinely
blocking); **Phases 1–6 run in parallel lanes** once Phase 0 exit criteria
are met; **Phases 7–9 layer on top continuously** rather than waiting for
everything above to finish.

---

## 13. Exit Metrics for 1.0 (make these dashboards, not prose)

- Render-perf budget (§3.10) met on both platforms, tracked in CI.
- Cold start, hot-reload latency, and release binary size published
  against RN/Flutter baselines.
- Stdlib primitive count and coverage against a defined "top 20 mobile UI
  patterns" checklist (target: all 20 covered without a WebView escape
  hatch).
- Zero open `unwrap`/`expect`/`panic!`/`!!`/`try!` outside tests (already a
  stated bar in AGENTS.md — turn it into a CI-enforced count of exactly 0).
- DevTools feature parity checklist against Flutter DevTools/RN DevTools,
  each item shipped and demoed, not scaffolded.
- Beta-tracked median time-to-diagnose and time-to-fix for a runtime error,
  compared against a matched RN/Flutter cohort.
