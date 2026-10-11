# Handoff — Flux bug-hunt rounds 86–105

**For:** the next agent continuing this session
**Repo:** `https://github.com/elcoosp/flux`
**Working dir:** `/Users/adm/Documents/Repos/flux`
**HEAD at handoff:** see `git log --oneline -1`
**Session duration:** rounds 86–105 (twenty deep rounds)

---

## 1. Mission

The user's standing goal remains: **keep hunting until rounds stop
producing real findings.** Rounds 86–105 produced **18 real fixes with
regression tests** and **35 documented gaps** filed as issues.

---

## 2. Real fixes landed (18)

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
  line comments.
- **F57** — `wire/span.rs::encode_str` wrote a u16 length prefix while
  `decode_str` read a version-dispatched prefix. **Production wire path.**
- **F59** — `HostAnnounceFrame::from_bytes` ignored its own version byte,
  so v2 HostAnnounce frames failed to decode. **Production wire path.**

## 3. Documented gaps (~35)

F16, F17, F25, F26, F28–F31, F33, F34, F38–F41, F43, F44, F47, F48,
F50–F52, F54–F56, F58.

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
round-105 commit.

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
9. **Python heredocs via `python3 -` do NOT auto-import `sys`.**
10. **`cargo test -p <crate>` runs doctests last.**
11. **`git add` + `git commit`** — always confirm with `git status`
    after a "committed" message.

---

## 7. Session history — rounds 86–105

| Round | Finding |
|---|---|
| 86 | F22–F24, F27, F32, F35 (fixes); F25, F26, F28–F31, F33, F34 (filed) |
| 87 | **F36**, **F37** (fixes); F38 |
| 88 | **F42** (fix); F39–F41, F43 |
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
| 102 | cross-check: no new asymmetries |
| 103 | F58 (named-record destructure binds whole record) |
| 104 | env.rs / checker/tests.rs clean; no findings |
| 105 | **F59** (fix; `HostAnnounceFrame` decoder ignored frame version) |

**Session owner:** the user
**Date of handoff:** 2026-10-11
**Convention reminder:** the user runs scripts as `./wr.sh`. Respond
with exactly one fenced `bash` block per turn. No explanatory prose
outside the block.
