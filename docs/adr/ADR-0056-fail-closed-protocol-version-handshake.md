# ADR-0056: Fail-closed protocol-version handshake

- **Status:** Accepted
- **Date:** 2026-08-29
- **Supersedes / related:** FLUX-050, Appendix D (wire protocol), ADR-0044
  (result cells), ADR-0045 (capability bridge), ADR-0055 (`Result[T, E]`)

## Context

The Flux wire protocol (Appendix D) begins every frame with a 6-byte header:
`magic(4) | version(1) | kind(1)`. Both native hosts (`runtimes/android/host`,
`runtimes/ios/FluxHost`) decoded the `version` byte but **never checked it**.
An old host talking to a newer dev server — or a new host to an older server —
would silently mis-decode every frame: field offsets shift between protocol
versions, so a stale host reads garbage node ids, prop indices, and signal
seeds. The result is a corrupt shadow tree, a crash deep in the reconciler, or
(worse) a silently wrong UI that looks alive but renders attacker-controlled
bytes.

This is the *update-integrity* gap: there is no enforced contract that the
bytes on the wire match what the host was built to parse. In a production OTA
path a downgrade or a mismatched server/host pair must fail loudly, not quietly.

## Decision

Both hosts **fail closed** on a protocol-version mismatch:

1. `FrameDeserializer` (Android) and `FrameDeserializer` (iOS) read the version
   byte immediately after the magic check. If `version != PROTOCOL_VERSION`,
   they throw a typed wire error (`WireError` on Android,
   `WireError.unsupportedVersion` on iOS) **before** any tree decoding.
2. `PROTOCOL_VERSION` is a named host constant (`0x01`) — never a hardcoded
   literal at the call site — so a future bump is a one-line change on both
   hosts plus the Rust server (`crates/flux-ir-serde/src/frame.rs`).
3. The thrown error is rendered by the existing host error path as a **red
   banner** (Appendix E §E.6), exactly like any other `WireError`. It never
   panics into native code.
4. A mismatch is actionable: the banner tells the operator to update the host
   or the dev server. No silent fallback to "best-effort decode".

This is verified by round-trip tests on both hosts
(`FrameDeserializerTest.rejectsProtocolVersionMismatchFailClosed` on Android,
`RuntimeGapTests.testRejectsProtocolVersionMismatchFailClosed` on iOS) that flip
the version byte and assert the typed error.

## Consequences

- **Security:** an old host can no longer be fed a new server's incompatible
  frames and mis-parsed into a wrong/corrupt tree. The handshake is the
  first line of update integrity.
- **Operability:** a host/server version skew is now a one-line red banner, not
  a 30-minute "why is the screen blank" debugging session.
- **Cost:** a genuine, intentional protocol bump requires touching three sites
  (Rust server + two hosts) and bumping `PROTOCOL_VERSION`. That is the desired
  friction — a wire-format change is a breaking change and must be deliberate.
- **Out of scope (follow-up):** cryptographic code signing of the server's
  frames / host binary attestation. This ADR covers *structural* integrity
  (version contract), not *authenticity*. A future ADR may layer signature
  verification on top of this handshake.

## Security advantage (consumer framing)

For a downstream app team shipping Flux in production, the practical guarantee
is: **a Flux host will refuse to render anything it does not understand.** A
supply-chain event that swaps in an incompatible or malicious dev server cannot
produce a silently-mis-rendered screen — the handshake rejects the first frame
and shows the operator exactly what is wrong. This is the property that makes
"write-once, hot-reloaded UI" safe to put in front of real users.
