//! Low-level Appendix D binary codec (FLUX-013).
//!
//! This module owns the byte-level encoding/decoding for every wire type in
//! Appendix D §D.2–§D.11: [`Value`], [`NodeRef`], [`Child`], [`PropDiff`],
//! [`ClosureRef`], handler definitions, string entries, state deltas and
//! source-map deltas. All integers are little-endian and every layout mirrors
//! the appendix byte-for-byte so the Swift/Kotlin production deserializers —
//! which read the same spec — stay in lock-step.
//!
//! The [`Writer`] and [`Reader`] are the only allocation-free primitives; the
//! typed encode/decode functions build on them. The decoder is used by the
//! round-trip tests and by the [`crate::frame`] decoders; it is *not* a
//! production path (the host apps ship their own).

use flux_syntax::{
    Child, ClosureRef, FileId, HandlerId, NodeId, NodeKind, NodeRef, Patch, PropDiff, Props,
    SignalId, SourceExcerpt, Span, StringId, Value,
};

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

/// A grow-only little-endian byte sink.
pub(crate) struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    pub(crate) fn new() -> Self {
        Self { buf: Vec::new() }
    }

    /// Builds a `Writer` around a caller-owned buffer, clearing it first.
    ///
    /// The dev server encodes a frame on every hot-reload edit; reusing one
    /// scratch `Vec<u8>` across frames (instead of `new()`'s fresh allocation
    /// each call) keeps that hot path allocation-free after warm-up. The buffer
    /// is `clear()`ed (capacity preserved), not dropped.
    pub(crate) fn from_vec(mut buf: Vec<u8>) -> Self {
        buf.clear();
        Self { buf }
    }

    pub(crate) fn u8(&mut self, value: u8) {
        self.buf.push(value);
    }

    pub(crate) fn u16(&mut self, value: u16) {
        self.buf.extend_from_slice(&value.to_le_bytes());
    }

    pub(crate) fn u32(&mut self, value: u32) {
        self.buf.extend_from_slice(&value.to_le_bytes());
    }

    pub(crate) fn u64(&mut self, value: u64) {
        self.buf.extend_from_slice(&value.to_le_bytes());
    }

    pub(crate) fn bytes(&mut self, value: &[u8]) {
        self.buf.extend_from_slice(value);
    }

    /// Number of bytes written so far; used by callers that reserve a length
    /// slot and back-patch it after the body is encoded.
    pub(crate) fn buf_len(&self) -> usize {
        self.buf.len()
    }

    /// Overwrites the `u32` little-endian value at `offset` (must already be
    /// allocated in the buffer). Used to back-patch a length prefix once the
    /// body size is known.
    pub(crate) fn patch_u32_at(&mut self, offset: usize, value: u32) {
        let bytes = value.to_le_bytes();
        self.buf[offset..offset + 4].copy_from_slice(&bytes);
    }

    pub(crate) fn into_vec(self) -> Vec<u8> {
        self.buf
    }
}

/// A cursor over a little-endian byte source.
pub(crate) struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    /// Current read offset, used by callers to detect end-of-buffer.
    pub(crate) fn pos(&self) -> usize {
        self.pos
    }

    /// Bytes still available to read from the current position.
    #[must_use]
    pub(crate) fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.pos)
    }

    /// Rejects a declared element `count` that is impossible to satisfy with the
    /// bytes still available (LANE-D, OOM hardening).
    ///
    /// Every decoded element occupies at least one byte on the wire, so a count
    /// larger than `remaining()` can never be fulfilled — it is corruption, not a
    /// real collection. Without this guard an attacker-controlled `u32` count in
    /// an `Init` frame would drive `Vec::with_capacity(count)` to attempt a
    /// multi-gigabyte allocation and abort the process (libFuzzer flags this as
    /// `out-of-memory`). We fail with [`WireError::Truncated`] instead.
    pub(crate) fn ensure_capacity(
        &self,
        count: usize,
        context: &'static str,
    ) -> Result<(), WireError> {
        if count > self.remaining() {
            return Err(WireError::Truncated {
                at: self.pos,
                needed: count,
                context,
                available: self.remaining(),
            });
        }
        Ok(())
    }

    pub(crate) fn take(
        &mut self,
        needed: usize,
        context: &'static str,
    ) -> Result<&'a [u8], WireError> {
        let available = self.bytes.len() - self.pos;
        if available < needed {
            return Err(WireError::Truncated {
                at: self.pos,
                needed,
                context,
                available,
            });
        }
        let slice = &self.bytes[self.pos..self.pos + needed];
        self.pos += needed;
        Ok(slice)
    }

    pub(crate) fn u8(&mut self, context: &'static str) -> Result<u8, WireError> {
        Ok(self.take(1, context)?[0])
    }

    pub(crate) fn u16(&mut self, context: &'static str) -> Result<u16, WireError> {
        let mut buf = [0_u8; 2];
        buf.copy_from_slice(self.take(2, context)?);
        Ok(u16::from_le_bytes(buf))
    }

    pub(crate) fn u32(&mut self, context: &'static str) -> Result<u32, WireError> {
        let mut buf = [0_u8; 4];
        buf.copy_from_slice(self.take(4, context)?);
        Ok(u32::from_le_bytes(buf))
    }

    pub(crate) fn u64(&mut self, context: &'static str) -> Result<u64, WireError> {
        let mut buf = [0_u8; 8];
        buf.copy_from_slice(self.take(8, context)?);
        Ok(u64::from_le_bytes(buf))
    }

    pub(crate) fn i64(&mut self, context: &'static str) -> Result<i64, WireError> {
        Ok(self.u64(context)? as i64)
    }

    pub(crate) fn bytes(
        &mut self,
        len: usize,
        context: &'static str,
    ) -> Result<&'a [u8], WireError> {
        self.take(len, context)
    }
}

// ── Value (D.5) ───────────────────────────────────────────────────────────

pub(crate) fn encode_value(w: &mut Writer, value: &Value) {
    w.u8(value.tag());
    match value {
        Value::Null => {}
        Value::Int(i) => w.u64(*i as u64),
        Value::Float(f) => {
            // Canonicalise NaN so any NaN bit pattern round-trips to one value
            // (Rust's `NaN != NaN`, so a non-canonical NaN would fail equality
            // on decode). Matches `Value::hash_into`'s treatment.
            let canonical = if f.is_nan() { f64::NAN } else { *f };
            w.u64(canonical.to_bits());
        }
        Value::Bool(b) => w.u8(u8::from(*b)),
        Value::Str(id) | Value::HandlerRef(id) => w.u32(*id),
        Value::List(items) => {
            w.u16(items.len() as u16);
            for item in items {
                encode_value(w, item);
            }
        }
        Value::Record(fields) => {
            w.u16(fields.len() as u16);
            for (index, val) in fields {
                w.u16(*index);
                encode_value(w, val);
            }
        }
        // `Value` is `#[non_exhaustive]`; an unknown variant cannot be encoded
        // without a tag, so it is skipped. The value codec is exercised only
        // on values lowered by the (known) type checker.
        _ => {}
    }
}

/// Encodes a [`Value`] into a standalone Appendix D §D.5 blob (no frame header).
///
/// This is the on-the-wire storage encoding the host `StorageBackend`s persist:
/// a `set` writes this blob, a `get` decodes it back. The `flux-parity` harness
/// uses it to drive the persistence-parity trace (FLUX-082) without inventing a
/// second codec.
#[must_use]
pub fn encode_value_blob(value: &Value) -> Vec<u8> {
    let mut w = Writer::new();
    encode_value(&mut w, value);
    w.into_vec()
}

/// Decodes a [`Value`] from a standalone Appendix D §D.5 blob.
///
/// Returns [`WireError`] on a truncated or corrupt blob — the exact failure a
/// host `StorageBackend.get` must catch and treat as `absent` (FLUX-080/081),
/// never propagate as a host crash.
pub fn decode_value_blob(blob: &[u8]) -> Result<Value, WireError> {
    let mut r = Reader::new(blob);
    decode_value(&mut r)
}

const TAG_NULL: u8 = 0x00;
const TAG_INT: u8 = 0x01;
const TAG_FLOAT: u8 = 0x02;
const TAG_BOOL: u8 = 0x03;
const TAG_STR: u8 = 0x04;
const TAG_HANDLER: u8 = 0x05;
const TAG_LIST: u8 = 0x06;
const TAG_RECORD: u8 = 0x07;

pub(crate) fn decode_value(r: &mut Reader<'_>) -> Result<Value, WireError> {
    let tag = r.u8("value.tag")?;
    match tag {
        TAG_NULL => Ok(Value::Null),
        TAG_INT => Ok(Value::Int(r.i64("value.int")?)),
        TAG_FLOAT => Ok(Value::Float(f64::from_bits(r.u64("value.float")?))),
        TAG_BOOL => Ok(Value::Bool(r.u8("value.bool")? != 0)),
        TAG_STR => Ok(Value::Str(r.u32("value.str")?)),
        TAG_HANDLER => Ok(Value::HandlerRef(r.u32("value.handler")?)),
        TAG_LIST => {
            let count = r.u16("value.list.count")?;
            r.ensure_capacity(count as usize, "value.list")?;
            let mut items = Vec::with_capacity(count as usize);
            for _ in 0..count {
                items.push(decode_value(r)?);
            }
            Ok(Value::List(items))
        }
        TAG_RECORD => {
            let count = r.u16("value.record.count")?;
            r.ensure_capacity(count as usize, "value.record")?;
            let mut fields = Vec::with_capacity(count as usize);
            for _ in 0..count {
                let index = r.u16("value.record.index")?;
                let val = decode_value(r)?;
                fields.push((index, val));
            }
            Ok(Value::Record(fields))
        }
        other => Err(WireError::InvalidTag {
            tag: other,
            context: "value",
            at: r.pos - 1,
        }),
    }
}

// ── Child (D.4) ───────────────────────────────────────────────────────────

pub(crate) fn encode_child(w: &mut Writer, child: &Child) {
    match child {
        Child::Node(id) => {
            w.u8(0x01);
            w.u32(*id);
        }
        Child::Splice { items } => {
            w.u8(0x02);
            w.u16(items.len() as u16);
            for (key, id) in items {
                w.u64(*key);
                w.u32(*id);
            }
        }
        // `Child` is `#[non_exhaustive]` for future slot kinds; we cannot
        // encode an unknown kind, so we emit nothing rather than panic. The
        // dev server rejects trees containing unknown children before they
        // reach serialization (AGENTS.md: no `unreachable!` in prod).
        _ => {}
    }
}

pub(crate) fn decode_child(r: &mut Reader<'_>) -> Result<Child, WireError> {
    let tag = r.u8("child.tag")?;
    match tag {
        0x01 => Ok(Child::Node(r.u32("child.node")?)),
        0x02 => {
            let count = r.u16("child.splice.count")?;
            r.ensure_capacity(count as usize, "child.splice")?;
            let mut items = Vec::with_capacity(count as usize);
            for _ in 0..count {
                let key = r.u64("child.splice.key")?;
                let id = r.u32("child.splice.node")?;
                items.push((key, id));
            }
            Ok(Child::Splice { items })
        }
        other => Err(WireError::InvalidTag {
            tag: other,
            context: "child",
            at: r.pos - 1,
        }),
    }
}

// ── Props ──────────────────────────────────────────────────────────────────

pub(crate) fn encode_props(w: &mut Writer, props: &Props) {
    w.u16(props.fields().len() as u16);
    for (index, value) in props.fields() {
        w.u16(*index);
        encode_value(w, value);
    }
}

pub(crate) fn decode_props(r: &mut Reader<'_>) -> Result<Props, WireError> {
    let count = r.u16("props.count")?;
    r.ensure_capacity(count as usize, "props")?;
    let mut fields = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let index = r.u16("props.index")?;
        let value = decode_value(r)?;
        fields.push((index, value));
    }
    Ok(Props::from_fields(fields))
}

// ── Span ─────────────────────────────────────────────────────────────────

pub fn encode_span(w: &mut Writer, span: &Span) {
    w.u32(span.file_id);
    w.u32(span.start);
    w.u32(span.end);
}

pub fn decode_span(r: &mut Reader<'_>) -> Result<Span, WireError> {
    let file_id = r.u32("span.file")?;
    let start = r.u32("span.start")?;
    let end = r.u32("span.end")?;
    Ok(Span::new(file_id, start, end))
}

/// Writes a length-prefixed UTF-8 string (u16 byte length + bytes), matching the
/// layout `frame.rs::encode_str` uses for `Error`/`Hello` payloads.
pub(crate) fn encode_str(w: &mut Writer, s: &str) {
    w.u16(s.len() as u16);
    w.bytes(s.as_bytes());
}

/// Reads a length-prefixed UTF-8 string (u16 byte length + bytes).
pub(crate) fn decode_str(r: &mut Reader<'_>, ctx: &'static str) -> Result<String, WireError> {
    let len = r.u16(ctx)? as usize;
    let raw = r.bytes(len, ctx)?;
    std::str::from_utf8(raw)
        .map(str::to_owned)
        .map_err(|_| WireError::InvalidUtf8 {
            context: ctx,
            at: r.pos(),
        })
}

// ── Node (D.3) ─────────────────────────────────────────────────────────────

pub(crate) fn encode_node(w: &mut Writer, node: &NodeRef) {
    w.u32(node.id);
    w.u8(node.kind.tag());
    w.u32(node.component_id);
    encode_props(w, &node.props);
    w.u16(node.children.len() as u16);
    for child in &node.children {
        encode_child(w, child);
    }
    w.u16(node.handlers.len() as u16);
    for handler in &node.handlers {
        w.u32(*handler);
    }
    encode_span(w, &node.span);
}

pub(crate) fn decode_node(r: &mut Reader<'_>) -> Result<NodeRef, WireError> {
    let id = r.u32("node.id")?;
    let kind_tag = r.u8("node.kind")?;
    let kind = NodeKind::from_tag(kind_tag).ok_or(WireError::InvalidTag {
        tag: kind_tag,
        context: "node.kind",
        at: r.pos - 1,
    })?;
    let component_id = r.u32("node.component_id")?;
    let props = decode_props(r)?;
    let child_count = r.u16("node.child_count")?;
    r.ensure_capacity(child_count as usize, "node.children")?;
    let mut children = Vec::with_capacity(child_count as usize);
    for _ in 0..child_count {
        children.push(decode_child(r)?);
    }
    let handler_count = r.u16("node.handler_count")?;
    r.ensure_capacity(handler_count as usize, "node.handlers")?;
    let mut handlers = Vec::with_capacity(handler_count as usize);
    for _ in 0..handler_count {
        handlers.push(r.u32("node.handler")?);
    }
    let span = decode_span(r)?;
    Ok(NodeRef {
        id,
        kind,
        component_id,
        props,
        children,
        handlers,
        span,
    })
}

// ── PropDiff (D.6) ──────────────────────────────────────────────────────────

pub(crate) fn encode_prop_diff(w: &mut Writer, diff: &PropDiff) {
    w.u16(diff.changes.len() as u16);
    for (index, value) in &diff.changes {
        w.u16(*index);
        encode_value(w, value);
    }
    w.u16(diff.removals.len() as u16);
    for index in &diff.removals {
        w.u16(*index);
    }
}

pub(crate) fn decode_prop_diff(r: &mut Reader<'_>) -> Result<PropDiff, WireError> {
    let change_count = r.u16("propdiff.change_count")?;
    r.ensure_capacity(change_count as usize, "propdiff.changes")?;
    let mut changes = Vec::with_capacity(change_count as usize);
    for _ in 0..change_count {
        let index = r.u16("propdiff.index")?;
        let value = decode_value(r)?;
        changes.push((index, value));
    }
    let removal_count = r.u16("propdiff.removal_count")?;
    r.ensure_capacity(removal_count as usize, "propdiff.removals")?;
    let mut removals = Vec::with_capacity(removal_count as usize);
    for _ in 0..removal_count {
        removals.push(r.u16("propdiff.removal")?);
    }
    Ok(PropDiff { changes, removals })
}

// ── ClosureRef (D.7) ────────────────────────────────────────────────────────

pub(crate) fn encode_closure_ref(w: &mut Writer, closure: &ClosureRef) {
    w.u64(closure.hash);
    w.u32(closure.bytecode_offset);
    w.u16(closure.bytecode_len);
    w.u16(closure.captured_signals.len() as u16);
    for signal in &closure.captured_signals {
        w.u32(*signal);
    }
    encode_span(w, &closure.span);
    // ADR-0057: trailing server-computed source excerpt (gated by `has`), so a
    // VM fault maps `offset → handler → path:line:col + snippet` offline. Absent
    // on v1-derived trees (no source text) and decode-skipped there.
    match &closure.excerpt {
        Some(ex) => {
            w.u8(1);
            w.u32(ex.file_id);
            w.u32(ex.byte_start);
            w.u32(ex.byte_end);
            w.u16(ex.line);
            w.u16(ex.col);
            encode_str(w, &ex.snippet);
        }
        None => w.u8(0),
    }
}

pub(crate) fn decode_closure_ref(r: &mut Reader<'_>) -> Result<ClosureRef, WireError> {
    let hash = r.u64("closure.hash")?;
    let bytecode_offset = r.u32("closure.offset")?;
    let bytecode_len = r.u16("closure.len")?;
    let signal_count = r.u16("closure.signal_count")?;
    r.ensure_capacity(signal_count as usize, "closure.signals")?;
    let mut captured_signals = Vec::with_capacity(signal_count as usize);
    for _ in 0..signal_count {
        captured_signals.push(r.u32("closure.signal")?);
    }
    let span = decode_span(r)?;
    let excerpt = if r.u8("closure.excerpt.present")? != 0 {
        Some(SourceExcerpt {
            file_id: FileId::from(r.u32("closure.excerpt.file")?),
            byte_start: r.u32("closure.excerpt.start")?,
            byte_end: r.u32("closure.excerpt.end")?,
            line: r.u16("closure.excerpt.line")?,
            col: r.u16("closure.excerpt.col")?,
            snippet: decode_str(r, "closure.excerpt.snippet")?,
        })
    } else {
        None
    };
    Ok(ClosureRef {
        hash,
        bytecode_offset,
        bytecode_len,
        captured_signals,
        span,
        excerpt,
    })
}

// ── HandlerDef stream + bytecode blob (D.8 + D.12 handler section) ─────────

/// Encodes one `HandlerDef` entry (Appendix D §D.8): the `HandlerId` followed
/// by its `ClosureRef` body.
pub(crate) fn encode_handler_def(w: &mut Writer, id: HandlerId, closure: &ClosureRef) {
    w.u32(id);
    encode_closure_ref(w, closure);
}

/// Decodes one `HandlerDef` entry (Appendix D §D.8).
pub(crate) fn decode_handler_def(r: &mut Reader<'_>) -> Result<(HandlerId, ClosureRef), WireError> {
    let id = r.u32("handler.id")?;
    let closure = decode_closure_ref(r)?;
    Ok((id, closure))
}

/// Encodes the shared bytecode blob (Appendix D §D.12 handler section): a
/// `u32` byte length followed by the raw little-endian bytecode.
pub(crate) fn encode_bytecode_blob(w: &mut Writer, blob: &[u8]) {
    w.u32(blob.len() as u32);
    w.bytes(blob);
}

/// Decodes the shared bytecode blob, returning the raw bytes (Appendix D §D.12).
pub(crate) fn decode_bytecode_blob(r: &mut Reader<'_>) -> Result<Vec<u8>, WireError> {
    let len = r.u32("bytecode_blob.len")? as usize;
    r.bytes(len, "bytecode_blob").map(|slice| slice.to_vec())
}

// ── StringEntry (D.9) ─────────────────────────────────────────────────────

pub(crate) fn encode_string_entry(w: &mut Writer, id: StringId, text: &str) {
    w.u32(id);
    w.u16(text.len() as u16);
    w.bytes(text.as_bytes());
}

pub(crate) fn decode_string_entry(r: &mut Reader<'_>) -> Result<(StringId, String), WireError> {
    let id = r.u32("string.id")?;
    let len = r.u16("string.len")? as usize;
    let raw = r.bytes(len, "string.bytes")?;
    let text = std::str::from_utf8(raw).map_err(|_| WireError::InvalidUtf8 {
        context: "string",
        at: r.pos - len,
    })?;
    Ok((id, text.to_owned()))
}

// ── StateDelta (D.10) ───────────────────────────────────────────────────────

/// A delta over the live signal graph (Appendix D §D.10).
#[derive(Clone, Debug, Default)]
#[allow(dead_code)]
pub(crate) struct StateDelta {
    /// `(signal_id, value)` pairs, in any order.
    pub cells: Vec<(SignalId, Value)>,
}

impl StateDelta {
    #[allow(dead_code)]
    pub(crate) fn encode(w: &mut Writer, delta: &StateDelta) {
        w.u16(delta.cells.len() as u16);
        for (signal, value) in &delta.cells {
            w.u32(*signal);
            encode_value(w, value);
        }
    }

    #[allow(dead_code)]
    pub(crate) fn decode(r: &mut Reader<'_>) -> Result<StateDelta, WireError> {
        let count = r.u16("state.count")?;
        r.ensure_capacity(count as usize, "state.cells")?;
        let mut cells = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let signal = r.u32("state.signal")?;
            let value = decode_value(r)?;
            cells.push((signal, value));
        }
        Ok(StateDelta { cells })
    }
}

// ── SourceMapDelta (D.11) ───────────────────────────────────────────────────

/// New or changed source-file path mappings (Appendix D §D.11).
#[derive(Clone, Debug, Default)]
#[allow(dead_code)]
pub(crate) struct SourceMapDelta {
    /// `(file_id, path)` pairs.
    pub files: Vec<(FileId, String)>,
}

impl SourceMapDelta {
    #[allow(dead_code)]
    pub(crate) fn encode(w: &mut Writer, delta: &SourceMapDelta) {
        w.u16(delta.files.len() as u16);
        for (file_id, path) in &delta.files {
            w.u32(*file_id);
            w.u16(path.len() as u16);
            w.bytes(path.as_bytes());
        }
    }

    #[allow(dead_code)]
    pub(crate) fn decode(r: &mut Reader<'_>) -> Result<SourceMapDelta, WireError> {
        let count = r.u16("srcmap.count")?;
        r.ensure_capacity(count as usize, "srcmap.files")?;
        let mut files = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let file_id = r.u32("srcmap.file")?;
            let len = r.u16("srcmap.path_len")? as usize;
            let raw = r.bytes(len, "srcmap.path")?;
            let path = std::str::from_utf8(raw).map_err(|_| WireError::InvalidUtf8 {
                context: "srcmap.path",
                at: r.pos - len,
            })?;
            files.push((file_id, path.to_owned()));
        }
        Ok(SourceMapDelta { files })
    }
}

// ── Patch (D.2) ─────────────────────────────────────────────────────────────

pub(crate) fn encode_patch(w: &mut Writer, patch: &Patch) {
    w.u8(patch.tag());
    match patch {
        Patch::Replace { id, node } => {
            w.u32(*id);
            encode_node(w, node);
        }
        Patch::Update { id, props_diff } => {
            w.u32(*id);
            encode_prop_diff(w, props_diff);
        }
        Patch::Insert {
            parent,
            index,
            node,
        } => {
            w.u32(*parent);
            w.u16(*index);
            encode_node(w, node);
        }
        Patch::Remove { id } => {
            w.u32(*id);
        }
        Patch::Reorder { parent, keys } => {
            w.u32(*parent);
            w.u16(keys.len() as u16);
            for key in keys {
                w.u32(*key);
            }
        }
        Patch::Handler { id, closure } => {
            w.u32(*id);
            encode_closure_ref(w, closure);
        }
        Patch::Reattach {
            old_id,
            new_id,
            node,
        } => {
            w.u32(*old_id);
            w.u32(*new_id);
            encode_node(w, node);
        }
        // `Patch` is `#[non_exhaustive]`. An unknown variant cannot be encoded
        // without a wire tag, so it is skipped; the differ/pre-flight stage
        // guarantees only known variants reach the serializer.
        _ => {}
    }
}

pub(crate) fn decode_patch(r: &mut Reader<'_>) -> Result<Patch, WireError> {
    let tag = r.u8("patch.tag")?;
    match tag {
        0x01 => {
            let id = r.u32("patch.replace.id")?;
            let node = decode_node(r)?;
            Ok(Patch::Replace { id, node })
        }
        0x02 => {
            let id = r.u32("patch.update.id")?;
            let props_diff = decode_prop_diff(r)?;
            Ok(Patch::Update { id, props_diff })
        }
        0x03 => {
            let parent = r.u32("patch.insert.parent")?;
            let index = r.u16("patch.insert.index")?;
            let node = decode_node(r)?;
            Ok(Patch::Insert {
                parent,
                index,
                node,
            })
        }
        0x04 => {
            let id = r.u32("patch.remove.id")?;
            Ok(Patch::Remove { id })
        }
        0x05 => {
            let parent = r.u32("patch.reorder.parent")?;
            let key_count = r.u16("patch.reorder.keys")?;
            r.ensure_capacity(key_count as usize, "patch.reorder")?;
            let mut keys = Vec::with_capacity(key_count as usize);
            for _ in 0..key_count {
                keys.push(r.u32("patch.reorder.key")?);
            }
            Ok(Patch::Reorder { parent, keys })
        }
        0x06 => {
            let id = r.u32("patch.handler.id")?;
            let closure = decode_closure_ref(r)?;
            Ok(Patch::Handler { id, closure })
        }
        0x07 => {
            let old_id = r.u32("patch.reattach.old_id")?;
            let new_id = r.u32("patch.reattach.new_id")?;
            let node = decode_node(r)?;
            Ok(Patch::Reattach {
                old_id,
                new_id,
                node,
            })
        }
        other => Err(WireError::InvalidTag {
            tag: other,
            context: "patch",
            at: r.pos - 1,
        }),
    }
}

// ── ADR-0027 signal-graph metadata (FA-IRWIRE, T13/T14) ──────────────────────

/// One node's ADR-0027 Phase 2/3 signal-graph metadata on the wire
/// (Appendix D — gated by `FLAG_NODE_HAS_SIGNAL_DEPS`).
///
/// `deps` is the distinct, ascending `READ_SIGNAL` ids the node's prop and
/// control expressions read (`signal_deps`, T13). `thunk` is the optional
/// compiled prop-thunk `ClosureRef` (`prop_thunk`, T14); `None` for
/// control-only nodes. `layout` is the `prop_layout` — `record-field position →
/// prop index` mapping (T14). When `thunk` is `Some` its `captured_signals`
/// must equal `deps` (the thunk is the single source of truth); a decoder that
/// sees `thunk` without `deps` must reject the frame.
#[derive(Clone, Debug)]
pub struct NodeSignalMeta {
    /// The node this metadata describes.
    pub node_id: NodeId,
    /// Distinct, ascending signal ids the node reads.
    pub deps: Vec<SignalId>,
    /// Optional compiled prop thunk; `None` for control-only nodes.
    pub thunk: Option<ClosureRef>,
    /// `prop_layout`: record-field position → prop index.
    pub layout: Vec<u16>,
    /// For a `ForEach` node, the dedicated per-element `item` signal slot the
    /// body's row thunks read. The host allocates a fresh per-row signal seeded
    /// with `list[i]` and rewrites each row thunk's `READ_SIGNAL item_slot` to
    /// it when expanding the list (FLUX-072 / ADR-0050). `None` for every other
    /// node kind.
    pub item_slot: Option<SignalId>,
}

/// Encodes a `NodeSignalMeta` entry (Appendix D, ADR-0027 section).
///
/// Layout: `node_id(u32) | deps_count(u16) | deps(u32)* | thunk_present(u8)
/// | thunk(ClosureRef)? | layout_count(u16) | layout(u16)*`.
pub(crate) fn encode_signal_meta(w: &mut Writer, meta: &NodeSignalMeta) {
    w.u32(meta.node_id);
    w.u16(meta.deps.len() as u16);
    for &signal in &meta.deps {
        w.u32(signal);
    }
    match &meta.thunk {
        Some(closure) => {
            w.u8(1);
            encode_closure_ref(w, closure);
        }
        None => w.u8(0),
    }
    w.u16(meta.layout.len() as u16);
    for &idx in &meta.layout {
        w.u16(idx);
    }
    match meta.item_slot {
        Some(slot) => {
            w.u8(1);
            w.u32(slot);
        }
        None => w.u8(0),
    }
}

/// Decodes a `NodeSignalMeta` entry (Appendix D, ADR-0027 section).
///
/// Returns an error if a thunk is present but `deps` is empty — a thunk
/// without dependency data is unusable for pruning and must be rejected
/// (Appendix D §T13; `FLAG_NODE_HAS_SIGNAL_DEPS` gate).
pub(crate) fn decode_signal_meta(r: &mut Reader<'_>) -> Result<NodeSignalMeta, WireError> {
    let node_id = NodeId::from(r.u32("signal_meta.node")?);
    let dep_count = r.u16("signal_meta.deps.count")?;
    r.ensure_capacity(dep_count as usize, "signal_meta.deps")?;
    let mut deps = Vec::with_capacity(dep_count as usize);
    for _ in 0..dep_count {
        deps.push(SignalId::from(r.u32("signal_meta.deps.signal")?));
    }
    let thunk_present = r.u8("signal_meta.thunk.present")?;
    let thunk = if thunk_present != 0 {
        Some(decode_closure_ref(r)?)
    } else {
        None
    };
    // INV-1 (FA-IRWIRE): a thunk without dependency data is unusable for
    // pruning — reject the frame rather than ship a silent no-op.
    if thunk.is_some() && deps.is_empty() {
        return Err(WireError::InvalidTag {
            tag: thunk_present,
            context: "signal_meta.thunk_without_deps",
            at: r.pos,
        });
    }
    let layout_count = r.u16("signal_meta.layout.count")?;
    r.ensure_capacity(layout_count as usize, "signal_meta.layout")?;
    let mut layout = Vec::with_capacity(layout_count as usize);
    for _ in 0..layout_count {
        layout.push(r.u16("signal_meta.layout.idx")?);
    }
    let item_slot_present = r.u8("signal_meta.item_slot.present")?;
    let item_slot = if item_slot_present != 0 {
        Some(SignalId::from(r.u32("signal_meta.item_slot.id")?))
    } else {
        None
    };
    Ok(NodeSignalMeta {
        node_id,
        deps,
        thunk,
        layout,
        item_slot,
    })
}

/// Encodes a `Vec<NodeSignalMeta>` section: a `u16` count followed by entries.
pub(crate) fn encode_signal_meta_section(w: &mut Writer, metas: &[NodeSignalMeta]) {
    w.u16(metas.len() as u16);
    for meta in metas {
        encode_signal_meta(w, meta);
    }
}

/// Decodes a `Vec<NodeSignalMeta>` section: a `u16` count followed by entries.
pub(crate) fn decode_signal_meta_section(
    r: &mut Reader<'_>,
) -> Result<Vec<NodeSignalMeta>, WireError> {
    let count = r.u16("signal_meta.section.count")?;
    r.ensure_capacity(count as usize, "signal_meta.section")?;
    let mut metas = Vec::with_capacity(count as usize);
    for _ in 0..count {
        metas.push(decode_signal_meta(r)?);
    }
    Ok(metas)
}
