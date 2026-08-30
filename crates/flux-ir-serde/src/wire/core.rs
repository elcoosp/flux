//! Shared wire primitives and the raw byte-level codec core (Appendix D).
//!
//! This module owns the [`WireError`] type, the [`MAX_FRAME_BYTES`] ceiling, the
//! [`validate_bytecode`] guard, and the allocation-free [`Writer`]/[`Reader`]
//! cursor pair. The typed per-wire-type encoders/decoders in the sibling
//! modules build on these primitives.

/// A decode failure with an actionable, span-free description.
///
/// The decoder is test-only, so the error carries the byte offset and the
/// field being read — enough to localise a corrupt frame during debugging
/// without dragging a [`Span`] through the wire layer.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum WireError {
    /// The buffer ended before `needed` more bytes could be read while
    /// decoding `context` at byte offset `at`.
    #[error(
        "truncated frame decoding {context}: need {needed} bytes at offset {at}, buffer has {available}"
    )]
    Truncated {
        /// Byte offset where the read was attempted.
        at: usize,
        /// Bytes still required.
        needed: usize,
        /// Short field description.
        context: &'static str,
        /// Bytes actually available from `at` to the end of the buffer.
        available: usize,
    },
    /// An unknown tag byte was found where `context` was expected.
    #[error("invalid tag {tag:#04x} decoding {context} at offset {at}")]
    InvalidTag {
        /// The offending tag byte.
        tag: u8,
        /// Short field description.
        context: &'static str,
        /// Byte offset of the tag.
        at: usize,
    },
    /// A string field was not valid UTF-8.
    #[error("invalid UTF-8 in {context} at offset {at}")]
    InvalidUtf8 {
        /// Short field description.
        context: &'static str,
        /// Byte offset of the string.
        at: usize,
    },
    /// A handler closure blob decoded into a bytecode program that is not
    /// safe to hand to the VM: an instruction's operand bytes run past the end
    /// of the blob, a jump/relative-offset target lands outside `[0, len)`, or
    /// an unknown opcode byte appears. The decoder must reject such programs
    /// rather than let the VM index out of bounds (LANE-D).
    #[error("malformed bytecode in {context}: {detail} at offset {at}, blob is {blob_len} bytes")]
    MalformedBytecode {
        /// Short field description (e.g. `closure.bytecode`).
        context: &'static str,
        /// Human-readable reason: `truncated-operands`, `jump-out-of-range`, or
        /// `unknown-opcode`.
        detail: &'static str,
        /// Byte offset within the blob where the fault was detected.
        at: usize,
        /// Total length of the decoded bytecode blob.
        blob_len: usize,
    },
    /// A frame declared a payload larger than the hard ceiling
    /// ([`MAX_FRAME_BYTES`]), so the decoder refuses to allocate or scan it
    /// (defense in depth beyond the per-dispatch gas/gas cap).
    #[error("frame too large: declared {declared} bytes exceeds ceiling {ceiling} in {context}")]
    FrameTooLarge {
        /// Short field description (e.g. `init.payload` / `delta.payload`).
        context: &'static str,
        /// Declared payload length in bytes.
        declared: usize,
        /// The hard ceiling in bytes.
        ceiling: usize,
    },
}

/// Hard ceiling on a decoded `Init`/`Delta` payload (Appendix D §D.12 + D.1),
/// defense in depth beyond the 16 MiB per-dispatch allocation cap. A frame that
/// declares more than this is rejected outright by the decoder; the host never
/// allocates or scans an attacker-controlled multi-gigabyte buffer.
pub const MAX_FRAME_BYTES: usize = 64 * 1024 * 1024;

/// Validates a decoded handler closure blob as a self-consistent bytecode
/// program before the VM ever runs it (LANE-D, task 2).
///
/// A program is valid when every instruction's operand bytes fit inside the
/// blob and every `Jump`/`CondJump`/`CondJumpNot` relative target (`i32`,
/// offset from the *next* instruction) lands on an instruction boundary within
/// `[0, len)`. An unknown opcode byte is likewise rejected — the wire only
/// carried known [`crate::Opcode`]s (Appendix E §E.1), so a stray byte is corruption,
/// not a future opcode the decoder should silently pass through.
///
/// Returns `Err(WireError::MalformedBytecode)` on the first fault; `Ok(())`
/// when the blob is an empty program (no handlers) or a fully valid one.
///
/// # Examples
///
/// ```ignore
/// use flux_ir_serde::wire::validate_bytecode;
/// // `HALT` is a valid empty-ish program.
/// assert!(validate_bytecode(&[0x00]).is_ok());
/// // A `JUMP` of `0` (self-fall-through to the next instruction) is in range.
/// assert!(validate_bytecode(&[0x60, 0, 0, 0, 0, 0x00]).is_ok());
/// // A `JUMP` whose target runs past the blob is rejected.
/// assert!(validate_bytecode(&[0x60, 0xFF, 0xFF, 0xFF, 0xFF]).is_err());
/// ```
#[must_use = "bytecode validation is only meaningful if its Result is checked"]
pub fn validate_bytecode(bytecode: &[u8]) -> Result<(), WireError> {
    use flux_syntax::opcode::Opcode;

    let len = bytecode.len();
    if len == 0 {
        return Ok(());
    }

    // Pass 1: decode every instruction, recording its byte offset and the
    // byte offset of the instruction that follows it. This mirrors the VM's own
    // `decode_program` layout checks (truncation + unknown opcode) but returns
    // the wire [`WireError`] instead of the VM's [`VmError`].
    #[derive(Clone, Copy)]
    struct Decoded {
        offset: usize,
        next_offset: usize,
    }
    let mut instrs: Vec<Decoded> = Vec::with_capacity(len / 2);
    let mut ip = 0usize;
    while ip < len {
        let opcode = match Opcode::from_byte(bytecode[ip]) {
            Some(op) => op,
            None => {
                return Err(WireError::MalformedBytecode {
                    context: "closure.bytecode",
                    detail: "unknown-opcode",
                    at: ip,
                    blob_len: len,
                });
            }
        };
        let n = opcode.operand_len() as usize;
        let start = ip + 1;
        let end = start + n;
        if end > len {
            return Err(WireError::MalformedBytecode {
                context: "closure.bytecode",
                detail: "truncated-operands",
                at: ip,
                blob_len: len,
            });
        }
        let next_offset = end;
        instrs.push(Decoded {
            offset: ip,
            next_offset,
        });
        ip = end;
    }

    // Pass 2: every jump must land on an instruction boundary inside the blob.
    for decoded in &instrs {
        let opcode = Opcode::from_byte(bytecode[decoded.offset]).expect("known opcode from pass 1");
        let jump = match opcode {
            Opcode::Jump => Some(read_i32(bytecode, decoded.offset + 1)),
            Opcode::CondJump | Opcode::CondJumpNot => Some(read_i32(bytecode, decoded.offset + 1)),
            _ => None,
        };
        let Some(relative) = jump else { continue };
        let base = decoded.next_offset as i64;
        let target = base + i64::from(relative);
        if target < 0 || target > len as i64 {
            return Err(WireError::MalformedBytecode {
                context: "closure.bytecode",
                detail: "jump-out-of-range",
                at: decoded.offset,
                blob_len: len,
            });
        }
        // The target must coincide with an instruction boundary.
        if !instrs.iter().any(|d| d.offset as i64 == target) {
            return Err(WireError::MalformedBytecode {
                context: "closure.bytecode",
                detail: "jump-out-of-range",
                at: decoded.offset,
                blob_len: len,
            });
        }
    }

    Ok(())
}

/// Reads a little-endian `i32` from `bytecode[start..start+4]`.
///
/// Callers only invoke this at an already-validated operand slice (pass 1 has
/// confirmed the four bytes exist), so the slice is in bounds by construction.
fn read_i32(bytecode: &[u8], start: usize) -> i32 {
    i32::from_le_bytes([
        bytecode[start],
        bytecode[start + 1],
        bytecode[start + 2],
        bytecode[start + 3],
    ])
}
