---
id: FLUX-070
status: done
lane: LANE-F
phase: "Phase 0/2"
blocked_by: []
labels:
  - capability
  - lowering
  - async
  - codegen
  - ios
  - android
source: ADR-0045 §6 (out-of-scope items) — "Capability IDL + codegen declaring `async fn` per method and generating registry entries"
related_adrs:
  - ADR-0044
  - ADR-0045
---

# FLUX-070: Declare `async fn` per capability method and make the lowerer + codegen honor it

- **Lane:** LANE-F (lowering/compiler surface; pairs with FLUX-063's lane)
- **Depends on:** none
- **Source:** ADR-0045 §6 — the unified sync/async bridge is implemented on all three
  runtimes, but the "declaring `async fn` per method and generating registry entries"
  half (the piece that *decides* sync-vs-async) is explicitly out of scope and unstarted.
- **Related ADRs:** ADR-0044 (result cells / suspend), ADR-0045 (signal-as-future bridge)

## Problem Statement

ADR-0045 §3 says the lowering path decides sync-vs-async from a per-method `async fn`
flag in the capability IDL: a `fn` method lowers `CALL_CAP` + direct `ReadSignal`
(cell already `Ready`, no park); an `async fn` method lowers `CALL_CAP` + `AWAIT`
(future_reg == result_reg, cell `Pending` until the host resolves it). Today that
decision is impossible because the flag does not exist:

- `stdlib/capabilities.flux` declares every method as `fn` — there is no `async fn`
  token, so Camera.take / Geolocation.get / Push etc. cannot be marked async.
- `CAPABILITY_IDL` in `crates/flux-types/src/capabilities.rs:76` (`MethodIdl` at
  `:18`) carries only `name` + `id` — no sync/async marker. `CapabilityIdl` has no
  accessor for the flag.
- `emit_call_cap` in `crates/flux-ir/src/lower/bytecode.rs:1060` emits `CALL_CAP` and
  returns `result_reg`, but emits **no** `AWAIT` and never consults any async flag
  (grep `AWAIT` in `bytecode.rs` returns only the `Await` lowering at `:749`, which is
  for expression `await`, not capability calls). So even an async capability call
  lowers to `CALL_CAP` alone and never suspends in compiled code.
- `flux-codegen-{swift,kotlin}` emit **no** `Task {}` / `suspend` capability-call
  sites at all (grep for `async|suspend|await|Task|CALL_CAP` in both crates returns
  0 hits) — the release path does not yet honor async capability resolution.

Until this lands, the ADR-0045 contract is half-built: the VM/executor *can* suspend
and resume (FLUX-064's host halves are merged — `FluxBytecodeVM` `resume` on both
hosts, `AsyncResolver` landed), but nothing in the compiler actually produces a
suspending call for an async capability.

## Solution

1. **IDL flag.** Add `is_async: bool` to `MethodIdl` (`capabilities.rs:18`) and an
   `async fn` token in the `capability` grammar so `stdlib/capabilities.flux` can mark
   methods. Keep `CAPABILITY_IDL` authoritative: `Camera.take`, `Clipboard.get`,
   `Geolocation.get`, `Storage.get`, and the real-sensor async methods are `async fn`;
   pure in-memory sync methods (`Storage.set` dev echo, `Router.navigate`) stay `fn`.
2. **Lowerer.** `emit_call_cap` (`bytecode.rs:1060`) consults the method's async flag
   (resolve via `CapabilityIdl` / `cap_method_id_for`); when async, append
   `AWAIT result_reg, result_reg` after `CALL_CAP` (mirroring ADR-0045 §3). No branch
   in the VM — the cell state drives park-vs-continue.
3. **Codegen.** `flux-codegen-{swift,kotlin}` emit `Task {}` / `suspend` for an awaited
   capability call (release path), reusing the `AsyncResolver` shape FLUX-064 landed.
4. **Registry codegen (ADR-0045 §6).** Generate the `(cap_id, method_id)` registry
   entries from `CAPABILITY_IDL` so the native tables cannot drift from the IDL
   (currently `CapabilityRegistry.kt` `makeDev` and `Registry.swift` are hand-written).

## Implementation Decisions

- Single source of truth remains `CAPABILITY_IDL`; the stdlib `.flux` `async fn`
  annotations and the native registry codegen are derived from it (preserve the
  existing `tests/capability_codegen_parity` / `tests/capability_permission_parity`
  guards).
- Do NOT branch the VM on sync-vs-async — only the emitted opcode changes (ADR-0045 §3).

## Testing Decisions

- Extend `flux-parity/tests/async_resume_wire.rs`: a handler calling an `async fn`
  capability lowers to `CALL_CAP` + `AWAIT`, parks on `Pending`, and resumes to `Halt`
  with the value on cell resolution.
- A lowering unit test asserting `Camera.take` (async) emits `AWAIT` and `Router.navigate`
  (sync) does not.
- Keep `tests/isa-vectors/call_cap_basic.json` green (sync path unchanged).

## Out of Scope

- The Hello-frame capability negotiation (ADR-0045 §6 last bullet) is **already merged**:
  `crates/flux-devserver/src/server/session.rs:157` `handle_hello` validates the host's
  advertised capabilities against `required_capabilities()` and rejects a missing set with
  an `Error` frame. No work needed there.
- The six new concrete capabilities (push/biometric/background/fs/deep-link/sensors) are
  FLUX-045 — separate breadth task.
