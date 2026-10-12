# Handoff — Flux bug-hunt rounds 86–111

**For:** the next agent continuing this session
**Repo:** `https://github.com/elcoosp/flux`
**Working dir:** `/Users/adm/Documents/Repos/flux`
**HEAD at handoff:** see `git log --oneline -1`
**Session duration:** rounds 86–111 (twenty-six deep rounds)

---

## 1. Mission

The user's standing goal remains: **keep hunting until rounds stop
producing real findings.** Rounds 86–111 produced **22 real fixes with
regression tests** and **42 documented gaps** filed as issues.

---

## 2. Real fixes landed (22)

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
- **F46** — `validate_bytecode` read `COND_JUMP` target at the wrong offset.
- **F49** — `flux doctor`'s stdlib parse-check reported `[ok]` when the script was not found.
- **F53** — Parser pre-scan counted brackets inside `//` line comments.
- **F57** — `wire/span.rs::encode_str` u16 vs version-dispatched prefix.
- **F59** — `HostAnnounceFrame::from_bytes` ignored its own version byte.
- **F60** — `record R { }` multi-line without commas rejected.
- **F63** — Handler-call codegen dropped named-arg labels (`f(n: 1)` → `f(1)`).
- **F65** — Nested call inside an outer call collapsed to the placeholder.
- Plus late follow-ups closing test bodies missing their closing braces
  (F24/F35/F58) and fixing the F30 assertion recalibration.

## 3. Documented gaps (~42)

F16, F17, F25, F26, F28–F31, F33, F34, F38–F41, F43, F44, F47, F48,
F50–F52, F54–F56, F58, F61, F62, F64.

Notable:

- **F50** — `flux doctor`'s `req_major` mis-models `>=` / `=` semver.
- **F51** — `ParseError::render` mis-aligns the caret for non-ASCII.
- **F52** — Generic-ADT field bindings do not substitute `T`.
- **F54** — Nested string in `{…}` interpolation cannot contain braces.
- **F55** — `flux fmt` silently drops every user comment (data loss).
- **F56** — `hash_fields` XOR-fold cancels duplicate `PropIdx` entries.
- **F58** — `let { x, y } = p` binds the whole record for named records.
- **F61** — List-separator grammar is inconsistent across productions.
- **F62** — Nominal record cannot be initialized with a `{ … }` literal.
- **F64** — Positional Flux call to a Swift function emits `f(1)` without
  the required label.

---

## 4. Current state

All workspace tests green. Working tree clean; `HEAD` at the last
round-111 commit.

---

## 5. Remaining work

### 5.1 Still not swept

- `crates/flux-ir/src/lower/bytecode.rs` — many `compile_value` arms.
- `crates/flux-devserver/src/pipeline.rs` — 1.6k lines.
- `runtimes/ios/`, `runtimes/android/` — Swift / Kotlin sources.
- `crates/flux-lsp/src/lib.rs::did_change` — partially swept.

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
8. **Brace-balance check after appending tests.** Rust warns
   `unnameable_test_items` but compiles silently on the wrong shape.
   Always verify with a script after appending:
   `python3 -c '...' # check brace balance and #[test]-at-depth-0`
9. **Python heredocs via `python3 -` do NOT auto-import `sys`.**
10. **`cargo test -p <crate>` runs doctests last.**
11. **`git add` + `git commit`** — always confirm with `git status`
    after a "committed" message.

---

## 7. Session history — rounds 86–111

| Round | Finding |
|---|---|
| 86 | F22–F24, F27, F32, F35 (fixes); F25, F26, F28–F31, F33, F34 |
| 87 | **F36**, **F37** (fixes); F38 |
| 88 | **F42** (fix); F39–F41, F43 |
| 89 | **F45** (fix); F44 |
| 90 | **F46** (fix) |
| 91 | F47 |
| 92 | F48 |
| 93 | **F49** (fix) |
| 94 | F50 |
| 95 | F51 |
| 96 | F52 |
| 97 | **F53** (fix); F54 |
| 98 | F55 |
| 99 | probes only |
| 100 | F56 |
| 101 | **F57** (fix) |
| 102 | cross-check |
| 103 | F58 |
| 104 | env.rs / checker/tests.rs clean |
| 105 | **F59** (fix) |
| 106 | **F60** (fix); F61 |
| 107 | emitter.rs clean |
| 108 | bytecode.rs list-method paths clean |
| 109 | F62 |
| 110 | **F63** (fix); F64 |
| 111 | **F65** (fix) |

**Session owner:** the user
**Date of handoff:** 2026-10-12
**Convention reminder:** the user runs scripts as `./wr.sh`. Respond
with exactly one fenced `bash` block per turn. No explanatory prose
outside the block.
