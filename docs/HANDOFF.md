# Handoff — Flux bug-hunt rounds 86–104

**For:** the next agent continuing this session
**Repo:** `https://github.com/elcoosp/flux`
**Working dir:** `/Users/adm/Documents/Repos/flux`
**HEAD at handoff:** see `git log --oneline -1`
**Session duration:** rounds 86–104 (nineteen deep rounds)

---

## 1. Mission

The user's standing goal remains: **keep hunting until rounds stop
producing real findings.** Rounds 86–104 produced **17 real fixes with
regression tests** and **33 documented gaps** filed as issues.

---

## 2. Real fixes landed (17)

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
  **Production frame-decode path.**
- **F49** — `flux doctor`'s stdlib parse-check reported `[ok]` when the
  parse-check script was not found.
- **F53** — `flux-parser`'s nesting pre-scan counted brackets inside `//`
  line comments, rejecting any file with 17+ unmatched brackets in prose.
- **F57** — `wire/span.rs::encode_str` wrote a u16 length prefix while
  `decode_str` read a version-dispatched (u32 at v3) prefix. Any
  `ClosureRef` carrying a `SourceExcerpt` desynced the decoder on v3.
  **Production wire path.**
- Late follow-ups: closed the F24/F35/F58 test bodies (missing braces)
  and fixed the F30 assertion.

## 3. Documented gaps (~33)

F16, F17, F25, F26, F28–F31, F33, F34, F38–F41, F43 (removed as fix),
F44, F47, F48, F50–F52, F54–F56, F58.

Notable:

- **F50** — `flux doctor`'s `req_major` mis-models `>=` / `=` semver.
- **F51** — `ParseError::render` mis-aligns the caret for non-ASCII source.
- **F52** — Generic-ADT field bindings do not substitute `T`.
- **F54** — Nested string in `{…}` interpolation cannot contain braces.
- **F55** — `flux fmt` silently drops every user comment (data loss).
- **F56** — `hash_fields` XOR-fold cancels duplicate `PropIdx` entries.
- **F58** — `let { x, y } = p` binds the whole record for named records.

---

## 4. Current state

All workspace tests green. Working tree clean; `HEAD` at the last
round-104 commit.

---

## 5. Corrections to earlier handoffs

- **`checker.rs` is 235 lines**, not "10k" as previously claimed. The
  large surface is the `checker/` submodule directory.
- **`flux-perf-harness` was never "unread"**; it was swept in round 89
  and its `MetricRecord::mean()` overflow issue is filed as F44.

---

## 6. Remaining work

### 6.1 Still not swept

- `crates/flux-ir/src/lower/bytecode.rs` — many `compile_value` arms.
- `crates/flux-ir-serde/src/telemetry.rs` — encode path sampled.
- `crates/flux-devserver/src/pipeline.rs` — 1.6k lines; compile-tree path.
- `runtimes/ios/`, `runtimes/android/` — Swift / Kotlin sources.
- `crates/flux-lsp/src/lib.rs::did_change` — partially swept.

### 6.2 Non-gating limitations (unchanged)

- `testSwitchThroughput` (iOS) 2.0s budget can flake.
- Kotlin `RenderPerfHarnessTest` p95 ≤ 3ms budget is a Release-mode target.

---

## 7. Critical gotchas

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
   `python3 -c "..." # check brace balance and #[test]-at-depth-0`
9. **Python heredocs via `python3 -` do NOT auto-import `sys`.**
10. **`cargo test -p <crate>` runs doctests last.**
11. **`git add` + `git commit`** — always confirm with `git status`
    after a "committed" message; the add can silently no-op.
12. **When a Python heredoc fails, the script continues with the OLD
    state.** No rollback. Check the intermediate state before assuming
    an atomic apply.

---

## 8. Session history — rounds 86–104

| Round | Finding |
|---|---|
| 86 | F22, F23, F24, F27, F32, F35 (fixes); F25, F26, F28–F31, F33, F34 (filed) |
| 87 | **F36**, **F37** (fixes); F38 |
| 88 | **F42** (fix); F39, F40, F41, F43 |
| 89 | **F45** (fix); F44 |
| 90 | **F46** (fix; production `validate_bytecode` offset) |
| 91 | F47 (build output collisions) |
| 92 | F48 (doctor mod-tests prefix over-broad) |
| 93 | **F49** (fix; doctor parse-check false [ok]) |
| 94 | F50 (doctor `req_major` mis-models `>=`/`=`) |
| 95 | F51 (parser caret misaligns for non-ASCII) |
| 96 | F52 (generic ADT pattern fields not substituted) |
| 97 | **F53** (fix; pre-scan counts comment brackets); F54 |
| 98 | F55 (fmt drops every comment) |
| 99 | — (probes only) |
| 100 | F56 (XOR-fold cancels duplicate PropIdx) |
| 101 | **F57** (fix; `wire/span.rs::encode_str` version mismatch) |
| 102 | cross-check: no new asymmetries found |
| 103 | F58 (named-record destructure binds whole record) |
| 104 | env.rs / checker/tests.rs clean; no new findings |

**Session owner:** the user
**Date of handoff:** 2026-10-11
**Convention reminder:** the user runs scripts as `./wr.sh`. Respond
with exactly one fenced `bash` block per turn. No explanatory prose
outside the block.
