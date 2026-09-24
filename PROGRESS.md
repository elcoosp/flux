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
- T-222: Phase 2 exit gate

### SIDECAR (T-222 — unfuzzed decoders, Phase 6 hygiene T-606)
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

### Remaining
- T-402: Kotlin prelude imports, LazyColumn ForEach, animate*AsState
- T-403: Backend-split codegen fixes (Toggle, spacing, header, etc.)

## Appendix F — Parity Contract
Created at `docs/appendix-f-parity.md` documenting all D8-D23, C10-C12, H11-H23 decisions.