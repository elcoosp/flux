# Handoff — Flux bug-hunt rounds 26+ (audit plan execution, continued)

**For:** the next agent continuing this session
**Repo:** `https://github.com/elcoosp/flux`
**Working dir:** `/Users/adm/Documents/Repos/flux`
**Baseline commit:** `3bd7b1a8` (origin/main at session start)
**Commits ahead of origin/main:** see `git rev-list --count origin/main..HEAD`
**Session duration:** rounds 26–61

---

## 1. Mission

The session executed `docs/reviews/flux-audit-and-fix-plan.md` (F1–F15),
then kept hunting for real bugs across the un-swept areas of the monorepo
(devserver deep-read, parity recognizers, LSP providers, differ, codegen
backends, parser, wire codec, types), then landed the F16–F21 follow-ups.

The user's standing goal: **keep hunting until rounds stop producing real
findings.**

## 2. Current state — START HERE

### 2.1 All success criteria verified (rounds 59–61)

| Surface | Command | Result |
|---|---|---|
| Rust workspace check | `cargo check --workspace --all-targets` | clean (only `block v0.1.6` dep future-incompat) |
| Rust workspace clippy strict | `cargo clippy --workspace --all-targets -j1 -- -D warnings` | clean |
| Rust VM | `cargo test -p flux-vm-ref` | all pass |
| Wire-fixture gate | `bash scripts/wire-fixtures-gate.sh` | PASS (Rust + Kotlin agree) |
| iOS (Swift) | `cd runtimes/ios && xcodebuild test -scheme FluxApp -destination "id=<sim>"` | **47 passed, 2 skipped, 0 failures** |
| Kotlin host | `./gradlew :runtimes:android:host:test` | BUILD SUCCESSFUL |
| Kotlin app | `./gradlew :runtimes:android:app:compileDebugKotlin` | BUILD SUCCESSFUL |
| Working tree | `git status --short` | clean |

### 2.2 What landed — rounds 26–30 (F1–F15 audit plan)

Preserved from the earlier handoff; the audit plan's F1–F15 all landed with
regression tests: list-index operand, snapshot semantics, Kotlin decode,
async error, wire errors, watcher, init validation, security CI, etc.

### 2.3 What landed — rounds 31–44

Real bugs fixed:

- **Parity recognizer infinite-loop** — `parse_for_each`/`parse_items`
  swallowed `parse_body` errors via `unwrap_or_default()`, yielding
  `end = 0` to the caller which then re-ran the same token index forever.
- **Parser rejected spec §18.5 guard arms** — `s if s > 90 => "A"` failed
  at "expected `=>`, found `if`"; the whole `MatchPatternKind::Guard` path
  was unreachable. Fixed; guard round-trips through `flux fmt` too.
- **Guard-arm label dropped condition** — in `view_tree.rs` and
  `reduce.rs`, both produced just the binding name.
- **Handler-body calls replaced by placeholder** — `Auth.login("u")` in a
  handler emitted `/* unsupported expr */ 0`.
- **Kotlin/Swift guard codegen invalid** — Swift `case let .<name>(_)` (drop
  condition); Kotlin `is <name> ->` (invalid).
- **Differ reattach pairing nondeterministic** — `AHashSet::difference`
  iteration order decided which same-key candidate paired.
- **kotlinc/swiftc fixed-name temp collision** — per-PID scratch dirs.
- **debug_bridge hard-coded ports** — 17399/17400 raced with prior runs.

### 2.4 What landed — rounds 45–58 (F18–F21 follow-ups)

- **FLUX-F18 CLOSED** — the clippy lint gate
  (`#![warn(clippy::unwrap_used, clippy::expect_used, clippy::panic)]`
  + test-only allow) is landed in `flux-ir-serde` and `flux-devserver`.
  Required: a new `WireError::UnsupportedVariant` variant replacing 3
  `unreachable!()` sites; direct byte indexing replacing 3
  `try_into().unwrap()` reads in `frame.rs`; `validate_bytecode` reuses the
  pass-1 opcode; 25 panicking-shim sites annotated with scoped `#[allow]`
  and rationale.
- **FLUX-F19 CLOSED** — `await` as a bare handler statement was rejected
  by `compile_block_or_expr`. One arm added
  (`ExprKind::Await(_) => { let _ = self.compile_value(expr)?; Ok(()) }`),
  the codegen async path becomes reachable, both backends tested.
- **FLUX-F20 CLOSED** — Kotlin couldn't express guard arms. New
  `emit_match_with_guards` emits `run { val __flux_scrut = …; when { … } }`
  with `.let`-scoped bindings.
- **FLUX-F21 FILED** — `telemetry::encode_str/decode_str` are hard-coded
  `u16` even at v3 (ADR-0059 lists only `frame::encode_str` +
  `StringEntry.text`). Needs design decision.

### 2.5 All filed issues CLOSED (rounds 63–68)

- **FLUX-F16** CLOSED — canonical allocation cost model (8 bytes/slot)
  documented in `mlp-appendices.md` §E.3.1; Rust VM counter added (with
  `SuspendState.allocated_bytes`); Swift corrected (was `× 16` / flat `+8` /
  `>`); Kotlin was already correct; boundary ISA vector
  `tests/isa-vectors/memory_cap_boundary.json` added (Rust conformance
  106/106).
- **FLUX-F17** CLOSED — codegen source-map sidecar (`<stem>.flux.map.json`)
  emitted by `flux build`; both Swift and Kotlin `CrashReporter`s resolve
  top frames against it and populate `FluxError.excerpt`. Schema documented
  in `mlp-spec.md` §14.5.
- **FLUX-F21** CLOSED — ADR-0059 amended to include telemetry strings;
  `telemetry::encode_str`/`decode_str` are now version-dispatched (u32 at
  v3), with `Reader::version()` threading the frame version through nested
  sub-readers. `>u16::MAX` regression test added.

**No open issues in the audit plan or its follow-ups remain.**

## 3. Session conventions

Preserved verbatim:

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

## 4. Critical gotchas

1. **Never `pkill -f` anything generic.**
2. **`xcodebuild -destination "platform=iOS Simulator,id=<UDID>"` — the
   correct invocation for FluxHost's tests is the FluxApp scheme**:
   `cd runtimes/ios && xcodebuild test -scheme FluxApp -destination "id=<UDID>"`
   The FluxHost SwiftPM package's own test binary cannot be launched under
   macOS (UIKit unavailable) and the FluxHost scheme isn't configured for
   `test` from the project. Regenerate the project with `xcodegen generate`
   after pulling `project.yml`.
3. **Never call `xcrun simctl bootstatus -b`.** Use `simctl boot` (async)
   plus a bounded poll for `simctl list devices booted`. If the sim says
   "Booted" but won't launch the app (BSErrorCode RequestDenied), shutdown +
   warm-up resolves it — no need to erase.
4. **Never modify the user's `$HOME` or shell rc files.** Temp artifacts go
   in `/tmp/`.
5. **Swift VM has THREE exec paths.** Any opcode change must be applied to
   `run`, `runResumable`, and `runViaDispatchTable`.
6. **`proto_version = 3` and `protocolVersionMin = 2`.** v2 frames are
   still readable via the version-dispatched `Reader::count`.
7. **`docs/HANDOFF.md` is written in one shot.** Refresh in a dedicated
   commit.

## 5. Not-yet-deeply-audited areas

- `flux-cli/src/doctor/*` (agents_rules.rs, dependency_drift.rs, probe.rs)
- `flux-types/src/checker/{apply_callee,call,field,generics,infer,opt_field,reserved,scope,use_resolution}.rs` — only `unify` + `exhaust` read clean
- `flux-types/src/capabilities/*`
- `adapters/ui-swift/*.swift` and `adapters/ui-kotlin/*.kt`
- `flux-lsp/src/{completion,hover,goto_def}` — read clean
- `flux-devtools-ui/src/state.rs` — read clean
- `flux-parity/src/recognize_{swift,kotlin}.rs` — two bugs found + fixed

## 6. Recommended next steps

1. Deep-read the crates in §5 above; highest-yield targets are
   `flux-cli/src/doctor/` and `flux-types/src/checker/`.
2. Decide FLUX-F21 (extend ADR-0059 to telemetry strings?); low-risk fix
   either way.
3. Product decision on FLUX-F16 (canonical per-allocation cost model) and
   FLUX-F17 (embed source map in release builds).

## 7. Session history — rounds 26+

| Round | Finding |
|---|---|
| 26 | F1 (list index operand), F2 (snapshot semantics), F3 (Kotlin decode), F4 (async error), ADT/const/reserved/list.isEmpty |
| 27 | F5 (wire errors), F10 (panicking to_bytes), F7 (host crash sites) |
| 28 | F6 (watcher), F8 (init validation), F9 (security CI) |
| 29 | F11 (VM micro), F12 (docs), F13 (open items) |
| 30 | F14 (iOS render-perf), F15 (dispatch parity) |
| 31 | parity recognizer `unwrap_or_default()` infinite-loop |
| 32 | parser rejected spec §18.5 guard arms |
| 33 | guard-arm label dropped condition (codegen + parity) |
| 34 | handler-body calls replaced by placeholder; filed FLUX-F19 |
| 35 | Kotlin/Swift guard arms emit invalid source; filed FLUX-F20 |
| 36 | devserver deep-read — no new bugs |
| 37 | differ reattach pairing nondeterministic |
| 38 | kotlinc/swiftc fixed-name temp collision |
| 39 | devserver async_bridge/assets/server — clean |
| 40 | flux-cli subcommands — clean |
| 41 | wire frame.rs deep-read — F18 remains |
| 42 | guard round-trip through `flux fmt` — green |
| 43 | flux-types unify/exhaust — clean |
| 44 | debug_bridge fixed-port flake |
| 45 | handoff refresh (rounds 31–44) |
| 46 | session close + summary |
| 47 | `WireError::UnsupportedVariant` replaces 3 `unreachable!()` |
| 48 | `validate_bytecode` reuses pass-1 opcode (no expect) |
| 49 | telemetry v3 string widen attempted — reverted (encoder/decoder must change atomically); filed FLUX-F21 |
| 50 | frame.rs reads use direct byte indexing |
| 51 | F18 gate added to flux-ir-serde + 20 annotation sites + FLUX-F21 filed |
| 52 | F18 gate added to flux-devserver + 401-builder annotation |
| 53 | parity WireErrorKind handles UnsupportedVariant |
| 54 | (renumbered) final F18 gate commits |
| 55 | F18 marked CLOSED |
| 56 | F19 fix (bare `await` statement arm) |
| 57 | F19 regression tests + marked CLOSED |
| 58 | F20 fix (Kotlin guard chain) + marked CLOSED |
| 59 | re-verify VM + wire-fixture gate |
| 60 | Swift tests via FluxApp scheme: 47 passed / 2 skipped / 0 failures |
| 61 | Kotlin host + app: BUILD SUCCESSFUL |
| 62 | handoff refresh (rounds 45–61) |
| 63 | F21: ADR-0059 amended; telemetry strings widen at v3 (both sides atomic) |
| 64 | F16: canonical cost model decision (8 bytes/slot) |
| 65 | F16: Swift VM corrected (×16→×8, cap×8, `>=`) |
| 66 | F16: Rust counter + SuspendState field + boundary ISA vector; Rust 106/106 |
| 67 | F17: sidecar design (source-map emitted by `flux build`) |
| 68 | F17: sidecar + `write_source_map` + both CrashReporters; CLI regression test; F17 CLOSED |

**Session owner:** the user
**Convention reminder:** the user runs scripts as `./wr.sh`. Respond with
exactly one fenced `bash` block per turn. No explanatory prose outside the
block.
