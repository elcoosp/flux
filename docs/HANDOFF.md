# Handoff — Flux codebase audit + v3 wire protocol

**For:** the next agent continuing this session
**Repo:** `https://github.com/elcoosp/flux`
**Working dir:** `/Users/adm/Documents/Repos/flux`
**Baseline commit at session start:** `3e5d913e` ("fix(ios): skip setChildren on non-structural reconcile…")
**Current HEAD:** see `git log --oneline -1` (roughly 200+ commits ahead of `3e5d913e`)

---

## 1. Mission (what this session is for)

Two intertwined goals:

1. **Fix everything found in a codebase audit** — a detailed review of the Flux monorepo (~60k LOC Rust, ~7k LOC Kotlin+Swift adapters). The audit's full text is at `docs/reviews/flux-code-audit-report.md` (~175 findings across §1–§17). It covers the VM, wire codec, differ, IR, dev server, codegen, parity harness, DevTools UI, LSP, CLI, scripts, website, and both native adapter kits.

2. **Wire protocol v3** — the audit identified that the u16 length prefix on user-authored collections (lists, records, patches, string tables, state seeds, etc.) is a real ceiling. `u16::MAX = 65 535` items. A 70 000-item list was user-plausible; the encoder would panic (fixed → fallible in the audit), then the plan became to widen those specific prefixes to u32 in a versioned bump. **ADR-0059** documents the design. The current session is halfway through implementing v3.

Everything is done in **small, atomic, per-file commits** with tests green between commits.

---

## 2. Current state — where exactly to pick up

### 2.1 Audit work — status: mostly complete

The audit was triaged into sections. Most items were either fixed (with tests) or classified as out-of-scope design decisions. The final state is documented in section 6 below.

### 2.2 Wire protocol v3 — status: mid-implementation

The v3 protocol work is organized in phases:

| Phase | Description | Status |
|---|---|---|
| **R1** | Bump `PROTOCOL_VERSION` to 3; add `PROTOCOL_VERSION_MIN = 2`; `Reader::with_version` + `Reader::count` helper (u16 for v2, u32 for v3); version range check in `read_frame_type` | ✅ DONE |
| **R2a** | `WireError::LengthExceedsU32`; `Writer::version` + `set_version` + `count_prefix` + `u32_len_checked` | ✅ DONE |
| **R2b** | Every frame encoder calls `w.set_version(self.version)` before `write_magic_version` | ✅ DONE |
| **R2c** | Swap 20 encode + 19 decode sites from u16 to `count_prefix` / `Reader::count` | ✅ DONE |
| **R2e–k** | Iteratively fix cascade: missing `delta.handler_count`, `decode_str` version-awareness, `error.msg.len`, `init.srcmap.path.len`, `init.component_names.name_len`, dedup `set_version` | ✅ DONE |
| **R3a** | Recon: found `dump_fixtures.rs` example; ADR renumbered to 0059 | ✅ DONE |
| **R3b** | Rewrite `dump_fixtures.rs` to emit v2 **and** v3; add 5 v3 fixture tests; update README + gate script | 🟡 **NOT YET RUN** — see §4 |
| **S1** | Swift `FrameDeserializer.swift` — mirror R1/R2's version dispatch | ⬜ NOT STARTED |
| **K1** | Kotlin `FrameDeserializer.kt` — mirror R1/R2's version dispatch | ⬜ NOT STARTED |
| **V** | Full green across Rust nextest + xcodebuild + gradlew | ⬜ NOT STARTED |

**The next action is R3b** — the exact script is preserved in §4 below.

---

## 3. Session conventions (do not deviate)

Established by the user during this session. A fresh agent must follow all of them or the loop breaks.

### 3.1 One commit per file

Never bundle. Even a two-file patch is two commits. The commit message convention is:

- `fix(scope): <what changed>` — bug fix
- `feat(scope): <what changed>` — new capability
- `chore(scope): <what changed>` — cleanup, dep bump, comment
- `test(scope): <what changed>` — test-only
- `refactor(scope): <what changed>` — pure restructure
- `docs(scope): <what changed>` — docs only
- `perf(scope): <what changed>` — perf-only

`scope` is a short crate/module tag (`ir`, `wire`, `devserver`, `codegen`, `parity`, `lsp`, `cli`, `devtools-ui`, `ios`, `adapters-kotlin`, `website`, `ci`).

Commit each file right after its patch succeeds, **before** moving to the next file. If a patch fails, the previously committed files stay; the failed one is retried in a follow-up script.

### 3.2 Script format

Every "turn" is one bash script the user pastes as `./wr.sh`. Structure:

```bash
#!/usr/bin/env bash
set -uo pipefail

echo "=== Batch name ==="

# 1. Recon section: `sed`, `grep`, `cat` to dump exact anchors
# 2. Patch section: python3 with temp files (see 3.3)
# 3. Verify: cargo check / cargo test / xcodebuild / gradlew
# 4. Commit: one `git add` + `git commit` per file
```

**No `set -e`** — errors are logged but the script continues to the end (unless a step explicitly `exit 1`s). This lets every patch attempt land and every subsequent check run even after a partial failure, so the user gets full diagnostics from one turn.

### 3.3 Surgical patches

**Never** use `sed` for anything but trivial single-line substitutions where the old and new text cannot contain the delimiter. **All multi-line / complex / whitespace-sensitive patches** go through this pattern:

```bash
OLD_TMP=$(mktemp); NEW_TMP=$(mktemp)
cat > "$OLD_TMP" << 'EOF'
... exact old text as it appears in the file ...
EOF
cat > "$NEW_TMP" << 'EOF'
... exact new text ...
EOF
if python3 - "$OLD_TMP" "$NEW_TMP" "$path" << 'PYEOF'
import sys
old = open(sys.argv[1]).read()
new = open(sys.argv[2]).read()
p = sys.argv[3]
c = open(p).read()
if old not in c:
    print(f"ERROR: anchor not found in {p}", file=sys.stderr)
    sys.exit(1)
open(p, 'w').write(c.replace(old, new, 1))
PYEOF
then rm -f "$OLD_TMP" "$NEW_TMP"; else rm -f "$OLD_TMP" "$NEW_TMP"; exit 1; fi
```

**Rules:**
- The anchor must be a byte-exact block of current source. Never guess; if it's not in the recon dump, run a recon first.
- If the anchor contains an `EOF` line, that line breaks the heredoc — split the anchor around it or use a different marker. The safest is to **recon the exact text first** and only patch after seeing it.
- `python3 - "$OLD" "$NEW" "$FILE"` passing paths as argv is the pattern; do not embed file content in Python source strings.
- `sys.exit(1)` on anchor miss so the surrounding shell can decide to skip the commit for that file.

### 3.4 Tooling gotchas

**macOS has no `timeout`.** Use a manual poll loop:

```bash
SOME_CMD &
PID=$!
START=$(date +%s)
while kill -0 "$PID" 2>/dev/null; do
    sleep 15
    ELAPSED=$(( $(date +%s) - START ))
    if [ "$ELAPSED" -ge 300 ]; then
        echo "TIMEOUT"; kill -KILL "$PID" 2>/dev/null; break
    fi
done
wait "$PID" 2>/dev/null
RC=$?
```

**Never pipe into `tail` / `grep` for long-running commands** if you need to see output as it streams. `tail` buffers everything until EOF, so the user perceives a hang. Redirect to a file and tail the file after the command completes:

```bash
LOG=/tmp/whatever.log
cargo test > "$LOG" 2>&1
RC=$?
tail -30 "$LOG"
```

**Swift simulator hanging**: `xcrun simctl bootstatus "$SIM" -b` can block forever after a fresh erase. **Never call it.** Let `xcodebuild test -destination "platform=iOS Simulator,id=..."` boot the sim itself. Use `xcrun simctl boot` (async) if you must pre-boot. If a Swift run appears stuck, `pkill -f 'xcodebuild test'` and re-run; `xcrun simctl erase "$SIM"` if needed. Progress updates on the Swift side can be minutes apart (buffered by xcodebuild); the 90-second "no progress" detector is a false positive — allow several minutes for the first cold build.

**Stale cargo processes**: at the top of any script that does a full workspace run, `pkill -f 'nextest'`, `pkill -f 'cargo'`, `pkill -f 'flux-devserver'`, `pkill -f 'xcodebuild test'`, then `sleep 2`. Otherwise an orphaned process holds the cargo lock and every subsequent command "hangs" on `Blocking waiting for file lock on package cache`.

### 3.5 Test recipes

**Rust:**
```bash
cargo nextest run --workspace -j1   # serialized; some devserver tests are sensitive to parallelism
cargo check --workspace --all-targets   # fast pre-flight
```

`-j1` avoids flaky interaction between the `notify`-based file-watcher tests.

**Swift (adapters/ui-swift):**
```bash
SIM=$(xcrun simctl list devices available 2>&1 | grep -E '^\s+iPhone' | head -1 | grep -oE '[0-9A-Fa-f-]{36}')
xcrun simctl boot "$SIM" 2>/dev/null || true
cd adapters/ui-swift
xcodebuild test -scheme FluxUIKit -destination "platform=iOS Simulator,id=$SIM" > /tmp/xc.log 2>&1
RC=$?
cd -
tail -30 /tmp/xc.log
```

Scheme name is `FluxUIKit`. The package has a `.testTarget` under `Tests/FluxUIKitTests`. `xcodebuild -list` shows the scheme.

**Kotlin (adapters/ui-kotlin and runtimes/android/host):**
```bash
./gradlew :adapters:ui-kotlin:test --console=plain --no-daemon > /tmp/kt.log 2>&1
RC=$?
tail -30 /tmp/kt.log
```

First run downloads Gradle. `--no-daemon` avoids orphaned daemons. `JAVA_HOME` and Android SDK at `$HOME/Library/Android/sdk` are pre-configured.

### 3.6 Cargo / workspace layout

- Workspace root: `Cargo.toml` at repo root
- 16 crates under `crates/` (list via `find crates -maxdepth 2 -name Cargo.toml`)
- Some crates use `workspace = true` deps
- `flux-devtools-ui` depends on `gpui` which is a git dependency — first build takes minutes; subsequent builds cache
- `flux-ir-serde` has integration tests under `tests/` that consume fixtures from `fixtures/wire/`

---

## 4. Next action — R3b (v3 fixtures)

Run this exact script as `./wr.sh`. If it fails, the tail will pinpoint which anchor missed.

```bash
#!/usr/bin/env bash
set -uo pipefail

echo "=== R3b: v3 fixtures (init_v3.bin / delta_v3.bin) + tests + README + gate ==="

# --- Rewrite dump_fixtures.rs to emit v2 AND v3 (full source in session) ---
# The v2 and v3 fixtures share structural content; only frame.version differs.
# Full replacement source is in the session history.

# --- Run generator ---
cargo run -q -p flux-ir-serde --example dump_fixtures

# --- Add 5 v3 tests to fixtures_golden.rs ---
# init_v3_fixture_decodes / init_v3_fixture_round_trips
# delta_v3_fixture_decodes / delta_v3_fixture_round_trips
# v3_fixture_is_wider_than_v2  (anti-reversion guard)

# --- Rewrite fixtures/wire/README.md ---
# Document the 5 fixtures, the ADR-0059 site list, the version-dispatch contract.

# --- Update scripts/wire-fixtures-gate.sh ---
# Change presence loop from `init_v2.bin delta_v2.bin unsupported-version.bin`
# to `init_v2.bin delta_v2.bin init_v3.bin delta_v3.bin unsupported-version.bin`

# --- Run tests ---
cargo test -p flux-ir-serde > /tmp/r3b.log 2>&1; RC=$?
tail -30 /tmp/r3b.log

# --- Commit one file at a time ---
# dump_fixtures.rs, fixtures_golden.rs, README.md, wire-fixtures-gate.sh,
# init_v3.bin, delta_v3.bin (unsupported-version.bin may not change)
```

The exact content of the `dump_fixtures.rs` rewrite and the test bodies were in the previous turn. **Ask the user to re-paste the previous turn's R3b script if it's missing** — they have it in the terminal log.

After R3b, `fixtures/wire/` should contain:

```
init_v2.bin             (~163 bytes, version byte 0x02)
delta_v2.bin            (~37 bytes, version byte 0x02)
init_v3.bin             (larger than init_v2, version byte 0x03)
delta_v3.bin            (larger than delta_v2, version byte 0x03)
unsupported-version.bin (version byte 0x04)
README.md
```

`cargo test -p flux-ir-serde` should be green (~100 tests).

---

## 5. Following phases — S1 (Swift) and K1 (Kotlin)

### S1 — Swift `FrameDeserializer.swift`

**File:** `runtimes/ios/FluxHost/Sources/FluxHost/FrameDeserializer.swift` (and `ByteReader.swift` for the reader type).

**Current state** (from earlier recon):
- `static let protocolVersion: UInt8 = 2` — must become 3
- No `PROTOCOL_VERSION_MIN` — add as `static let protocolVersionMin: UInt8 = 2`
- `guard version == Self.protocolVersion else { throw .unsupportedVersion }` — change to `guard version >= protocolVersionMin && version <= protocolVersion`
- Reader has no `count()` helper — add one that reads u16 for v2, u32 for v3

**The change:**

1. `ByteReader` gains `let version: UInt8` (set at frame-decode time) and `func count(context: String) throws -> UInt32` (u16 for v2, u32 for v3).

2. Every length-prefix read at the ADR-0059 sites (the 16 categories listed in `fixtures/wire/README.md` §"ADR-0059 site list") swaps `try r.u16()` → `try r.count(...)`.

3. The version-check guard changes to the range `[2, 3]`.

**Verification:** `xcodebuild test -scheme FluxUIKit` — the Swift fixture test (`WireDecodeTests`) must decode `init_v2.bin`, `delta_v2.bin`, `init_v3.bin`, `delta_v3.bin`, and reject `unsupported-version.bin`.

**Sequencing hint:** do the interface first (add `version` + `count()` + range check), compile, then do a single pass over the length sites. If any Swift file refuses to compile, the error will name the exact offset. Iterate.

### K1 — Kotlin `FrameDeserializer.kt`

**Files:** `runtimes/android/host/src/main/kotlin/dev/flux/host/wire/FrameDeserializer.kt` and `Frame.kt` (for the reader type). Same change shape as Swift — add a `version: UByte` on the reader, `count(): UInt` helper, swap the 19 length sites, range check `[2u, 3u]`.

The current Kotlin decoder hardcodes `PROTOCOL_VERSION: UByte = 0x02u` and accepts only v2 (per the audit D8 comment). Update to `3u` and accept `[2u, 3u]`.

**Verification:** `./gradlew :runtimes:android:host:test` — the `FrameDeserializerTest` (host) and `WireFixtureContractTest` (app) must decode the v3 fixtures and reject `unsupported-version.bin`.

---

## 6. Audit fix summary (context for what's already done)

Since `3e5d913e`, the session fixed:

**Rust correctness / security / perf:**
- VM: `NEG_I64` wrapping, AWAIT cell-id bound check, resume start-offset validation
- Wire codec: bounded `decode_value` recursion, decode_closures bound by min-def-bytes, `write_closures` HashMap + dupe check, `LengthExceedsU16` error variant, `u16_len_checked`, full fallible-encoder cascade
- Devserver: `AsyncBridge::clear_session` clears `parked`, constant-time token compare, DevTools-inbound-only-`DebugCommand`, module-loader path-traversal guard, `Access` event filter, assets 32MB cap, websocket limits, `root_ids` O(n), devtools delta ships only referenced strings/closures
- Differ: topological Insert ordering, `is_root_of_new` HashSet
- IR: `detect_prop_index_collision` + `Emitter::intern_prop_index` shared registry; **`lower_with_handler_base`** seeding handler ids across files
- Codegen: `Backend::named_arg`, `form_control` hook, Router startDestination escaping + `NavigationPath` seeding, `<T>` clause preserved, `collect_handler` priority, `binop_symbol` sentinel, `Bridge::components` BTreeMap
- Parity: tokenizer escape tracking, JSON UTF-8 + `\uXXXX`, `Divergence::render` frame-index walk, single source of truth for `normalize_view_name`/`is_container`
- LSP: `did_open` publishes diagnostics, `did_close`, `position_to_offset` clamp, UTF-8-aware diagnostic end
- CLI: `--no-types` negatable, doctor fn-base-depth fix, adb banner skip, sources symlink guard, `fnv1a32` file ids
- DevTools UI: keyboard repaint, timeline slider rebuild, `RefCell` snapshots, `flame_rows` O(n), `state_at` in-place replay, checkpointing, `ViewFrame::component_name: Arc<str>`
- **Multi-host sessions**: `DeviceSession` owns per-host state; host picker in title bar

**iOS adapters:**
- TextInput `editingChanged` relay (was dead `onChangeText`)
- Checkbox toggle-before-fire, Picker reload+clamp, Screen/Container/SafeArea multi-child stack, ScrollView orientation, TextArea placeholder overlay + single height cap, Image generation token + Auto Layout size, font slot 0 fix, identifier-scoped `removeAction`, Gesture error detach, Router VC containment, WebHost `decidePolicyFor`, TextField setter runs handler

**Kotlin adapters:**
- `reconcileChildren` strict, `ContainerAdapter` by childIds, `clearChildren` O(1), `getHandlerOrNull`, WebView src hoist, `nodeId` threaded through `FluxAdapter::bindHandler` + 26 overrides

**Website / scripts:**
- `make-fixtures` array bug, i18n trace-player prefix fix, homepage ADR link, dead dir removal, size-gate panic regex, wire-fixtures sim discovery, run-perf-harness timeout, provision-compose cache wipe + curl timeouts, check-ownership NUL-delimited

**Deferred (design decisions, not defects):**
- Client-side `try_to_bytes` sweep across all third-party callers (bounded-payload frames only; low value)
- Multi-host UI beyond the picker (icons, connection age — cosmetic)
- `apply_event` checkpoint interval tuning (`CHECKPOINT_INTERVAL = 256`)

---

## 7. Emergency recovery

### 7.1 Restore a file to last commit

```bash
git checkout -- path/to/file.rs
git status --porcelain
```

### 7.2 Reset a broken branch state

If a script left the tree half-broken and you want to restart the batch:

```bash
git status --porcelain    # list modified files
git diff                  # see what's half-applied
git checkout -- <files>   # selectively restore
```

Never `git reset --hard` unless the user explicitly says so — the session's commits are intentional.

### 7.3 Tests failing after a `cargo` invocation

Almost always a stale process holding the lock. Kill and retry:

```bash
pkill -f 'nextest' ; pkill -f 'cargo' ; pkill -f 'flux-devserver'
sleep 2
cargo nextest run --workspace -j1
```

### 7.4 Swift test hangs

```bash
pkill -f 'xcodebuild test'
SIM=$(xcrun simctl list devices available | grep iPhone | head -1 | grep -oE '[0-9A-Fa-f-]{36}')
xcrun simctl shutdown "$SIM" 2>/dev/null
xcrun simctl erase "$SIM"    # if sim is in a bad state
```

Then re-run — do NOT call `simctl bootstatus -b`.

### 7.5 Kotlin gradle stuck

```bash
pkill -f gradle
rm -rf .gradle/caches/*/  # only if the lock is corrupt
./gradlew --stop
./gradlew :adapters:ui-kotlin:test --no-daemon
```

---

## 8. Reference — repo map

| Area | Path |
|---|---|
| Workspace root | `Cargo.toml` |
| VM (reference oracle) | `crates/flux-vm-ref/src/vm.rs` |
| Wire codec | `crates/flux-ir-serde/src/{frame.rs, telemetry.rs, resume.rs, wire/*.rs}` |
| Wire version constants | `crates/flux-ir-serde/src/frame.rs` (search `PROTOCOL_VERSION`) |
| IR lowering | `crates/flux-ir/src/lower/{mod.rs, bytecode.rs}` |
| Differ | `crates/flux-differ/src/diff/*.rs` |
| Dev server | `crates/flux-devserver/src/{pipeline.rs, server/*.rs, watch.rs}` |
| Codegen core | `crates/flux-codegen-core/src/{emitter.rs, backend.rs, primitives.rs}` |
| Parity | `crates/flux-parity/src/*.rs` |
| DevTools UI | `crates/flux-devtools-ui/src/{state.rs, app.rs, views/*.rs}` |
| LSP | `crates/flux-lsp/src/*.rs` |
| CLI | `crates/flux-cli/src/*.rs` |
| iOS adapters | `adapters/ui-swift/Sources/FluxUIKit/*.swift` |
| iOS host runtime | `runtimes/ios/FluxHost/Sources/FluxHost/*.swift` |
| Kotlin adapters | `adapters/ui-kotlin/src/main/kotlin/dev/flux/ui/*.kt` |
| Android host runtime | `runtimes/android/host/src/main/kotlin/dev/flux/host/*.kt` |
| Fixtures | `fixtures/wire/*.bin` |
| Audit report | `docs/reviews/flux-code-audit-report.md` |
| ADR (v3) | `docs/adr/0059-wire-protocol-v3.md` |
| ADR (fail-closed protocol version) | `docs/adr/0056-fail-closed-protocol-version-handshake.md` |
| Website | `website/` (Astro + React) |
| VS Code extension | `editors/vscode/` |
| CI scripts | `scripts/` |

---

## 9. Success criteria for the v3 work

By the end, all three checks must be green:

1. **Rust**: `cargo nextest run --workspace -j1` — 750+ tests, 0 failures
2. **Swift**: `xcodebuild test -scheme FluxUIKit` on a booted iOS simulator — 77 tests, 0 failures
3. **Kotlin**: `./gradlew :adapters:ui-kotlin:test :runtimes:android:host:test` — BUILD SUCCESSFUL

And the three-decoder fixture gate must pass:

```bash
bash scripts/wire-fixtures-gate.sh
```

This runs the Rust, Kotlin, and Swift fixture tests against the same `fixtures/wire/*.bin` set and exits non-zero if any decoder disagrees with the other two on accept/reject for any fixture.

---

## 10. Session history (chronological, for reference)

Batches landed, in order:

1. **Sprint 1** — VM/codegen/devserver criticals (11 files): `NEG_I64`, `decode_value` bound, `AsyncBridge` clear, `Access` filter, `root_ids` HashSet, Toggle form shapes, Swift Router VCs, iOS TextInput relay, etc.
2. **Sprint 2** — `Backend::named_arg` + `form_control` hook + Kotlin/Swift overrides
3. **Sprint 3** — Emitter indent memo, macOS `timeout` shim, Swift sim discovery, provision-compose cache wipe
4. **Sprint 4** — `Budgets::with_ceiling`, save_to_photon tests, differ topological insert
5. **Sprint 5** — iOS multiple adapter fixes (font slot, UIAction dedupe, checkbox stale-value, picker reload, multi-child stack, ScrollView orientation, TextArea, Image generation)
6. **LSP + devtools** — `did_close`, keyboard repaint, timeline slider rebuild, log/network delegate snapshots, `flame_rows` O(n)
7. **Parity canonicalization** — tokenizer, JSON, divergence render, `binop_symbol` sentinel, `normalize_view_name` single source
8. **Kotlin adapter** — `nodeId` threading, `reconcileChildren` strict, `getHandlerOrNull`, `WebViewAdapter` hoist
9. **DevTools multi-host** — `DeviceSession` ownership, host picker
10. **Wire encoder cascade** — `LengthExceedsU16`, `u16_len_checked`, fallible encoders, H14 panic-to-error for >64KB handlers, `write_closures` HashMap
11. **IR prop-index collision detection** — `detect_prop_index_collision`, `Emitter::intern_prop_index` shared registry
12. **IR handler-id seeding** — `lower_with_handler_base` for multi-file
13. **Wire v3 protocol R1** — version range check, `Reader::with_version` + `count`, `PROTOCOL_VERSION_MIN`
14. **Wire v3 protocol R2a–c** — `Writer::version`, `count_prefix`, 20+19 site swap, cascade fixes
15. **Wire v3 protocol R2e–k** — dedup, version-aware decode_str, missing handler_count, fixture regen
16. **Audit renumbering** — ADR 0001 → 0059; multi-host picker; `ViewFrame::component_name: Arc<str>`

The full list is `git log --oneline 3e5d913e..HEAD | wc -l` — around 200 commits.

---

## 11. What to do if you get stuck

1. **Anchor miss**: recon the exact bytes with `sed -n 'N,Mp' file` before patching. Never guess.
2. **Borrow error**: the compiler will tell you exactly which lifetimes to add. Rerun with the suggestion.
3. **Type mismatch on `Reader::count` / `Writer::count_prefix`**: they return `u32` / `Result<(), WireError>`. Cast explicitly with `as usize` if downstream code expects `usize`.
4. **Test counts drift**: some tests assert byte-length values that change when the wire layout shifts. Update the assertion, don't fight it — the shape is intentional.
5. **Files in the working tree you didn't touch**: leave them. `.gitignore`, `docs/reviews/`, `justfile` are user-managed WIP that must not be reset.

---

**Session owner:** the user
**Date of handoff:** 2026-10-05
**Session duration:** multiple days, ~200 commits
