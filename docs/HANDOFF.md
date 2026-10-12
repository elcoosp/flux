# Handoff — Flux bug-hunt rounds 86–116

**For:** the next agent continuing this session
**Repo:** `https://github.com/elcoosp/flux`
**Working dir:** `/Users/adm/Documents/Repos/flux`
**HEAD at handoff:** see `git log --oneline -1`
**Session duration:** rounds 86–116 (thirty-one deep rounds)
**User run convention:** every reply is exactly one fenced ```bash block, no prose outside. The user executes it via `./wr.sh`.

---

## 1. Mission

The user's standing goal: **keep hunting until rounds stop producing
real findings.** Rounds 86–116 produced **24 real fixes with regression
tests** and **~46 documented gaps** filed as issues (many with
`#[ignore]`d regression tests pinning the current behavior).

---

## 2. Real fixes landed this session (24)

Commits, each with a regression test or pin, in reverse-chronological
order. **Run `git log --oneline` to see the full sequence.**

- **F22** — `alignment`/`color`/`label` props dropped by codegen emitters.
- **F23** — `flux-lsp::semantic_tokens::comment_spans` did not track backslash escapes.
- **F24** — Release-path recognizers left codegen-escaped string content in the reduced tree.
- **F27** — `_` wildcard in `match` over ADT rejected by the type checker.
- **F32** — `Checkbox`/`TextArea` names did not round-trip through `normalize_view_name`.
- **F35** — `Picker`/`DatePicker` names did not round-trip either.
- **F36** — `flux-differ`'s `root_position` was O(n²) per call.
- **F37** — Dead `field_indices` map built by the type checker but never read.
- **F42** — `flux init` scaffold used Kotlin-style `${count}`.
- **F45** — `flux-lsp::did_close` did not invalidate the debounced publish task.
- **F46** — `validate_bytecode` read `COND_JUMP` target at wrong offset.
- **F49** — `flux doctor`'s stdlib parse-check reported `[ok]` when script missing.
- **F53** — Parser pre-scan counted brackets inside `//` line comments.
- **F57** — `wire/span.rs::encode_str` u16 vs version-dispatched prefix.
- **F59** — `HostAnnounceFrame::from_bytes` ignored its own version byte.
- **F60** — `record R { }` multi-line without commas rejected.
- **F63** — Handler-call codegen dropped named-arg labels (`f(n: 1)` → `f(1)`).
- **F65** — Nested call inside an outer call collapsed to the placeholder.
- **F66** — `referenced_strings_and_handlers` dropped `Patch::Reattach`'s node.
- **F69** — `flux doctor` scanned SwiftPM's `.build/` directory.
- Late follow-ups: closed missing test-body braces on F24, F35, F58, F62,
  F68; accepted the F65 follow-up snapshots; hoisted the F69 test to
  module level.

---

## 3. Documented gaps (~46, all under `docs/issues/`)

Each has an `#[ignore]`d regression test or a pin-the-current-behavior
test in `crates/flux-parity/tests/parity.rs` (or the relevant crate).
When a fix lands, un-ignore and flip the assertion.

Notable open issues:

- **F16** — Kotlin/Swift disagree on per-allocation cost.
- **F17** — `CrashReporter` does not resolve the top frame to a component id.
- **F25** — Parity recognizers cannot recover a `match` node in the view body.
- **F26** — `batch { … }` / `untrack { … }` in a handler rejected by IR lowering.
- **F28** — `match`-arm bindings not in scope in the arm body.
- **F29** — Assignment to a lambda parameter rejected at IR lowering.
- **F30** — Generic trait bounds parsed but never enforced.
- **F31** — `xs[0]` subscript silently splits into two expressions.
- **F33** — `Theme { … }` elided by both codegen backends.
- **F34** — `Gesture { … }` yields `Column` (Swift) / `Stack` (Kotlin).
- **F38** — Lowercase `null` / Swift `nil` give confusing "unbound name" error.
- **F39** — Dev-server `AsyncBridge::resume` has no production caller.
- **F40** — Dispatch-report degradation is silent; `UnknownNode` swallowed.
- **F41** — `set_signal_deps` only called from tests; minimal-patch mechanism is a prod no-op.
- **F44** — `MetricRecord::mean()` panics on sum overflow.
- **F47** — `flux build` silently clobbers generated files on stem/component-name collision.
- **F48** — `flux doctor`'s `mod tests` detection is over-broad.
- **F50** — `flux doctor`'s `req_major` mis-models `>=` / `=` semver.
- **F51** — `ParseError::render` mis-aligns the caret for non-ASCII.
- **F52** — Generic-ADT field bindings do not substitute `T`.
- **F54** — Nested string in `{…}` interpolation cannot contain braces.
- **F55** — `flux fmt` silently drops every user comment (**data loss**).
- **F56** — `hash_fields` XOR-fold cancels duplicate `PropIdx` entries.
- **F58** — `let { x, y } = p` binds the whole record for named records.
- **F61** — List-separator grammar is inconsistent across productions.
- **F62** — Nominal `record P { … }` cannot be initialized from a `{ … }` literal.
- **F64** — Positional Flux call to a Swift function emits `f(1)` without the required label.
- **F68** — Generic top-level `fn` is monomorphic across call sites.

---

## 4. Current repo state

| Surface | Command | Result |
|---|---|---|
| Rust workspace | `cargo test --workspace` | green (~131 test groups) |
| Zero warnings | `cargo check --workspace --all-targets` | none from our crates |
| Commits ahead | `git rev-list --count origin/main..HEAD` | ~211 |

Working tree clean.

---

## 5. Suggested next targets

### 5.1 Under-swept files
- `crates/flux-ir/src/lower/bytecode.rs` — many `compile_value` arms.
- `crates/flux-devserver/src/pipeline.rs` — 1.6k lines.
- `runtimes/ios/`, `runtimes/android/` — Swift / Kotlin sources
  (SwiftPM `.build` is now skipped by doctor, so scanning them for
  force-unwraps is meaningful).
- `crates/flux-lsp/src/lib.rs::did_change` — partially swept.

### 5.2 Lower-hanging classes of bug that have proven fruitful
- **Version-mismatched encode/decode pairs** — F57, F59 came from this.
  Grep for `u16_len_checked` vs `r.count` asymmetry in
  `crates/flux-ir-serde/src/wire/`.
- **Silent codegen placeholder fallbacks** — F63, F65 came from this.
  Any `unsupported_placeholder()` path in
  `crates/flux-codegen-*` is a candidate for a real rendering.
- **`flux doctor` false positives** — F48, F49, F69 all came from
  running `flux doctor --strict` and reading the output.
- **Grammar inconsistencies** — F60, F61 came from systematic probes
  of each list-like production.

### 5.3 Known structural traps
- **THREE `render_expr` implementations exist and must stay in sync**:
  - `crates/flux-codegen-core/src/expressions.rs` (codegen path)
  - `crates/flux-codegen-core/src/view_tree.rs` (release walker)
  - `crates/flux-parity/src/bridge.rs` (dev reducer)

  A change to one for parity reasons almost certainly needs the same
  change in the other two. The F65 follow-up is a worked example.

---

## 6. Critical gotchas (accumulated)

1. **Never `pkill -f` anything generic.**
2. **`xcodebuild -destination "platform=iOS Simulator,id=<UDID>"` hangs.** Use `name=<iPhone>&OS=<version>` or `-xctestrun`.
3. **Never call `xcrun simctl bootstatus -b`.** Use `simctl boot` (async) + a poll.
4. **Never modify the user's `$HOME` or shell rc files.** Temp artifacts go in `/tmp/`.
5. **Swift VM has THREE exec paths.** Any opcode change must be applied to `run`, `runResumable`, and `runViaDispatchTable`.
6. **`proto_version = 3` and `protocolVersionMin = 2`.** v2 frames are still readable via the version-dispatched `Reader::count`.
7. **Brace-balance check after every test append.** This bug bit us
   *six times* in a row (F24, F35, F58, F62, F68 — and the F65
   snapshot). Rust warns `unnameable_test_items` or `function is never
   used` but compiles silently on the wrong shape. Always verify with a
   script that checks `final depth == 0` AND that every `#[test]` is
   at depth 0.
8. **`timeout` command does not exist on macOS.** Use a poll loop
   with `kill -TERM`/`kill -KILL` around a backgrounded command.
9. **Python heredocs via `python3 -` do NOT auto-import `sys`.** Always
   `import sys` before any `sys.exit(...)`.
10. **`git add` + `git commit`** — always confirm with `git status`
    after a "committed" message; the add can silently no-op if the
    path was wrong.
11. **The user's wr.sh wrapper uses `set -uo pipefail`, NOT `-e`.**
    Errors inside a script never abort it; always check the step-by-step
    output for what actually landed.
12. **`docs/HANDOFF.md`** — refresh in a dedicated commit, not bundled
    with code changes.

---

## 7. Session history (rounds 86–116, abbreviated)

| Round | Highlights |
|---|---|
| 86 | F22–F24, F27, F32, F35 (fixes); F25, F26, F28–F31, F33, F34 (filed) |
| 87 | **F36**, **F37** (fixes); F38 |
| 88 | **F42** (fix); F39–F41, F43 |
| 89 | **F45** (fix); F44 |
| 90 | **F46** (fix) |
| 91–92 | F47, F48 |
| 93 | **F49** (fix) |
| 94–96 | F50, F51, F52 |
| 97 | **F53** (fix); F54 |
| 98–100 | F55, F56 |
| 101 | **F57** (fix) |
| 102 | cross-check |
| 103–104 | F58, env.rs clean |
| 105 | **F59** (fix) |
| 106 | **F60** (fix); F61 |
| 107–108 | emitter.rs, bytecode.rs list-method paths clean |
| 109–110 | F62, **F63** (fix); F64 |
| 111 | **F65** (fix) |
| 112 | **F66** (fix) |
| 113 | delta.rs clean; F67 speculatively filed and withdrawn |
| 114 | F68 |
| 115 | **F69** (fix) |
| 116 | (this handoff) |

---

## 8. How to run the session

- Emit **exactly one** fenced ```bash block per turn. No prose outside
  the block.
- Each script starts with `set -uo pipefail` (no `-e`).
- Log every operation with `echo "=== step N: …"`.
- Bound long-running commands with a poll loop, never a raw `timeout`.
- One commit per file (per logical change). Scope tags `fix(scope)`,
  `test(scope)`, `chore(scope)`, `docs(scope)`.
- At the end of a round, refresh `docs/HANDOFF.md` in its own commit.

**Success criteria for the phase** (all hold as of this handoff):

1. All three VMs green.
2. Wire-fixture gate green.
3. Zero warnings from our crates.
4. Working tree clean.
5. Any change to one VM mirrored in the other two where applicable.
