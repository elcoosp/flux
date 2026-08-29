# Reconnect & Resync Model (dev wire, Appendix D §D.13 / FR-017)

This documents how a dropped dev-server connection is recovered, end to end,
so the "save → photon" path is robust to transient network drops (WiFi blip,
device sleep, server restart). Grounded in the current transport code as of
this writing — not an aspirational spec.

## The invariant

The dev server is **stateless with respect to a single host connection**. Every
accepted socket is independent (`crates/flux-devserver/src/server/session.rs::serve_client`):
the server does not track per-host session cursor state beyond the in-memory
broadcast queue. Therefore **re-sync is implicit**: a reconnecting host re-sends
`Hello`, and the server answers with a **fresh full `Init`** frame carrying the
entire current tree. There is no separate "request full state" opcode and no
delta-resume protocol — by design, because the full tree is the safe, simple
fixed point and stays small (Appendix D §D.12.2 flattens the whole arena into
one `Init`).

Consequence: a drop never requires a 15-second app reboot (a concern raised in
Roast #2 §7). The host keeps its last rendered tree on screen, shows a reconnect
banner, and re-pulls the full tree on the next successful socket open.

## iOS host (FR-017, complete)

`runtimes/ios/Sources/Host/FluxWebSocketTransport.swift`:

- On any socket drop (`onError` / `onClose` / send failure) → `handleDrop()`
  cancels the socket, sets `.reconnecting`, and calls `scheduleReconnect()`.
- `scheduleReconnect()` waits `retryInterval` (1.0 s, FR-017) and calls
  `connect()` again.
- `connect()` re-sends the `Hello` handshake. The server's `run_session` only
  fans out `Init` after a `Hello`, so the reopen immediately pulls the full tree.
- `FluxAppMain.swift` renders a `.reconnecting` banner driven by the transport
  status; it clears on reconnect. Covered by `FluxHostConnectionTests.swift`
  (connection-state + banner TDD).

This is the reference implementation of the resync model.

## Android host (gap — see below)

`runtimes/android/host/.../transport/OkHttpTransport.kt` documents a
"Reconnecting..." state (Appendix D §D.13) and sets `connected = false` on
`onFailure`/`onClosed`, but **does not contain an automatic retry timer** in the
transport itself. The reconnect intent is real (the `onResume` rebind model is
referenced from the iOS transport docstring) but the self-healing 1-second retry
loop that iOS has is **not** present in `OkHttpTransport`. This is a known
asymmetry: an Android drop currently relies on an external lifecycle rebind to
re-open the socket rather than an in-transport retry.

**Action:** port the iOS `handleDrop` → 1 s `scheduleReconnect` → `connect`
pattern into `OkHttpTransport` (or the Android executor that owns it) so both
hosts self-heal identically. File as a follow-up (see FLUX-075 candidate list).

## What is NOT resynced, and why that is fine

- **In-flight patches lost during a drop:** discarded. The next `Init` carries
  the authoritative full tree, so any patch that arrived-but-wasn't-applied is
  superseded by the full state. No partial-apply hazard.
- **Host-side signal graph:** rebuilt from the re-pushed `Init` seed (the server
  re-seeds state on `Init`). Local signal writes made while disconnected are not
  persisted server-side — by design, the server is the source of truth and a
  disconnect means "re-derive from current compile."
- **Capability session / pending result cells:** a drop abandons in-flight
  `AWAIT`s; the next handler invocation re-runs from scratch. Acceptable for dev.

## Security note (related: `flux dev --token`)

When the server is started with `--token`, the reconnect `Hello` must carry the
same token or it is rejected (Appendix D §D.12.1, `FLUX-071` token work). The
host reads its token from the `FLUX_DEV_TOKEN` environment variable (both
`FluxWebSocketTransport` and `OkHttpTransport` append it to `Hello`), so a
reconnect automatically re-presents it — no extra code path needed.

## Open questions

- Should the server keep a per-connection monotonic sequence so a reconnect can
  optionally request "delta since seq N" instead of a full `Init`? Not needed
  today (full `Init` is cheap and correct); revisit only if `Init` size becomes a
  measured LAN bottleneck (see FLUX-073 end-to-end budget).
- Android auto-retry timer (above) — land for parity with iOS.
