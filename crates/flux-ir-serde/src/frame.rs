//! Wire frame construction and header codec (Appendix D §D.1, §D.12).
//!
//! Every frame begins with the fixed 16-byte header (magic, version, sequence,
//! flags, and three counts) followed by a typed payload. The typed-frame API
//! (`Frame::hello`/`init`/`delta`/`error`/`heartbeat`) lays out each payload
//! per Appendix D §D.12 and §D.9; production decoders in Swift/Kotlin read the
//! same structure.

use crate::wire::{
    Reader, WireError, Writer, decode_node, decode_patch, decode_string_entry, decode_value,
    encode_node, encode_patch, encode_string_entry, encode_value,
};
use flux_syntax::{FileId, NodeRef, Patch, SignalId, Span, StringId, StringTable, Value};

/// Magic bytes `"FLUX"` in little-endian (`0x465C5558`).
pub const MAGIC: u32 = 0x465C_5558;
/// Current wire protocol version.
pub const PROTOCOL_VERSION: u8 = 1;

const FLAG_FULL_TREE: u8 = 1 << 0;
const FLAG_ERROR: u8 = 1 << 1;
const FLAG_HEARTBEAT: u8 = 1 << 2;
const FLAG_HELLO: u8 = 1 << 3;

/// The kind of a wire frame, derived from the `flags` bitfield (Appendix D §D.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameKind {
    /// Handshake frame (Host → Server, Appendix D §D.12.1).
    Hello,
    /// Full-tree `Init` frame (Appendix D §D.12.2).
    Init,
    /// Patch delta frame.
    Delta,
    /// Error frame.
    Error,
    /// Heartbeat.
    Heartbeat,
}

impl FrameKind {
    fn flag(self) -> u8 {
        match self {
            FrameKind::Hello => FLAG_HELLO,
            FrameKind::Init => FLAG_FULL_TREE,
            FrameKind::Delta => 0,
            FrameKind::Error => FLAG_ERROR,
            FrameKind::Heartbeat => FLAG_HEARTBEAT,
        }
    }

    fn from_flags(flags: u8) -> FrameKind {
        if flags & FLAG_ERROR != 0 {
            FrameKind::Error
        } else if flags & FLAG_HEARTBEAT != 0 {
            FrameKind::Heartbeat
        } else if flags & FLAG_FULL_TREE != 0 {
            FrameKind::Init
        } else if flags & FLAG_HELLO != 0 {
            FrameKind::Hello
        } else {
            FrameKind::Delta
        }
    }
}

/// Marker type carrying the frame-construction API. Use `Frame::hello`,
/// `Frame::init`, `Frame::delta`, `Frame::error`, `Frame::heartbeat` and their
/// `from_*_bytes` decoders.
#[derive(Debug)]
pub struct Frame;

// ── shared header helpers ───────────────────────────────────────────────────

fn write_header(w: &mut Writer, kind: FrameKind) {
    w.u8(MAGIC as u8);
    w.u8((MAGIC >> 8) as u8);
    w.u8((MAGIC >> 16) as u8);
    w.u8((MAGIC >> 24) as u8);
    w.u8(PROTOCOL_VERSION);
    w.u32(0); // sequence — set by caller before shipping
    w.u8(kind.flag());
    w.u16(0);
    w.u16(0);
    w.u16(0);
}

/// Splits a frame buffer into `(version, seq, kind, payload)`.
fn read_header(bytes: &[u8]) -> Result<(u8, u32, FrameKind, &[u8]), WireError> {
    if bytes.len() < 16 {
        return Err(WireError::Truncated {
            at: 0,
            needed: 16,
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
    let seq = u32::from_le_bytes(bytes[5..9].try_into().unwrap());
    let kind = FrameKind::from_flags(bytes[9]);
    Ok((version, seq, kind, &bytes[16..]))
}

// ── str helpers (not exported by wire.rs) ───────────────────────────────────

fn encode_str(w: &mut Writer, s: &str) {
    w.u16(s.len() as u16);
    w.bytes(s.as_bytes());
}

fn decode_str(r: &mut Reader<'_>) -> Result<String, WireError> {
    let len = r.u16("str.len")? as usize;
    let raw = r.bytes(len, "str")?;
    std::str::from_utf8(raw)
        .map(str::to_owned)
        .map_err(|_| WireError::InvalidUtf8 {
            context: "str",
            at: r.pos() - len,
        })
}

fn encode_span(w: &mut Writer, span: &Span) {
    w.u32(span.file_id);
    w.u32(span.start);
    w.u32(span.end);
}

fn decode_span(r: &mut Reader<'_>) -> Result<Span, WireError> {
    let file_id = r.u32("span.file")?;
    let start = r.u32("span.start")?;
    let end = r.u32("span.end")?;
    Ok(Span::new(file_id, start, end))
}

// ── Hello (D.12.1) ──────────────────────────────────────────────────────────

/// A decoded `Hello` handshake frame (Appendix D §D.12.1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HelloFrame {
    /// Protocol version.
    pub version: u8,
    /// Monotonic sequence number.
    pub seq: u32,
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
            seq: 0,
            kind: FrameKind::Hello,
            platform: platform.to_owned(),
            device: device.to_owned(),
            capabilities: capabilities.to_vec(),
        }
    }

    /// Decodes a `Hello` frame, or `None` on a malformed/short buffer.
    #[must_use]
    pub fn from_hello_bytes(bytes: &[u8]) -> Option<HelloFrame> {
        let (version, seq, kind, payload) = read_header(bytes).ok()?;
        if kind != FrameKind::Hello {
            return None;
        }
        let mut r = Reader::new(payload);
        let platform = decode_str(&mut r).ok()?;
        let device = decode_str(&mut r).ok()?;
        let cap_count = r.u16("hello.caps").ok()?;
        let mut capabilities = Vec::with_capacity(cap_count as usize);
        for _ in 0..cap_count {
            let name = decode_str(&mut r).ok()?;
            let ver = r.u32("hello.cap.ver").ok()?;
            let feat_count = r.u16("hello.cap.feats").ok()?;
            let mut feats = Vec::with_capacity(feat_count as usize);
            for _ in 0..feat_count {
                feats.push(decode_str(&mut r).ok()?);
            }
            capabilities.push((name, ver, feats));
        }
        Some(HelloFrame {
            version,
            seq,
            kind,
            platform,
            device,
            capabilities,
        })
    }
}

impl HelloFrame {
    /// Encodes the `Hello` frame per Appendix D §D.1 + §D.12.1.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = Writer::new();
        write_header(&mut w, FrameKind::Hello);
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
}

impl Frame {
    /// Builds an `Init` (full-tree) frame (Appendix D §D.12.2).
    #[must_use]
    pub fn init(
        root: &NodeRef,
        state_seed: &[(SignalId, Value)],
        source_map: &[(FileId, String)],
        table: &StringTable,
    ) -> InitFrame {
        InitFrame {
            version: PROTOCOL_VERSION,
            seq: 0,
            kind: FrameKind::Init,
            root: root.clone(),
            state_seed: state_seed.to_vec(),
            source_map: source_map.to_vec(),
            string_table: table.clone(),
        }
    }

    /// Decodes an `Init` frame.
    pub fn from_init_bytes(bytes: &[u8]) -> Result<InitFrame, WireError> {
        let (version, seq, kind, payload) = read_header(bytes)?;
        if kind != FrameKind::Init {
            return Err(WireError::InvalidTag {
                tag: 0,
                context: "frame.kind.init",
                at: 9,
            });
        }
        let mut r = Reader::new(payload);
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
            let path = decode_str(&mut r)?;
            source_map.push((fid, path));
        }
        let str_count = r.u16("init.strings")?;
        let mut string_table = StringTable::new();
        for _ in 0..str_count {
            let (id, text) = decode_string_entry(&mut r)?;
            // Interning in ID order reproduces the original dense IDs (the table
            // assigns IDs from zero in insertion order).
            let _ = id;
            string_table.intern(&text);
        }
        Ok(InitFrame {
            version,
            seq,
            kind,
            root,
            state_seed,
            source_map,
            string_table,
        })
    }
}

impl InitFrame {
    /// Encodes the `Init` frame per Appendix D §D.1 + §D.12.2.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = Writer::new();
        write_header(&mut w, FrameKind::Init);
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
        w.u16(entries.len() as u16);
        for (id, text) in &entries {
            encode_string_entry(&mut w, *id, text);
        }
        w.into_vec()
    }
}

// ── Delta (D.12.3) ───────────────────────────────────────────────────────────

/// A decoded `Delta` (patch) frame.
#[derive(Clone, Debug)]
pub struct DeltaFrame {
    /// Protocol version.
    pub version: u8,
    /// Monotonic sequence number.
    pub seq: u32,
    /// Frame kind (always `Delta`).
    pub kind: FrameKind,
    /// Patch stream.
    pub patches: Vec<Patch>,
    /// Newly interned strings carried by this frame.
    pub strings: Vec<(StringId, String)>,
}

impl Frame {
    /// Builds a `Delta` frame carrying `patches` and a string delta.
    #[must_use]
    pub fn delta(patches: &[Patch], strings: &[(StringId, String)]) -> DeltaFrame {
        DeltaFrame {
            version: PROTOCOL_VERSION,
            seq: 0,
            kind: FrameKind::Delta,
            patches: patches.to_vec(),
            strings: strings.to_vec(),
        }
    }

    /// Decodes a `Delta` frame.
    pub fn from_delta_bytes(bytes: &[u8]) -> Result<DeltaFrame, WireError> {
        let (version, seq, kind, payload) = read_header(bytes)?;
        if kind != FrameKind::Delta {
            return Err(WireError::InvalidTag {
                tag: 0,
                context: "frame.kind.delta",
                at: 9,
            });
        }
        let mut r = Reader::new(payload);
        let patch_count = r.u16("delta.patch_count")?;
        let mut patches = Vec::with_capacity(patch_count as usize);
        for _ in 0..patch_count {
            patches.push(decode_patch(&mut r)?);
        }
        let str_count = r.u16("delta.string_count")?;
        let mut strings = Vec::with_capacity(str_count as usize);
        for _ in 0..str_count {
            strings.push(decode_string_entry(&mut r)?);
        }
        Ok(DeltaFrame {
            version,
            seq,
            kind,
            patches,
            strings,
        })
    }
}

impl DeltaFrame {
    /// Encodes the `Delta` frame per Appendix D §D.1 + §D.2/§D.9.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = Writer::new();
        write_header(&mut w, FrameKind::Delta);
        w.u16(self.patches.len() as u16);
        for patch in &self.patches {
            encode_patch(&mut w, patch);
        }
        w.u16(self.strings.len() as u16);
        for (id, text) in &self.strings {
            encode_string_entry(&mut w, *id, text);
        }
        w.into_vec()
    }
}

// ── Error (D.12.4) ──────────────────────────────────────────────────────────

/// A decoded `Error` frame (Appendix D §D.12.4).
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
        let (version, seq, kind, payload) = read_header(bytes)?;
        if kind != FrameKind::Error {
            return Err(WireError::InvalidTag {
                tag: 0,
                context: "frame.kind.error",
                at: 9,
            });
        }
        let mut r = Reader::new(payload);
        let message = decode_str(&mut r)?;
        let has_span = r.u8("error.span_flag")?;
        let span = if has_span != 0 {
            Some(decode_span(&mut r)?)
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

impl ErrorFrame {
    /// Encodes the `Error` frame per Appendix D §D.1 + §D.12.4.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = Writer::new();
        write_header(&mut w, FrameKind::Error);
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
        let (version, seq, kind, _payload) = read_header(bytes).ok()?;
        if kind != FrameKind::Heartbeat {
            return None;
        }
        Some(HeartbeatFrame { version, seq, kind })
    }
}

impl HeartbeatFrame {
    /// Encodes the `Heartbeat` frame per Appendix D §D.1.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = Writer::new();
        write_header(&mut w, FrameKind::Heartbeat);
        w.into_vec()
    }
}
