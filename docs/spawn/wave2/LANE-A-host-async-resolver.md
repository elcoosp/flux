# LANE-A — Real host AsyncResolver (native async for AWAIT)

**PREREQ:** Wave 1 (esp. LANE-C caps) committed GREEN. Do NOT dispatch until the
capability registry shape (`CapabilityRegistry.dev` / `Registry.dev`) is frozen in `main`.
**Dispatch:** Wave 2. Do NOT delegate; Louis runs this in his own session.
**Owned directories (exclusive):**
- `runtimes/ios/FluxHost/Sources/FluxHost/FluxExecutor.swift` (add real `AsyncResolver`
  conformers; keep `PassthroughAsyncResolver` as the test default)
- `runtimes/ios/FluxHost/Sources/FluxHost/FluxBytecodeVM.swift` (ONLY the resume path that
  calls `asyncResolver.resolve` — NOT the suspend/encode logic; see UNSAFE below)
- `runtimes/android/host/src/main/kotlin/dev/flux/host/FluxExecutor.kt` (real `AsyncResolver`
  impl + `dispatchAsync`/`resolve`)
- `runtimes/ios/Tests/**`, `runtimes/android/host/src/test/**` (add resolver tests)
**UNSAFE (do NOT patch — ADR-0044/0045 in-flight lane):** the VM dispatch SUSPEND/RESUME
sites — `flux-vm-ref/src/vm.rs` (`runResumable`/`SuspendState`), `FluxBytecodeVM.swift`
`runResumable` op-decode, `StepResult.kt` suspend handling. Those are the async agent's
lane. You only supply a RESOLVER that the existing `await` path calls; you do NOT change
how `AWAIT` parks or how `Resume` is decoded.

## Context (grounded)
`AWAIT` (0xE0) parks a handler and captures a future handle into `future_reg`; the host
resolves it via `AsyncResolver.resolve(_:)`. Today both hosts use `PassthroughAsyncResolver`
(iOS, `FluxExecutor.swift:48`) / an inline object (Android, `FluxExecutor.kt:140`) that
returns the handle unchanged — so a handler that `await`s `Camera.take` / a timer / network
suspends forever (the cell stays `Pending`). ADR-0045's signal-as-future cell + AWAIT/RESUME
wire frames are merged; only the real native resolver is missing.

## Tasks (TDD — test a suspended-then-resumed handler first)
1. **iOS real resolver.** Add `URLSessionAsyncResolver` (network) and `TimerAsyncResolver`
   (delay) conforming to `AsyncResolver` (`FluxExecutor.swift:39`). For a `Pending` cell
   holding a capability result-handle, bridge to `URLSession.data(from:)` /
   `Task.sleep`; on completion call `signals.resolveCell(id, value)` (the existing resume
   path). Keep `PassthroughAsyncResolver` as the default so headless tests stay sync.
2. **Android real resolver.** Implement `AsyncResolver` (`FluxExecutor.kt:48 suspend fun
   resolve`) with a `CoroutineAsyncResolver` that bridges `Pending` cells to
   `kotlinx.coroutines` (`suspendCancellableCoroutine` / `delay`); resolves via
   `signalStore.resolveCell`. The existing `dispatchAsync` (`FluxExecutor.kt:282`) already
   awaits `resolve` — keep its shape.
3. **Test.** A handler that `CALL_CAP(1,99)` (async-deferred, allocated `Pending` cell) then
   `AWAIT`s it: assert the handler DOES NOT complete until the resolver settles, then
   resumes with the value; the signal graph state is preserved across suspend (no dropped
   writes). Mirror the oracle `async_deferred` vector on both hosts via a real `FluxBytecodeVM.run`.

## Acceptance gates (DoD)
- iOS: `xcodebuild -scheme FluxApp test` — your resolver tests pass; existing 7 tests still
  green; SwiftLint / `SWIFT_TREAT_WARNINGS_AS_ERRORS` clean.
- Android: `./gradlew :runtimes:android:host:test` — resolver tests pass. (Pre-existing
  ktlint violations in `ShadowTree.kt`/`HelloFrame.kt` are the android-runtime agent's WIP;
  do not sweep — your resolver files must be ktlint-clean on their own lines.)
- No `unreachable!`/`!!`/`try!` in production; every public item documented.
- `git commit --only <your files>` — no `git add -A` (shared index hazard).

## Pitfalls
- Do NOT touch VM suspend/resume decode (UNSAFE above) — if it breaks, flag the async agent.
- Keep `PassthroughAsyncResolver` as the test default so existing sync `await` tests don't
  regress; the live host injects the real resolver.
- A suspended handler must not leak the reactive dispatcher; resolve back onto it (§3.7).
