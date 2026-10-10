# Handoff — Flux bug-hunt rounds 26+ (audit plan execution, continued)

**For:** the next agent continuing this session
**Repo:** `https://github.com/elcoosp/flux`
**Working dir:** `/Users/adm/Documents/Repos/flux`
**Baseline commit:** `3bd7b1a8` (origin/main at session start)
**Commits ahead of origin/main:** see `git rev-list --count origin/main..HEAD`
**Session duration:** rounds 26–44

---

## 1. Mission

The session executed `docs/reviews/flux-audit-and-fix-plan.md` (F1–F15),
then kept hunting for real bugs across the un-swept areas of the monorepo
(devserver deep-read, parity recognizers, LSP providers, differ, codegen
backends, parser, wire codec, types). Every finding was fixed with a
regression test or filed as a follow-up issue (F16–F20).

The user's standing goal: **keep hunting until rounds stop producing real
findings.**

## 2. What landed this session (rounds 32–44)

### 2.1 Real bugs fixed

- **Parser guard arms** (round 32): `s if s > 90 => "A"` — the documented
  §18.5 syntax — was rejected with "expected `=>`, found `if`". The entire
  `MatchPatternKind::Guard` code path (checker, exhaustiveness, codegen,
  LSP goto-def) was unreachable. Fixed in `parser.rs`; regression test
  `guard_probe.rs` + `guard_roundtrip.rs` added.
- **Parity recognizer swallow-error → infinite loop** (round 31): both
  `parse_for_each` and `parse_items` used `parse_body(tokens, m).unwrap_or_default()`
  — an unbalanced body returned `end = 0`, and the caller assigned it to its
  own loop index `i`, spinning forever on the same token stream. Now
  propagates the parse error.
- **Guard arm label dropped condition** (round 33): `pattern_label` in both
  `view_tree.rs` (codegen) and `reduce.rs` (parity) returned only the binding
  name, so `s if s > 90` and `s if s > 80` produced identical labels — a
  parity comparison could not distinguish them. Both now include the
  canonicalized condition.
- **Kotlin/Swift guard codegen emitted invalid source** (round 35):
  - Swift: `case let .<name>(_)` (dropped the condition, invalid for a
    non-enum binding). Now `case let <name> where <cond>` — the real Swift
    guard form.
  - Kotlin: `is <name> ->` was invalid Kotlin that also dropped the
    condition. Now emits an explicit `/* unsupported match guard */ ->`
    marker. **Proper Kotlin fix filed as FLUX-F20.**
- **Handler bodies with a call were replaced by placeholders** (round 34):
  `render_stmt` fell through to `unsupported_placeholder` for any
  `ExprKind::Call` that was not `RouterNav.navigate`. `onPress: { Auth.login("u") }`
  emitted `/* unsupported expr */ 0` instead of the call. Fixed by adding a
  `render_call` helper that renders the callee and args faithfully; the
  `Await` branch now also routes through it. Regression tests added in both
  backends.
- **Differ reattach pairing nondeterministic** (round 37): `reattach_pairs`
  iterated `inserted`, whose order came from `AHashSet::difference` (per-process
  randomized). Two same-key candidates paired with "whichever came first",
  so the frame bytes varied between runs. Fixed by sorting `inserted` before
  the pairing call. Regression test `reattach_pairs_is_deterministic_when_candidates_share_a_key`.

### 2.2 Test flakes fixed (all three in the same class: shared fixed names)

- **kotlinc test** (round 38): wrote `flux_codegen_kotlin_<name>.kt` under
  the shared `TMPDIR` — a concurrent `cargo test` process removed it while
  the running `kotlinc` was reading. Fixed with a per-PID scratch dir.
- **swiftc test** (round 38): same, `flux_codegen_swift_<name>.swift`.
- **debug_bridge devtools tests** (round 44): hard-coded ports `17399`/`17400`
  raced with a prior listener. Fixed by splitting `serve_devtools` into a
  listener-taking `serve_devtools_on` and binding `:0` in tests.

### 2.3 Hygiene

- Dropped unused `ExprKind` import in the new guard test.
- Corrected a stale doc-comment in `bytecode.rs` that claimed guard arms
  "match unconditionally" (they actually return `Err`).
- `serve_devtools_on` is `pub(crate)` — no `unreachable_pub` warning.

## 3. Current state — START HERE

### 3.1 All tests green

| Surface | Command | Result |
|---|---|---|
| Rust workspace | `cargo test --workspace -j1` | all pass |
| Clippy strict | `cargo clippy --workspace --all-targets -j1 -- -D warnings` | clean |
| `cargo check` | `cargo check --workspace --all-targets` | clean |
| flux-parser guard tests | `cargo test -p flux-parser --test guard_probe --test guard_roundtrip` | 2 pass |

The two kotlinc/swiftc tests (`generated_kotlin_parses` /
`generated_swift_type_checks`) are slow (>2 min each) and require a
toolchain on `PATH`; they **skip** when missing and pass when present.

### 3.2 Working tree

Clean. All changes committed.

### 3.3 Filed issues (new this session)

- **FLUX-F19** — `await` in a handler is rejected by IR lowering, making
  the codegen async path unreachable. ADR-0044/0045 promise first-class
  `await`; the compiler bails before codegen can render it.
- **FLUX-F20** — Kotlin codegen cannot express a match guard arm. Swift got
  a real `where` fix; Kotlin emits an explicit unsupported marker until an
  `if/else if` chain lowering lands.
- (pre-existing) **FLUX-F16** — allocation-cap parity between Kotlin/Swift.
- (pre-existing) **FLUX-F17** — crash reporter source-map resolution.
- (pre-existing) **FLUX-F18** — F10 clippy gate blocked by ~25 mechanical
  panicking encoders in `flux-ir-serde` / `flux-devserver`.

## 4. Session conventions

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

## 5. Critical gotchas

1. **Never `pkill -f` anything generic.**
2. **`xcodebuild -destination "platform=iOS Simulator,id=<UDID>"` hangs.**
   Use `name=<iPhone>&OS=<version>` or `-xctestrun`.
3. **Never call `xcrun simctl bootstatus -b`.** Use `simctl boot` (async)
   plus a bounded poll for `simctl list devices booted`.
4. **Never modify the user's `$HOME` or shell rc files.** Temp artifacts go
   in `/tmp/`.
5. **Swift VM has THREE exec paths.** Any opcode change must be applied to
   `run`, `runResumable`, and `runViaDispatchTable`.
6. **`proto_version = 3` and `protocolVersionMin = 2`.** v2 frames are
   still readable via the version-dispatched `Reader::count`.
7. **The `docs/HANDOFF.md` you are reading was written in one shot at the
   close of this session.** Refresh in a dedicated commit.

## 6. Success criteria for the phase

1. All three VMs green.
2. Wire-fixture gate green.
3. Zero warnings from our crates.
4. Working tree clean (or only docs/ pending review).
5. Any change to one VM mirrored in the other two where applicable.

(1)–(5) hold.

## 7. Session history — rounds 26+

| Round | Finding |
|---|---|
| 26 | F1 (list index operand), F2 (snapshot semantics), F3 (Kotlin decode), F4 (async error), ADT/const/reserved/list.isEmpty |
| 27 | F5 (wire errors), F10 (panicking to_bytes), F7 (host crash sites) |
| 28 | F6 (watcher), F8 (init validation), F9 (security CI) |
| 29 | F11 (VM micro), F12 (docs), F13 (open items) |
| 30 | F14 (iOS render-perf), F15 (dispatch parity) |
| 31 | parity recognizer `unwrap_or_default()` infinite-loop risk |
| 32 | parser rejected spec §18.5 guard arms |
| 33 | guard-arm label dropped condition (codegen + parity) |
| 34 | handler-body calls replaced by placeholder; filed FLUX-F19 |
| 35 | Kotlin/Swift guard arms emit invalid source; Swift fixed, filed FLUX-F20 |
| 36 | devserver deep-read (pipeline, session, dispatch, watch) — no new bugs |
| 37 | differ reattach pairing nondeterministic |
| 38 | kotlinc/swiftc fixed-name temp collision |
| 39 | devserver async_bridge/assets/server — no new bugs |
| 40 | flux-cli subcommands — clean |
| 41 | wire frame.rs deep-read — F18 remains (mechanical) |
| 42 | guard round-trip through `flux fmt` — green |
| 43 | flux-types unify/exhaust — clean |
| 44 | debug_bridge fixed-port flake |

**Session owner:** the user
**Convention reminder:** the user runs scripts as `./wr.sh`. Respond with
exactly one fenced `bash` block per turn. No explanatory prose outside the
block.
