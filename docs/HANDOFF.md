# Handoff — Flux bug-hunt rounds 86–89 (full sweep)

**For:** the next agent continuing this session
**Repo:** `https://github.com/elcoosp/flux`
**Working dir:** `/Users/adm/Documents/Repos/flux`
**HEAD at handoff:** see `git log --oneline -1`
**Session duration:** rounds 86, 87, 88, 89 (four deep rounds)

---

## 1. Mission

The user's standing goal remains: **keep hunting until rounds stop
producing real findings.**

Round 86 was a whole-audit sweep across parity, LSP, devtools, differ,
VM. Round 87 focused on the differ and telemetry. Round 88 swept the
dev-server and CLI. Round 89 focused on `flux-perf-harness` and
`flux-lsp` deep reads. Between them, the sessions produced **11 real
fixes with regression tests** and **19 documented gaps** filed as
issues with `#[ignore]`d regression tests (where a test is meaningful).

---

## 2. Real fixes landed (11)

- **F22** — `alignment`/`color`/`label` props dropped by codegen
  emitters. Fix: emptied the dev reducer's `RECOGNISED` list.
- **F23** — `flux-lsp::semantic_tokens::comment_spans` did not track
  escapes (a string ending in `"\\"` hid every following `//` comment).
- **F24** — Release-path recognizers left codegen-escaped string
  content in the reduced tree; added
  `bridge::canonicalize_release_expr`.
- **F27** — `_` wildcard in `match` over ADT rejected by the type
  checker. Fix: treat `Variant { name: "_" }` as a wildcard in all
  three passes.
- **F32** — `Checkbox`/`TextArea` names did not round-trip through
  `normalize_view_name`.
- **F35** — `Picker`/`DatePicker` names did not round-trip either.
- **F36** — `flux-differ`'s `root_position` was O(n²) per call.
- **F37** — Dead `field_indices` map built by the type checker but
  never read by the emitter; removed entirely.
- **F42** — `flux init` scaffold used Kotlin-style `${count}` instead
  of Flux `{count}`.
- **F45** — `flux-lsp::did_close` did not invalidate an in-flight
  debounced publish task; stale non-empty diagnostics could arrive
  after close.

## 3. Documented gaps (19, all with `#[ignore]`d regression tests unless noted)

- **F16**, **F17** — from earlier rounds (allocation cap, crash map).
- **F25** — parity recognizers cannot recover a `match` node.
- **F26** — `batch { … }` / `untrack { … }` in a handler rejected at IR lowering.
- **F28** — `match`-arm bindings not in scope in the arm body.
- **F29** — assignment to a lambda parameter rejected at IR lowering.
- **F30** — generic trait bounds parsed but never enforced.
- **F31** — `xs[0]` subscript syntax silently splits into two exprs.
- **F33** — `Theme { … }` elided by both codegen backends.
- **F34** — `Gesture { … }` yields `Column` (Swift) / `Stack` (Kotlin).
- **F38** — lowercase `null` / Swift `nil` give a confusing "unbound
  name" error (should say `Null`).
- **F39** — dev-server `AsyncBridge::resume` has no production caller
  (hosts implement their own local resolver; unused scaffolding).
- **F40** — dispatch-report degradation is silent; `UnknownNode` is
  swallowed.
- **F41** — `set_signal_deps` only called from tests; the
  minimal-patch mechanism is a production no-op.
- **F43** — stale `crates/flux-cli/main.flux/` fixture removed (no
  test added; the deletion is the fix).
- **F44** — `MetricRecord::mean()` panics on sum overflow; no test
  added because the fix shape is a design call.

---

## 4. Current state — START HERE

### 4.1 All tests green

| Surface | Command | Result |
|---|---|---|
| Rust workspace | `cargo test --workspace` | green (EXIT=0, 119 groups) |
| Parity | `cargo test -p flux-parity` | green (21 passed, ~14 ignored) |
| Differ | `cargo test -p flux-differ` | green |
| Types | `cargo test -p flux-types` | green |
| Devserver | `cargo test -p flux-devserver` | green |
| CLI | `cargo test -p flux-cli` | green |
| LSP | `cargo test -p flux-lsp` | green |
| Perf | `cargo test -p flux-perf-harness` | green |
| Zero warnings | `cargo check --workspace` | none from our crates |

### 4.2 Working tree

Clean; `HEAD` at the last round-89 commit.

---

## 5. Remaining work

### 5.1 Filed issues

F16, F17, F22–F45 (see §2 and §3).

### 5.2 Still not swept (or swept briefly)

- `crates/flux-types/src/checker/generics.rs`, `scope.rs` (partial),
  `tests.rs` not line-by-line.
- `crates/flux-types/src/checker.rs` — 10k lines; only `new()` and
  `CalleeShape` read in depth.
- `crates/flux-ir/src/lower/bytecode.rs` — many `compile_value` arms
  not read line-by-line.
- `crates/flux-ir-serde/src/telemetry.rs` — encode path sampled;
  decode path complete but not exhaustively fuzzed.
- `crates/flux-devserver/src/pipeline.rs` — 1.6k lines; the
  compile tree path deserves a focused line-by-line read.
- `crates/flux-cli/src/build.rs` — 807 lines; test assertions read
  but toolchain-invocation paths deserve a security review.
- `crates/flux-cli/src/doctor/` — 1.2k lines; spot-read.
- `runtimes/ios/` and `runtimes/android/` — never read this session
  (Swift / Kotlin surfaces). The iOS `AsyncResolver` protocol was
  inspected enough to correct F39, but not fully.

### 5.3 Non-gating limitations (unchanged)

- `testSwitchThroughput` (iOS) 2.0s budget can flake.
- Kotlin `RenderPerfHarnessTest` p95 ≤ 3ms budget is a Release-mode
  target.

---

## 6. Critical gotchas

1. **Never `pkill -f` anything generic.**
2. **`xcodebuild -destination "platform=iOS Simulator,id=<UDID>"` hangs.**
   Use `name=<iPhone>&OS=<version>` or `-xctestrun`.
3. **Never call `xcrun simctl bootstatus -b`.**
4. **Never modify the user's `$HOME` or shell rc files.** Temp artifacts
   go in `/tmp/`.
5. **Swift VM has THREE exec paths.** Any opcode change must be applied
   to `run`, `runResumable`, and `runViaDispatchTable`.
6. **`proto_version = 3` and `protocolVersionMin = 2`.**
7. **`docs/HANDOFF.md`** — refresh in a dedicated commit.
8. **Appending to `crates/flux-parity/tests/parity.rs`**: verify the
   F24 test's closing `}` is present. It has been missing once (round
   86o), which silently nested every subsequent test inside it. Rust
   warns `function is never used` but compiles.
9. **Python heredocs via `python3 -` do NOT auto-import `sys`.** Add
   `import sys` before any `sys.exit(...)`.
10. **`cargo test -p <crate>` runs doctests last.** A misspelled test
    file name silently runs 0 tests and exits 0.
11. **Assertion tests must be run with `--ignored`** once to verify
    they still describe the *current* behavior; when a fix lands, the
    `#[ignore]` is removed and the assertion flipped.

---

## 7. Success criteria for the phase

1. All three VMs green.
2. Wire-fixture gate green.
3. Zero warnings from our crates.
4. Working tree clean (or only docs/ pending review).
5. Any change to one VM mirrored in the other two where applicable.

All of (1)–(5) hold as of this handoff.

---

## 8. Session history — rounds 86–89

| Round | Finding |
|---|---|
| 86a–86n | F22, F23, F24, F27, F32, F35 (fixes); F25, F26, F28–F31, F33, F34 (filed) |
| 86o–86p | F24 test body closure + F30 assertion recalibration |
| 87a–87f | **F36** (differ quadratic); **F37** (dead field_indices); F38 (lowercase null) |
| 88a–88c | F39 (async bridge unused); F40 (silent dispatch degradation); F41 (signal_deps unwired) |
| 88d | **F42** (scaffold \${count} → {count}) |
| 88e | F43 (stale main.flux fixture removed) |
| 89a | **F44** (filed; `mean()` overflow panic) |
| 89b | **F45** (real fix; LSP `did_close` invalidates debounced publish) |

**Session owner:** the user
**Date of handoff:** 2026-10-11
**Convention reminder:** the user runs scripts as `./wr.sh`. Respond
with exactly one fenced `bash` block per turn. No explanatory prose
outside the block.
