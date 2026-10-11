# Handoff — Flux bug-hunt rounds 86–92

**For:** the next agent continuing this session
**Repo:** `https://github.com/elcoosp/flux`
**Working dir:** `/Users/adm/Documents/Repos/flux`
**HEAD at handoff:** see `git log --oneline -1`
**Session duration:** rounds 86–92 (seven deep rounds)

---

## 1. Mission

The user's standing goal remains: **keep hunting until rounds stop
producing real findings.** Rounds 86–92 produced **14 real fixes with
regression tests** and **24 documented gaps** filed as issues with
`#[ignore]`d or pin-the-current-behavior regression tests.

---

## 2. Real fixes landed (14)

- **F22** — `alignment`/`color`/`label` props dropped by codegen emitters.
- **F23** — `flux-lsp::semantic_tokens::comment_spans` did not track backslash escapes.
- **F24** — Release-path recognizers left codegen-escaped string content in the reduced tree.
- **F27** — `_` wildcard in `match` over ADT rejected by the type checker.
- **F32** — `Checkbox`/`TextArea` names did not round-trip through `normalize_view_name`.
- **F35** — `Picker`/`DatePicker` names did not round-trip either.
- **F36** — `flux-differ`'s `root_position` was O(n²) per call.
- **F37** — Dead `field_indices` map built by the type checker but never read.
- **F42** — `flux init` scaffold used Kotlin-style `${count}`.
- **F45** — `flux-lsp::did_close` did not invalidate the in-flight debounced publish task.
- **F46** — `validate_bytecode` read `COND_JUMP` target at the wrong offset
  (`+1` instead of `+2`) — this is on the **production frame-decode path**,
  so any handler with an `if`/`when`/`match` was rejected with a spurious
  `MalformedBytecode`. Highest-impact fix of the session.

## 3. Documented gaps (24)

F16, F17, F25, F26, F28, F29, F30, F31, F33, F34, F38, F39, F40, F41,
F43 (removed as fix), F44, F47, F48.

See `docs/issues/` for the full set. Notable ones:

- **F47** — `flux build` silently clobbers generated files on stem / component-name collision.
- **F48** — `flux doctor`'s `mod tests` detection is over-broad (`mod tests_impl`,
  `mod tests;` both suppress findings).
- **F41** — `set_signal_deps` only called from tests; minimal-patch mechanism is a prod no-op.
- **F39** — Dev-server `AsyncBridge` is unused scaffolding (hosts have their own resolver).

---

## 4. Current state

All crates and workspace tests green. `cargo test --workspace` reports
119 `test result: ok` groups and 0 failures. `cargo check --workspace`
clean.

Working tree clean; `HEAD` at the last round-92 commit.

---

## 5. Remaining work

### 5.1 Still not swept (or swept briefly)

- `crates/flux-types/src/checker/generics.rs`, `scope.rs` (partial), `tests.rs`.
- `crates/flux-types/src/checker.rs` — 10k lines; only `new()` + `CalleeShape`.
- `crates/flux-ir/src/lower/bytecode.rs` — many `compile_value` arms unread.
- `crates/flux-ir-serde/src/telemetry.rs` — encode path sampled.
- `crates/flux-devserver/src/pipeline.rs` — 1.6k lines; compile-tree path.
- `crates/flux-cli/src/doctor/dependency_drift.rs` — not deeply read.
- `crates/flux-cli/src/doctor/probe.rs` — not deeply read.
- `runtimes/ios/`, `runtimes/android/` — Swift / Kotlin sources.

### 5.2 Non-gating limitations (unchanged)

- `testSwitchThroughput` (iOS) 2.0s budget can flake.
- Kotlin `RenderPerfHarnessTest` p95 ≤ 3ms budget is a Release-mode target.

---

## 6. Critical gotchas

1. **Never `pkill -f` anything generic.**
2. **`xcodebuild -destination "platform=iOS Simulator,id=<UDID>"` hangs.**
3. **Never call `xcrun simctl bootstatus -b`.**
4. **Never modify the user's `$HOME` or shell rc files.**
5. **Swift VM has THREE exec paths.**
6. **`proto_version = 3` and `protocolVersionMin = 2`.**
7. **`docs/HANDOFF.md`** — refresh in a dedicated commit.
8. **Appending to `crates/flux-parity/tests/parity.rs`**: verify the F24
   test's closing `}` is present (missed once in round 86o, silently
   nesting every subsequent test).
9. **Python heredocs via `python3 -` do NOT auto-import `sys`.**
10. **`cargo test -p <crate>` runs doctests last.**
11. **Assertion tests must be run with `--ignored` once** to verify they
    describe the *current* behavior; when a fix lands, flip them.

---

## 7. Session history — rounds 86–92

| Round | Finding |
|---|---|
| 86 | F22, F23, F24, F27, F32, F35 (fixes); F25, F26, F28–F31, F33, F34 (filed) |
| 87 | **F36**, **F37** (fixes); F38 (filed) |
| 88 | **F42** (fix); F39, F40, F41, F43 |
| 89 | **F45** (fix); F44 |
| 90 | **F46** (fix; production `validate_bytecode` offset bug) |
| 91 | F47 (build output collisions, filed with pins) |
| 92 | F48 (doctor mod-tests prefix over-broad, filed with pins) |

**Session owner:** the user
**Date of handoff:** 2026-10-11
**Convention reminder:** the user runs scripts as `./wr.sh`. Respond
with exactly one fenced `bash` block per turn. No explanatory prose
outside the block.
