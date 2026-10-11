# Handoff — Flux bug-hunt rounds 86–87 (audit + differ/telemetry sweeps)

**For:** the next agent continuing this session
**Repo:** `https://github.com/elcoosp/flux`
**Working dir:** `/Users/adm/Documents/Repos/flux`
**HEAD at handoff:** see `git log --oneline -1`
**Session duration:** rounds 86–87 (deep multi-round sweep)

---

## 1. Mission

The user's standing goal remains: **keep hunting until rounds stop
producing real findings.**

Round 86 executed a full audit sweep (parity, LSP, devtools, differ,
VM) and produced 6 real fixes + 8 documented gaps. Round 87 followed
up with a differ/telemetry sweep, produced 1 real fix (F36) and 2
documented gaps (F37, F38).

---

## 2. What landed

### 2.1 Real fixes with regression tests

- **F22** — `alignment`/`color`/`label` props dropped by the codegen
  emitters. Fix: emptied the dev reducer's `RECOGNISED` list so the
  reduced trees agree. Test: `container_props_do_not_break_parity`.
- **F23** — `flux-lsp::semantic_tokens::comment_spans` did not track
  backslash escapes (a string ending in `"\\"` hid every subsequent
  `//` comment). Test:
  `comment_after_escaped_backslash_string_is_detected` and companion.
- **F24** — release-path recognizers left codegen-escaped string
  content in the reduced tree. Fix: `bridge::canonicalize_release_expr`
  walks a composite fragment, unescaping embedded literals and
  collapsing the `unsupported` placeholder. Tests:
  `string_escapes_in_condition_do_not_break_parity`,
  `composite_condition_with_embedded_string_escapes`,
  `release_canonicalizer_*`.
- **F27** — `_` wildcard in a `match` over an ADT was rejected by the
  type checker with a misleading error. Fix: treat
  `Variant { name: "_" }` as a wildcard in the checker, the Swift
  emitter, and the Kotlin emitter.
- **F32** — `Checkbox` / `TextArea` primitive names did not round-trip
  through `normalize_view_name`. Fix: fold `Checkbox → Toggle` and
  `TextEditor` / `OutlinedTextField → TextArea`.
- **F35** — `Picker` / `DatePicker` did not round-trip either. Fix:
  fold `DropdownMenu → Picker` and `DatePickerDialog → DatePicker`.
- **F36** — `flux-differ`'s `root_position` was O(n²) per call
  (nested `all_ids()` walks) and called once per inserted root. Fix:
  precompute the ordered `new_roots` list once from `new_index`;
  both quadratic helpers removed.
- **F37** — `field_indices` map built by the type checker and threaded
  through `TypedAST` / `IrMetadata` was never read (the emitter keys
  `GET_FIELD`/`SET_FIELD` by `prop_index_for_name`, FNV-1a of the
  field name). Removed the entire dead plumbing.

### 2.2 Documented gaps (with `#[ignore]`d regression tests)

- **F25** — parity recognizers cannot recover a `match` node in the
  view body.
- **F26** — `batch { … }` / `untrack { … }` in a handler rejected at
  IR lowering.
- **F28** — `match`-arm pattern bindings not in scope in the arm body.
- **F29** — assignment to a lambda parameter rejected by IR lowering.
- **F30** — generic trait bounds are parsed but never enforced.
- **F31** — `xs[0]` subscript syntax silently splits into two
  expressions.
- **F33** — `Theme { … }` is elided by both codegen backends.
- **F34** — `Gesture { … }` yields `Column` (Swift) / `Stack` (Kotlin).
- **F38** — lowercase `null` / Swift `nil` give a confusing "unbound
  name" error (not the correct `Null` literal spelling).

### 2.3 Late structural fix (round 86o)

The F24 test's closing `}` was missing — every F25–F35 test added
after it was silently nested inside the F24 test body. Rust warns
`function is never used` but compiles. Two follow-up commits closed
the F24 body and removed the compensating stray trailing `}`.

---

## 3. Current state — START HERE

### 3.1 All tests green

| Surface | Command | Result |
|---|---|---|
| Rust workspace | `cargo test --workspace` | green |
| Parity (default) | `cargo test -p flux-parity` | green (21 passed, 12+ ignored) |
| Parity (forced ignored) | `cargo test -p flux-parity --test parity -- --ignored` | green |
| Differ | `cargo test -p flux-differ` | green |
| Types | `cargo test -p flux-types` | green |
| Zero warnings | `cargo check --workspace` | none from our crates |

The Swift/Kotlin/Gradle surfaces were not re-run this session; no code
changed there.

### 3.2 Working tree

Clean; `HEAD` at the last commit.

### 3.3 Files touched

- `crates/flux-parity/src/{reduce.rs, bridge.rs}`
- `crates/flux-parity/src/recognize_{swift,kotlin}/*`
- `crates/flux-parity/tests/{parity.rs, props_compare.rs}`
- `crates/flux-lsp/src/semantic_tokens.rs`
- `crates/flux-types/src/{lib.rs, checker.rs, checker/field.rs, checker/scope.rs}`
- `crates/flux-codegen-core/src/view_tree.rs`
- `crates/flux-codegen-{swift,kotlin}/src/backend_impl.rs`
- `crates/flux-differ/src/diff/{algorithm.rs, tree.rs}`
- `crates/flux-ir/src/lower/{mod.rs, bytecode.rs, mono.rs}`
- New issues: `docs/issues/FLUX-F22…md` through `FLUX-F38…md`

---

## 4. Remaining work

### 4.1 Filed issues (all have `#[ignore]`d regression tests)

- F16 / F17 (from earlier handoff) — see `docs/issues/`.
- F22–F38 as listed in §2.

### 4.2 Still not swept

- `crates/flux-types/src/checker/` — `infer.rs`, `use_resolution.rs`,
  `apply_callee.rs`, `field.rs`, `opt_field.rs`, `reserved.rs` read;
  `generics.rs`, `scope.rs` (partial), `tests.rs` not line-by-line.
- `crates/flux-ir-serde/src/telemetry.rs` — 1.3k lines; only
  `EnrichedTelemetryEvent` encode path sampled.
- `crates/flux-devserver/src/*` — partially swept.
- `crates/flux-devtools-ui/src/{app.rs, views/*}` — read, no new
  findings beyond the earlier comment cleanup.
- `crates/flux-codegen-swift/kotlin` — F32/F33/F34/F35 came from
  primitive-by-primitive probes; more emitters may still have
  un-round-tripped names.

### 4.3 Non-gating limitations (unchanged)

- `testSwitchThroughput` (iOS) 2.0s budget can flake.
- Kotlin `RenderPerfHarnessTest` p95 ≤ 3ms budget is a Release-mode
  target.

---

## 5. Critical gotchas

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
8. **When appending to `crates/flux-parity/tests/parity.rs`, verify the
   F24 test's closing `}` is present.** It has been missing once
   (round 86o), which silently nested every subsequent test inside it.
   Rust warns `function is never used` but does NOT fail.
9. **Python heredocs via `python3 -` do NOT auto-import `sys`.** Add
   `import sys` before any `sys.exit(...)`.
10. **`cargo test -p <crate>` runs doctests last.** A misspelled test
    file name silently runs 0 tests and exits 0.

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
| 86a | F22 (container props dropped; parity restored) |
| 86b | F23 (`comment_spans` escape bug) |
| 86c | F24 (release-side string escapes not unescaped) |
| 86d | F24 follow-up (composite-expression walker) |
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
| 87a | **F36** (real fix; differ root-position quadratic) |
| 87b | Probes (18 combos) — all pass parity or fail at parse |
| 87c | Probes (20 combos) — all pass parity |
| 87d | **F37** (real fix; dead field_indices map removed) |
| 87e | F38 (filed; lowercase null/nil DX gap) |
| 87f | Probes (20 combos) — no new findings |

**Session owner:** the user
**Date of handoff:** 2026-10-11
**Convention reminder:** the user runs scripts as `./wr.sh`. Respond
with exactly one fenced `bash` block per turn. No explanatory prose
outside the block.
