# Appendix F — iOS/Android Parity Contract

This document records the canonical cross-platform behavioral decisions so the two native hosts never drift.

## D8 — Handshake versions
Only wire protocol v2 is supported (`PROTOCOL_VERSION = 2`). Android `FrameDeserializer.kt` rejects v1.

## D9 — Malformed node-kind
Both decoders degrade per-node: skip the node, emit a diagnostic count, and surface via the error overlay. Never fail the whole frame for one bad node.

## D10 — Splice key width
ForEach splice keys are u64 throughout (no u32 truncation at the hash input).

## D11 — Presence flags
Presence is `!= 0` on both platforms (iOS previously deviated with `== 1`; now normalized).

## D12 — StorageBackend
Atomic rename on write (write temp + `FileManager.replaceItem`/rename). Corrupt entry quarantined (deleted) on read. NaN rejected on write. Mirrors Kotlin `StorageBackend.kt`.

## D13 — HTTP defaults
15 s timeout (`URLSessionConfiguration.timeoutIntervalForRequest = 15`). Error body `"{}"` + real status propagation. Shared `HttpRequestStore` between capability registry and async resolver.

## D14 — Absent-prop policy
An absent prop RETAINS the previous value (do not reset to `""` / `0`). Both kits honor this.

## D15 — Gesture default kind
A missing/unknown gesture `kind` is an error surfaced to the overlay, NOT a default recognizer.

## D16 — Stack semantics
iOS `StackAdapter` renders z-overlay (children stacked), matching Kotlin `LinearAdapters.kt` STACK handling.

## D17 — Error overlay
Both platforms map byte-offset spans to line:col using the file text. Shared helper duplicated per platform; unit-tested on both.

## D18 — WebHost missing src
Absent `src` clears the view. Non-http(s) `src` ⇒ error surface.

## D19 — TextInput placeholder/caret
(a) Do not clear the placeholder on absent (retain semantics, D14). (b) Caret moves must NOT fire `onChangeText`.

## D20 — ScrollView
`setChildren` preserves the content host view; only swaps its arranged subviews. Orientation applied (horizontal ⇒ `alwaysBounceHorizontal` + axis-constrained layout).

## D21 — Image cache
Memory-only LRU is canonical (Kotlin). iOS mirrors this.

## D22 — String-id reserved ranges
Kotlin fallback ids ≥ 0x8000_0000; iOS local interning ≥ 0xC000_0000. Documented, no code change.

## D23 — Prop thunk permission
Prop thunks execute under the real permission checker (not `AllowAllPermissionChecker`). Thunk faults surface via `lastFluxError`.

## D5 — Color/Font record decoding
Server lowers Color/Font records **positionally** (fields 0..3 / 0..2). Both kits read by position, not FNV-hashed names.

## D6 — Alignment encoding
Alignment is a record `{0: int}` where 0=start, 1=center, 2=end (mirrors Color's positional record). Both kits read the int.

## D7 — Signal graph batching
Swift signal graph batches writes and flushes in ascending signal-id order (Kotlin parity). Intermediate writes are NOT observable mid-batch.

## C10/D4 — Capability permissions
Caps 14 (Http) and 15 (Persist) map to `.network` and `.storage` permission kinds respectively.

## C11/D3 — List-op operand widths
LIST_INSERT: 3 bytes (list, idx, val). LIST_REMOVE: 2 bytes (list, idx). Both platforms agree.

## C12 — ForEach id space
`desired` is expressed in the same id space as built rows (`deriveForEachChildId`). Teardown only destroys unreachable nodes.

## H11 — Telemetry compile-out
Telemetry emits only in DEBUG builds. Hot path is clean in Release.

## H12 — thunkBlobs merge
`thunkBlobs` merges per frame instead of replacing (stale-thunk loss fixed).

## H23 — STR_LEN
Returns digit count of id as proxy; real string length requires string table access which `exec_tail` doesn't have. Panics fixed for id 0.
