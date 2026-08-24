//! Wire frame construction and header codec (Appendix D §D.1, §D.12).
//!
//! The frame family uses two header shapes:
//! - **Delta** frames follow the generic D.1 header: `magic(4) version(1)
//!   frame_type(1)=0x04 seq(4) flags(1) patch_count(2) handler_count(2)
//!   string_count(2)`, payload at offset 16.
//! - **Hello / Init / Error / Heartbeat** follow the D.12 handshake layout:
//!   `magic(4) version(1) frame_type(1)` then a type-specific payload
//!   (Hello has no sequence number; Init/Error carry `seq` at offset 6).
//!
//! All integers are little-endian. Production decoders in Swift/Kotlin (FLUX-007,
//! FLUX-008) read the same byte layout.
//!
//! **Handler transport (Gap G1):** `Init` and `Delta` frames optionally carry a
//! handler section — a shared `bytecode` blob plus a `HandlerDef` stream (D.8)
//! whose `ClosureRef`s index that blob by offset/length. This is the wire bridge
//! the dev server needs to ship handler bodies; before it, closures had no
//! transport.

use crate::wire::{
    Reader, WireError, Writer, decode_bytecode_blob, decode_handler_def, decode_node, decode_patch,
    decode_string_entry, decode_value, encode_bytecode_blob, encode_handler_def, encode_node,
    encode_patch, encode_string_entry, encode_value,
};
use flux_ir::ClosureIR;
use flux_syntax::{
    FileId, HandlerId, NodeRef, Patch, SignalId, Span, StringId, StringTable, Value,
};

/// Magic bytes `"FLUX"` in little-endian (`0x465C5558`).
pub const MAGIC: u32 = 0x465C_5558;
/// Current wire protocol version.
pub const PROTOCOL_VERSION: u8 = 1;

/// `frame_type` byte at header offset 5 (Appendix D §D.12).
pub const FRAME_HELLO: u8 = 0x01;
/// `frame_type` for the `Init` (full-tree) frame.
pub const FRAME_INIT: u8 = 0x02;
/// `frame_type` for the `Error` frame.
pub const FRAME_ERROR: u8 = 0x03;
/// `frame_type` for the `Delta` (patch) frame (D.1).
pub const FRAME_DELTA: u8 = 0x04;
/// `frame_type` for the `Heartbeat` frame (D.12.5).
pub const FRAME_HEARTBEAT: u8 = 0x05;

/// Bit flags inside a `Delta` frame's `flags` byte (D.1).
///
/// These name the reserved `flags` bits; the devserver sets/reads them when it
/// ships `StateDelta`/`SourceMapDelta`/string-delta sections (D.10–D.11). The
/// Delta encoder here leaves `flags` at 0 by default, so the constants are not
/// yet referenced in-tree — kept as the normative bit map.
#[allow(dead_code)]
pub const FLAG_FULL_TREE: u8 = 1 << 0;
/// Error frame flag.
#[allow(dead_code)]
pub const FLAG_ERROR: u8 = 1 << 1;
/// Heartbeat flag.
#[allow(dead_code)]
pub const FLAG_HEARTBEAT: u8 = 1 << 2;
/// Carries a `StateDelta` after the strings.
#[allow(dead_code)]
pub const FLAG_HAS_STATE_DELTA: u8 = 1 << 3;
/// Carries a `SourceMapDelta` after the state delta.
#[allow(dead_code)]
pub const FLAG_HAS_SRC_MAP_DELTA: u8 = 1 << 4;
/// Carries a `StringEntry` delta (otherwise `string_count` is 0).
#[allow(dead_code)]
pub const FLAG_HAS_STRING_DELTA: u8 = 1 << 5;

/// The kind of a wire frame, derived from the `frame_type` byte (Appendix D §D.12).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameKind {
    /// Handshake frame (Host → Server, Appendix D §D.12.1).
    Hello,
    /// Full-tree `Init` frame (Appendix D §D.12.2).
    Init,
    /// Error frame.
    Error,
    /// Patch delta frame.
    Delta,
    /// Heartbeat.
    Heartbeat,
}

impl FrameKind {
    fn type_byte(self) -> u8 {
        match self {
            FrameKind::Hello => FRAME_HELLO,
            FrameKind::Init => FRAME_INIT,
            FrameKind::Error => FRAME_ERROR,
            FrameKind::Delta => FRAME_DELTA,
            FrameKind::Heartbeat => FRAME_HEARTBEAT,
        }
    }

    fn from_type_byte(tag: u8) -> Option<FrameKind> {
        match tag {
            FRAME_HELLO => Some(FrameKind::Hello),
            FRAME_INIT => Some(FrameKind::Init),
            FRAME_ERROR => Some(FrameKind::Error),
            FRAME_DELTA => Some(FrameKind::Delta),
            FRAME_HEARTBEAT => Some(FrameKind::Heartbeat),
            _ => None,
        }
    }
}

/// Marker type carrying the frame-construction API. Use `Frame::hello`,
/// `Frame::init`, `Frame::delta`, `Frame::error`, `Frame::heartbeat` and their
/// `from_*_bytes` decoders.
#[derive(Debug)]
pub struct Frame;

// ── shared primitive helpers ────────────────────────────────────────────────

fn write_magic_version(w: &mut Writer) {
    w.u32(MAGIC);
    w.u8(PROTOCOL_VERSION);
}

/// Writes the frame-level handler section (Gap G1, Appendix D §D.8 + §D.12):
/// a shared `bytecode` blob, then a `HandlerDef` stream whose `ClosureRef`s
/// index that blob by `bytecode_offset`/`bytecode_len`.
fn write_closures(w: &mut Writer, closures: &[ClosureIR]) {
    if closures.is_empty() {
        encode_bytecode_blob(w, &[]);
        return;
    }
    // Concatenate every closure's bytecode into one blob and record each
    // closure's offset within it, so the `ClosureRef` indices stay stable.
    let mut blob: Vec<u8> = Vec::new();
    let mut offsets: Vec<(HandlerId, u32, u16)> = Vec::with_capacity(closures.len());
    for closure in closures {
        let offset = blob.len() as u32;
        blob.extend_from_slice(&closure.bytecode);
        offsets.push((closure.id, offset, closure.bytecode.len() as u16));
    }
    encode_bytecode_blob(w, &blob);
    w.u16(closures.len() as u16);
    for closure in closures {
        let (_, offset, len) = offsets
            .iter()
            .find(|(id, _, _)| *id == closure.id)
            .copied()
            .expect("closure id present in offsets");
        let closure_ref = flux_syntax::ClosureRef {
            hash: crate::hash_closure(&closure.bytecode, &closure.captured_signals),
            bytecode_offset: offset,
            bytecode_len: len,
            captured_signals: closure.captured_signals.clone(),
            span: closure.span,
        };
        encode_handler_def(w, closure.id, &closure_ref);
    }
}

/// Validates the magic + version prefix and returns the `frame_type` byte and
/// the remaining payload.
fn read_frame_type(bytes: &[u8]) -> Result<(u8, FrameKind, &[u8]), WireError> {
    if bytes.len() < 6 {
        return Err(WireError::Truncated {
            at: 0,
            needed: 6,
            context: "frame.header",
            available: bytes.len(),
        });
    }
    let magic = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
    if magic != MAGIC {
        return Err(WireError::InvalidTag {
            tag: (magic & 0xFF) as u8,
            context: "frame.magic",
            at: 0,
        });
    }
    let version = bytes[4];
    if version != PROTOCOL_VERSION {
        return Err(WireError::InvalidTag {
            tag: version,
            context: "frame.version",
            at: 4,
        });
    }
    let kind = FrameKind::from_type_byte(bytes[5]).ok_or(WireError::InvalidTag {
        tag: bytes[5],
        context: "frame.type",
        at: 5,
    })?;
    Ok((version, kind, &bytes[6..]))
}

// ── str helpers (not exported by wire.rs) ───────────────────────────────────

fn encode_str(w: &mut Writer, s: &str) {
    w.u16(s.len() as u16);
    w.bytes(s.as_bytes());
}

fn decode_str(r: &[u8], pos: &mut usize) -> Result<String, WireError> {
    let len = r
        .get(*pos..*pos + 2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]) as usize)
        .ok_or(WireError::Truncated {
            at: *pos,
            needed: 2,
            context: "str.len",
            available: r.len(),
        })?;
    *pos += 2;
    let raw = r.get(*pos..*pos + len).ok_or(WireError::Truncated {
        at: *pos,
        needed: len,
        context: "str",
        available: r.len(),
    })?;
    *pos += len;
    std::str::from_utf8(raw)
        .map(str::to_owned)
        .map_err(|_| WireError::InvalidUtf8 {
            context: "str",
            at: *pos - len,
        })
}

fn encode_span(w: &mut Writer, span: &Span) {
    w.u32(span.file_id);
    w.u32(span.start);
    w.u32(span.end);
}

fn read_u32(r: &[u8], pos: &mut usize, ctx: &'static str) -> Result<u32, WireError> {
    let b = r.get(*pos..*pos + 4).ok_or(WireError::Truncated {
        at: *pos,
        needed: 4,
        context: ctx,
        available: r.len(),
    })?;
    *pos += 4;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

// ── Hello (D.12.1) ──────────────────────────────────────────────────────────

/// A decoded `Hello` handshake frame (Appendix D §D.12.1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HelloFrame {
    /// Protocol version.
    pub version: u8,
    /// Frame kind (always `Hello`).
    pub kind: FrameKind,
    /// Host platform, e.g. `"ios"` or `"android"`.
    pub platform: String,
    /// Device model string.
    pub device: String,
    /// `(capability, version, features)` triples advertised by the host.
    pub capabilities: Vec<(String, u32, Vec<String>)>,
}

impl Frame {
    /// Builds a `Hello` handshake frame (Appendix D §D.12.1).
    #[must_use]
    pub fn hello(
        platform: &str,
        device: &str,
        capabilities: &[(String, u32, Vec<String>)],
    ) -> HelloFrame {
        HelloFrame {
            version: PROTOCOL_VERSION,
            kind: FrameKind::Hello,
            platform: platform.to_owned(),
            device: device.to_owned(),
            capabilities: capabilities.to_vec(),
        }
    }

    /// Decodes a `Hello` frame, or `None` on a malformed/short buffer.
    #[must_use]
    pub fn from_hello_bytes(bytes: &[u8]) -> Option<HelloFrame> {
        let (version, kind, payload) = read_frame_type(bytes).ok()?;
        if kind != FrameKind::Hello {
            return None;
        }
        let mut pos = 0;
        let platform = decode_str(payload, &mut pos).ok()?;
        let device = decode_str(payload, &mut pos).ok()?;
        let cap_count =
            u16::from_le_bytes([payload.get(pos).copied()?, payload.get(pos + 1).copied()?])
                as usize;
        pos += 2;
        let mut capabilities = Vec::with_capacity(cap_count);
        for _ in 0..cap_count {
            let name = decode_str(payload, &mut pos).ok()?;
            let ver = read_u32(payload, &mut pos, "hello.cap.ver").ok()?;
            let feat_count =
                u16::from_le_bytes([payload.get(pos).copied()?, payload.get(pos + 1).copied()?])
                    as usize;
            pos += 2;
            let mut feats = Vec::with_capacity(feat_count);
            for _ in 0..feat_count {
                feats.push(decode_str(payload, &mut pos).ok()?);
            }
            capabilities.push((name, ver, feats));
        }
        Some(HelloFrame {
            version,
            kind,
            platform,
            device,
            capabilities,
        })
    }
}

impl HelloFrame {
    /// Encodes the `Hello` frame per Appendix D §D.12.1.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = Writer::new();
        write_magic_version(&mut w);
        w.u8(self.kind.type_byte());
        encode_str(&mut w, &self.platform);
        encode_str(&mut w, &self.device);
        w.u16(self.capabilities.len() as u16);
        for (name, ver, feats) in &self.capabilities {
            encode_str(&mut w, name);
            w.u32(*ver);
            w.u16(feats.len() as u16);
            for f in feats {
                encode_str(&mut w, f);
            }
        }
        w.into_vec()
    }
}

// ── Init (D.12.2) ────────────────────────────────────────────────────────────

/// A decoded `Init` (full-tree) frame (Appendix D §D.12.2).
#[derive(Clone, Debug)]
pub struct InitFrame {
    /// Protocol version.
    pub version: u8,
    /// Monotonic sequence number.
    pub seq: u32,
    /// Frame kind (always `Init`).
    pub kind: FrameKind,
    /// The full root node of the reactive tree.
    pub root: NodeRef,
    /// Initial signal-graph values.
    pub state_seed: Vec<(SignalId, Value)>,
    /// Source-file path mappings.
    pub source_map: Vec<(FileId, String)>,
    /// The string table the tree resolves against.
    pub string_table: StringTable,
    /// Handler closures shipped with the tree (Gap G1). Empty when the tree
    /// carries no handlers.
    pub closures: Vec<ClosureIR>,
}

impl Frame {
    /// Builds an `Init` (full-tree) frame (Appendix D §D.12.2).
    #[must_use]
    pub fn init(
        root: &NodeRef,
        state_seed: &[(SignalId, Value)],
        source_map: &[(FileId, String)],
        table: &StringTable,
        closures: &[ClosureIR],
    ) -> InitFrame {
        InitFrame {
            version: PROTOCOL_VERSION,
            seq: 0,
            kind: FrameKind::Init,
            root: root.clone(),
            state_seed: state_seed.to_vec(),
            source_map: source_map.to_vec(),
            string_table: table.clone(),
            closures: closures.to_vec(),
        }
    }

    /// Decodes an `Init` frame.
    pub fn from_init_bytes(bytes: &[u8]) -> Result<InitFrame, WireError> {
        let (version, kind, payload) = read_frame_type(bytes)?;
        if kind != FrameKind::Init {
            return Err(WireError::InvalidTag {
                tag: 0,
                context: "frame.kind.init",
                at: 5,
            });
        }
        let mut r = Reader::new(payload);
        let seq = r.u32("init.seq")?;
        let root = decode_node(&mut r)?;
        let seed_count = r.u16("init.seed")?;
        let mut state_seed = Vec::with_capacity(seed_count as usize);
        for _ in 0..seed_count {
            let sig = SignalId::from(r.u32("init.seed.sig")?);
            let val = decode_value(&mut r)?;
            state_seed.push((sig, val));
        }
        let sm_count = r.u16("init.srcmap")?;
        let mut source_map = Vec::with_capacity(sm_count as usize);
        for _ in 0..sm_count {
            let fid = FileId::from(r.u32("init.srcmap.file")?);
            let len = r.u16("init.srcmap.path.len")? as usize;
            let raw = r.bytes(len, "init.srcmap.path")?;
            let path = std::str::from_utf8(raw).map(str::to_owned).map_err(|_| {
                WireError::InvalidUtf8 {
                    context: "init.srcmap.path",
                    at: r.pos(),
                }
            })?;
            source_map.push((fid, path));
        }
        // D.12.2: `string_count` is a u32.
        let str_count = r.u32("init.string_count")? as usize;
        let mut entries: Vec<(StringId, String)> = Vec::with_capacity(str_count);
        for _ in 0..str_count {
            entries.push(decode_string_entry(&mut r)?);
        }
        entries.sort_by_key(|(id, _)| *id);
        let mut string_table = StringTable::new();
        for (_, text) in &entries {
            string_table.intern(text);
        }
        // D.12 handler section (Gap G1): a shared bytecode blob followed by a
        // `HandlerDef` stream; each `ClosureRef` indexes the blob.
        let closures = decode_closures(&mut r)?;
        Ok(InitFrame {
            version,
            seq,
            kind,
            root,
            state_seed,
            source_map,
            string_table,
            closures,
        })
    }
}

/// Decodes a frame's handler section: the shared bytecode blob (D.12) followed
/// by a `HandlerDef` stream (D.8). Each `HandlerDef`'s `ClosureRef` indexes the
/// blob by `bytecode_offset`/`bytecode_len`. An empty blob means no handlers.
fn decode_closures(r: &mut Reader<'_>) -> Result<Vec<ClosureIR>, WireError> {
    let blob = decode_bytecode_blob(r)?;
    if blob.is_empty() {
        return Ok(Vec::new());
    }
    let handler_count = r.u16("closures.count")? as usize;
    let mut closures = Vec::with_capacity(handler_count);
    for _ in 0..handler_count {
        let (id, closure_ref) = decode_handler_def(r)?;
        let start = closure_ref.bytecode_offset as usize;
        let end = start + usize::from(closure_ref.bytecode_len);
        let bytecode = blob
            .get(start..end)
            .ok_or(WireError::Truncated {
                at: start,
                needed: end.saturating_sub(start),
                context: "closure.bytecode",
                available: blob.len(),
            })?
            .to_vec();
        closures.push(ClosureIR {
            id,
            bytecode,
            captured_signals: closure_ref.captured_signals,
            span: closure_ref.span,
            param_types: Vec::new(),
            return_type: flux_syntax::TypeId::from(0u32),
        });
    }
    Ok(closures)
}

impl InitFrame {
    /// Encodes the `Init` frame per Appendix D §D.1 + §D.12.2 + §D.12 handler section.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = Writer::new();
        write_magic_version(&mut w);
        w.u8(self.kind.type_byte());
        w.u32(self.seq);
        encode_node(&mut w, &self.root);
        w.u16(self.state_seed.len() as u16);
        for (sig, val) in &self.state_seed {
            w.u32(*sig);
            encode_value(&mut w, val);
        }
        w.u16(self.source_map.len() as u16);
        for (fid, path) in &self.source_map {
            w.u32(*fid);
            encode_str(&mut w, path);
        }
        let entries: Vec<(StringId, String)> = self
            .string_table
            .iter()
            .map(|(id, text)| (id, text.to_owned()))
            .collect();
        // D.12.2: `string_count` is a u32.
        w.u32(entries.len() as u32);
        for (id, text) in &entries {
            encode_string_entry(&mut w, *id, text);
        }
        // D.12 handler section (Gap G1): shared blob, then HandlerDef stream.
        write_closures(&mut w, &self.closures);
        w.into_vec()
    }
}

// ── Delta (D.1) ──────────────────────────────────────────────────────────────

/// A decoded `Delta` (patch) frame.
#[derive(Clone, Debug)]
pub struct DeltaFrame {
    /// Protocol version.
    pub version: u8,
    /// Monotonic sequence number.
    pub seq: u32,
    /// Frame kind (always `Delta`).
    pub kind: FrameKind,
    /// Delta flags (D.1 bitfield).
    pub flags: u8,
    /// Patch stream.
    pub patches: Vec<Patch>,
    /// Newly interned strings carried by this frame.
    pub strings: Vec<(StringId, String)>,
    /// Handler closures carried by this frame (Gap G1). Empty when the delta
    /// introduces no new handlers.
    pub closures: Vec<ClosureIR>,
}

impl Frame {
    /// Builds a `Delta` frame carrying `patches`, a string delta and a handler
    /// closure delta.
    #[must_use]
    pub fn delta(
        seq: u32,
        flags: u8,
        patches: &[Patch],
        strings: &[(StringId, String)],
        closures: &[ClosureIR],
    ) -> DeltaFrame {
        DeltaFrame {
            version: PROTOCOL_VERSION,
            seq,
            kind: FrameKind::Delta,
            flags,
            patches: patches.to_vec(),
            strings: strings.to_vec(),
            closures: closures.to_vec(),
        }
    }

    /// Decodes a `Delta` frame.
    pub fn from_delta_bytes(bytes: &[u8]) -> Result<DeltaFrame, WireError> {
        let (version, kind, payload) = read_frame_type(bytes)?;
        if kind != FrameKind::Delta {
            return Err(WireError::InvalidTag {
                tag: 0,
                context: "frame.kind.delta",
                at: 5,
            });
        }
        if payload.len() < 10 {
            return Err(WireError::Truncated {
                at: 6,
                needed: 10,
                context: "delta.header",
                available: payload.len(),
            });
        }
        let seq = u32::from_le_bytes(payload[0..4].try_into().unwrap());
        let flags = payload[4];
        let patch_count = u16::from_le_bytes([payload[5], payload[6]]) as usize;
        let _handler_count = u16::from_le_bytes([payload[7], payload[8]]);
        let str_count = u16::from_le_bytes([payload[9], payload[10]]) as usize;
        let mut r = Reader::new(&payload[11..]);
        let mut patches = Vec::with_capacity(patch_count);
        for _ in 0..patch_count {
            patches.push(decode_patch(&mut r)?);
        }
        let mut strings = Vec::with_capacity(str_count);
        for _ in 0..str_count {
            strings.push(decode_string_entry(&mut r)?);
        }
        // D.12 handler section (Gap G1): shared blob, then HandlerDef stream.
        let closures = decode_closures(&mut r)?;
        Ok(DeltaFrame {
            version,
            seq,
            kind,
            flags,
            patches,
            strings,
            closures,
        })
    }
}

impl DeltaFrame {
    /// Encodes the `Delta` frame per Appendix D §D.1 + §D.2/§D.9 + §D.12 handler section.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = Writer::new();
        write_magic_version(&mut w);
        w.u8(self.kind.type_byte());
        w.u32(self.seq);
        w.u8(self.flags);
        w.u16(self.patches.len() as u16);
        w.u16(self.closures.len() as u16); // D.1 handler_count (now meaningful)
        w.u16(self.strings.len() as u16);
        for patch in &self.patches {
            encode_patch(&mut w, patch);
        }
        for (id, text) in &self.strings {
            encode_string_entry(&mut w, *id, text);
        }
        write_closures(&mut w, &self.closures);
        w.into_vec()
    }
}

// ── Error (D.12.3) ──────────────────────────────────────────────────────────

/// A decoded `Error` frame (Appendix D §D.12.3).
#[derive(Clone, Debug)]
pub struct ErrorFrame {
    /// Protocol version.
    pub version: u8,
    /// Monotonic sequence number.
    pub seq: u32,
    /// Frame kind (always `Error`).
    pub kind: FrameKind,
    /// Human-readable error message.
    pub message: String,
    /// Source span where the error occurred, if known.
    pub span: Option<Span>,
}

impl Frame {
    /// Builds an `Error` frame.
    #[must_use]
    pub fn error(seq: u32, message: &str, span: Option<Span>) -> ErrorFrame {
        ErrorFrame {
            version: PROTOCOL_VERSION,
            seq,
            kind: FrameKind::Error,
            message: message.to_owned(),
            span,
        }
    }

    /// Decodes an `Error` frame.
    pub fn from_error_bytes(bytes: &[u8]) -> Result<ErrorFrame, WireError> {
        let (version, kind, payload) = read_frame_type(bytes)?;
        if kind != FrameKind::Error {
            return Err(WireError::InvalidTag {
                tag: 0,
                context: "frame.kind.error",
                at: 5,
            });
        }
        let mut r = Reader::new(payload);
        let seq = r.u32("error.seq")?;
        let msg_len = r.u16("error.msg.len")? as usize;
        let raw = r.bytes(msg_len, "error.msg")?;
        let message =
            std::str::from_utf8(raw)
                .map(str::to_owned)
                .map_err(|_| WireError::InvalidUtf8 {
                    context: "error.msg",
                    at: r.pos(),
                })?;
        let has_span = r.u8("error.span_flag")?;
        let span = if has_span != 0 {
            Some(decode_span_from_reader(&mut r)?)
        } else {
            None
        };
        Ok(ErrorFrame {
            version,
            seq,
            kind,
            message,
            span,
        })
    }
}

/// Decodes a `Span` from the shared `Reader` (mirrors `decode_span` but on the
/// reader type the Error frame uses).
fn decode_span_from_reader(r: &mut Reader<'_>) -> Result<Span, WireError> {
    let file_id = r.u32("span.file")?;
    let start = r.u32("span.start")?;
    let end = r.u32("span.end")?;
    Ok(Span::new(file_id, start, end))
}

impl ErrorFrame {
    /// Encodes the `Error` frame per Appendix D §D.1 + §D.12.3.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = Writer::new();
        write_magic_version(&mut w);
        w.u8(self.kind.type_byte());
        w.u32(self.seq);
        encode_str(&mut w, &self.message);
        match &self.span {
            Some(span) => {
                w.u8(1);
                encode_span(&mut w, span);
            }
            None => w.u8(0),
        }
        w.into_vec()
    }
}

// ── Heartbeat (D.12.5) ──────────────────────────────────────────────────────

/// A decoded `Heartbeat` frame (Appendix D §D.12.5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeartbeatFrame {
    /// Protocol version.
    pub version: u8,
    /// Monotonic sequence number.
    pub seq: u32,
    /// Frame kind (always `Heartbeat`).
    pub kind: FrameKind,
}

impl Frame {
    /// Builds a `Heartbeat` frame.
    #[must_use]
    pub fn heartbeat(seq: u32) -> HeartbeatFrame {
        HeartbeatFrame {
            version: PROTOCOL_VERSION,
            seq,
            kind: FrameKind::Heartbeat,
        }
    }

    /// Decodes a `Heartbeat` frame.
    #[must_use]
    pub fn from_heartbeat_bytes(bytes: &[u8]) -> Option<HeartbeatFrame> {
        let (version, kind, payload) = read_frame_type(bytes).ok()?;
        if kind != FrameKind::Heartbeat {
            return None;
        }
        if payload.len() < 4 {
            return None;
        }
        let seq = u32::from_le_bytes(payload[0..4].try_into().unwrap());
        Some(HeartbeatFrame { version, seq, kind })
    }
}

impl HeartbeatFrame {
    /// Encodes the `Heartbeat` frame per Appendix D §D.12.5.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = Writer::new();
        write_magic_version(&mut w);
        w.u8(self.kind.type_byte());
        w.u32(self.seq);
        w.into_vec()
    }
}
