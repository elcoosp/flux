# Flux Follow-up Index — deferred PRD work → concrete issues

This file closes the loop on every "Deferred (documented follow-up)" /
"Blocker" / "remain follow-up" bullet in `CHANGELOG.md` (and the partial
`FLUX-011` / unlanded async host halves). Each deferred item now has its own
`FLUX-0XX` issue under `docs/issues/` with a `source:` pointer back here, so
there is **no dangling follow-up** — every deferred line resolves to a tracked
issue, and the LSP work (PRD-O's deferred server) is promoted into its own
`flux-lsp` crate built on `async-lsp`.

## LSP / editor (PRD-O deferred — the `flux-lsp` crate)

| CHANGELOG deferred (§PRD-O) | Issue | Notes |
|---|---|---|
| real LSP server (split from `flux-cli` JSON emitter) | FLUX-024 | new `crates/flux-lsp` on `async-lsp` (^0.2.4) + `lsp-types` (^0.97); manifest request filed |
| `flux lsp` type-checking (needs `flux-types`) | FLUX-025 | extends CLI + server pipeline |
| VS Code extension (highlight + LSP client + hot-reload + run on device) | FLUX-026 | thin client over `flux-lsp` |
| go-to-def / hover / completion | FLUX-027 | over compiler symbol data |
| native on-device error overlay (PRD-K `FluxError` + `Span`) | FLUX-028 | host screen, never webview |
| incremental `didChange` + debounced re-analysis | FLUX-029 | async-lsp server behaviour |

## Docs / ecosystem (PRD-R deferred)

| CHANGELOG deferred (§PRD-R) | Issue |
|---|---|
| docs website + en/es i18n-drift checker | FLUX-030 |
| getting-started / cookbook guide set | FLUX-031 |
| RN / Flutter migration guides | FLUX-032 |
| troubleshooting guide keyed to `FluxError` | FLUX-033 |
| headless `.flux` app testing framework (reuse `flux-parity`) | FLUX-034 |
| release crash reporting (Swift/Kotlin) | FLUX-035 |
| state-management patterns + app i18n + showcase apps | FLUX-036 |

## Stdlib (PRD-N deferred — `ScrollView` was the template)

> Reconciliation note (2026-08-30): the stdlib `.flux` sources + Android adapter
> coverage for FLUX-037/038/042/043/044 have since landed; several of these issues
> were relabeled `done`/`partial` in their own frontmatter. The remaining real
> stdlib gaps are iOS adapter parity for those primitives + ScrollView/List
> (FLUX-056, blocked) + form (FLUX-040) + gesture (FLUX-041) primitives on both
> platforms. Trust each issue's `status:` field over this pointer table.

| CHANGELOG deferred (§PRD-N) | Issue |
|---|---|
| `Stack`/`Grid`/`Spacer`/`SafeArea` | FLUX-037 |
| `Modal`/`Sheet`/`Dialog` | FLUX-038 |
| `Image` local + remote caching | FLUX-039 |
| form primitives (`Switch`/`Checkbox`/`Slider`/`Picker`/`DatePicker`/`TextArea`) | FLUX-040 |
| gestures (long-press / swipe / drag / pinch) | FLUX-041 |
| signal-graph animation primitive | FLUX-042 |
| design-token theming (codegen into SwiftUI/Compose) | FLUX-043 |
| a11y props through the adapter contract | FLUX-044 |

## Capabilities (PRD-Q deferred — contract locked)

| CHANGELOG deferred (§PRD-Q / roadmap §4/§6/§8) | Issue |
|---|---|
| six concrete capabilities (push/biometric/background/fs/deep-link/sensors) | FLUX-045 |
| user-facing native-module escape hatch | FLUX-046 |
| HTTP fetch/JSON + structured persistence | FLUX-047 |
| WebView escape hatch | FLUX-048 |
| permission gate + CALL_CAP threat model | FLUX-049 |
| production update-integrity story (ADR-0050) | FLUX-050 |

## Language maturity (PRD-S deferred — ADR-gated)

| CHANGELOG deferred (§PRD-S) | Issue |
|---|---|
| list-comprehension / iteration syntax | FLUX-051 |
| slot/children composition for containers | FLUX-052 |
| nullable / optional chaining ergonomics | FLUX-053 |
| structural vs nominal prop typing | FLUX-054 |
| in-language `Result` / error propagation | FLUX-055 |

## Performance (PRD-T deferred)

| CHANGELOG deferred (§PRD-T) | Issue |
|---|---|
| large-list scroll benchmark (needs `ScrollView`) | FLUX-056 |
| RN/Flutter published comparison — web-research only (no external apps built in repo) | FLUX-057 `partial` |

## DevTools (PRD-P deferred)

| CHANGELOG deferred (§PRD-P) | Issue |
|---|---|
| signal-graph dependency-edge rendering | FLUX-058 |
| timeline / flamegraph from PRD-J `MetricRecord` | FLUX-059 |
| network inspector + structured log viewer | FLUX-060 |
| multi-device connect | FLUX-061 |
| on-device verification evidence ("ship it, not scaffold it") | FLUX-062 |

## Correctness / architecture blockers (not under a PRD "deferred" header)

| CHANGELOG location | Issue | Why |
|---|---|---|
| §FLUX-011 PARTIAL — 6/10 B.3 fail at `flux-ir` lowering (`unsupported handler operand/expression`) | FLUX-063 | lowering fix, not codegen; gates 10/10 parity |
| §Roadmap Phase 2 async wire — host `resume` call sites MERGED; codegen `Task`/`suspend` emission still unlanded | FLUX-064 | release path cannot yet suspend on a real capability; sync/async decision is FLUX-070 |
| AGENTS.md §0.2 Axis 2 — iOS not converged to declarative tier | FLUX-065 | blocking Phase 0 decision (ADR-0048 Phase 0/1) |
| AGENTS.md §0.2 — no on-device render-perf test on either platform | FLUX-066 | CI-gated on-device §3.10 harness |
| roadmap §0.5 — mutation testing + toolchain compat matrix | FLUX-067 | `cargo-mutants` is a CI binary, not a manifest dep |
| roadmap §0.5/§11 + ADR-0036 — `flux build` invoke toolchain + distribution artifacts | FLUX-068 | AAR/xcframework + embed guide |
| §PRD-U deferred — dogfood + closed beta + bug bash evidence | FLUX-069 | the 1.0 evidence gates |

## Counters

- Issues created: **46** (FLUX-024 … FLUX-069).
- Manifest requests filed: `flux-lsp` (new crate) + `async-lsp` + `lsp-types`
  (FLUX-024/025) in `MANIFEST_REQUESTS.md`.
- No code changed; these are planning artifacts only. Each issue is self-contained
  (problem / solution / decisions / tests / out-of-scope) and carries `lane`,
  `phase`, `blocked_by`, and `labels` following the existing PRD-* frontmatter
  convention, so they drop straight into the parallel-agent dispatch model.
