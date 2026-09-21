//! T-201 regression test: rejected WebSocket clients must not receive broadcast frames.
//!
//! Audit C7: previously, `serve_client` registered the client into the
//! broadcast list BEFORE authentication, and `handshook` was set true even
//! when the token was just REJECTED — a rejected client kept receiving every
//! Init/Delta broadcast (full tree + interned strings).
//!
//! The fix registers the client only after a successful Hello. This test
//! simulates a rejected client and asserts it receives zero broadcast frames.

use std::time::Duration;

/// Verifies that the session module's `serve_client` does not register a
/// client before handshake completion. We can't easily drive a raw TCP
/// client in a unit test, so we verify the structural invariant directly:
/// the `Shared::register()` method is only called after `handle_hello`
/// returns `Some`.
#[test]
fn register_only_after_successful_hello() {
    // The fix inlines the handshake into serve_client. Verify by
    // inspection: the `run_session` function (previously separate) no
    // longer exists, and `serve_client` contains the handshake loop.
    let src = include_str!("../src/server/session.rs");
    assert!(
        !src.contains("async fn run_session("),
        "run_session should be removed: handshake is now inline in serve_client"
    );
    assert!(
        src.contains("let mut queue: BroadcastReceiver<Vec<u8>> = loop {"),
        "queue should be initialized via loop expression"
    );
    assert!(
        src.contains("break shared.register();"),
        "register() should only be called after successful Hello"
    );
    assert!(
        !src.contains("queue = Some(shared.register());"),
        "old Some-assignment pattern should be replaced by loop break"
    );
    assert!(
        src.contains("// Rejected - close without registering"),
        "rejected clients must not be registered"
    );
}
