//! Async suspension bridge: host `AwaitSuspend` → server `Resume`
//! (roadmap Phase 2, ADR-0044 result cells / ADR-0045 capability bridge).
//!
//! A host handler that hits `AWAIT` on a `Pending` result cell parks and reports
//! the suspension. The value it waits for is produced by an async capability
//! whose completion the server observes, so the server owns the resumption:
//!
//! ```text
//! host  --AwaitSuspend{handler, cell, resume_ip}-->  server   (park)
//! host  <--Resume{handler, cell, value}------------  server   (settle)
//! ```
//!
//! [`AsyncBridge`] is the registry in between. It is deliberately transport-free
//! (it takes and returns wire bytes and values, never a socket) so it unit-tests
//! on the plain runtime, and it settles out-of-order: a capability that resolves
//! *before* the host reports its suspension still resumes correctly, because the
//! early value is retained and delivered when the suspension arrives. Without
//! that, a fast capability would deadlock the handler forever.

use std::collections::HashMap;

use flux_ir_serde::{AwaitSuspendFrame, ResumeFrame};
use flux_syntax::{HandlerId, SignalId, Value};

/// A parked handler continuation, keyed by the cell it awaits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Parked {
    handler_id: HandlerId,
    resume_ip: u32,
}

/// A value that settled before its suspension was reported.
#[derive(Clone, Debug, PartialEq)]
struct Settled {
    value: Value,
    is_error: bool,
}

/// Pairs host suspensions with capability completions and emits `Resume` frames.
#[derive(Debug, Default)]
pub struct AsyncBridge {
    /// Suspensions reported by the host and not yet resumed.
    parked: HashMap<SignalId, Parked>,
    /// Completions that arrived before their suspension was reported.
    early: HashMap<SignalId, Settled>,
}

impl AsyncBridge {
    /// Creates an empty bridge.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a host suspension from its wire bytes.
    ///
    /// Returns the `Resume` frame to send back immediately when the awaited cell
    /// had already settled, or `None` when the handler must stay parked.
    ///
    /// # Errors
    ///
    /// Returns the frame's [`flux_ir_serde::WireError`] when `bytes` is not a
    /// well-formed `AwaitSuspend` frame; the caller surfaces it as a protocol
    /// error rather than silently dropping the suspension.
    pub fn on_await_suspend(
        &mut self,
        bytes: &[u8],
    ) -> Result<Option<Vec<u8>>, flux_ir_serde::WireError> {
        let frame = AwaitSuspendFrame::from_bytes(bytes)?;
        Ok(self.park(frame))
    }

    /// Records a decoded suspension, resuming at once if the cell already
    /// settled.
    #[must_use]
    pub fn park(&mut self, frame: AwaitSuspendFrame) -> Option<Vec<u8>> {
        if let Some(settled) = self.early.remove(&frame.cell) {
            return Some(resume_bytes(frame.handler_id, frame.cell, &settled));
        }
        self.parked.insert(
            frame.cell,
            Parked {
                handler_id: frame.handler_id,
                resume_ip: frame.resume_ip,
            },
        );
        None
    }

    /// Settles `cell` with a successful `value`.
    ///
    /// Returns the `Resume` frame bytes when a handler is parked on the cell;
    /// otherwise the value is retained until its suspension is reported.
    #[must_use]
    pub fn settle(&mut self, cell: SignalId, value: Value) -> Option<Vec<u8>> {
        self.settle_inner(cell, value, false)
    }

    /// Settles `cell` with a capability error payload.
    #[must_use]
    pub fn settle_error(&mut self, cell: SignalId, value: Value) -> Option<Vec<u8>> {
        self.settle_inner(cell, value, true)
    }

    fn settle_inner(&mut self, cell: SignalId, value: Value, is_error: bool) -> Option<Vec<u8>> {
        let settled = Settled { value, is_error };
        match self.parked.remove(&cell) {
            Some(parked) => Some(resume_bytes(parked.handler_id, cell, &settled)),
            None => {
                self.early.insert(cell, settled);
                None
            }
        }
    }

    /// Returns the bytecode offset the handler parked on `cell` resumes at, if
    /// any. Exposed so a caller can correlate a suspension with its handler
    /// without re-decoding the frame.
    #[must_use]
    pub fn resume_ip(&self, cell: SignalId) -> Option<u32> {
        self.parked.get(&cell).map(|p| p.resume_ip)
    }

    /// Number of handlers currently parked.
    #[must_use]
    pub fn parked_len(&self) -> usize {
        self.parked.len()
    }
}

fn resume_bytes(handler_id: HandlerId, cell: SignalId, settled: &Settled) -> Vec<u8> {
    let frame = if settled.is_error {
        ResumeFrame::error(handler_id, cell, settled.value.clone())
    } else {
        ResumeFrame::ready(handler_id, cell, settled.value.clone())
    };
    frame.to_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(bytes: &[u8]) -> ResumeFrame {
        ResumeFrame::from_bytes(bytes).expect("valid resume frame")
    }

    #[test]
    fn suspension_then_completion_resumes_the_parked_handler() {
        let mut bridge = AsyncBridge::new();
        let parked = bridge.park(AwaitSuspendFrame::new(4, 77, 32));
        assert!(
            parked.is_none(),
            "no value yet, so the handler stays parked"
        );
        assert_eq!(bridge.parked_len(), 1);
        assert_eq!(bridge.resume_ip(77), Some(32));

        let bytes = bridge.settle(77, Value::Int(9)).expect("resumes");
        let frame = decode(&bytes);
        assert_eq!(frame.handler_id, 4);
        assert_eq!(frame.cell, 77);
        assert_eq!(frame.value, Value::Int(9));
        assert!(!frame.is_error);
        assert_eq!(bridge.parked_len(), 0, "the suspension is consumed");
    }

    #[test]
    fn completion_before_suspension_still_resumes() {
        // A capability that resolves faster than the host reports its suspension
        // must not deadlock the handler.
        let mut bridge = AsyncBridge::new();
        assert!(bridge.settle(5, Value::Bool(true)).is_none());
        let bytes = bridge
            .park(AwaitSuspendFrame::new(2, 5, 16))
            .expect("the early value resumes the handler immediately");
        let frame = decode(&bytes);
        assert_eq!(frame.handler_id, 2);
        assert_eq!(frame.value, Value::Bool(true));
        assert_eq!(bridge.parked_len(), 0);
    }

    #[test]
    fn error_completion_resumes_down_the_error_path() {
        let mut bridge = AsyncBridge::new();
        let _ = bridge.park(AwaitSuspendFrame::new(1, 3, 8));
        let bytes = bridge.settle_error(3, Value::Int(-1)).expect("resumes");
        assert!(decode(&bytes).is_error);
    }

    #[test]
    fn a_cell_resumes_at_most_once() {
        let mut bridge = AsyncBridge::new();
        let _ = bridge.park(AwaitSuspendFrame::new(1, 3, 8));
        assert!(bridge.settle(3, Value::Null).is_some());
        assert!(
            bridge.settle(3, Value::Null).is_none(),
            "a second completion must not re-resume the same continuation"
        );
    }

    #[test]
    fn independent_cells_do_not_cross_resume() {
        let mut bridge = AsyncBridge::new();
        let _ = bridge.park(AwaitSuspendFrame::new(1, 10, 4));
        let _ = bridge.park(AwaitSuspendFrame::new(2, 20, 8));
        let bytes = bridge.settle(20, Value::Int(1)).expect("resumes");
        assert_eq!(decode(&bytes).handler_id, 2);
        assert_eq!(bridge.parked_len(), 1, "cell 10 is still parked");
    }

    #[test]
    fn wire_bytes_are_decoded_and_parked() {
        let mut bridge = AsyncBridge::new();
        let bytes = AwaitSuspendFrame::new(6, 60, 24).to_bytes();
        assert!(bridge.on_await_suspend(&bytes).expect("decodes").is_none());
        assert_eq!(bridge.resume_ip(60), Some(24));
    }

    #[test]
    fn malformed_suspension_is_an_error_not_a_silent_drop() {
        let mut bridge = AsyncBridge::new();
        assert!(bridge.on_await_suspend(&[0, 1, 2]).is_err());
        assert_eq!(bridge.parked_len(), 0);
    }
}
