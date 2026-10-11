# Handoff — Flux bug-hunt rounds 86–90

**For:** the next agent continuing this session
**Repo:** `https://github.com/elcoosp/flux`
**Working dir:** `/Users/adm/Documents/Repos/flux`
**HEAD at handoff:** see `git log --oneline -1`
**Session duration:** rounds 86, 87, 88, 89, 90 (five deep rounds)

---

## 1. Mission

The user's standing goal remains: **keep hunting until rounds stop
producing real findings.** Rounds 86–90 produced **13 real fixes with
regression tests** and **20 documented gaps** filed as issues with
`#[ignore]`d regression tests (where a test is meaningful).

---

## 2. Real fixes landed (13)

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
  (`offset + 1` instead of `offset + 2`) on every **production frame
  decode** — any handler with an `if`/`when`/`match` would have been
  rejected with a spurious `MalformedBytecode`.

## 3. Documented gaps (20, all with `#[ignore]`d regression tests unless noted)

F16, F17 (earlier rounds), F25, F26, F28, F29, F30, F31, F33, F34,
F38, F39, F40, F41, F43 (removed as fix), F44.

See `docs/issues/` for the full set.

---

## 4. Current state — START HERE

### 4.1 All tests green

| Surface | Command | Result |
|---|---|---|
| Rust workspace | `cargo test --workspace` | green |
| All crates tested individually | green |
| Zero warnings | `cargo check --workspace` | none from our crates |

The Swift/Kotlin/Gradle surfaces were not re-run this session; no code
changed there.

### 4.2 Working tree

Clean; `HEAD` at the last round-90 commit.

---

## 5. Remaining work

### 5.1 Filed issues

F16, F17, F25, F26, F28–F31, F33, F34, F38–F41, F44 (see `docs/issues/`).

### 5.2 Still not swept (or swept briefly)

- `crates/flux-types/src/checker/generics.rs`, `scope.rs` (partial), `tests.rs`.
- `crates/flux-types/src/checker.rs` — 10k lines; only `new()` + `CalleeShape`.
- `crates/flux-ir/src/lower/bytecode.rs` — many `compile_value` arms.
- `crates/flux-ir-serde/src/telemetry.rs` — encode path sampled.
- `crates/flux-devserver/src/pipeline.rs` — 1.6k lines; compile-tree path.
- `crates/flux-cli/src/build.rs`, `doctor/`.
- `crates/flux-lsp/src/lib.rs::did_change` (partially swept; F45 fixed there).
- `runtimes/ios/`, `runtimes/android/` — Swift / Kotlin sources.

### 5.3 Non-gating limitations (unchanged)

- `testSwitchThroughput` (iOS) 2.0s budget can flake.
- Kotlin `RenderPerfHarnessTest` p95 ≤ 3ms budget is a Release-mode target.

---

## 6. Critical gotchas

1. **Never `pkill -f` anything generic.**
2. **`xcodebuild -destination "platform=iOS Simulator,id=<UDID>"` hangs.**
3. **Never call `xcrun simctl bootstatus -b`.**
4. **Never modify the user's `$HOME` or shell rc files.** Temp artifacts → `/tmp/`.
5. **Swift VM has THREE exec paths.**
6. **`proto_version = 3` and `protocolVersionMin = 2`.**
7. **`docs/HANDOFF.md`** — refresh in a dedicated commit.
8. **Appending to `crates/flux-parity/tests/parity.rs`**: verify the F24
   test's closing `}` is present. It has been missing once (round 86o),
   silently nesting every subsequent test. Rust warns but compiles.
9. **Python heredocs via `python3 -` do NOT auto-import `sys`.**
10. **`cargo test -p <crate>` runs doctests last.** Misspelled test file
    names silently run 0 tests and exit 0.
11. **Verify assertion tests once with `--ignored`** to make sure they
    still describe the *current* behavior; when a fix lands, remove the
    `#[ignore]` and flip the assertion.

---

## 7. Session history — rounds 86–90

| Round | Finding |
|---|---|
| 86 | F22, F23, F24, F27, F32, F35 (fixes); F25, F26, F28–F31, F33, F34 (filed) |
| 87 | F36, F37 (fixes); F38 (filed) |
| 88 | F42 (fix); F39, F40, F41, F43 (filed/removed) |
| 89 | F45 (fix); F44 (filed) |
| 90 | **F46** (fix — production-impacting `validate_bytecode` offset bug) |

**Session owner:** the user
**Date of handoff:** 2026-10-11
**Convention reminder:** the user runs scripts as `./wr.sh`. Respond
with exactly one fenced `bash` block per turn. No explanatory prose
outside the block.
