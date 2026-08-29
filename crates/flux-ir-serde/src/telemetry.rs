//! DevTools bidirectional telemetry wire codec (ADR-0039, Appendix D §D.12).
//!
//! Two new frame kinds extend the protocol:
//! - `Telemetry` (`0x10`, Host → Server): a batch of [`TelemetryEvent`]s
//!   streaming debug state out of the host VM / signal graph / reconciler.
//! - `DebugCommand` (`0x11`, Server → Host): a single control command
//!   (`Pause`, `Resume`, `Step`, `SetBreakpoint`, …) driving VM execution.
//!
//! Both reuse the shared `MAGIC`/`PROTOCOL_VERSION` header and the
//! `encode_value`/`decode_value` primitives from [`crate::wire`], so the bytes
//! stay byte-compatible with the Swift/Kotlin production decoders' conventions
//! (ADR-0039). All integers are little-endian.
//!
//! This module is intentionally isolated from `frame.rs`: it owns the
//! DevTools-specific unions and does not touch the existing `Hello`/`Init`/
//! `Delta`/`Error`/`Heartbeat` construction API, which is edited by a separate
//! in-flight change.

use flux_syntax::{EffectId, NodeId, SignalId, Span, Value};

use crate::frame::{MAGIC, PROTOCOL_VERSION};
use crate::wire::{
    Reader, WireError, Writer, decode_span, decode_value, encode_span, encode_value,
};

/// `frame_type` byte for the `Telemetry` frame (Host → Server), Appendix D §D.12.
pub const FRAME_TELEMETRY: u8 = 0x10;
/// `frame_type` byte for the `DebugCommand` frame (Server → Host), Appendix D §D.12.
pub const FRAME_DEBUG_COMMAND: u8 = 0x11;
/// `frame_type` byte for the `HostAnnounce` frame (Server → DevTools), Appendix
/// D §D.12. Carries the host identity the dev server learned during the `Hello`
/// handshake so the DevTools UI can show *which* device is being debugged
/// (e.g. an iOS Simulator vs an Android phone).
pub const FRAME_HOST_ANNOUNCE: u8 = 0x12;

/// A single axis-aligned rectangle in layout space (device points).
///
/// Mirrors the `CGRect`/`Rect` the reconciler reports; kept as a plain value so
/// it serializes without a dependency on the platform geometry types.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    /// Left edge (x origin).
    pub x: f64,
    /// Top edge (y origin).
    pub y: f64,
    /// Width.
    pub width: f64,
    /// Height.
    pub height: f64,
}

impl Rect {
    /// Encodes the rect as four little-endian `f64` fields.
    fn encode(w: &mut Writer, rect: &Rect) {
        w.u64(rect.x.to_bits());
        w.u64(rect.y.to_bits());
        w.u64(rect.width.to_bits());
        w.u64(rect.height.to_bits());
    }

    fn decode(r: &mut Reader<'_>) -> Result<Rect, WireError> {
        let x = f64::from_bits(r.u64("rect.x")?);
        let y = f64::from_bits(r.u64("rect.y")?);
        let width = f64::from_bits(r.u64("rect.width")?);
        let height = f64::from_bits(r.u64("rect.height")?);
        Ok(Rect {
            x,
            y,
            width,
            height,
        })
    }
}

/// The 16 VM registers (r0–r15) captured in a [`TelemetryEvent::VmStep`].
///
/// Boxed so the `VmStep` variant does not dominate the `TelemetryEvent` enum's
/// size (clippy `large_enum_variant`).
pub type Registers = Box<[Value; 16]>;

/// A debug telemetry event emitted by the host runtime.
///
/// Each variant is length-prefixed and encoded as a union (tag byte + fields)
/// so the decoder can skip unknown variants without desync. The host emits raw
/// IDs (`bytecode_offset`, `NodeId`); source-span enrichment happens
/// server-side (ADR-0039 / Phase 3), so the wire payload stays tiny.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum TelemetryEvent {
    /// Emitted after each VM instruction executes.
    VmStep {
        /// Instruction pointer the instruction was fetched at.
        bytecode_offset: u32,
        /// Opcode byte of the executed instruction.
        opcode: u8,
        /// Snapshot of registers r0–r15 after the step.
        registers: Registers,
        /// Remaining gas after the step.
        gas_remaining: u32,
    },
    /// Emitted when a signal's value changes.
    SignalWrite {
        /// Signal cell that changed.
        signal_id: SignalId,
        /// Pre-write value (or `Value::Null` if uninitialized).
        old_value: Value,
        /// Post-write value.
        new_value: Value,
        /// Effect IDs triggered by the write (SolidJS-style dependents).
        triggered_effect_ids: Vec<EffectId>,
    },
    /// Emitted when the reconciler mutates a native view.
    ViewMutation {
        /// IR node backing the native view.
        node_id: NodeId,
        /// Platform-native view handle (opaque u64; resolved by the host only).
        native_view_id: u64,
        /// Parent IR node id, so the DevTools can rebuild the component-tree
        /// hierarchy. `0` only when the node is the tree root.
        parent_id: NodeId,
        /// `0`=Add, `1`=Remove, `2`=Update, `3`=Layout.
        mutation_kind: u8,
        /// New layout frame when the mutation carries one.
        frame: Option<Rect>,
    },
    /// Emitted when a handler starts or finishes.
    HandlerInvocation {
        /// Handler that ran.
        handler_id: u32,
        /// `true` = started, `false` = finished.
        is_start: bool,
        /// Gas consumed; present only on finish (`is_start == false`).
        gas_used: Option<u32>,
    },
}

/// Tag byte for [`TelemetryEvent::VmStep`].
const EVENT_VM_STEP: u8 = 0x01;
/// Tag byte for [`TelemetryEvent::SignalWrite`].
const EVENT_SIGNAL_WRITE: u8 = 0x02;
/// Tag byte for [`TelemetryEvent::ViewMutation`].
const EVENT_VIEW_MUTATION: u8 = 0x03;
/// Tag byte for [`TelemetryEvent::HandlerInvocation`].
const EVENT_HANDLER_INVOCATION: u8 = 0x04;

impl TelemetryEvent {
    /// Encodes this event into `w` as a length-prefixed union (without the
    /// outer frame header — see [`TelemetryFrame`]).
    fn encode_into(&self, w: &mut Writer) {
        // Reserve a 4-byte length slot, encode the body, then back-patch the
        // length so the decoder can skip an unknown tag cleanly.
        let start = w.buf_len();
        w.u32(0);
        match self {
            TelemetryEvent::VmStep {
                bytecode_offset,
                opcode,
                registers,
                gas_remaining,
            } => {
                w.u8(EVENT_VM_STEP);
                w.u32(*bytecode_offset);
                w.u8(*opcode);
                for reg in registers.iter() {
                    encode_value(w, reg);
                }
                w.u32(*gas_remaining);
            }
            TelemetryEvent::SignalWrite {
                signal_id,
                old_value,
                new_value,
                triggered_effect_ids,
            } => {
                w.u8(EVENT_SIGNAL_WRITE);
                w.u32(*signal_id);
                encode_value(w, old_value);
                encode_value(w, new_value);
                w.u16(triggered_effect_ids.len() as u16);
                for id in triggered_effect_ids {
                    w.u32(*id);
                }
            }
            TelemetryEvent::ViewMutation {
                node_id,
                native_view_id,
                parent_id,
                mutation_kind,
                frame,
            } => {
                w.u8(EVENT_VIEW_MUTATION);
                w.u32(*node_id);
                w.u64(*native_view_id);
                w.u32(*parent_id);
                w.u8(*mutation_kind);
                match frame {
                    Some(rect) => {
                        w.u8(1);
                        Rect::encode(w, rect);
                    }
                    None => w.u8(0),
                }
            }
            TelemetryEvent::HandlerInvocation {
                handler_id,
                is_start,
                gas_used,
            } => {
                w.u8(EVENT_HANDLER_INVOCATION);
                w.u32(*handler_id);
                w.u8(u8::from(*is_start));
                match gas_used {
                    Some(gas) => {
                        w.u8(1);
                        w.u32(*gas);
                    }
                    None => w.u8(0),
                }
            }
        }
        let end = w.buf_len();
        // Back-patch the length (body + tag + the length field itself).
        let len = (end - start - 4) as u32;
        w.patch_u32_at(start, len);
    }

    /// Decodes one length-prefixed event from `r`.
    fn decode_from(r: &mut Reader<'_>) -> Result<TelemetryEvent, WireError> {
        let body_len = r.u32("telemetry.event.len")? as usize;
        // Bound the cursor to the declared body so a corrupt length cannot
        // read past this event into the next one.
        let body = r.take(body_len, "telemetry.event.body")?;
        let mut inner = Reader::new(body);
        let tag = inner.u8("telemetry.event.tag")?;
        match tag {
            EVENT_VM_STEP => {
                let bytecode_offset = inner.u32("vm_step.offset")?;
                let opcode = inner.u8("vm_step.opcode")?;
                let mut registers = Box::new(core::array::from_fn(|_| Value::Null));
                for slot in registers.iter_mut() {
                    *slot = decode_value(&mut inner)?;
                }
                let gas_remaining = inner.u32("vm_step.gas")?;
                Ok(TelemetryEvent::VmStep {
                    bytecode_offset,
                    opcode,
                    registers,
                    gas_remaining,
                })
            }
            EVENT_SIGNAL_WRITE => {
                let signal_id = inner.u32("signal_write.id")?;
                let old_value = decode_value(&mut inner)?;
                let new_value = decode_value(&mut inner)?;
                let count = inner.u16("signal_write.effects.count")?;
                let mut triggered_effect_ids = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    triggered_effect_ids.push(inner.u32("signal_write.effect")?);
                }
                Ok(TelemetryEvent::SignalWrite {
                    signal_id,
                    old_value,
                    new_value,
                    triggered_effect_ids,
                })
            }
            EVENT_VIEW_MUTATION => {
                let node_id = inner.u32("view_mutation.node")?;
                let native_view_id = inner.u64("view_mutation.native")?;
                let parent_id = inner.u32("view_mutation.parent")?;
                let mutation_kind = inner.u8("view_mutation.kind")?;
                let frame = match inner.u8("view_mutation.frame.present")? {
                    0 => None,
                    _ => Some(Rect::decode(&mut inner)?),
                };
                Ok(TelemetryEvent::ViewMutation {
                    node_id,
                    native_view_id,
                    parent_id,
                    mutation_kind,
                    frame,
                })
            }
            EVENT_HANDLER_INVOCATION => {
                let handler_id = inner.u32("handler_invocation.id")?;
                let is_start = inner.u8("handler_invocation.is_start")? != 0;
                let gas_used = match inner.u8("handler_invocation.gas.present")? {
                    0 => None,
                    _ => Some(inner.u32("handler_invocation.gas")?),
                };
                Ok(TelemetryEvent::HandlerInvocation {
                    handler_id,
                    is_start,
                    gas_used,
                })
            }
            other => Err(WireError::InvalidTag {
                tag: other,
                context: "telemetry.event",
                at: r.pos() - 1,
            }),
        }
    }
}

/// A decoded `Telemetry` frame (Appendix D §D.12, kind `0x10`).
///
/// Layout: `MAGIC(4) version(1) kind(0x10) event_count(2) [events...]`.
#[derive(Clone, Debug, PartialEq)]
pub struct TelemetryFrame {
    /// Protocol version.
    pub version: u8,
    /// Number of contained events.
    pub event_count: u16,
    /// The batched telemetry events (raw, host → server).
    pub events: Vec<TelemetryEvent>,
}

/// A `Telemetry` frame whose payload is server-enriched (server → DevTools).
///
/// Shares the byte layout of [`TelemetryFrame`] (the same `0x10` kind); the
/// only difference is the event payload type.
#[derive(Clone, Debug, PartialEq)]
pub struct EnrichedTelemetryFrame {
    /// Protocol version.
    pub version: u8,
    /// Number of contained events.
    pub event_count: u16,
    /// The batched enriched telemetry events.
    pub events: Vec<EnrichedTelemetryEvent>,
}

impl TelemetryFrame {
    /// Encodes this frame per Appendix D §D.12 (kind `0x10`).
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = Writer::new();
        w.u32(MAGIC);
        w.u8(PROTOCOL_VERSION);
        w.u8(FRAME_TELEMETRY);
        w.u16(self.event_count);
        for event in &self.events {
            event.encode_into(&mut w);
        }
        w.into_vec()
    }

    /// Decodes a `Telemetry` frame, or `None` if the header is malformed or the
    /// frame kind is not `0x10`.
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Option<TelemetryFrame> {
        let mut r = Reader::new(bytes);
        let magic = r.u32("telemetry.magic").ok()?;
        if magic != MAGIC {
            return None;
        }
        let version = r.u8("telemetry.version").ok()?;
        if version != PROTOCOL_VERSION {
            return None;
        }
        let kind = r.u8("telemetry.kind").ok()?;
        if kind != FRAME_TELEMETRY {
            return None;
        }
        let event_count = r.u16("telemetry.event_count").ok()?;
        let mut events = Vec::with_capacity(event_count as usize);
        for _ in 0..event_count {
            match TelemetryEvent::decode_from(&mut r) {
                Ok(event) => events.push(event),
                // Truncation inside the batch is fatal for the whole frame.
                Err(_) => return None,
            }
        }
        Some(TelemetryFrame {
            version,
            event_count,
            events,
        })
    }
}

impl EnrichedTelemetryFrame {
    /// Encodes this frame per Appendix D §D.12 (kind `0x10`).
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = Writer::new();
        w.u32(MAGIC);
        w.u8(PROTOCOL_VERSION);
        w.u8(FRAME_TELEMETRY);
        w.u16(self.event_count);
        for event in &self.events {
            event.encode_into(&mut w);
        }
        w.into_vec()
    }

    /// Decodes an enriched `Telemetry` frame, or `None` if malformed.
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Option<EnrichedTelemetryFrame> {
        let mut r = Reader::new(bytes);
        let magic = r.u32("telemetry.magic").ok()?;
        if magic != MAGIC {
            return None;
        }
        let version = r.u8("telemetry.version").ok()?;
        if version != PROTOCOL_VERSION {
            return None;
        }
        let kind = r.u8("telemetry.kind").ok()?;
        if kind != FRAME_TELEMETRY {
            return None;
        }
        let event_count = r.u16("telemetry.event_count").ok()?;
        let mut events = Vec::with_capacity(event_count as usize);
        for _ in 0..event_count {
            match EnrichedTelemetryEvent::decode_from(&mut r) {
                Ok(event) => events.push(event),
                Err(_) => return None,
            }
        }
        Some(EnrichedTelemetryFrame {
            version,
            event_count,
            events,
        })
    }
}

/// A `HostAnnounce` frame (Server → DevTools, Appendix D §D.12, kind `0x12`).
///
/// Sent once per host connection so the DevTools client knows *which* device it
/// is inspecting. The dev server learns this from the host's `Hello` handshake
/// (`platform`, `device`, advertised `capabilities`) and forwards it to every
/// subscribed DevTools client.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostAnnounceFrame {
    /// Protocol version.
    pub version: u8,
    /// Host platform, e.g. `"ios"` or `"android"`.
    pub platform: String,
    /// Device model string (e.g. `UIDevice.current.model` on iOS).
    pub device: String,
    /// Capabilities the host advertised at handshake (`(name, version, features)`).
    pub capabilities: Vec<(String, u32, Vec<String>)>,
}

impl HostAnnounceFrame {
    /// Encodes this frame per Appendix D §D.12 (kind `0x12`).
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = Writer::new();
        w.u32(MAGIC);
        w.u8(PROTOCOL_VERSION);
        w.u8(FRAME_HOST_ANNOUNCE);
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

    /// Decodes a `HostAnnounce` frame, or `None` if malformed / wrong kind.
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Option<HostAnnounceFrame> {
        let mut r = Reader::new(bytes);
        let magic = r.u32("host_announce.magic").ok()?;
        if magic != MAGIC {
            return None;
        }
        let version = r.u8("host_announce.version").ok()?;
        if version != PROTOCOL_VERSION {
            return None;
        }
        let kind = r.u8("host_announce.kind").ok()?;
        if kind != FRAME_HOST_ANNOUNCE {
            return None;
        }
        let platform = decode_str(&mut r, "host_announce.platform").ok()?;
        let device = decode_str(&mut r, "host_announce.device").ok()?;
        let cap_count = r.u16("host_announce.cap_count").ok()? as usize;
        let mut capabilities = Vec::with_capacity(cap_count);
        for _ in 0..cap_count {
            let name = decode_str(&mut r, "host_announce.cap.name").ok()?;
            let ver = r.u32("host_announce.cap.ver").ok()?;
            let feat_count = r.u16("host_announce.cap.feat_count").ok()? as usize;
            let mut feats = Vec::with_capacity(feat_count);
            for _ in 0..feat_count {
                feats.push(decode_str(&mut r, "host_announce.cap.feat").ok()?);
            }
            capabilities.push((name, ver, feats));
        }
        Some(HostAnnounceFrame {
            version,
            platform,
            device,
            capabilities,
        })
    }
}
/// `MAGIC(4) version(1) kind(0x11) command_id(4) payload_len(2) payload`.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum DebugCommand {
    /// Pause VM execution immediately.
    Pause,
    /// Resume VM execution.
    Resume,
    /// Execute exactly one VM instruction, then pause.
    Step,
    /// Set a breakpoint at a bytecode offset.
    SetBreakpoint {
        /// Bytecode offset to halt at.
        bytecode_offset: u32,
    },
    /// Remove a breakpoint.
    ClearBreakpoint {
        /// Bytecode offset to clear.
        bytecode_offset: u32,
    },
    /// Request a full state snapshot (base state for time-travel, ADR-0042).
    RequestSnapshot,
}

/// Tag byte for [`DebugCommand::Pause`].
const CMD_PAUSE: u8 = 0x01;
/// Tag byte for [`DebugCommand::Resume`].
const CMD_RESUME: u8 = 0x02;
/// Tag byte for [`DebugCommand::Step`].
const CMD_STEP: u8 = 0x03;
/// Tag byte for [`DebugCommand::SetBreakpoint`].
const CMD_SET_BREAKPOINT: u8 = 0x04;
/// Tag byte for [`DebugCommand::ClearBreakpoint`].
const CMD_CLEAR_BREAKPOINT: u8 = 0x05;
/// Tag byte for [`DebugCommand::RequestSnapshot`].
const CMD_REQUEST_SNAPSHOT: u8 = 0x06;

impl DebugCommand {
    /// Encodes the command payload (without the frame header) into `w`.
    fn encode_payload(&self, w: &mut Writer) {
        match self {
            DebugCommand::Pause => w.u8(CMD_PAUSE),
            DebugCommand::Resume => w.u8(CMD_RESUME),
            DebugCommand::Step => w.u8(CMD_STEP),
            DebugCommand::SetBreakpoint { bytecode_offset } => {
                w.u8(CMD_SET_BREAKPOINT);
                w.u32(*bytecode_offset);
            }
            DebugCommand::ClearBreakpoint { bytecode_offset } => {
                w.u8(CMD_CLEAR_BREAKPOINT);
                w.u32(*bytecode_offset);
            }
            DebugCommand::RequestSnapshot => w.u8(CMD_REQUEST_SNAPSHOT),
        }
    }

    /// Decodes a command payload (the bytes following `payload_len`).
    fn decode_payload(r: &mut Reader<'_>) -> Result<DebugCommand, WireError> {
        let tag = r.u8("debug_command.tag")?;
        match tag {
            CMD_PAUSE => Ok(DebugCommand::Pause),
            CMD_RESUME => Ok(DebugCommand::Resume),
            CMD_STEP => Ok(DebugCommand::Step),
            CMD_SET_BREAKPOINT => {
                let bytecode_offset = r.u32("debug_command.bp.offset")?;
                Ok(DebugCommand::SetBreakpoint { bytecode_offset })
            }
            CMD_CLEAR_BREAKPOINT => {
                let bytecode_offset = r.u32("debug_command.bp.clear")?;
                Ok(DebugCommand::ClearBreakpoint { bytecode_offset })
            }
            CMD_REQUEST_SNAPSHOT => Ok(DebugCommand::RequestSnapshot),
            other => Err(WireError::InvalidTag {
                tag: other,
                context: "debug_command",
                at: r.pos() - 1,
            }),
        }
    }
}

/// A decoded `DebugCommand` frame (Appendix D §D.12, kind `0x11`).
#[derive(Clone, Debug, PartialEq)]
pub struct DebugCommandFrame {
    /// Protocol version.
    pub version: u8,
    /// Monotonic command identifier (echoed by the host in its response).
    pub command_id: u32,
    /// The decoded command.
    pub command: DebugCommand,
}

impl DebugCommandFrame {
    /// Builds a `DebugCommand` frame for `command` with `command_id`.
    #[must_use]
    pub fn new(command_id: u32, command: DebugCommand) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            command_id,
            command,
        }
    }

    /// Encodes this frame per Appendix D §D.12 (kind `0x11`).
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        // Compute the payload length first by encoding into a scratch writer.
        let mut scratch = Writer::new();
        self.command.encode_payload(&mut scratch);
        let payload = scratch.into_vec();

        let mut w = Writer::new();
        w.u32(MAGIC);
        w.u8(PROTOCOL_VERSION);
        w.u8(FRAME_DEBUG_COMMAND);
        w.u32(self.command_id);
        w.u16(payload.len() as u16);
        w.bytes(&payload);
        w.into_vec()
    }

    /// Decodes a `DebugCommand` frame, or `None` if malformed / wrong kind.
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Option<DebugCommandFrame> {
        let mut r = Reader::new(bytes);
        let magic = r.u32("debugcmd.magic").ok()?;
        if magic != MAGIC {
            return None;
        }
        let version = r.u8("debugcmd.version").ok()?;
        if version != PROTOCOL_VERSION {
            return None;
        }
        let kind = r.u8("debugcmd.kind").ok()?;
        if kind != FRAME_DEBUG_COMMAND {
            return None;
        }
        let command_id = r.u32("debugcmd.id").ok()?;
        let payload_len = r.u16("debugcmd.payload_len").ok()? as usize;
        let payload = r.take(payload_len, "debugcmd.payload").ok()?;
        let mut inner = Reader::new(payload);
        let command = DebugCommand::decode_payload(&mut inner).ok()?;
        Some(DebugCommandFrame {
            version,
            command_id,
            command,
        })
    }
}

/// Enriched telemetry: a raw [`TelemetryEvent`] paired with the `.flux` source
/// span the dev server resolved for it (ADR-0039 / Phase 3).
///
/// `VmStep` and `ViewMutation` carry a `source_span`; `SignalWrite` and
/// `HandlerInvocation` are enriched with the span of the signal/handler they
/// reference. Events the server cannot resolve a span for carry `None`.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum EnrichedTelemetryEvent {
    /// A VM step, enriched with the source span of its bytecode offset.
    VmStep {
        /// Instruction pointer the instruction was fetched at.
        bytecode_offset: u32,
        /// Opcode byte of the executed instruction.
        opcode: u8,
        /// Snapshot of registers r0–r15 after the step.
        registers: Registers,
        /// Remaining gas after the step.
        gas_remaining: u32,
        /// `.flux` source span of the instruction, if resolvable.
        source_span: Option<Span>,
    },
    /// A signal write, enriched with the signal's declaration span.
    SignalWrite {
        /// Signal cell that changed.
        signal_id: SignalId,
        /// Pre-write value.
        old_value: Value,
        /// Post-write value.
        new_value: Value,
        /// Effect IDs triggered by the write.
        triggered_effect_ids: Vec<EffectId>,
        /// `.flux` source span of the signal declaration, if resolvable.
        source_span: Option<Span>,
    },
    /// A view mutation, enriched with the node's source span.
    ViewMutation {
        /// IR node backing the native view.
        node_id: NodeId,
        /// Platform-native view handle.
        native_view_id: u64,
        /// Parent IR node id (the DevTools rebuilds the tree from this).
        parent_id: NodeId,
        /// `0`=Add, `1`=Remove, `2`=Update, `3`=Layout.
        mutation_kind: u8,
        /// New layout frame when the mutation carries one.
        frame: Option<Rect>,
        /// `.flux` source span of the node, if resolvable.
        source_span: Option<Span>,
    },
    /// A handler invocation, enriched with the handler's source span.
    HandlerInvocation {
        /// Handler that ran.
        handler_id: u32,
        /// `true` = started, `false` = finished.
        is_start: bool,
        /// Gas consumed; present only on finish.
        gas_used: Option<u32>,
        /// `.flux` source span of the handler, if resolvable.
        source_span: Option<Span>,
    },
}

impl EnrichedTelemetryEvent {
    /// Encodes this enriched event into `w` as a length-prefixed union.
    fn encode_into(&self, w: &mut Writer) {
        let start = w.buf_len();
        w.u32(0);
        match self {
            EnrichedTelemetryEvent::VmStep {
                bytecode_offset,
                opcode,
                registers,
                gas_remaining,
                source_span,
            } => {
                w.u8(EVENT_VM_STEP);
                w.u32(*bytecode_offset);
                w.u8(*opcode);
                for reg in registers.iter() {
                    encode_value(w, reg);
                }
                w.u32(*gas_remaining);
                encode_optional_span(w, *source_span);
            }
            EnrichedTelemetryEvent::SignalWrite {
                signal_id,
                old_value,
                new_value,
                triggered_effect_ids,
                source_span,
            } => {
                w.u8(EVENT_SIGNAL_WRITE);
                w.u32(*signal_id);
                encode_value(w, old_value);
                encode_value(w, new_value);
                w.u16(triggered_effect_ids.len() as u16);
                for id in triggered_effect_ids {
                    w.u32(*id);
                }
                encode_optional_span(w, *source_span);
            }
            EnrichedTelemetryEvent::ViewMutation {
                node_id,
                native_view_id,
                parent_id,
                mutation_kind,
                frame,
                source_span,
            } => {
                w.u8(EVENT_VIEW_MUTATION);
                w.u32(*node_id);
                w.u64(*native_view_id);
                w.u32(*parent_id);
                w.u8(*mutation_kind);
                match frame {
                    Some(rect) => {
                        w.u8(1);
                        Rect::encode(w, rect);
                    }
                    None => w.u8(0),
                }
                encode_optional_span(w, *source_span);
            }
            EnrichedTelemetryEvent::HandlerInvocation {
                handler_id,
                is_start,
                gas_used,
                source_span,
            } => {
                w.u8(EVENT_HANDLER_INVOCATION);
                w.u32(*handler_id);
                w.u8(u8::from(*is_start));
                match gas_used {
                    Some(gas) => {
                        w.u8(1);
                        w.u32(*gas);
                    }
                    None => w.u8(0),
                }
                encode_optional_span(w, *source_span);
            }
        }
        let end = w.buf_len();
        let len = (end - start - 4) as u32;
        w.patch_u32_at(start, len);
    }

    /// Decodes one length-prefixed enriched event from `r`.
    fn decode_from(r: &mut Reader<'_>) -> Result<EnrichedTelemetryEvent, WireError> {
        let body_len = r.u32("enriched.event.len")? as usize;
        let body = r.take(body_len, "enriched.event.body")?;
        let mut inner = Reader::new(body);
        let tag = inner.u8("enriched.event.tag")?;
        match tag {
            EVENT_VM_STEP => {
                let bytecode_offset = inner.u32("vm_step.offset")?;
                let opcode = inner.u8("vm_step.opcode")?;
                let mut registers = Box::new(core::array::from_fn(|_| Value::Null));
                for slot in registers.iter_mut() {
                    *slot = decode_value(&mut inner)?;
                }
                let gas_remaining = inner.u32("vm_step.gas")?;
                let source_span = decode_optional_span(&mut inner)?;
                Ok(EnrichedTelemetryEvent::VmStep {
                    bytecode_offset,
                    opcode,
                    registers,
                    gas_remaining,
                    source_span,
                })
            }
            EVENT_SIGNAL_WRITE => {
                let signal_id = inner.u32("signal_write.id")?;
                let old_value = decode_value(&mut inner)?;
                let new_value = decode_value(&mut inner)?;
                let count = inner.u16("signal_write.effects.count")?;
                let mut triggered_effect_ids = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    triggered_effect_ids.push(inner.u32("signal_write.effect")?);
                }
                let source_span = decode_optional_span(&mut inner)?;
                Ok(EnrichedTelemetryEvent::SignalWrite {
                    signal_id,
                    old_value,
                    new_value,
                    triggered_effect_ids,
                    source_span,
                })
            }
            EVENT_VIEW_MUTATION => {
                let node_id = inner.u32("view_mutation.node")?;
                let native_view_id = inner.u64("view_mutation.native")?;
                let parent_id = inner.u32("view_mutation.parent")?;
                let mutation_kind = inner.u8("view_mutation.kind")?;
                let frame = match inner.u8("view_mutation.frame.present")? {
                    0 => None,
                    _ => Some(Rect::decode(&mut inner)?),
                };
                let source_span = decode_optional_span(&mut inner)?;
                Ok(EnrichedTelemetryEvent::ViewMutation {
                    node_id,
                    native_view_id,
                    parent_id,
                    mutation_kind,
                    frame,
                    source_span,
                })
            }
            EVENT_HANDLER_INVOCATION => {
                let handler_id = inner.u32("handler_invocation.id")?;
                let is_start = inner.u8("handler_invocation.is_start")? != 0;
                let gas_used = match inner.u8("handler_invocation.gas.present")? {
                    0 => None,
                    _ => Some(inner.u32("handler_invocation.gas")?),
                };
                let source_span = decode_optional_span(&mut inner)?;
                Ok(EnrichedTelemetryEvent::HandlerInvocation {
                    handler_id,
                    is_start,
                    gas_used,
                    source_span,
                })
            }
            other => Err(WireError::InvalidTag {
                tag: other,
                context: "enriched.event",
                at: r.pos() - 1,
            }),
        }
    }
}

/// Re-wraps a raw [`TelemetryEvent`] as an [`EnrichedTelemetryEvent`] with no
/// source span. The dev server attaches spans via [`enrich_with_span`] (which
/// calls this then overrides the span field); this no-span variant is the
/// client-side fallback when the server has not yet resolved a mapping.
///
/// [`enrich_with_span`]: crate::telemetry::enrich_with_span
#[must_use]
pub fn enrich_telemetry(event: TelemetryEvent) -> EnrichedTelemetryEvent {
    match event {
        TelemetryEvent::VmStep {
            bytecode_offset,
            opcode,
            registers,
            gas_remaining,
        } => EnrichedTelemetryEvent::VmStep {
            bytecode_offset,
            opcode,
            registers,
            gas_remaining,
            source_span: None,
        },
        TelemetryEvent::SignalWrite {
            signal_id,
            old_value,
            new_value,
            triggered_effect_ids,
        } => EnrichedTelemetryEvent::SignalWrite {
            signal_id,
            old_value,
            new_value,
            triggered_effect_ids,
            source_span: None,
        },
        TelemetryEvent::ViewMutation {
            node_id,
            native_view_id,
            parent_id,
            mutation_kind,
            frame,
        } => EnrichedTelemetryEvent::ViewMutation {
            node_id,
            native_view_id,
            parent_id,
            mutation_kind,
            frame,
            source_span: None,
        },
        TelemetryEvent::HandlerInvocation {
            handler_id,
            is_start,
            gas_used,
        } => EnrichedTelemetryEvent::HandlerInvocation {
            handler_id,
            is_start,
            gas_used,
            source_span: None,
        },
    }
}

/// Enriches a raw [`TelemetryEvent`] with a resolved source `span`, falling back
/// to [`enrich_telemetry`] (no span) when `span` is `None`. Used by the dev
/// server bridge (Phase 3) once it has resolved the `.flux` source location.
#[must_use]
pub fn enrich_with_span(event: TelemetryEvent, span: Option<Span>) -> EnrichedTelemetryEvent {
    let mut enriched = enrich_telemetry(event);
    match &mut enriched {
        EnrichedTelemetryEvent::VmStep { source_span, .. }
        | EnrichedTelemetryEvent::SignalWrite { source_span, .. }
        | EnrichedTelemetryEvent::ViewMutation { source_span, .. }
        | EnrichedTelemetryEvent::HandlerInvocation { source_span, .. } => *source_span = span,
    }
    enriched
}

/// Encodes an optional source `Span` as a present byte followed by the span.
fn encode_optional_span(w: &mut Writer, span: Option<Span>) {
    match span {
        Some(span) => {
            w.u8(1);
            encode_span(w, &span);
        }
        None => w.u8(0),
    }
}

/// Decodes an optional source `Span`.
fn decode_optional_span(r: &mut Reader<'_>) -> Result<Option<Span>, WireError> {
    match r.u8("event.span.present")? {
        0 => Ok(None),
        _ => Ok(Some(decode_span(r)?)),
    }
}

/// Length-prefixed UTF-8 string encoder (little-endian `u16` length).
fn encode_str(w: &mut Writer, s: &str) {
    w.u16(s.len() as u16);
    w.bytes(s.as_bytes());
}

/// Length-prefixed UTF-8 string decoder (little-endian `u16` length).
fn decode_str(r: &mut Reader<'_>, ctx: &'static str) -> Result<String, WireError> {
    let len = r.u16(ctx)?.into();
    let raw = r.bytes(len, ctx)?;
    std::str::from_utf8(raw)
        .map(str::to_owned)
        .map_err(|_| WireError::InvalidUtf8 {
            context: ctx,
            at: 0,
        })
}
