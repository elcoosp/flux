# Flux Codebase Audit — Bugs, Kotlin↔Swift Drift, and the Path to Production

**Scope:** full monorepo dump (668 files, ~105k lines): Rust workspace (16 crates), Kotlin Android runtime + adapter kit, Swift iOS runtime + adapter kit, stdlib (`.flux`), CI workflows, build scripts, examples, website/vscode.

**Method:** six parallel deep-audit passes (Swift runtime; Kotlin↔Swift drift matrix; Rust wire/differ/devserver; Rust compiler crates; CI/build/FLUX-092 verdict; stdlib/examples/adapter tests), followed by first-hand verification of every Critical finding against the actual source. Every finding below cites `file:line` from your dump. Items marked **[verified]** were re-checked line-by-line by the orchestrator, not just pattern-matched.

**Verdict up front:**

| Area | State |
|---|---|
| Wire decode (host-side ByteReader/FrameDeserializer) | ✅ Solid, byte-compatible with Rust encoder |
| Language pipeline (parser→types→**IR lowering**) | ❌ **Not production-ready** — silent miscompilation of ordinary handlers |
| Differ (hot-reload patches) | ❌ Order/correctness bugs; **its test suite doesn't even compile** |
| Devserver | ❌ Token gate bypassable; unauthenticated DoS paths |
| Swift iOS host | ❌ Http/Persist capabilities **cannot execute at all**; reconciler destroys view identity every frame; leaks |
| Kotlin Android host | ⚠️ Mostly ahead of iOS, but ForEach reconcile reuses destroyed views; no list-op gap |
| Kotlin↔Swift parity | ❌ Two different ForEach hash algorithms; three different list-op decodings; prop-decode divergence |
| Release codegen (Kotlin backend) | ❌ Emits non-compiling Compose for everyday features |
| Parity harness | ❌ Structurally blind to the drift it exists to catch (props never compared) |
| CI / release gating | ❌ Release gate can **never go green** today; gradle wrapper broken in CI |

Bottom line: the architecture is sound and the codec layer is genuinely good, but **this codebase cannot ship as-is**. The single scariest file is `crates/flux-ir/src/lower/bytecode.rs` (silently wrong bytecode), and the single scariest runtime is the Swift host's capability/reconciler path. Section 8 gives the phased fix plan; nothing ships before Phases 0–3 are done.

---

## 1. P0 — Critical blockers (ship = broken)

### C1. Register allocator never frees registers → silent wrong arithmetic in any handler with ~13+ temporaries **[verified]**
`crates/flux-ir/src/lower/bytecode.rs:487-491`
```rust
fn alloc_reg(&mut self) -> u8 {
    let r = self.reg;
    self.reg = self.reg.saturating_add(1).min(14);
    r
}
```
Registers are monotonically consumed and never freed. Once exhausted, **every allocation returns r14** while older values are still live. Concrete failure: the 6th `count = count + 1` in one handler compiles to `READ_SIGNAL r14, count; LOAD_INT_CONST r14, 1; ADD r14, r14, r14` — the constant clobbers the signal read and the ADD sums a clobbered register, so `count` becomes `2` instead of `count+1`. Silently, on device.
**Fix:** free temporaries after last use (decrement `self.reg` when a temp's consumers are emitted), or error at exhaustion (`HandlerCompileError::RegistersExhausted`). Never alias live registers. Add a regression test compiling 20 sequential statements and asserting distinct registers.

### C2. Opcode selection is syntactic, not typed → every Float/String/Int-comparison faults at runtime
`crates/flux-ir/src/lower/bytecode.rs:1187-1233`
All arithmetic/comparisons emit I64 opcodes; `ADD_F64`/`EqF64`/`StrEq` are never emitted. Against the VM (`crates/flux-vm-ref/src/vm.rs:461-570`):
- `total = total + 0.5` → `ADD_I64` on a Float → `TypeMismatch` fault.
- `name == "bob"` → `EQ_I64` on Str → fault.
- `a == b` on two Int signals hits the Ident/Ident heuristic → `BOOL_EQ` on ints → fault.
**Fix:** select the opcode from the type checker's recorded expression type (the NodeId bridge already exists — `crates/flux-codegen-core/src/bridge.rs`). Add typed-arith conformance vectors.

### C3. `?.` reads the wrong field-index space
`crates/flux-ir/src/lower/bytecode.rs:1276`
`GET_FIELD` for `?.` uses `method_id_for("", field)` (blake3) while every other access uses `prop_index_for_name` (FNV u16). `user?.name` reads a different slot than `user.name`/`SET_FIELD` wrote → always Null/garbage.
**Fix:** use `prop_index_for_name` consistently. Test: `user?.name` and `user.name` must produce identical bytecode indexes.

### C4. Literal `match` arms are value-blind
`crates/flux-ir/src/lower/bytecode.rs:967-975`
`match n { 1 => A, 2 => B, _ => C }` compiles each literal arm to `MATCH_TAG` carrying only the *type* tag — every Int matches arm A. `exhaust.rs:41` returns Ok for primitive scrutinees, so nothing rejects it either.
**Fix:** emit `MATCH_TAG` + an equality check against the literal (or reject literal patterns in handlers until supported).

### C5. Handler-constructed enum variants carry no tag → `match` over them never matches
`crates/flux-ir/src/lower/bytecode.rs:1426-1450` (construction), `crates/flux-vm-ref/src/vm.rs:715-723` (consumption)
The VM reads the match tag from the record's field 0, but `Task(...)` built inside a handler produces a plain payload record. The unit test hand-seeds `Record([(0, Int(tag))])` to pass — masking the bug.
**Fix:** prepend `(PropIdx(0), Int(variant_tag(name)))` in both construction paths (handler + static seed, `crates/flux-ir/src/lower/mod.rs:883-894`).

### C6. Component inlining mints duplicate NodeIds
`crates/flux-ir/src/lower/mod.rs:678-683,770-827`, `crates/flux-ir/src/lower/ids.rs:66-68`
`try_inline_component` re-lowers a body per call site; ids derive only from the body's own spans → identical ids at every inlinable call site, duplicating ids already packed at declaration (`arena.rs:149` silently keeps the last). Corrupts reconcile/patch on the common `TaskRow(task: item)`-in-ForEach path.
**Fix:** mix the call-site id into the id derivation as parent, or refuse inlining when the body was already packed.

### C7. WebSocket token gate is bypassable — rejected clients still receive every broadcast frame **[verified]**
`crates/flux-devserver/src/server/session.rs:58, 77-79, 201-221`
```rust
let queue = shared.register();            // registered BEFORE auth
...
if is_hello(&bytes) { handshook = true; } // set even when the token was just REJECTED
```
`serve_client` registers the client into the broadcast list before the Hello check. When `handle_hello` rejects a bad token it only sends an Error frame; the session keeps running and `handshook` becomes true, so the rejected client stays in `Shared::clients` and receives every subsequent `Init`/`Delta` (full tree + interned strings). This defeats the documented purpose of `--token` for `--ws-host 0.0.0.0` LAN exposure.
**Fix:** register the client only after successful validation; close the socket on rejection; never set `handshook` for a rejected handshake. Test: connect with a bad token, assert zero broadcast frames received.

### C8. Differ emits multi-insert in hash-set order → host child order corrupts **[verified]**
`crates/flux-differ/src/diff/algorithm.rs:114-115, 125-137`
```rust
let inserted: Vec<NodeId> = new_ids.difference(&old_ids).copied().collect();
```
`old_ids`/`new_ids` are `AHashSet`s; iteration order is arbitrary, but `Patch::Insert` carries an **absolute index into the new tree**. Old `[x]`, new `[a, b, x]`: applying `b@1` before `a@0` yields `[a, x, b]` on the host (Android applies in stream order, `ShadowTree.kt:510-515`).
**Fix:** sort `inserted` by `(parent, index)` before emitting. Golden test: multi-insert must produce ascending indices per parent.

### C9. New top-level components are silently dropped on hot reload
`crates/flux-differ/src/diff/algorithm.rs:129` + `crates/flux-devserver/src/pipeline/tree.rs:42-45, 63`
`new_index.get(id)` is `None` for arena roots (only nodes with an arena parent are indexed), so the differ never emits `Insert` for a new root — while removal *is* emitted (asymmetric). Adding a top-level component during hot reload never reaches the host.
**Fix:** special-case roots and emit Insert against the stable synthetic wrapper id used by `tree.rs`.

### C10. Swift: Http/Persist capabilities can never execute (double fault) **[verified]**
1. `runtimes/ios/FluxHost/Sources/FluxHost/Permission.swift:65-82` — `requiredPermission` has **no cases for cap 14 (Http) / 15 (Persist)**; falls to `default: nil`, and the VM gate treats `nil` as unknown ⇒ unconditional `CAPABILITY_DENIED` (`FluxBytecodeVM.swift:510-516`). The Rust mirror maps `(14,_)→Network`, `(15,_)→Storage` (`crates/flux-types/src/capabilities/permission.rs:172-176`). Tests stay green only because they call `registry.lookup(...)!(...)` directly, bypassing the gate.
2. `Registry.swift:247-250` mints `httpPersistEntries(store: HttpRequestStore(), ...)` inside `.dev` while `FluxExecutor.swift:165` builds `HttpAsyncResolver` with a **different** store — the resolver never finds the pending request, so cells settle to the wrong value. `Flux047HttpPersistTests.testHttpGetJsonResolvesToRecordViaResolver` is structurally red (asserts `.record` on what is actually `.int`).
**Fix:** add `case 14: .network`, `case 15: .storage` (plus the missing `PermissionKind` cases); construct the registry from `httpPersistEntries(store: httpRequests, transport: httpTransport, ...)` and stop hardcoding `capRegistry: .dev`. Make the existing test actually go through `CALL_CAP`.

### C11. Three runtimes decode LIST_INSERT/LIST_REMOVE three different ways — and the reference VM panics **[verified]**
Compiler emits `LIST_INSERT list(u8), idx(u8), val(u8)` / `LIST_REMOVE list(u8), idx(u8)` (`crates/flux-ir/src/lower/bytecode.rs:613-624`); normative widths 3/2 (`crates/flux-syntax/src/opcode/decode.rs:137-138`).
- **Kotlin** (`Opcode.kt:76-77`, `StepResult.kt:280-300`): widths 3/2, reads `u8(0),u8(1),u8(2)` — ✅ correct.
- **Swift** (`OpCodes.swift:187-188`): operandLen **4/3** → pc skips one byte after every list op (misalignment cascade); `LIST_REMOVE` reads `u8(1),u8(2)` — wrong registers. Any `list.insert`/`list.remove` mis-executes on iOS.
- **Rust oracle** (`crates/flux-vm-ref/src/vm.rs:649-667`): reads `u8(1),u8(2),u8(3)` (insert) / `u8(2)` (remove) — **beyond the 3/2-byte operand window** ⇒ `operands[3]` index-out-of-bounds **panic** on any compiled list insert/remove. The oracle — the behavioral ground truth of Appendix E — cannot execute its own compiler's bytecode for these ops, and its conformance vectors must be silently avoiding them.
**Fix:** oracle reads `u8(0)/u8(1)/u8(2)` (insert) and `u8(0)/u8(1)` (remove) with widths 3/2; Swift widths → 3/2 and positions → `u8(0…)`. Add ISA vectors for both ops to all three runtimes (Swift currently has none; `FluxBytecodeVmTest.kt:267-285` has them).

### C12. Android ForEach reconcile tears down and reuses destroyed rows **[verified]**
`runtimes/android/host/src/main/kotlin/dev/flux/host/shadow/ShadowTree.kt:866-920`
`desired` is built from raw `deriveForEachRowId(foreachId, i)` ids (line 866-877), but row `ShadowNode.id`s live in the **`deriveForEachChildId`** space (`build()` line 719 `id = wire.id` of the cloned wire). Therefore:
1. Teardown (`child.id !in desired`, lines 879-885) is **always true for every row** — every reconcile destroys and rebuilds all rows (identity churn; contradicts "reorder, never recreate").
2. `nodes[rowId] = built` (line 900) keys by raw rowId, which teardown never removes (it removes by derived id) — so from the **2nd list mutation onward, `nodes[rowId]` returns the just-destroyed node** and `adapter.update()` is called on a destroyed view (line 915). On-device this surfaces as a frozen/blank list after add+remove+add.
**Fix:** pick one id space for row roots and use it consistently in `desired`, `nodes`, `parents`, `expandedIndex`, and teardown. Add a test doing two consecutive list mutations and asserting adapters were never updated after `destroy`.

### C13. Release gate can never go green + Gradle CI is broken **[verified]**
- `docs/` does not exist in the dump ⇒ `docs/release/contract-versions.toml` is missing ⇒ `scripts/release-gate/check-contract-freeze.sh:41-45` exits 1 on every run (`release-gate.yml:75-83`). The flagship gate is dead.
- `gradle/wrapper/` contains only `gradle-wrapper.properties` — **`gradle-wrapper.jar` is not committed**, so every `./gradlew` invocation fails ⇒ `android-check.yml:176`, `compat-matrix.yml:148`, `artifact-publish.yml:92` all broken.
**Fix:** commit the contract manifest and the wrapper jar (or switch CI to a `gradle/actions/setup-gradle` install). Verify with a dry run of both workflows.

### C14. Force-unwrap gate is a silent no-op + stale manifest row can break the workspace build
- `scripts/ci-size-gate.sh:240,245` — the force-unwrap regex `[A-Za-z0-9_)\]]` terminates the char class at `\]`; `x!` does not match. Only `try!` is detected — the Swift/Kotlin force-unwrap rule never fires.
- `MANIFEST_REQUESTS.md:18` keeps a stale open row while `manifest-steward.sh:88`'s presence-check regex doesn't match `flux-perf-harness.workspace = true` (`crates/flux-devtools-ui/Cargo.toml:18`) — the next weekly cron run inserts an invalid TOML entry and **breaks the workspace on main**.
**Fix:** repair the char class and exclude `!=`; delete the stale row and make the steward validate TOML after editing (fail the PR, not main).

---

## 2. P1 — High severity (wrong behavior, crashes, or leaks in production paths)

### Swift iOS host

| # | Location | Issue | Fix |
|---|---|---|---|
| H1 | `FluxBytecodeVM.swift:301-304,869-872,1259-1262` | `Int64(v)` traps (crash) for NaN/±inf/out-of-range in `f64ToI64`; Rust oracle saturates (`vm.rs:546`) | Saturate explicitly: NaN→0, clamped min/max |
| H2 | `FluxBytecodeVM.swift:945-948` | Resumable interpreter pre-populates ALLOC_RECORD with positional nulls; `run` and oracle don't → after any AWAIT, records carry phantom null fields; positional thunk layout reads (reconciler `:937-955`) materialize nulls | Delete the pre-population loop in `execTailWith` |
| H3 | `ShadowTreeReconciler.swift:241` | `built.removeAll()` runs on **every** Delta frame → all native view identity destroyed per frame (scroll/focus/text lost), `adapter.destroy`/`onCleanup` skipped, same-frame remove/reorder operate on freshly rebuilt views | Only wipe when a Replace targeted the root |
| H4 | `ShadowTreeReconciler.swift:988-1003,196-199,510-774` | Subtree removal destroys only the root — descendants stay in `built` forever; full frames never destroy absent ids; ForEach expansion never prunes stale rows → unbounded memory growth + zombie UIControl targets | Diff `built.keys` against reachable ids post-reconcile; `adapter.destroy` + remove stale entries; prune ForEach row state |
| H5 | `FluxExecutor.swift:309-328,548` | Malformed frames only DEBUG-NSLog'd; `lastError`/`lastFluxError` never set → **no banner in Release**; `(try? Instruction.decode(bytecode)) ?? []` turns a corrupt handler into a silent empty program | Set a wire-kind FluxError in the catch; surface `invalidDispatch` on decode failure |
| H6 | `IOSNativeCapabilityHost.swift:193-219` | `fileSystemWrite` persists `dataField.value.description` (debug description, e.g. `str(12)`), not the payload; write/read asymmetric → all non-integer file data corrupted | Resolve `.str` via string table; render scalars via `renderForToString`; reject containers with `typeMismatch` |
| H7 | `IOSNativeCapabilityHost.swift:310-312,53-62` | `fileSignalID = 900_000 + pathId` — plain `+` on an unmasked FNV hash ⇒ **arithmetic-overflow crash**; also collides with `allocateCell()` (≥1_000_000) corrupting awaited capability cells | `&+` and mask into a reserved range below the allocator ceiling |
| H8 | `HttpCapabilities.swift:73-93`, `HttpCapabilities+Entries.swift:119-156`, `IOSNativeCapabilityHost.swift:127-161` | `URLSessionHttpTransport.request` blocks on a semaphore **on @MainActor** → UI frozen for the whole network round-trip; `pushStatus()` and `biometricAuthenticate()` also block main (Face ID dialog!) | Make `HttpTransport` async; restructure push/biometric as Pending-cell + `resolveCell` from completions |
| H9 | `FluxExecutor.swift:461-531,589-675` | `runHandlerAsync` works on `var store = graph` and commits `graph = store` at HALT — two interleaved dispatches (one parked on Http await) lose each other's signal writes | Merge per-resume delta instead of wholesale commit, or serialize dispatches until settle |
| H10 | `FluxAppMain.swift:87-92` | Error overlay reads `executor.lastFluxError` but `FluxExecutor` is not `ObservableObject` — SwiftUI never re-renders, **the overlay generally never appears** | `@Published`/ObservableObject (or NotificationCenter) so faults refresh the view |
| H11 | `Telemetry.swift` + `FluxBytecodeVM.swift:600,1413` | Header claims `#if DEBUG` compile-out, but nothing is guarded; per-instruction telemetry enum + 16-register copy + ~10k WS frames per 100k-gas tap, shipped in Release | Wrap module + emit sites in `#if DEBUG` as documented |
| H12 | `ShadowTreeReconciler.swift:145-151` | `thunkBlobs = blobs` **replaces** on every frame (sibling `thunkHandlerToNode` merges, with a comment warning exactly about this) → after any hot reload all other thunks lose bytecode; props silently fall back to stale statics | Merge into `thunkBlobs` |

### Rust core

| # | Location | Issue | Fix |
|---|---|---|---|
| H13 | `crates/flux-differ/src/diff/tests.rs:1-3` | Declares `mod common; mod patch_tests; mod reattach_tests;` — **none of these files exist**; `cargo test -p flux-differ` cannot compile; all differ tests dead **[verified]** | Restore the three files or delete the declarations |
| H14 | `crates/flux-ir-serde/src/frame.rs:177`, `emit.rs:47`, `:1105`, `wire/string_entry.rs:10`, `wire/child.rs:16`, `telemetry.rs:317` | Pervasive `len() as u16` on closures/strings/counts — a handler >64 KiB silently truncates and hosts slice the wrong blob range | Checked casts → hard error when exceeding prefix width |
| H15 | `crates/flux-devserver/src/server.rs:74-77`, `debug_bridge.rs:149,158` | Unbounded broadcast channels + no timeout for non-handshaked clients → unauthenticated remote memory-DoS (compounds C7) | Bounded channels + slow-client eviction policy |
| H16 | `crates/flux-devserver/src/watch.rs:137-170` | Compile under pipeline lock, broadcast after guard drops → a Hello served in between gets Init(new) then Delta(old→new) → host applies Remove/Insert of nodes it already has → corrupt tree | Broadcast under the lock, or hosts drop Deltas with seq ≤ last Init seq |
| H17 | `crates/flux-devserver/src/async_bridge.rs:40-114` | Server-wide bridge never pruned on disconnect; `settle()` has **zero non-test callers** → parked handlers never resume in production; stale `early` values can resume a future session's handler | Per-session bridge, clear on disconnect, wire capability completion into it |
| H18 | `crates/flux-parser/src/lexer.rs:413-414,508-542` | `-` before a digit always folds into a negative literal (no previous-token context): `x = x-1` silently becomes `x = x` plus dead `-1`; `count-1` (no spaces) broken, `count - 1` works — hidden from tests | Fold negative literals only when the previous significant token can't end a value, or add unary minus |
| H19 | `crates/flux-types/src/checker/generics.rs:155-183` | Per-fn `Supply::default()` resets var ids to 0 → collisions across forward-declared fns and with prelude Mono bindings; `id(1)` then `id("s")` spuriously mismatches or corrupts `resource`'s type | Allocate from the checker's own supply |
| H20 | `crates/flux-types/src/checker/apply_callee.rs:114,160` | `let _ = self.expect(...)` **discards type errors** for component-prop and record-ctor args; positional component args never checked — `Button(text: 42)` type-checks | Propagate the `Result` |
| H21 | `crates/flux-codegen-core/src/expressions.rs:68-82` | `render_string` does **no escaping at all** (decoded `\n`/`"`/`\` concatenated raw) for both backends: `Text("say \"hi\"")` emits invalid `Text("say "hi"")`; user text containing `"){ … }` escapes the literal into generated code | Per-backend text-escaping hook (Swift and Kotlin rules differ: Kotlin also needs `$` → `\$, the repo's own fixture `"tapped $${count} times"` is an immediate Kotlin build break) |
| H22 | `crates/flux-codegen-kotlin/src/backend_impl.rs:193-195,97-98,197-217` | Kotlin prelude missing 6 imports its output references (`Image`, `painterResource`, `items`, animation core, `GlobalScope`, `RoundedCornerShape`); ForEach emits `items(...)` inside plain `Column` (only valid in LazyListScope); `withAnimation` doesn't exist in Compose | Fix prelude; emit LazyColumn for ForEach; map animation to Compose `animate*AsState`/`Animatable` |
| H23 | `crates/flux-vm-ref/src/vm.rs:571-574,706` | `STR_LEN` = digit count of the string **id** (`u32::ilog10(0)` panics on id 0) — unreproducible semantics hosts can't mirror; `CALL_CAP` hardwires `CapabilityRegistry::with_parity_stubs()` so only 2 stub caps can ever execute | Return the real length from the string table; thread the registry through the VM constructor |
| H24 | `crates/flux-parity/src/reduce.rs:105`, `codegen-core/view_tree.rs:199-203`, both recognizers | Parity harness **never compares props** (hardcoded `vec![]`): different labels/colors/text between dev and release — and between backends — stay green | Thread props into `ViewNode` and compare them |

### Kotlin / cross-runtime

| # | Location | Issue | Fix |
|---|---|---|---|
| H25 | `shadow/ShadowTree.kt:927-929` vs `ShadowTreeReconciler.swift:647-667` | Kotlin and iOS use **completely different ForEach row/child id derivations** (mul+xor without marker bits vs FNV-0x01000193 with 0x8000/0xC000 markers). Android's unmarked derived ids can collide with real server node ids and clobber `nodes`/`expandedIndex`; DevTools traces show different ids per platform | Adopt one collision-safe scheme (Swift's FNV+marker) on both, generated from a single source, with shared golden vectors |
| H26 | `runtimes/android/app/build.gradle.kts:33-37` | Release build: `isMinifyEnabled=false`, no shrinkResources, no proguard rules, no signingConfig — CI publishes an unminified debuggable APK | Enable R8 + resource shrinking + signing config; commit proguard-rules.pro (OkHttp/Compose keep rules) |
| H27 | `runtimes/android/host/.../ShadowTree.kt:866-903` | (Same as C12 — listed here so Kotlin-fix sweeps don't miss it) | See C12 |

### CI / build

| # | Location | Issue | Fix |
|---|---|---|---|
| H28 | `.gitignore:41` vs `wire-fuzz.yml:61-77` | `fuzz/corpus` is gitignored but the workflow requires a committed corpus → drift check degenerates (0==N), fuzz runs seedless | Un-ignore corpus; commit real seeds |
| H29 | `CHANGELOG.md:21-32` (FLUX-078) | Claims `crates/flux-parity/tests/native_kit_parity.rs` gates adapter drift — the directory doesn't exist; the claimed guard is absent **[verified]** | Land the test or correct the changelog |
| H30 | `scripts/generate_for_each_hashes.sh` | Generates only two blake3 closure hashes into a file with **zero consumers**; constants match neither host's actual bytecode; no row-id constants generated at all; script lives in machine-specific `${HOME}/.hermes/scripts` | Delete or regenerate from real blobs with a consumer test |
| H31 | `perf-harness.yml:59-79` + `scripts/run-perf-harness.sh:53-79` | On-device budget gate is a no-op: gradle failure swallowed (`\|\| true`), iOS `xcodebuild` under `set +e` → green job with zero measurements | Fail when a platform's perf record is absent |

---

## 3. P2 — Medium severity (condensed; each needs a fix before calling the subsystem stable)

**Rust core**
1. `devserver/session.rs:362,381` — blocking `pipeline.lock()` taken inline on the async reactor; a compile stalls tokio workers. Route through the existing `blocking()` helper.
2. `devserver/watch.rs:119-130` + `pipeline.rs:495-498` — deleting a `.flux` file only logs; `Pipeline` has no removal API → deleted components keep rendering forever. Drop the FileId on ENOENT.
3. `devserver/server.rs:144-145` + `debug_bridge.rs:219-235` — DevTools endpoint hard-bound to `0.0.0.0:7333` ignoring config, **no token**, telemetry carries request URLs/body snippets; inline WS upgrade lets one silent client block all DevTools connections. Config-driven bind + token + per-connection upgrade.
4. `devserver/assets.rs:111-120` — traversal guard rejects `..`/absolute but never canonicalizes and follows symlinks → a symlink inside the project root serves any file on disk over unauthenticated :7332. Canonicalize + re-verify prefix.
5. `differ/diff/emit.rs:21-35` — handler ids present in only old/new are skipped; handlers aren't part of the content id → handler-set changes on a stable node ship nothing. Emit patches for added/removed handler ids.
6. `differ/diff/compare.rs:107` — `handlers_equal` ignores `captured_signals` (which `hash_closure` treats as behavior-changing) → stale signal wiring after hot reload.
7. `flux-syntax/src/ids/node_id.rs:79-83` — component_id folded into 7 bits (`^ (mul(0x9E) as u8)`): ids differing by multiples of 0x100 collide → structurally identical nodes of *different components* can share content ids. Fold as 4 raw bytes.
8. `flux-syntax/src/opcode.rs:175-241` — `Opcode::ALL` omits `BoolEq` though the enum/raw/decode/width tables have it (same drift class FLUX-078 fixed for list ops).
9. `flux-ir/src/lower/mod.rs:972-982` — `prop_index_for_name` truncates FNV-32 to u16; ~300 distinct prop names ⇒ ~50% birthday collision ⇒ `SET_FIELD` upserts silently overwrite each other. Keep u32 or detect collisions at compile time.
10. `flux-ir/src/lower/mono.rs:95-107` + `checker/infer.rs:33-35` — instantiation cursor desync (trailing-block-first vs args-first) → wrong specialization mapped to call sites.
11. `flux-ir/src/arena/blob.rs:48,76,82,137,146,183` — `as u16` count truncations (>65535 children/fields; >64 KiB thunks) silently corrupt packed data; unpack panics in `Cursor::take`.
12. `flux-ir/src/arena/content_address.rs:37-74,128-204` — unbounded recursion per tree depth; all roots get parent=0/position=0 → identical roots or duplicate ForEach keys collapse to one id and a node silently disappears.
13. `flux-ir/src/lower/mod.rs:568-578` — ForEach `signal_deps` omits the key expression's signal reads; a key reading a signal never re-renders. Also `checker/infer.rs:183` never verifies `key:` is a function of the element.
14. `flux-ir/src/lower/bytecode.rs:1235-1247` — double `await` in one expression: both results deposit into r0; second resume overwrites first → `await f() + await g()` yields `g+g`. MOV each result to a fresh register.
15. `flux-parser/src/parser.rs:1837-1847` — `ty()` fallback accepts any token as a "primitive" type (`let x: 5`, `x: )` build garbage ASTs). Restrict or error.
16. `flux-parser/src/parser.rs:1366,1016,1131,1614` — unbounded recursion (only `{` nesting is capped) → stack overflow on hostile input. Add a depth counter + parser fuzz target.
17. `flux-parser/src/fmt/expr.rs:380-394,270-294` — fmt drops same-precedence right-operand parens (`a - (b - c)` → `a - b - c`; format-on-save changes semantics) and never re-escapes `{`/`}` (interpolation strings mutate on format).
18. `flux-types/src/exhaust.rs:75-87` — any all-wildcard arm counts as catch-all even for a *different* ADT → non-exhaustive matches pass, then bytecode falls through doing nothing.
19. `flux-types/src/checker.rs:152-163` — `resolve` applies substitution at most 4 passes; deeper chains leak unresolved Vars into recorded types.
20. `flux-types/src/checker/field.rs:83-85` — field access on unknown Named/Variant/List bases silently yields a fresh var; typos hide.
21. `flux-parity/src/equivalence.rs:236-251,50-62,110-129` — `branch_bag_equal` zip-compares (swapped then/else passes); any single-child container elided (dev `Row{Text}` == release `Column{Text}`); `norm_cond` treats every `/` as comment opener (division truncates conditions).
22. `flux-parity/src/persistence.rs:187-197` — storage "byte-for-byte" parity compares presence + type tag only; wrong-value-same-type passes.
23. `flux-cli/src/build.rs:140-148` — Android writes **every compiled source to the constant `"MainActivity.kt"`** in one loop — multiple `.flux` files silently overwrite each other. Also `build.rs:95-108`: entry point = last-declared component (App-first layouts launch the wrong screen); `sources.rs:36-51` skips unreadable files with a warning; scaffold `init.rs:31-32` uses `onClick:` which `collect_handler` silently drops (dev increments, release does nothing).
24. `flux-lsp/src/lib.rs:132-488` — eight `.expect("mutex poisoned")` in server code; `flux-devtools-ui` has unconditional `eprintln!` debug prints in `views/component_tree.rs:60-292`.
25. Encoder desync risk: `wire/patch.rs:54-58`, `wire/value.rs:35-39`, `wire/child.rs:22-27` skip unknown `#[non_exhaustive]` variants after the parent count was written — one skip desyncs the whole stream. Error instead.

**Swift host (remaining)**
26. `FluxBytecodeVM.swift:1152-1417` — `runViaDispatchTable` (tests-only, but public): CALL_CAP lacks the FLUX-049 permission gate; `default: fatalError` on LIST_*/IS_NULL/AWAIT. Gate it, add the ops, or delete it.
27. `FluxValueJsonParser.swift:43-53` (+ Kotlin twin `FluxValueJson.kt:60-70`) — JSON objects parse to records whose field values are the interned **key names**; actual values discarded → `Http.getJson` cannot return data. Verify against the FLUX-047 contract; fix both sides together.
28. `WireDecodeTests.swift:76-169`, `DeserializeAllocPerfTests.swift:40` — hand-built frames omit the 1-byte kind field → against current decoder/Rust encoder these tests read garbage; either red in CI or not running. Fix layouts.
29. `FluxWebSocketTransport.swift:102-147` — in-flight receive fails after explicit `close()` → `handleDrop` schedules reconnect; user-closed transports resurrect themselves. Add a user-initiated-close flag. `appendLog` (not DEBUG-gated, unbounded file growth, force-unwrap) → `os_log`.
30. `FrameDeserializer.swift:186,200` — `handlerCount` parsed but ignored; blob section read unconditionally (brittle against the encoder contract). Gate on handlerCount or assert consistency.
31. `ShadowTreeReconciler.swift:883-972` — prop thunks run with `capRegistry: .dev` and `AllowAllPermissionChecker()` → a prop thunk can invoke any capability with full permissions; thunk throws silently degrade to stale props. Thread the executor's checker; surface faults.
32. `ShadowTreeReconciler.swift:646-667` — ForEach row ids derive from element **index**, discarding the wire splice `key: UInt64` → mid-list removal shifts every subsequent row id (destroy+recreate; FLUX-092 family). Same on Android (`ShadowTree.kt:927`) — see drift matrix D2.
33. `CrashReporter.swift:11` — "RELEASE-TODO: wire SignalExceptionHandler/NSSetUncaughtExceptionHandler" — release crash reporting is shape-only, never installed.
34. `FluxAppMain.swift:73-75` — `fatalError` on malformed `FLUX_WS_URL` at launch → degrade to loopback default + banner.
35. `FluxExecutor.swift:197,209` — server span mapped as `SourceSpan(fileID:line: s.start, column: s.end)` — byte offsets rendered as line/column in the overlay.

**Kotlin host (remaining)**
36. `ShadowTree.kt:1183` — sign-extends bytes in the local FNV (`toByte`-based) vs correct `toUByte` in `PropsIndex` — two FNV implementations on the same platform disagree.
37. `shadow/DirtyReconciler.kt` + `SignalGraph.kt` — propagation model is batched but `SignalGraph.observe/invalidate` have zero callers (dead reactive layer); document or delete.
38. `runtimes/android/app/src/main/AndroidManifest.xml:12` — `usesCleartextTraffic="true"` app-wide with no network-security-config limiting cleartext to the dev server.
39. `artifact-publish.yml:10-11 vs 87-98` — header claims host AAR; job publishes a debug APK (host module is pure kotlin-jvm). Stale docs + debuggable distribution.

**Stdlib / fixtures / adapters**
40. `fixtures/wire/` contains only README.md — `unsupported-version.bin` doesn't exist; Kotlin test silently skips (`assumeTrue`), Swift skips unless env var set, and the claimed Rust regeneration test doesn't exist → the FLUX-083 three-decoder version gate **never runs**.
41. `stdlib/README.md` — claims 12 files (29 exist) and cites `crates/flux-parser/tests/stdlib.rs` as the CI gate; that file doesn't exist. The only real gate is manual `parse-check.sh`.
42. Props declared in stdlib but read by **neither** kit: `TextInput.keyboardType` (index defined `PropsIndex.kt:75`, unread), `TextInput.ref`, `Text.overflow`. Type-checker accepts; both platforms drop.
43. `Toggle` is registered in both kits (`FluxUiKit.kt:46`, `AdapterKit.swift:371`) but no `stdlib/toggle.flux` exists (todo example removed it); `WebHost` likewise read via `src` prop by both kits but undeclared in stdlib.
44. iOS `TextAreaAdapter.swift:45,49` — placeholder written into `view.text` (round-trips as real text through onChange) and a height constraint added on **every** update (constraint pile-up). Kotlin has neither bug.
45. iOS `ImageAdapter.swift:58` — requires width AND height together; stdlib/Kotlin treat them independently.
46. `check-grammar.mjs` fails today: committed `syntaxes/flux.tmLanguage.json:44` lacks `record` (lexer supports it).

**P3 — Low / hygiene (batch into a cleanup PR)**
- Swift: `fdiv` −0.0 sign handling (`FluxBytecodeVM.swift:1422-1428`); latency math ÷1e6 mislabeled ms + hardcoded `statusCode: 200` (`HttpCapabilities+Entries.swift:140-145`); `internStringFrameBytes` sends protocol v1 + `UInt16` trap on >64 KB (`InternString.swift:82-93`); `assertCanonicalStringId` has zero production callers; `SignalGraph.observe/invalidate` dead; `FrameDeserializer.swift:89-93` dead `dbg` closure; `UserDefaults` used as a log (`FluxExecutor.swift:667`); `HelloFrame.swift:8` stale doc (says version 1); boxed `as!` casts (`FluxBytecodeVM.swift:531,1065`); `fluxTrace` never called.
- Rust: `FRAME_HOST_ANNOUNCE` and `FRAME_AWAIT_SUSPEND` both 0x12 (`telemetry.rs:34` vs `resume.rs:27`); `intern_into` interns `""` for non-UTF-8 (`frame.rs:1160-1164`); leftover per-node `tracing::debug!` in `build_init` (`pipeline.rs:743-746`); `validate_bytecode` pass-2 is O(n²) (`wire/core.rs:160-188`); `tree.rs` `Vec::contains` dedup O(n²) + "BFS" comment describes DFS; `reattach_pairs` quadratic (`algorithm.rs:159-189`); XOR-fold multiset hashes can cancel duplicates (`encode.rs:97-108`, `content_address.rs:150-157`); `SourceExcerpt::from_span` can panic mid-UTF-8 (`ids/span.rs:91-100`); `flux-vm-ref` EqF64 NaN==NaN true, `truthy()` accepts any Int, StrConcat overflow wraps, AWAIT ignores `result_reg` operand.
- Kotlin/CI misc: `ShadowTree.kt:25` vs iOS non-canonical string-id spaces differ (0x8000_0000-bias vs ≥0xC000_0000); `android-check.yml:82`/`compat-matrix.yml:127` `set -uo pipefail` without `-e`; hardcoded simulator names differ across workflows; no `distributionSha256Sum` on the gradle wrapper; `benchmarks.yml:19` skips bench on PRs while release-gate needs it (skipped = green); `checkout@v4` vs v5 inconsistency; duplicate `## [Unreleased]` in CHANGELOG + no FLUX-092 entry + website-check.yml also claims ticket "FLUX-092"; `FluxUIKit.swift:6-8` says "the nine declarative adapters" (27 registered); `Router.initialRouteName` dead (`router.flux:19`) + `capability Router`/`compo Router` name collision; event-verb vocabulary inconsistent (`onPress`/`onChange`/`onValueChange`/`onGesture`).
- Fuzz gaps: only `decode_frame.rs` exists. Unfuzzed socket-facing decoders: `TelemetryFrame`, `DebugCommandFrame`, `AwaitSuspendFrame`/`ResumeFrame`, `DispatchReport`, `HostAnnounceFrame`, intern-string paths, `decode_value_blob`, `validate_bytecode`. No fuzz targets for flux-differ or flux-syntax.

---

## 4. Kotlin ↔ Swift drift matrix (the cross-platform contract)

Severity: **CRITICAL** = breaks cross-platform consistency or corrupts state; HIGH = visible behavior difference; MED = divergence you'll hit in real apps; LOW = cosmetic/hygiene. "Fix side" = the side that should change (that's where the correct behavior already exists or the change is smaller).

| ID | Subsystem | Kotlin (Android) | Swift (iOS) | Severity | Fix side |
|---|---|---|---|---|---|
| D1 | ForEach row/child id derivation | `ShadowTree.kt:927-929` — `foreachId*2654435761 + i*40503 + 0x9E3779B9`; child `(rowId*K1 ^ origId*K2) ^ 0x55555555`, **no marker bits** | `ShadowTreeReconciler.swift:647-667` — FNV `&* 0x01000193` with `\| 0x8000_0000` / `\| 0xC000_0000` markers | **CRITICAL** | Adopt one generated scheme (Swift's collision-safe form) on both; add shared golden vectors. Android's unmarked ids can collide with real server node ids |
| D2 | ForEach row identity | Row keyed by **positional index**; splice `key:` ignored (`ShadowTree.kt:868,890`) | Also index-keyed; splice `key: UInt64` discarded (`:646-653`); and stale rows never torn down | **CRITICAL** | Both: derive row ids from the splice key (examples/todo already passes `key: fn(t) { t.label }` — dead in dev); Swift additionally prunes stale rows |
| D3 | LIST_INSERT/LIST_REMOVE | Widths 3/2, operands `u8(0..2)` ✅ (`Opcode.kt:76-77`, `StepResult.kt:280`) | Widths **4/3**, remove reads `u8(1),u8(2)` → pc misalignment + wrong registers (`OpCodes.swift:187-188`) | **CRITICAL** | Swift: widths 3/2, positions 0-based; add ISA vectors (none exist on iOS) |
| D4 | Capability permission table | `Permission.kt` covers caps 1–15 incl. Http/Persist | `Permission.swift:65-82` missing caps 14/15 → **Http/Persist hard-denied** | **CRITICAL** | Swift: add cases 14→`.network`, 15→`.storage` |
| D5 | Color/Font record decoding | `Props.kt:58-76` expects **FNV-hashed field names** | `Color.swift` (r=0,g=1,b=2,a=3), `TextAdapter.swift:58` (size=field 1) positional | **CRITICAL** | Server lowers stdlib `Color = RGB(Float,Float,Float)` / `Font(...)` **positionally** → colors/fonts silently dropped on Android. Port Swift's positional decode to Kotlin |
| D6 | `alignment` prop | `TextAdapter.kt:42` expects a **string**; Column/Row **never read alignment** (`LinearAdapters.kt:59-61`, `STACK_ALIGNMENT` unused) | `TextAdapter.swift:35`, `ColumnAdapter.swift:37` expect record `{0:"start"\|"center"\|"end"}` | HIGH | stdlib declares an ADT (`Alignment.Leading/Center/…`) that the IR can't even encode yet (`lower/mod.rs:934,940` lowers variant idents to Null). Pick ONE encoding (record of int), implement in IR + both kits |
| D7 | Signal graph propagation | Batched: `pending` + `flush()` in id order (`SignalGraph.kt`) | Synchronous notify inside `write` (`SignalGraph.swift:117-129`) — intermediate states visible; observe/invalidate dead | HIGH | Make Swift match Kotlin's batched/topological flush (Android is the reference) |
| D8 | Handshake protocol version | Accepts v1+v2 (`FrameDeserializer.kt:43`) | v2 only | MED | Kotlin: drop v1 (Rust encoder emits 2) |
| D9 | Malformed node-kind policy | Degrades per-node (skip node) | Fails the whole frame | MED | Pick one (per-node degrade + error surface) on both |
| D10 | Splice keys | u64 key truncated to u32 (collision risk) | Full u64 kept | MED | Kotlin: keep u64 |
| D11 | Presence flags | `!= 0` | `== 1` (a `2` decodes differently) | MED | Both: `!= 0` |
| D12 | StorageBackend | MessagePack + atomic-rename; corrupt entry **silently deleted**; NaN persists | JSON + CrashReporter; corrupt entry **persists and fails every read** | MED | Unify: atomic rename + quarantine-delete + NaN rejection on both |
| D13 | HTTP capability defaults | 15s timeout; error → `"{}"` body, status 0 telemetry | No timeout; error → `""` body; **always emits statusCode 200** to DevTools | MED | iOS: add timeout, propagate real status/failure |
| D14 | Absent-prop policy | Resets to `""`/0.0 on absent | Retains old value | MED | Pick one (retain is usually right for hot reload); encode in Appendix F |
| D15 | Gesture default `kind` | Missing kind → no recognizer | Missing kind → **longPress** attached | MED | Missing kind → error surface on both (it's a programming error) |
| D16 | `Stack` container | z-overlay | Vertical list | MED | One semantic (z-overlay per stdlib docs); fix iOS |
| D17 | Error overlay | Full ADR-0057: snippet + caret + tiers | Message + span only; both render byte offsets as line:col | MED | Port Android's overlay to iOS; map spans to line/col on both |
| D18 | `WebHost` missing `src` | Clears the view | Keeps last page; only iOS enforces http/https scheme | MED | iOS: clear on absent; make scheme policy shared |
| D19 | TextInput placeholder/caret | Placeholder persists; no caret-change events | Placeholder clears on absent; fires `onChangeText` on caret moves (`textFieldDidChangeSelection`) | MED | iOS: don't fire on selection changes; retain placeholder semantics |
| D20 | ScrollView children | Content host stable | `setChildren` **detaches its own content host** (grabs `subviews.first`, then removes *all* subviews incl. that host) → blank scroll view; `orientation` never applied | HIGH | iOS: keep the content host, swap its arranged subviews; apply orientation |
| D21 | Image cache | LRU memory-only (`ImageCache.kt`) | URLCache disk+memory (`ImageCache.swift`) | LOW | Acceptable drift — but encode the contract (memory-only vs disk) in Appendix F |
| D22 | Non-canonical string-id spaces | 0x8000_0000-biased fallback | ≥0xC000_0000 local interning | LOW | Unify the reserved ranges in the spec |
| D23 | Prop-thunk permission context | Executor's checker threaded | `AllowAllPermissionChecker` + `.dev` registry in `materializeProps` | HIGH | iOS: thread the real checker/registry |
| D24 | Test coverage symmetry | Has: list-op vectors, a11y tests, registry test, storage-docatch, ScrollView | Has: color/font decode tests, Button/TextInput/Text/Props dedicated tests | MED | Port each side's missing tests (see §6) |
| D25 | Codegen (release path) | Kotlin backend: missing imports, `items()` outside LazyListScope, nonexistent `withAnimation`, Toggle hardcode, leading commas, no zero-field variants, `await` untranslated | Swift backend: structurally valid output; TextField wired to `onEditingChanged` (Bool) with `.constant` binding = read-only field | **CRITICAL** | See H21/H22 + asymmetry list in §5 |

**Subsystem verdict:** Kotlin is ahead on VM/opcode decode, signal graph, error surfacing, layout adapters; Swift is ahead on prop-record decoding and id-collision safety; neither is ahead on ForEach identity (both index-keyed, different hashes). The wire frame layer itself is byte-identical — the drift concentrates at the **semantic edges** (props, identity, capabilities).

---

## 5. Release codegen asymmetry (SwiftUI vs Jetpack Compose) — dedicated list

These produce **different released apps from the same source**, which the parity harness cannot see (H24):

1. **Toggle** — shared emitter hardcodes Swift `Toggle(isOn: .constant(v)) {}` for both backends (`codegen-core/emitter.rs:471-480`) → invalid Kotlin.
2. **ForEach** — Swift `ForEach(c, id: \.id)` valid anywhere; Kotlin `items(c, key=…)` requires a `LazyListScope` that's never emitted (`codegen-kotlin/backend_impl.rs:97-98`).
3. **Animate** — Swift `withAnimation(Animation.x)` exists; Kotlin emits the same name (nonexistent) and drops `bouncy`/`smooth` curve mapping.
4. **Button styles** — Cupertino→`.bordered` vs rounded-shape-with-missing-import; Material→`.borderedProminent` vs plain Button (distinction lost).
5. **Container spacing axis** — Swift `(spacing:)` axis-agnostic; Kotlin always `verticalArrangement` (wrong axis for Row).
6. **Component header** — Kotlin emits leading commas (`    , other: String`, pre-2.2 syntax) and no `data object` for zero-field variants.
7. **Guard/match arms** — Guard pattern → Swift `default:` (catch-all swallowing later arms) vs Kotlin `is Type ->` (exact) — **different semantics from identical source**.
8. **TextField** — Swift `onEditingChanged` (Bool callback, `.constant` binding) vs Kotlin `onValueChange` (String) — the Swift field can't display typed text.
9. **Router** — Swift hijacks a state literally named `route` (`$route`) into `NavigationPath()` regardless of declared type; Kotlin hardcodes `startDestination = "home"`.
10. **String escaping** — Kotlin additionally needs `$` → `\$`; repo's own scaffold `"tapped $${count} times"` is an immediate Kotlin compile error.
11. **Async** — `render_handler_body` emits Swift's `await` verbatim inside Kotlin's `GlobalScope.launch { … }`.

---

## 6. The missing safety nets (why CI stayed green while all of the above shipped)

1. **Differ test suite can't compile** (H13) — the crate most responsible for hot-reload correctness has zero running tests.
2. **Parity harness ignores props** (H24) and elides single-child containers — dev/release divergence on button labels, colors, branch selection is invisible.
3. **Capability tests bypass the permission gate** — Swift Http/Persist denial (C10) undetected.
4. **FLUX-092 regression test bypasses the seam it protects**: `ForEachRemoveBugTest.kt:187` dispatches with `forEachRowContext.keys.first()` (an id from the map itself) instead of the id `RenderButton` actually forwards — the renderer→dispatch seam that broke on device has no JVM coverage.
5. **Wire fixture gate never runs** (fixture file missing; Kotlin `assumeTrue` skip; Swift env-gated skip; Rust test absent).
6. **ISA vector coverage gaps**: no list-op vectors on iOS; oracle itself panics on list ops (C11); `Opcode::ALL` missing `BoolEq`.
7. **Size gate no-op** (C14), **perf gate no-op** (H31), **release gate dead** (C13), **adr-numbering workflow can never trigger** (paths don't exist), **compat-matrix** `continue-on-error: true` while release-gate `needs` it, **no workflow defines `concurrency:`** (16/16), **zero third-party actions pinned by SHA**, **benchmarks skipped on PRs but release-gate trusts the result**.
8. **No stdlib↔adapter contract sweep** — nothing fails when stdlib declares a prop (`keyboardType`, `ref`, `overflow`) or a component (`Toggle`, `WebHost`) that one or both kits never read, or when an adapter expects a different encoding than the IR produces (`alignment`, `Color`, `Font`).
9. **`tests/isa-vectors/` is referenced by `project.yml:89`, `android-check.yml:171`, `mutation-testing.yml` but absent from the repo** — the "tri-platform parity vector" surface isn't backed by files.
10. **UI tests defined but not in the scheme** (`project.yml:62-104` — `FluxAppUITests` never runs); iOS lacks ScrollView/registry/storage-docatch tests; Android lacks color/font-decode tests.

---

## 7. What's verified-good (don't churn these)

- Host-side wire **decoders** (Kotlin `ByteReader`/`FrameDeserializer`, Swift `ByteReader`/`FrameDeserializer`): bounds-safe, byte-compatible with the Rust encoder for Init/Delta/Error incl. field order, magic `0x465C5558`, version 2.
- Kotlin **StepResult** list ops + CALL_CAP permission gate: correct and oracle-consistent.
- `#\[forbid(unsafe_code)]` holds across flux-ir/flux-types — zero `unsafe`; arena is safe.
- Jump-target patching in `bytecode.rs` is correct vs the VM decoder; div/rem-by-zero guarded; gas/HALT contract honored; decode is total.
- `check-ownership.sh`/`dump-files.sh` sound; iOS 16 deployment target + Swift 6 mode consistent across both Swift packages; Android version catalog matches the hardcoded CI BOM.
- `capabilities.flux` ↔ `CAPABILITY_IDL` ↔ both `CapabilityRegistry`s agree for caps 1–15 (table content, not the Swift gate wiring).
- `parse-check.sh` genuinely validates all 29 stdlib files (it's just the *only* gate — see §6.8).
- FLUX-092 as originally reported (wrong row removed) is **fixed**: the renderer forwards the button leaf id, `seedRowContext` resolves it, and the item slot is written per row before dispatch. The remaining defect is C12 (destroyed-row reuse), not the original bug.

---

## 8. Production-readiness roadmap (ordered, with exit criteria)

### Phase 0 — Unblock CI (1–2 days) → *exit: release-gate, android-check, wire-fuzz runnable*
1. Commit `docs/release/contract-versions.toml` (C13) and `gradle-wrapper.jar` (C13).
2. Restore or delete `crates/flux-differ/src/diff/{common,patch_tests,reattach_tests}.rs` (H13).
3. Fix size-gate regex (C14); delete stale `MANIFEST_REQUESTS.md` row + TOML-validate the steward (C14); un-ignore `fuzz/corpus` (H28).
4. Delete or repair `scripts/generate_for_each_hashes.sh` (H30); add `permissions:` + `concurrency:` to all workflows; SHA-pin third-party actions.

### Phase 1 — Language correctness (the compiler must not lie) (1 week) → *exit: no silent miscompiles*
1. C1 register allocator (free temps or error).
2. C2 typed opcode selection + C3 `?.` field index + C5 variant tags + C4 literal match — one pass over `compile_value`/`compile_call` keyed off checker-recorded types.
3. C6 inline NodeId duplication; H19 generics supply; H20 discarded type errors; H18 lexer negative literals.
4. Gate: new conformance suite — every expression form compiles and runs identically through flux-vm-ref with golden vectors.

### Phase 2 — Wire, differ, devserver trustworthiness (1 week) → *exit: hot reload can't corrupt host trees; LAN mode is actually protected*
1. C7 auth bypass + H15 bounded queues; H17 AsyncBridge lifecycle; H16 watch/broadcast race.
2. C8 insert ordering; C9 multi-root inserts; H14 u16 checked casts; emit.rs handler patches (P2.5); compare.rs captured_signals (P2.6).
3. Gate: fuzz targets for all socket-facing decoders; differ golden tests for multi-insert/remove/move/reattach.

### Phase 3 — Host runtimes (1–2 weeks) → *exit: capabilities work, no leaks, no main-thread freezes, errors surface*
1. Swift: C10 (permission table + single store), D3 list widths, H1/H2 VM parity, H3/H4 reconciler identity + leak sweep, H12 thunkBlobs merge, H5/H10 error surfacing, H8 async transport, H6/H7 native capability host.
2. Android: C12 reconcile id-space unification; D1 row-id scheme unification (with shared golden vectors); D2 splice-key row identity on both.
3. Cross-platform: D5/D6 prop decode (one encoding, both kits), D7 signal flush, D8–D19 policy unification (one PR each, encoded in Appendix F).
4. Gate: `tests/isa-vectors/` actually committed and run on all three runtimes; ForEach E2E through the real renderer seam; leak test (reconcile 1000 list mutations, assert `built`/`nodes` bounded).

### Phase 4 — Release codegen (3–5 days) → *exit: `flux build` output compiles on both platforms for the stdlib examples*
1. H21 string escaping (incl. Kotlin `$`); H22 Kotlin prelude/ForEach/Animate; Toggle/backend hooks (§5.1); TextField (§5.8); Guard-arm semantics (§5.7); build.rs output naming + entry point (P2.23).
2. Gate: CI step that runs `flutter`-style "compile the generated code" for examples/counter, todo, router with both native toolchains (make the release gate real, not optional).

### Phase 5 — Parity + contract enforcement (3–5 days) → *exit: the harness would have caught everything in §4*
1. H24 props in the parity model; fix the four equivalence holes (P2.21); recognizer strictness; persistence value comparison (P2.22).
2. Add the stdlib↔kit prop sweep (§6.8), the missing `native_kit_parity` test (H29), wire fixtures + three-decoder version gate (P2.40), and the test-symmetry ports (D24).

### Phase 6 — Release hardening (2–3 days) → *exit: shippable artifacts*
1. Android R8+shrink+signing (H26); cleartext traffic scoped to dev (P2.38); publish AAR not debug APK (P2.39).
2. Swift telemetry `#if DEBUG` compile-out (H11); install crash handlers (P2.33); fix perf-gate (H31) and compat-matrix staleness (P2.13 in CI list).
3. Debug-output purge: devtools `eprintln!`s, Swift `print(`, Kotlin test `println`s, `pipeline.rs` per-node debug loop, UserDefaults logging.
4. CHANGELOG repair (duplicate Unreleased, FLUX-092 entry, phantom docs references).

**Definition of done for "production ready":** Phases 0–3 complete and green, Phase 4 gate compiling all examples for both platforms, Phase 5 harness red-on-revert for at least one finding from each P0/P1 category, and the release gate publishing signed, minified artifacts from a tag.
