//! Async suspension wire codec — `AwaitSuspend` / `Resume` (roadmap Phase 2,
//! ADR-0044 result cells, ADR-0045 capability bridge).
//!
//! A handler that hits `AWAIT` on a `Pending` result cell cannot finish on the
//! host alone: the value it is waiting for is produced by an async capability
//! whose completion the dev server observes. Two frames close that loop:
//!
//! - `AwaitSuspend` (`0x12`, Host → Server): "handler `H` parked on cell `C`;
//!   its continuation resumes at bytecode offset `resume_ip`". This is the
//!   `SuspendState` identity — the host keeps the register file and gas locally,
//!   so only the three ids travel, keeping the frame fixed-width.
//! - `Resume` (`0x13`, Server → Host): "cell `C` of handler `H` settled with
//!   `value`; re-enter the continuation". The host looks its parked
//!   `SuspendState` up by `(handler_id, cell)` and calls `resume`.
//!
//! Both reuse the shared `MAGIC`/`PROTOCOL_VERSION` header and the Appendix D
//! §D.5 value encoding, and all integers are little-endian. The module is
//! deliberately separate from `frame.rs` so it does not collide with the
//! `Hello`/`Init`/`Delta` construction API.

use flux_syntax::{HandlerId, SignalId, Value};

use crate::frame::{MAGIC, PROTOCOL_VERSION};
use crate::wire::{Reader, WireError, Writer, decode_value, encode_value};

/// `frame_type` byte for the `AwaitSuspend` frame (Host → Server).
pub const FRAME_AWAIT_SUSPEND: u8 = 0x12;
/// `frame_type` byte for the `Resume` frame (Server → Host).
pub const FRAME_RESUME: u8 = 0x13;

/// A host-reported handler suspension (Host → Server, frame `0x12`).
///
/// Layout after the shared `magic(4) version(1) frame_type(1)` prefix:
/// `handler_id(u32) | cell(u32) | resume_ip(u32)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AwaitSuspendFrame {
    /// The handler closure that parked.
    pub handler_id: HandlerId,
    /// The result cell it is awaiting (`CALL_CAP`'s returned signal id).
    pub cell: SignalId,
    /// Bytecode offset the host's continuation re-enters at.
    pub resume_ip: u32,
}

impl AwaitSuspendFrame {
    /// Builds an `AwaitSuspend` frame.
    #[must_use]
    pub const fn new(handler_id: HandlerId, cell: SignalId, resume_ip: u32) -> Self {
        Self {
            handler_id,
            cell,
            resume_ip,
        }
    }

    /// Encodes the frame.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = Writer::new();
        w.u32(MAGIC);
        w.u8(PROTOCOL_VERSION);
        w.u8(FRAME_AWAIT_SUSPEND);
        w.u32(self.handler_id);
        w.u32(self.cell);
        w.u32(self.resume_ip);
        w.into_vec()
    }

    /// Decodes an `AwaitSuspend` frame.
    ///
    /// # Errors
    ///
    /// Returns [`WireError`] when the magic, version or frame type does not
    /// match, or when the buffer is short of the fixed 18-byte layout.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, WireError> {
        let mut r = expect_header(bytes, FRAME_AWAIT_SUSPEND, "await_suspend")?;
        let handler_id = r.u32("await_suspend.handler_id")?;
        let cell = r.u32("await_suspend.cell")?;
        let resume_ip = r.u32("await_suspend.resume_ip")?;
        Ok(Self {
            handler_id,
            cell,
            resume_ip,
        })
    }
}

/// A server-issued resumption (Server → Host, frame `0x13`).
///
/// Layout after the shared `magic(4) version(1) frame_type(1)` prefix:
/// `handler_id(u32) | cell(u32) | is_error(u8) | value(D.5)`.
///
/// `is_error` distinguishes a settled `Ready(value)` from an `Error(value)`
/// cell (ADR-0044): a faulting capability must resume the handler down its
/// error path rather than deliver the payload as a normal result.
#[derive(Clone, Debug, PartialEq)]
pub struct ResumeFrame {
    /// The handler closure to resume.
    pub handler_id: HandlerId,
    /// The result cell that settled.
    pub cell: SignalId,
    /// Whether the cell settled as an error.
    pub is_error: bool,
    /// The settled value delivered into the continuation's `r0`.
    pub value: Value,
}

impl ResumeFrame {
    /// Builds a `Resume` frame for a cell that settled successfully.
    #[must_use]
    pub const fn ready(handler_id: HandlerId, cell: SignalId, value: Value) -> Self {
        Self {
            handler_id,
            cell,
            is_error: false,
            value,
        }
    }

    /// Builds a `Resume` frame for a cell that faulted.
    #[must_use]
    pub const fn error(handler_id: HandlerId, cell: SignalId, value: Value) -> Self {
        Self {
            handler_id,
            cell,
            is_error: true,
            value,
        }
    }

    /// Encodes the frame.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = Writer::new();
        w.u32(MAGIC);
        w.u8(PROTOCOL_VERSION);
        w.u8(FRAME_RESUME);
        w.u32(self.handler_id);
        w.u32(self.cell);
        w.u8(u8::from(self.is_error));
        encode_value(&mut w, &self.value);
        w.into_vec()
    }

    /// Decodes a `Resume` frame.
    ///
    /// # Errors
    ///
    /// Returns [`WireError`] when the header does not match, when the buffer is
    /// truncated, or when the value payload is malformed.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, WireError> {
        let mut r = expect_header(bytes, FRAME_RESUME, "resume")?;
        let handler_id = r.u32("resume.handler_id")?;
        let cell = r.u32("resume.cell")?;
        let is_error = r.u8("resume.is_error")? != 0;
        let value = decode_value(&mut r)?;
        Ok(Self {
            handler_id,
            cell,
            is_error,
            value,
        })
    }
}

/// Validates the shared frame header and returns a reader positioned at the
/// frame's payload.
fn expect_header<'a>(
    bytes: &'a [u8],
    frame_type: u8,
    context: &'static str,
) -> Result<Reader<'a>, WireError> {
    let mut r = Reader::new(bytes);
    let magic = r.u32("frame.magic")?;
    if magic != MAGIC {
        return Err(WireError::InvalidTag {
            tag: 0,
            context,
            at: 0,
        });
    }
    let version = r.u8("frame.version")?;
    if version != PROTOCOL_VERSION {
        return Err(WireError::InvalidTag {
            tag: version,
            context,
            at: 4,
        });
    }
    let tag = r.u8("frame.type")?;
    if tag != frame_type {
        return Err(WireError::InvalidTag {
            tag,
            context,
            at: 5,
        });
    }
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn await_suspend_round_trips() {
        let frame = AwaitSuspendFrame::new(7, 42, 96);
        let back = AwaitSuspendFrame::from_bytes(&frame.to_bytes()).expect("decodes");
        assert_eq!(back, frame);
    }

    #[test]
    fn await_suspend_header_and_offsets_match_d12() {
        let bytes = AwaitSuspendFrame::new(0x0A0B_0C0D, 0x1112_1314, 0x2122_2324).to_bytes();
        assert_eq!(bytes.len(), 18, "fixed-width frame");
        assert_eq!(
            u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            MAGIC
        );
        assert_eq!(bytes[4], PROTOCOL_VERSION);
        assert_eq!(bytes[5], FRAME_AWAIT_SUSPEND);
        assert_eq!(
            u32::from_le_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]),
            0x0A0B_0C0D
        );
        assert_eq!(
            u32::from_le_bytes([bytes[10], bytes[11], bytes[12], bytes[13]]),
            0x1112_1314
        );
        assert_eq!(
            u32::from_le_bytes([bytes[14], bytes[15], bytes[16], bytes[17]]),
            0x2122_2324
        );
    }

    #[test]
    fn resume_round_trips_ready_value() {
        let frame = ResumeFrame::ready(3, 9, Value::Int(-17));
        let back = ResumeFrame::from_bytes(&frame.to_bytes()).expect("decodes");
        assert_eq!(back, frame);
        assert!(!back.is_error);
    }

    #[test]
    fn resume_round_trips_error_value() {
        let frame = ResumeFrame::error(3, 9, Value::Bool(true));
        let back = ResumeFrame::from_bytes(&frame.to_bytes()).expect("decodes");
        assert!(
            back.is_error,
            "an errored cell must stay an error on the wire"
        );
        assert_eq!(back.value, Value::Bool(true));
    }

    #[test]
    fn resume_header_matches_d12() {
        let bytes = ResumeFrame::ready(1, 2, Value::Null).to_bytes();
        assert_eq!(bytes[4], PROTOCOL_VERSION);
        assert_eq!(bytes[5], FRAME_RESUME);
        // handler_id, cell, is_error, then the D.5 Null tag (0x00).
        assert_eq!(bytes[14], 0x00, "is_error=false");
        assert_eq!(bytes[15], 0x00, "Null value tag");
    }

    #[test]
    fn wrong_frame_type_is_rejected() {
        let bytes = ResumeFrame::ready(1, 2, Value::Null).to_bytes();
        assert!(
            AwaitSuspendFrame::from_bytes(&bytes).is_err(),
            "a Resume frame must not decode as AwaitSuspend"
        );
    }

    #[test]
    fn truncated_await_suspend_is_rejected() {
        let bytes = AwaitSuspendFrame::new(1, 2, 3).to_bytes();
        assert!(AwaitSuspendFrame::from_bytes(&bytes[..12]).is_err());
    }
}
