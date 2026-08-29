# ADR-0057: Capability permission threat model

- **Status:** Accepted
- **Date:** 2026-08-29
- **Supersedes / related:** FLUX-049, FLUX-046 (native-module escape hatch),
  FLUX-048 (WebView escape hatch), ADR-0044 (result cells), ADR-0045
  (capability bridge), ADR-0055 (`Result[T, E]`), ADR-0056 (fail-closed
  handshake)

## Context

Capabilities are the *only* extension point out of the sandbox: `CALL_CAP`
reaches camera, storage, clipboard, geolocation, WebView, and arbitrary native
SDKs wrapped as a capability (FLUX-046). If `CALL_CAP` to a gated capability
were not enforced, any `.flux` source (or a malicious frame from a compromised
server) could read the camera or exfiltrate storage. The threat model must
answer: who can call what, when is it denied, and what happens on denial?

Rust already had `gate_call` + a `PermissionKind` table (`flux-types/src/
capabilities.rs`), but the **native hosts did not wire it into `CALL_CAP`
dispatch** — the gate existed server-side only. A host rendering untrusted
frames had no local enforcement.

## Decision

1. **Denied grant is a typed, non-fatal error — never a crash.** A `CALL_CAP`
   to a capability whose `required_permission(capId, methodId)` is ungranted
   throws a typed VM error (`VmErrorKind.CAPABILITY_DENIED` on both Kotlin and
   Swift). The host error path renders it as a red banner (Appendix E §E.6).
   The dispatch returns a settled `Error` result cell (ADR-0044) so `.flux`
   code can read `Result[_, _]` and branch, exactly as ADR-0055 intends.
2. **The gate is threaded through the whole VM dispatch path.** Both hosts take
   a `PermissionChecker` into `FluxBytecodeVM.run` / `runResumable` / `resume`
   and into `FluxExecutor`. The production executor injects an OS-backed
   checker (e.g. `ContextCompat.checkSelfPermission` on Android); the default
   `AllowAllPermissionChecker` preserves the dev loop while keeping the gate
   code live on every path.
3. **`PermissionKind` + `required_permission` are mirrored 1:1 on both hosts**
   (`Permission.kt` / `Permission.swift`) against the Rust table. A capability's
   required permission is derived from `(capId, methodId)`, **never** hardcoded
   per call site, so the two hosts cannot drift from the server.
4. **Escape-hatch capabilities are gated by intent.** `WebView` (cap 12) maps
   to `PermissionKind.None` — it is sandbox-contained (no OS grant, the web
   content cannot reach host APIs); `NativeModule` (cap 13) maps to
   `PermissionKind.NativeModule`, an explicit allow-list grant the consumer
   must approve, because a wrapped third-party SDK runs arbitrary native code.
5. **Unknown capability id is denied, not resolved.** A `CALL_CAP` to a cap id
   the host has not registered fails closed (typed error), so an untrusted
   frame cannot reach an undeclared entry point.

## Fuzz / dispatch coverage note

- The `CALL_CAP` dispatch is the highest-value fuzz target: random
  `(capId, methodId, argsReg)` tuples must never panic, must respect the gate,
  and must settle a cell. The gate + typed-error path makes every malformed
  dispatch a *visible* red banner rather than a *silent* native fault.
- Coverage requirement (both hosts): a denied-grant round-trip test
  (`RuntimeFixesTest.testDeniedPermissionFaultsCallCapAsCapabilityDenied` on
  Android, `CapabilityRoundTripTests.testDeniedPermissionFaultsCallCapAs
  CapabilityDenied` on iOS) asserts the settlement is a typed error and the VM
  returns normally (no throw past the executor). An unknown-cap test asserts the
  same fail-closed behavior.
- Recommended follow-up (not landed): a property test that, for every
  `(capId, methodId)` in the mirrored `required_permission` table, a denied
  `PermissionChecker` yields `CAPABILITY_DENIED`, and a granted one yields the
  same result as `AllowAllPermissionChecker`.

## Consequences

- **Security:** untrusted `.flux` sources and compromised dev servers cannot
  escalate through `CALL_CAP`. The gate is enforced on-device, not just
  server-side.
- **Operability:** a denied grant is a readable banner + a branchable `Result`,
  so app authors handle "permission not granted" as ordinary UI state.
- **Cost:** every new capability must declare its required `PermissionKind` in
  three places (Rust `capabilities.rs`, Android `Permission.kt`, iOS
  `Permission.swift`). That is deliberate — a missing gate entry is a security
  hole, so the friction is the control.
