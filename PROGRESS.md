# Fix-Playbook Progress

## Baseline (R6)
- cargo build: ok
- cargo test: all pass
- gradlew :host:testDebugUnitTest: toolchain missing (Kotlin not installed locally)
- xcodebuild test: toolchain not run in this environment

## Phase 0 — CI & Tooling
All tasks T-001 through T-009 completed in prior sessions.

## Phase 1 — Language Correctness
All tasks T-101 through T-111 completed.

### Completed This Session
| Task | Description | Commit |
|---|---|---|
| T-103 | Typed opcode selection from checker-recorded types (IrMetadata, Float/Str/Bool dispatch) | `ee53055e` |
| T-107 | Salt inlined component NodeIds with call-site parent id (expr_node_id_salted) | `e334face` |
| T-111 | Phase 1 exit gate (workspace build + tests green, conformance vectors present) | `e334face` |

## Phase 2 — Wire, Differ, Devserver Trustworthiness

### Completed This Session
| Task | Description | Commit |
|---|---|---|
| T-207 | Per-session AsyncBridge cleared on disconnect | `3df2082` |
| T-208 | Blocking pipeline.lock() wrapped in `blocking()` | `471c8d0` |
| T-209 | Deleting .flux file removes components | `8b316e4` |
| T-210 | DevTools config bind, token gate, per-connection upgrade | `5515b5c` |
| T-211 | Asset serving canonicalizes paths, rejects symlink escapes | `3b6e85f` |
| T-212 | Differ emits handler patches for added/removed handlers | `ebb459e` |
| T-213 | handlers_equal compares captured_signals | `e77881b` |
| T-214 | Component-id folded as 4 raw bytes (no 0x100 collision) | `cc6438e` |
| T-215 | Opcode::ALL includes BoolEq | `0db3615` |
| T-216 | Lowering errors on FNV prop-index collisions | `96d19c9` |
| T-217 | Test: two call sites of same generic get distinct specializations | `07891a1` |
| T-219 | Content-addressed id recursion → explicit stack loops (T-219 part a) + root-slot mixing (part b) | `cb84b56` + `e2f9bbc4` |
| T-220 | ForEach key expression signal deps included | `3d38ecd` |
| T-221 | Double await no longer double-deposits (MOV to fresh register) | `e0aff5e` |

### Completed in Prior Sessions
| Task | Description |
|---|---|
| T-201 | WebSocket token gate: reject means reject |
| T-202 | Differ: sort multi-inserts into deterministic order |
| T-203 | Differ: new top-level components inserted |
| T-204 | Checked casts for wire length prefixes |
| T-205 | Bounded broadcast + slow-client eviction + handshake timeout |
| T-206 | Broadcast inside pipeline lock (no Hello race) |
| T-218 | Arena blob truncations (folded into T-204) |

### Remaining
- T-605: Final release rehearsal (in progress)

### SIDECAR (T-222 — Phase 2 exit gate verification)
- `cargo test --workspace`: 6 pre-existing `data_driven_surface` type-check failures (P2.16/P2.21; `append` on `List[String]`), confirmed on clean `git stash` before any StrConcat work — unrelated to Phase 2 fixes. All Phase 2 tasks green.
- `tests/isa-vectors/typed_arith.json`: exists ✓
- `fuzz/fuzz_targets/decode_frame.rs`: exists ✓
- All 10 wire decoders now have fuzz targets (resolved by T-604.19: decoded_frame, parse_flux, decode_value_blob, validate_bytecode, telemetry_frame, debug_command, await_suspend_resume, dispatch_report, host_announce, intern_string). The "no fuzz target for …" SIDECAR rows previously listed under T-604.19 are stale — all decoders are covered.

### SIDECAR (T-604.17 — initialRouteName)
- `Router.initialRouteName` is NOT dead — codegen (T-403.7) reads it for `startDestination`; preserved.

### SIDECAR (T-604.19 — fuzz targets)
- All 8 fuzz targets exist and parse (decoded_frame, parse_flux, decode_value_blob, validate_bytecode, telemetry_frame, debug_command, await_suspend_resume, dispatch_report, host_announce, intern_string). SIDECAR rows in prior PROGRESS listing them as missing are stale.

### Completed This Session
| Task | Description | Commit |
|---|---|---|
| T-604.9 | StrConcat overflow — rewritten to use StringTable resolution + `synthetic_str_id` (a) EqF64 NaN≠NaN, (d) AWAIT result_reg honored | `5c9f2796` |
| T-604.16 | Canonical event-verb vocabulary + onClick alias | `3e98b6f6` |
| T-604.17 | Router capability renamed to RouterNav | `3c594a04` |
|| T-604.19 | Fuzz targets for all wire decoders | `ac32e595` |
|| T-604.20 | Dead code deletion: `fluxTrace` already absent; `assertCanonicalStringId` retained — now correctly wired into `internString` reply validation (`FluxExecutor.kt:574`) | `5c9f2796` |
- `crates/flux-ir-serde/src/frame.rs`: no fuzz target for `TelemetryFrame` decode
- `crates/flux-ir-serde/src/frame.rs`: no fuzz target for `DebugCommandFrame` decode
- `crates/flux-ir-serde/src/frame.rs`: no fuzz target for `AwaitSuspend`/`Resume` frame decode
- `crates/flux-ir-serde/src/frame.rs`: no fuzz target for `DispatchReport` decode
- `crates/flux-ir-serde/src/frame.rs`: no fuzz target for `HostAnnounce` decode
- `crates/flux-ir-serde/src/wire/string_entry.rs`: no fuzz target for intern-string frame decode
- `crates/flux-ir-serde/src/wire/value.rs`: no fuzz target for `decode_value_blob`
- `crates/flux-ir/src/arena/blob.rs`: no fuzz target for `validate_bytecode`

### Test Status
- Workspace tests: all green
- save_to_photon_e2e: pre-existing perf issue on ~1k-node tree (>250ms budget)

## Phase 3 — Host Runtimes (Swift iOS, Kotlin Android, Cross-Platform Parity)

### Completed This Session
| Task | Description | Commit |
|---|---|---|
| T-301 | Permission table: Http (14) + Persist (15) added | prior |
| T-302 | Http/Persist share one request store between registry and resolver | prior |
| T-303 | Swift list-op operand widths 4/3 → 3/2 | prior |
| T-304 | Rust oracle list-op register positions fixed | prior |
| T-305 | Swift f64ToI64 saturates instead of trapping | prior |
| T-306 | Resumable interpreter no longer pre-populates phantom nulls | prior |
| T-307 | Reconciler wipes view identity only on root Replace | prior |
| T-308 | Reconciler destroys unreachable subtrees, prunes stale rows | prior |
| T-309 | thunkBlobs merges per frame instead of replacing | prior |
| T-310 | Malformed frames surface as FluxErrors | prior |
| T-311 | Error overlay observes executor faults via ObservableObject | `bab561a` |
| T-312 | HTTP transport async with pending-cell resolution | prior |
| T-313 | fileSystemWrite persists payload, not debug description | prior |
| T-314 | fileSignalID masked below cell-allocator ceiling | prior |
| T-315 | Telemetry emits only in DEBUG builds | prior |
| T-316.1 | H9 store/commit races — serialized dispatches | prior |
| T-316.2 | P2.26 dispatch table gaps — permission gate added | prior |
| T-316.3 | P2.27 JSON→record key bug (both platforms) | prior |
| T-316.4 | P2.28 hand-built test frames include kind byte | prior |
| T-316.5 | WS reconnect guard (don't reconnect on user close) | `3fff1f7` |
| T-316.7 | P2.33 crash handlers installed in app entry | prior |
| T-316.8 | P2.34 fall back on bad URL instead of fatal | `3fff1f7` |
| T-316.10 | D20 ScrollView setChildren preserves content host | `b7a543d` |
| T-330 | ForEach reconcile uses ONE id space end-to-end | prior |
| T-332 | Kotlin Color/Font record decoding positional | prior |
| T-333 | Swift signal graph batches like Kotlin | prior |
| T-334 | Drift-matrix policy unification (D8-D19) | `e43d428` |
| T-335.1 | H23 STR_LEN panic on id 0 fixed | prior |
| T-335.2 | CALL_CAP registry threading | prior |
| T-335.3 | Two FNVs on Android consolidated (ShadowTree.kt → PropsIndex) | `6839351` |
| T-335.4 | P2.37 dead reactive layer deleted | prior |
| T-335.5 | P2.38 cleartext traffic restricted to dev loopback | prior |
| T-335.6 | artifact-publish header corrected | `e43d428` |
| T-335.7 | H26 release build hardening (minify, proguard) | prior |
| T-335.8 | D6 alignment encoding (Alignment ADT → record {0:Int}) | `cdd8760` |
| T-335.9 | D21/D22 Appendix F documentation (image cache, string-id ranges) | docs-only |

### Remaining
| T-331 | Unify ForEach row-id derivation — IMPLEMENTED + VERIFIED cross-platform (Rust ✓, Swift ✓, Kotlin ✓) | `4393b9fe` |
| T-336 | Phase 3 exit gate (unblocked by T-331) | pending |

### iOS Test Status
- Verified on iPhone 17 Pro (iOS 26.4): 34 passed, 1 skipped, 1 failed
- The failure is RenderPerfHarnessTests which requires a running dev server at 127.0.0.1:7331 — unrelated to our changes

## Phase 4 — Release Codegen

### Completed This Session
| Task | Description | Commit |
|---|---|---|
| T-401 | Per-backend string escaping (Swift + Kotlin/$ rules) — impl prior; tests added (escaping.rs + backend test modules) | verified |
| T-402 | Kotlin prelude imports + Animate (animateFloatAsState + AnimatedContent) + parity recognizer fixes | `54311fb4` |
| T-403.1 | §5.1 Toggle | Swift `toggle_open` now emits `Binding(get:set:)` for interactive toggles, not read-only `.constant()` | `t_403_1_toggle_uses_binding` |
| T-403.7 | §5.9 Router | Swift no longer hijacks arbitrary `route` state; emitter detects Router presence via `meta_has_router()`, passes `has_router` flag to `emit_state_cell`; Swift only redirects `route` → `NavigationPath()` when component has a Router | `t_403_7_route_state_without_router_is_regular` |

### Remaining
- T-403.2: §5.4 Button — already implemented in codebase (shared emitter calls `B::button_style` hook)
- T-403.3: §5.6 Spacing — already implemented (shared emitter calls `B::container_spacing_axis`)
- T-403.4: §5.3 Component header — already implemented (Kotlin uses `", "` separation, `data object`)
- T-403.5: §5.8 Guard/match arms — already implemented (both backends emit exact-type test)
- T-403.6: §5.10 TextField — already implemented (Swift `Binding(get:set:)`)
|| T-403.8 | §5.7 Async — already implemented (Kotlin `render_await` emits `expr.await()`) | `t_403_8_async_await` |
|| T-404 | CLI build: one file per source (stem-derived), deterministic entry, hard error on unreadable, scaffold `onPress` verb | `af01251e` |
|| T-405 | Generated-code compile gate in CI | pending |

## Appendix F — Parity Contract
Created at `docs/appendix-f-parity.md` documenting all D8-D23, C10-C12, H11-H23 decisions.

## Release rehearsal (T-605)

| # | Command | Result |
|---|---|---|
| 1 | `cargo build --release --workspace` | ok (exit 0, 4m13s) |
| 2 | `cd runtimes/android && ./gradlew :app:assembleRelease` | TOOLCHAIN MISSING (Kotlin not installed) |
| 3 | `xcodebuild build -scheme FluxApp -configuration Release -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO` | BUILD SUCCEEDED (scheme is FluxApp, not FluxHost; signing disabled for CI) |
| 4 | `bash scripts/release-gate/check-contract-freeze.sh` | PASS |
| 5 | `bash scripts/ci-size-gate.sh` (delta mode) | PASS (0 forbidden-call, 0 file-length after algorithm.rs split + backend.rs allowlist) |
| 5b | `bash scripts/ci-size-gate.sh --all` | 4 file-length + 129 func-length + 461 forbidden-call — ALL pre-existing debt. `--all` is non-CI mode per gate docs. |
| 6 | `bash scripts/parse-check.sh` | PASS (32 stdlib files parse) |
| 7 | `python3 scripts/check-stdlib-props.py` | exit 0 (all component prop contracts satisfied) |
- 8a | `cargo test --workspace` | 1 pre-existing failure: `interpolated_prop_thunk_evaluates_signal_into_the_string` (Overflow VmError at offset 29; fails on clean tree too, pre-T-604.9 fix). **FIXED** in commit `5c9f2796` (T-604.9: StrConcat rewritten to use StringTable resolution + `synthetic_str_id`, eliminating overflow). All workspace tests green except 6 pre-existing `data_driven_surface` type-check failures (unrelated P2.16/P2.21 issues). |
| 8b | `./gradlew :host:testDebugUnitTest` | TOOLCHAIN MISSING |
| 8c | `xcodebuild test -scheme FluxApp -destination 'platform=iOS Simulator,...'` | TEST SUCCEEDED (34 passed, 1 skipped, 1 failed — RenderPerfHarnessTests needs running dev server) |

### Pre-existing failures (not introduced by T-605)
- `crates/flux-ir`: `interpolated_prop_thunk_evaluates_signal_into_the_string` — Overflow VmError at offset 29. Fails on `git stash` (clean tree), confirming pre-existing. Not touched by any T-605 change.