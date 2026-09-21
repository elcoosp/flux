//! T-207 acceptance test: per-session AsyncBridge isolation.
//!
//! Audit H17: the server-wide bridge never pruned `early` values on disconnect,
//! so a stale completion from session 1 could resume a handler in session 2.
//! This test simulates two sequential sessions through the AsyncBridge directly:
//! session 1 parks a handler with a cell id that never resolves, then "disconnects"
//! (clear_early is called); session 2 parks a *different* handler on the same cell
//! id and must NOT receive the stale resume.

use flux_devserver::AsyncBridge;
use flux_ir_serde::AwaitSuspendFrame;
use flux_syntax::Value;

/// Session 1 settles a cell before its suspension is reported (early value).
/// Then the session "ends" (clear_early). Session 2 parks on the same cell id
/// but a different handler — it must NOT be resumed by session 1's stale value.
#[test]
fn stale_early_value_does_not_leak_across_sessions() {
    let mut bridge = AsyncBridge::new();

    // --- Session 1 ---
    // A capability resolves, but no suspension has been reported yet.
    let resumed = bridge.resume(42, Value::Int(1));
    assert!(
        resumed.is_none(),
        "no handler parked yet; the value is held as early"
    );

    // Session 1 "disconnects":
    bridge.clear_early();

    // --- Session 2 ---
    // A different handler parks on the same cell id (42). If the stale `early`
    // value from session 1 were still present, this park would immediately
    // return a Resume — the bug we are guarding against.
    let maybe_resume = bridge.park(AwaitSuspendFrame::new(
        /* handler_id= */ 99, /* cell= */ 42, /* resume_ip= */ 7,
    ));
    assert!(
        maybe_resume.is_none(),
        "session 2 must NOT be resumed by session 1's stale early value; \
         the early map must have been cleared on disconnect (audit H17)"
    );

    // Sanity: the handler is still parked, not erroneously resumed.
    assert_eq!(bridge.parked_len(), 1);
    assert_eq!(bridge.resume_ip(42), Some(7));
}

/// A second scenario: session 1 parks a handler (it stays parked, never
/// resolved), then disconnects. Session 2 parks a new handler on the same cell.
/// The old parked handler is orphaned (socket gone); session 2's handler must
/// not be confused with session 1's.
#[test]
fn parked_handler_from_session_one_is_orphaned_not_resumed_in_session_two() {
    let mut bridge = AsyncBridge::new();

    // Session 1: handler 10 parks on cell 5, never resumes.
    let resume = bridge.park(AwaitSuspendFrame::new(10, 5, 11));
    assert!(resume.is_none());
    assert_eq!(bridge.parked_len(), 1);

    // Session 1 disconnects.
    bridge.clear_early();

    // Session 2: a DIFFERENT handler (11) parks on the SAME cell (5).
    // The old parked entry for cell 5 is overwritten — not resumed — because
    // cell 5's handler from session 1 is gone (socket closed).
    let resume2 = bridge.park(AwaitSuspendFrame::new(11, 5, 22));
    assert!(
        resume2.is_none(),
        "session 2's handler must park fresh, not inherit session 1's parked state"
    );
    assert_eq!(
        bridge.parked_len(),
        1,
        "only session 2's handler is parked now"
    );
    assert_eq!(
        bridge.resume_ip(5),
        Some(22),
        "session 2's resume_ip must be recorded, not session 1's"
    );
}
