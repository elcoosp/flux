# Handoff — Flux bug-hunt round 87 (differ sweep + probes)

**For:** the next agent continuing this session
**Repo:** `https://github.com/elcoosp/flux`
**Working dir:** `/Users/adm/Documents/Repos/flux`
**HEAD at handoff:** see `git log --oneline -1`
**Session duration:** round 87 (one real fix; many probes clean)

---

## 1. Mission

The user's standing goal remains: **keep hunting until rounds stop
producing real findings.** Round 87 continued the sweep begun in round
86, focusing on `flux-differ`'s internals (flagged as under-audited in
the previous handoff), plus a broad set of primitive / router / form
probes.

The round produced **one real fix** (F36) and no new documented gaps.
The F25–F35 documented gaps from round 86 remain filed and `#[ignore]`d.

---

## 2. What landed this round (round 87)

### 2.1 Fixed with regression tests

- **F36** — `crates/flux-differ/src/diff/tree.rs::root_position` was
  O(n²) per call (nested `all_ids()` walks), and
  `is_root_of_new` was O(n) per inserted root. `algorithm.rs::diff`
  called both per inserted root, so a hot reload that adds k top-level
  components to an n-node arena cost O(k·n²). Fix: precompute the
  ordered `new_roots` list once from the already-built `new_index`
  parent map (FLUX-079) before the insert loop; the loop reads its
  position in O(r). Both helpers removed. Regression coverage is via
  the existing `flux-differ` test suite (round-trip + multi-root
  determinism tests).

### 2.2 Probes that found nothing

- Round-87a: 20 form-and-container combos (router + navigate, modal
  with content and dismiss, two-way text input, `SafeArea`,
  `ScrollView` + `Column`, `ForEach` in `ScrollView`, nested
  `Column`/`Row`, `Animate` with a signal) — all pass parity.
- Round-87b: 18 more (router single / multi / nested screen,
  `ForEach` with `key: fn(x) { x }` and `fn(t) { t.id }`, `Spacer`
  with/without children, `Image` with alt, `Text` with style/color,
  deeply nested containers, `if` inside `Row`, `TextInput` with a
  handler, `Animate` with a curve, modal-in-column, inline screen
  content, `ForEach` inside a screen) — all pass parity, except two
  cases that fail at *parse* (not a divergence):
  * `props { … }` block is not `props` syntax; the parser rejects it.
  * `type T = { id: Int, name: String }` is a record *declaration*
    syntax the parser rejects — the correct spelling is `type P = { x:
    Int, y: Int }` at the top level (already an F33-adjacent issue, see
    `docs/issues/`), or `record P { … }` (a different syntax that also
    does not currently parse — a probe hit this in round 86).

---

## 3. Current state — START HERE

### 3.1 All tests green

| Surface | Command | Result |
|---|---|---|
| Rust workspace | `cargo test --workspace` | green (EXIT=0 in round 86's final run) |
| Parity (default) | `cargo test -p flux-parity` | green (21 passed, 9 ignored) |
| Parity (forced ignored) | `cargo test -p flux-parity --test parity -- --ignored` | green (9 passed) |
| Differ | `cargo test -p flux-differ` | green |
| Zero warnings | `cargo check --workspace` | none from our crates |

The Swift/Kotlin/Gradle surfaces were not re-run this round; no code
changed in those directories.

### 3.2 Working tree

Clean; `HEAD` at the last round-87 commit.

### 3.3 Files touched this round

- `crates/flux-differ/src/diff/algorithm.rs` (insert loop rewritten)
- `crates/flux-differ/src/diff/tree.rs` (quadratic helpers removed)
- New issue: `docs/issues/FLUX-F36-differ-root-position-quadratic.md`

---

## 4. Remaining work

### 4.1 Filed issues (all have `#[ignore]`d regression tests in
`crates/flux-parity/tests/parity.rs`)

- `docs/issues/FLUX-F16-allocation-cap-parity.md` — Kotlin/Swift
  disagree on per-allocation cost.
- `docs/issues/FLUX-F17-crash-source-map.md` — `CrashReporter` does
  not resolve the top frame to a component id / source excerpt.
- F22–F35 (from round 86) — see the previous handoff section 2.2.

### 4.2 Still not swept (or swept briefly)

- `crates/flux-types/src/checker/` — 2000 lines; `infer.rs`,
  `use_resolution.rs`, `apply_callee.rs`, `field.rs`, `opt_field.rs`,
  `reserved.rs` read; `generics.rs`, `scope.rs` (partially),
  `tests.rs` not line-by-line.
- `crates/flux-types/src/checker.rs` — 10k lines; only `new()` and
  `CalleeShape` read.
- `crates/flux-ir/src/lower/bytecode.rs` — 2.8k lines; `compile_call`,
  `emit_call_cap`, `narrow_count`, `jump_placeholder` read but more
  `compile_value` arms not line-by-line.
- `crates/flux-ir-serde/src/telemetry.rs` — 1.3k lines.
- `crates/flux-devserver/src/*` — partially swept earlier.
- `crates/flux-codegen-swift/src/backend_impl.rs` and
  `crates/flux-codegen-kotlin/src/backend_impl.rs` — F32/F33/F34/F35
  came from primitive-by-primitive probes; more emitters may still
  have un-round-tripped names.

### 4.3 Non-gating limitations (unchanged)

- `testSwitchThroughput` (iOS) 2.0s budget can flake on a loaded
  machine.
- Kotlin `RenderPerfHarnessTest` p95 ≤ 3ms budget is a Release-mode
  target.

---

## 5. Critical gotchas

1. **Never `pkill -f` anything generic.**
2. **`xcodebuild -destination "platform=iOS Simulator,id=<UDID>"` hangs.**
   Use `name=<iPhone>&OS=<version>` or `-xctestrun`.
3. **Never call `xcrun simctl bootstatus -b`.** Use `simctl boot` (async)
   + a bounded poll for `simctl list devices booted`.
4. **Never modify the user's `$HOME` or shell rc files.** Temp artifacts
   go in `/tmp/`.
5. **Swift VM has THREE exec paths.** Any opcode change must be applied
   to `run`, `runResumable`, and `runViaDispatchTable`.
6. **`proto_version = 3` and `protocolVersionMin = 2`.** v2 frames are
   still readable via the version-dispatched `Reader::count`.
7. **`docs/HANDOFF.md`** — refresh in a dedicated commit, not bundled
   with code changes.
8. **When you append a new test to `crates/flux-parity/tests/parity.rs`,
   double-check the F24 test's closing `}` is present.** It has been
   missing once (round 86o), which silently nested every subsequent
   test inside it. Rust warns (`function is never used`) but compiles.
9. **Python heredocs via `python3 -` do NOT auto-import `sys`.** Add
   `import sys` before any `sys.exit(...)` or the snippet `NameError`s
   mid-script and skips its remaining work (this bit us twice in
   round 86).
10. **`cargo test -p <crate>` on the workspace runs the doctests last.**
    If a Python-injected test is misspelled (`cargo test --test foo`
    where `foo.rs` was not created), cargo silently runs 0 tests and
    exits 0 — verify the test name appears in the output.

---

## 6. Success criteria for the phase

1. All three VMs green.
2. Wire-fixture gate green.
3. Zero warnings from our crates.
4. Working tree clean (or only docs/ pending review).
5. Any change to one VM mirrored in the other two where applicable.

All of (1)–(5) hold as of this handoff.

---

## 7. Session history — rounds 86–87

| Round | Finding |
|---|---|
| 86a | F22 (container props dropped; parity green after emptying RECOGNISED) |
| 86b | F23 (`comment_spans` escape bug) |
| 86c | F24 (release-side string escapes not unescaped) |
| 86d | F24 follow-up (composite-expression walker; placeholder collapse) |
| 86e | F25 (filed; `match` recognizer gap) |
| 86f | F26 (filed; `batch`/`untrack` in handler) |
| 86g | **F27** (real fix; `_` wildcard over ADTs) |
| 86h | F28 (filed; match-arm bindings) |
| 86i | F29 (filed; assignment to non-signal) |
| 86j | F30 (filed; trait bounds unenforced) |
| 86k | F31 (filed; subscript silently splits) |
| 86l | **F32** (real fix; Checkbox/TextArea normalization) |
| 86m | F33, F34 (filed; Theme flatten, Gesture divergence) |
| 86n | **F35** (real fix; Picker/DatePicker normalization) |
| 86o | F24 test body closure + trailing-brace cleanup |
| 86p | F30 bound-violated test now asserts the actual (lowering) error |
| 87a | F36 (real fix; differ root-position quadratic) |
| 87b | Probes (18 combos) — all pass parity or fail at parse |
| 87c | Probes (20 combos) — all pass parity |

**Session owner:** the user
**Date of handoff:** 2026-10-11
**Convention reminder:** the user runs scripts as `./wr.sh`. Respond
with exactly one fenced `bash` block per turn. No explanatory prose
outside the block.
