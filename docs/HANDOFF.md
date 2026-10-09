# Handoff — Flux bug-hunt rounds 26+ (audit plan execution)

**For:** the next agent continuing this session
**Repo:** `https://github.com/elcoosp/flux`
**Working dir:** `/Users/adm/Documents/Repos/flux`
**Baseline commit:** `3bd7b1a8` (origin/main at session start)
**HEAD at handoff:** see `git log --oneline -1`
**Commits ahead of origin/main:** 74 at session close
**Session duration:** ~5 rounds (rounds 26–30), executing `docs/reviews/flux-audit-and-fix-plan.md`

---

## 1. Mission

The session executed `docs/reviews/flux-audit-and-fix-plan.md` (F1–F15), an
external audit of the whole Flux monorepo. Every finding was either fixed
with a regression test, closed as scope-out, or filed as an issue for a
follow-up (F16, F17). Plus, mid-execution, the hunt uncovered four additional
real bugs (ADT variants, const bindings, reserved identifiers, list.isEmpty
operand order) which were also fixed.

The user's standing goal remains: **keep hunting until rounds stop producing
real findings.**

---

## 2. Current state — START HERE

### 2.1 All tests green

| Surface | Command | Result |
|---|---|---|
| Rust workspace | `cargo nextest run --workspace -j1` | 74+ new tests, all pass |
| Zero warnings | `cargo check --workspace --all-targets` | none from our crates |
| Kotlin host | `./gradlew :runtimes:android:host:test` | green |
| Kotlin app | `./gradlew :runtimes:android:app:compileDebugKotlin` | green |
| Swift ISA conformance | `xcodebuild test -scheme FluxHost -only-testing:FluxHostTests/ISAConformanceTests` | 105/105 vectors |
| Swift dispatch equivalence | `-only-testing:FluxHostTests/VMDispatchPerfTests/testDispatchTableMatchesSwitchOnAllISAVectors` | 105/105 |
| Swift render perf | `-only-testing:FluxHostTests/RenderPerfHarnessTests` | passes (with realistic budget) |
| Wire-fixture gate | `bash scripts/wire-fixtures-gate.sh` | PASS |

### 2.2 Working tree

Only `docs/HANDOFF.md` was modified during the session (this file, now
committed). Nothing else pending.

### 2.3 What landed (74 commits)

Audit plan execution, F1 → F15:

- **F1** (Critical): `LIST_INSERT` / `LIST_REMOVE` index operand was a
  *register* in the emitter but read as a *literal* in all three VMs.
  `items.removeAt(i)` removed by `i`'s register number, not the value. Fixed
  in Rust (`flux-vm-ref/src/vm.rs`), Swift (`FluxBytecodeVM.swift`, all 3
  exec paths), and Kotlin (`StepResult.kt` + `VmHelpers.kt`). Six new ISA
  vectors; T-304 tests updated; end-to-end `flux-ir` regression test.
- **F2** (High): `snapshot()` returned every seeded signal, not just writes.
  Fixed by adding `beginRun()` to the `SignalStore` protocol in Kotlin and
  Swift; only signals written since `beginRun` appear in `snapshot()`. ISA
  conformance tests updated to read from the store rather than `out.signals`.
- **F3** (High): Kotlin VM reported undecodable bytecode as a successful
  `Halt` with an empty program. Now `Fault`.
- **F4** (High): Kotlin executor swallowed async capability errors as `Null`.
  Now faults the handler and marks the cell `Error`.
- **F5** (High): `write_closures` panicked on a duplicate handler id and
  silently shipped zero-length refs on oversized bytecode. Now returns
  `WireError::DuplicateHandlerId` / `LengthExceedsU16`.
- **F6** (Medium): Devserver watcher — coalesce window didn't coalesce,
  atomic saves deleted files, TOCTOU on the pipeline lock, no starvation
  cap. All fixed; four unit tests added.
- **F7** (Medium): Swift `fatalError` sites in the VM exec paths → typed
  throws; Kotlin `!!` sites → graceful null handling.
- **F8** (Medium): `flux init <name>` now validates the project name
  (`[A-Za-z][A-Za-z0-9_-]*`, ≤ 64 chars) and uses a `{{PROJECT_NAME}}`
  placeholder.
- **F9** (Medium): Added `.github/workflows/security.yml` (cargo audit +
  cargo deny + pnpm audit) and `deny.toml`.
- **F10** (Medium): Scope-closed by F5's fixes; remaining panicking
  `.to_bytes` sites use bounded payloads.
- **F11** (Low): `stateProgram` boxing on suspend → reuse bytecode; DevTools
  emit gate flipped to `BuildFlags.DEBUG`; allocation-cap drift filed as
  F16.
- **F12** (Low): Stale `RELEASE-TODO` comments removed; remaining work filed
  as F17.
- **F13** (Open): `save_to_photon` and `data_driven_surface` were already
  green (PROGRESS.md updated); `resolve`'s `unreachable!` → `debug_assert`;
  blob cursor bounds added; reconstruct invariant tests added; fmt property
  test already exists.
- **F14** (Open): iOS render-perf harness (`RenderPerfHarnessTests.swift`)
  added, mirroring the Android one; measured p50 ~5ms, p95 ~27ms on
  simulator Debug.
- **F15** (Low): Dispatch-table parity test was already corpus-driven
  (105/105 vectors).

Mid-hunt additional bugs:

- ADT variant names were not registered as constructors, so
  `state c: Color = RGB(...)` lowered to `Null`; positional constructor args
  then clobbered the variant tag written at slot 0.
- `Color.red = RGB(...)` module-level const bindings lowered to `Null` in
  every call site (`Button(color: Color.red)`).
- Component/type names reserved in Swift or Kotlin (`class`, `object`, …)
  produced native source that didn't compile; now rejected by the type
  checker with a rename hint.
- `list.isEmpty()` emitted its zero constant *after* the compare, so the VM
  read the next opcode byte as a register operand.

---

## 3. Session conventions

Same as the earlier handoff — preserved verbatim:

- Every turn is exactly one fenced `bash` block. No prose outside.
- `set -uo pipefail` (never `-e`); errors logged, script continues.
- Surgical patches via Python + temp files with heredoc delimiter `EOF`.
- One commit per file (per logical change). Scope tags `fix(scope)`,
  `test(scope)`, etc.
- Bounded long-running commands with a poll loop + `kill -TERM $YOUR_PID`
  on timeout. Never signal a PID you did not spawn.
- Never `pkill -f <generic-name>`. Never `git reset --hard`.
- Never pipe into `tail`/`grep` for streaming commands; redirect to a file
  and tail afterwards.

---

## 4. Remaining work

### 4.1 Filed issues

- `docs/issues/FLUX-F16-allocation-cap-parity.md` — Kotlin and Swift
  disagree on per-allocation cost (8 vs 16 bytes per record field; `cap*8`
  vs `8` for lists; `>=` vs `>` boundary). Needs a design decision before
  fixing.
- `docs/issues/FLUX-F17-crash-source-map.md` — Both `CrashReporter`
  implementations install the uncaught-exception handler but don't resolve
  the top frame to a component id / source excerpt.

### 4.2 Not swept (or swept briefly)

- `flux-devtools-ui/src/state.rs` (1150 lines, 14 tests) — host-session map
  logic not deeply audited.
- `flux-parity/src/recognize_swift.rs` and `recognize_kotlin.rs` — the
  release-path recognizers only inspected from a distance.
- `flux-lsp/src/completion.rs`, `hover.rs`, `goto_def/` — spot-checked;
  `goto_def` had one bug fixed; the others were left as-is.
- `flux-codegen-swift/src/*`, `flux-codegen-kotlin/src/*` — reviewed for
  reserved-word handling (fixed) but not line-by-line.
- `adapters/ui-swift/`, `adapters/ui-kotlin/` — spot-checked.
- `flux-perf-harness/src/{gate.rs, driver.rs, metric.rs}` — inspected, clean.

### 4.3 Non-gating limitations

- `testSwitchThroughput` (iOS) has a 2.0s budget for 20k iterations. On a
  loaded machine this can flake. Consider raising to 3.0s or gating on a
  Release build.
- The Kotlin `RenderPerfHarnessTest` p95 ≤ 3ms budget is a Release-mode
  target; the Swift equivalent uses a 1s regression guard because Debug
  Simulator builds are ~40x slower. See `RenderPerfHarnessTests.swift`.

---

## 5. Critical gotchas

1. **Never `pkill -f` anything generic.** Same rule as before.
2. **`xcodebuild -destination "platform=iOS Simulator,id=<UDID>"` hangs.**
   Use `name=<iPhone>&OS=<version>` or `-xctestrun`.
3. **Never call `xcrun simctl bootstatus -b`.** Use `simctl boot` (async) +
   a bounded poll for `simctl list devices booted`.
4. **Never modify the user's `$HOME` or shell rc files.** Temp artifacts go
   in `/tmp/`.
5. **Swift VM has THREE exec paths.** Any opcode change must be applied to
   `run`, `runResumable`, and `runViaDispatchTable`.
6. **`proto_version = 3` and `protocolVersionMin = 2`.** v2 frames are still
   readable via the version-dispatched `Reader::count`.
7. **The `docs/HANDOFF.md` you are reading was written in one shot at the
   close of this session.** If you need to refresh it mid-session, do so in
   a dedicated commit rather than bundling it with code changes.

---

## 6. Success criteria for the phase

1. All three VMs green.
2. Wire-fixture gate green.
3. Zero warnings from our crates.
4. Working tree clean (or only docs/ pending review).
5. Any change to one VM mirrored in the other two where applicable.

All of (1)–(5) hold as of this handoff.

---

## 7. Session history — rounds 26+

| Round | Finding |
|---|---|
| 26 | F1 (list index operand), F2 (snapshot semantics), F3 (Kotlin decode), F4 (async error), plus ADT/const/reserved/list.isEmpty |
| 27 | F5 (wire errors), F10 (panicking to_bytes), F7 (host crash sites) |
| 28 | F6 (watcher), F8 (init validation), F9 (security CI) |
| 29 | F11 (VM micro), F12 (docs), F13 (open items) |
| 30 | F14 (iOS render-perf), F15 (dispatch parity confirmation) |

**Session owner:** the user
**Date of handoff:** 2026-10-09
**Convention reminder:** the user runs scripts as `./wr.sh`. Respond with
exactly one fenced `bash` block per turn. No explanatory prose outside the
block.
