---
id: PRD-Q
status: open
lane: LANE-Q
phase: "Phase 6"
blocked_by:
  - PRD-K
labels:
  - epic
  - prd
  - capabilities
  - native
  - ios
  - android
  - escape-hatch
source: docs/roadmaps/flux-roadmap-to-1.0.md §1,§6,§12,§13
related_adrs:
  - ADR-0044
  - ADR-0045
---

# PRD-Q: Platform Capabilities Beyond the MLP Five + Native Escape Hatch

- **Lane:** LANE-Q (Phase 6)
- **Depends on:** PRD-K (error contract)
- **Source:** `docs/roadmaps/flux-roadmap-to-1.0.md` §1, §6, §12, §13
- **Related ADRs:** ADR-0044 (first-class async / result cells), ADR-0045 (unified sync/async
  capability bridge), AGENTS.md §3.4 (capability system), PRD-K (FluxError)

## Problem Statement

Only 5 platform capabilities exist (Camera, Storage, Router, Clipboard, Geolocation). For 90% use-
case coverage (1.0 §1.1) Flux needs push notifications, biometric auth, background tasks, file system,
deep linking, device sensors, and — critically — a native-module escape hatch so a team is never
blocked waiting on the framework for an integration Flux did not think of. Every new capability must
follow the existing `CALL_CAP` sync/async pattern and the "denied grant → typed error, never a crash"
contract from PRD-K.

## Solution

Add capabilities following the existing `CALL_CAP` sync/async pattern: push notifications (register/
receive/handle-tap), biometric auth (Face ID / fingerprint), background tasks / app lifecycle hooks,
file system (beyond key-value `Storage`), deep linking / universal links, and device sensors
(accelerometer at minimum). Ship a first-class native-module escape hatch: a documented way to wrap an
arbitrary native SDK as a capability without waiting on the framework team, with the same error
contract.

## User Stories

1. As a Fluff app developer, I want a Push capability (register/receive/handle-tap), so that my app can
   notify users.
2. As a Fluff app developer, I want a Biometric capability (Face ID / fingerprint), so that I can gate
   sensitive actions.
3. As a Fluff app developer, I want Background-task / lifecycle-hook capabilities, so that my app does
   work off the foreground.
4. As a Fluff app developer, I want a file-system capability beyond key-value `Storage`, so that I can
   read/write real files.
5. As a Fluff app developer, I want deep-linking / universal-link handling, so that my app opens to the
   right place from a link.
6. As a Fluff app developer, I want device-sensor access (accelerometer at minimum), so that motion-
   aware apps are possible.
7. As a Fluff app developer, I want a native-module escape hatch to wrap any native SDK as a capability,
   so that I am never blocked waiting on the framework team for an integration.
8. As a Fluff app developer, I want every new capability to return a typed `FluxError` on denial, never
   crash (PRD-K contract), so that permission failures are ordinary control flow.
9. As a Flux core engineer, I want capability ids derived deterministically on server and both hosts
   (AGENTS.md §3.4), so that a capability never desyncs across the wire.
10. As a Flux core engineer, I want async capabilities to settle their result cell via the injected
    `AsyncResolver` (ADR-0044/0045), so that the bridge stays uniform.

## Implementation Decisions

- **Uniform `CALL_CAP` shape:** every new capability registers `(capId, methodId) → impl` in the
  `CapabilityRegistry` threaded into the VM; sync capabilities settle the result cell before returning,
  async leave it `Pending` and settle via `AsyncResolver` (ADR-0045). No new opcode (AGENTS.md §3.4
  forbids new opcodes without an ADR).
- **Deterministic cap ids:** ids are derived by the same rule on server and both hosts (AGENTS.md §3.4);
  never hand-assigned. The escape hatch must produce deterministic ids for user-authored wrappers too.
- **Error contract from PRD-K:** denied grant → typed `FluxError`, never a crash; the escape hatch
  inherits the same contract so a poorly-written native wrapper cannot panic the host.
- **Escape hatch is first-class:** a documented, supported path (not a hidden workaround) to wrap a
  native SDK as a capability; it reuses `CapabilityRegistry` rather than bypassing it.
- **Capability surface belongs in stdlib:** capability declarations live in `capabilities.flux` /
  stdlib; new ones extend that surface the same way `router`/`clipboard` do today.

## Testing Decisions

- **Good test:** for each capability, a test asserting the denied-grant path yields a typed `FluxError`
  (never a crash) and that the async path settles its result cell to `Ready`/`Error` via `AsyncResolver`;
  for the escape hatch, a test wrapping a stub native SDK and exercising the same contract. Not tests of
  platform SDK internals.
- **Modules to test:** `CapabilityRegistry` registration + dispatch (fuzz per PRD-M), the host-side
  capability adapters (JVM + iOS), and the escape-hatch wrapper generator/contract.
- **Prior art:** existing Camera/Storage/Router/Clipboard/Geolocation capabilities and ADR-0044/0045's
  result-cell + `AsyncResolver` tests are the template.

## Out of Scope

- The capability *error taxonomy* hardening itself (PRD-K) — this PRD consumes that contract.
- The HTTP / structured-persistence data primitives behind forms (partially PRD-N UI, data here).
- `flux build` toolchain invocation (PRD-M).
- DevTools network inspector (PRD-P) — visualizes these calls but is separate.

## Further Notes

PRD-Q is what actually gets Flux to "90%" rather than "the 20 capabilities we thought of" — the escape
hatch is the load-bearing item. It depends on PRD-K so the error contract is stable before capabilities
multiply.
