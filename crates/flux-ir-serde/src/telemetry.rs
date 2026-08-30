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
        /// Resolved component/adapter name (e.g. `Row`, `Button`), for DevTools labels.
        component_name: String,
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
    /// Emitted when the HTTP capability (FLUX-047) issues a request, so the
    /// DevTools network inspector (FLUX-060) can show outbound traffic. Carries
    /// the method, URL, latency-sensitive metadata, and (optionally) a request
    /// body snippet — never the full body, to keep the telemetry frame small.
    NetworkRequest {
        /// Stable per-request id so the matching [`NetworkResponse`](Self::NetworkResponse)
        /// can be correlated (a response with no matching request is a protocol error).
        request_id: u32,
        /// HTTP method, e.g. `GET`, `POST`.
        method: String,
        /// Fully-qualified request URL.
        url: String,
        /// Optional request body snippet (truncated). `None` for GET/HEAD.
        body: Option<String>,
        /// Opaque capability id that issued the request (diagnostics: which
        /// `Http.fetch` call), carried as a u32 wire value.
        capability_id: u32,
    },
    /// Emitted when a pending HTTP request resolves, so the inspector can pair
    /// it with its [`NetworkRequest`](Self::NetworkRequest) and show status,
    /// latency, and a response snippet.
    NetworkResponse {
        /// The request id this response answers.
        request_id: u32,
        /// HTTP status code (e.g. `200`, `404`).
        status_code: u16,
        /// Latency in milliseconds, measured host-side from send to resolve.
        latency_ms: u32,
        /// Optional response body snippet (truncated). `None` for empty bodies.
        body: Option<String>,
        /// `0`=Pending, `1`=Ready, `2`=Error (cell state, ADR-0044). A `2` with a
        /// `body` snippet carries the error text instead of a payload.
        result_kind: u8,
    },
    /// Emitted when the render-perf harness (PRD-J / FLUX-059) reports a
    /// `MetricRecord` — the timeline/flamegraph data source in the DevTools.
    /// The full record travels as the stable JSON produced by
    /// `flux_perf_harness::MetricRecord::to_json` (the harness's canonical,
    /// parseable document — PRD-J Implementation Decisions), so no new wire
    /// *field* is introduced: the DevTools consumes the record verbatim.
    PerfRecord {
        /// The verbatim `MetricRecord` JSON emitted by the harness.
        json: String,
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
/// Tag byte for [`TelemetryEvent::NetworkRequest`].
const EVENT_NETWORK_REQUEST: u8 = 0x05;
/// Tag byte for [`TelemetryEvent::NetworkResponse`].
const EVENT_NETWORK_RESPONSE: u8 = 0x06;
/// Tag byte for [`TelemetryEvent::PerfRecord`].
const EVENT_PERF_RECORD: u8 = 0x07;

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
                component_name,
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
                let name_bytes = component_name.as_bytes();
                w.u32(name_bytes.len() as u32);
                w.bytes(name_bytes);
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
            TelemetryEvent::NetworkRequest {
                request_id,
                method,
                url,
                body,
                capability_id,
            } => {
                w.u8(EVENT_NETWORK_REQUEST);
                w.u32(*request_id);
                encode_str(w, method);
                encode_str(w, url);
                match body {
                    Some(b) => {
                        w.u8(1);
                        encode_str(w, b);
                    }
                    None => w.u8(0),
                }
                w.u32(*capability_id);
            }
            TelemetryEvent::NetworkResponse {
                request_id,
                status_code,
                latency_ms,
                body,
                result_kind,
            } => {
                w.u8(EVENT_NETWORK_RESPONSE);
                w.u32(*request_id);
                w.u16(*status_code);
                w.u32(*latency_ms);
                match body {
                    Some(b) => {
                        w.u8(1);
                        encode_str(w, b);
                    }
                    None => w.u8(0),
                }
                w.u8(*result_kind);
            }
            TelemetryEvent::PerfRecord { json } => {
                w.u8(EVENT_PERF_RECORD);
                encode_str(w, json);
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
                let name_len = inner.u32("view_mutation.name.len")? as usize;
                let name_raw = inner.bytes(name_len, "view_mutation.name")?;
                let component_name = String::from_utf8_lossy(name_raw).into_owned();
                Ok(TelemetryEvent::ViewMutation {
                    node_id,
                    native_view_id,
                    parent_id,
                    mutation_kind,
                    frame,
                    component_name,
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
            EVENT_NETWORK_REQUEST => {
                let request_id = inner.u32("network_request.id")?;
                let method = decode_str(&mut inner, "network_request.method")?;
                let url = decode_str(&mut inner, "network_request.url")?;
                let body = match inner.u8("network_request.body.present")? {
                    0 => None,
                    _ => Some(decode_str(&mut inner, "network_request.body")?),
                };
                let capability_id = inner.u32("network_request.cap")?;
                Ok(TelemetryEvent::NetworkRequest {
                    request_id,
                    method,
                    url,
                    body,
                    capability_id,
                })
            }
            EVENT_NETWORK_RESPONSE => {
                let request_id = inner.u32("network_response.id")?;
                let status_code = inner.u16("network_response.status")?;
                let latency_ms = inner.u32("network_response.latency")?;
                let body = match inner.u8("network_response.body.present")? {
                    0 => None,
                    _ => Some(decode_str(&mut inner, "network_response.body")?),
                };
                let result_kind = inner.u8("network_response.result")?;
                Ok(TelemetryEvent::NetworkResponse {
                    request_id,
                    status_code,
                    latency_ms,
                    body,
                    result_kind,
                })
            }
            EVENT_PERF_RECORD => {
                let json = decode_str(&mut inner, "perf_record.json")?;
                Ok(TelemetryEvent::PerfRecord { json })
            }
            other => Err(WireError::InvalidTag {
                tag: other,
                context: "telemetry.event",
                at: r.pos() - 1,
            }),
        }
    }
}

impl TelemetryEvent {
    /// Builds a [`PerfRecord`](Self::PerfRecord) telemetry event carrying the
    /// verbatim `MetricRecord` JSON produced by `flux_perf_harness::
    /// MetricRecord::to_json`. This is the single helper the dev server / harness
    /// uses to emit a render-perf record onto the `0x10` telemetry frame (FLUX-059
    /// / PRD-J) without introducing a new wire field — the record travels as the
    /// harness's canonical, parseable JSON document.
    #[must_use]
    pub fn perf_record(json: impl Into<String>) -> Self {
        Self::PerfRecord { json: json.into() }
    }
}
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
        /// Resolved component/adapter name (e.g. `Row`, `Button`), for DevTools labels.
        component_name: String,
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
    /// An outbound HTTP request (FLUX-047), enriched (the network inspector
    /// pairs it with its [`NetworkResponse`](Self::NetworkResponse)). Enrichment
    /// carries no source span for now (a future phase may attach the `.flux`
    /// `Http.fetch` call site); the variant stays field-compatible with the raw
    /// [`TelemetryEvent::NetworkRequest`].
    NetworkRequest {
        /// Stable per-request id.
        request_id: u32,
        /// HTTP method.
        method: String,
        /// Fully-qualified request URL.
        url: String,
        /// Optional request body snippet (truncated).
        body: Option<String>,
        /// Opaque capability id that issued the request.
        capability_id: u32,
        /// `.flux` source span of the `Http.fetch` call, if resolvable.
        source_span: Option<Span>,
    },
    /// A resolved HTTP response (FLUX-047), enriched.
    NetworkResponse {
        /// The request id this response answers.
        request_id: u32,
        /// HTTP status code.
        status_code: u16,
        /// Latency in milliseconds.
        latency_ms: u32,
        /// Optional response body snippet (truncated).
        body: Option<String>,
        /// `0`=Pending, `1`=Ready, `2`=Error (cell state, ADR-0044).
        result_kind: u8,
        /// `.flux` source span of the `Http.fetch` call, if resolvable.
        source_span: Option<Span>,
    },
    /// A render-perf harness `MetricRecord` (PRD-J / FLUX-059) — the DevTools
    /// timeline/flamegraph data source. Carries the verbatim `MetricRecord` JSON
    /// produced by `flux_perf_harness::MetricRecord::to_json`; the DevTools
    /// consumes it directly, so no new wire field is introduced. `PerfRecord`
    /// has no source span (it is not tied to a `.flux` location).
    PerfRecord {
        /// The verbatim `MetricRecord` JSON emitted by the harness.
        json: String,
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
                component_name,
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
                let name_bytes = component_name.as_bytes();
                w.u32(name_bytes.len() as u32);
                w.bytes(name_bytes);
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
            EnrichedTelemetryEvent::NetworkRequest {
                request_id,
                method,
                url,
                body,
                capability_id,
                source_span,
            } => {
                w.u8(EVENT_NETWORK_REQUEST);
                w.u32(*request_id);
                encode_str(w, method);
                encode_str(w, url);
                match body {
                    Some(b) => {
                        w.u8(1);
                        encode_str(w, b);
                    }
                    None => w.u8(0),
                }
                w.u32(*capability_id);
                encode_optional_span(w, *source_span);
            }
            EnrichedTelemetryEvent::NetworkResponse {
                request_id,
                status_code,
                latency_ms,
                body,
                result_kind,
                source_span,
            } => {
                w.u8(EVENT_NETWORK_RESPONSE);
                w.u32(*request_id);
                w.u16(*status_code);
                w.u32(*latency_ms);
                match body {
                    Some(b) => {
                        w.u8(1);
                        encode_str(w, b);
                    }
                    None => w.u8(0),
                }
                w.u8(*result_kind);
                encode_optional_span(w, *source_span);
            }
            EnrichedTelemetryEvent::PerfRecord { json } => {
                w.u8(EVENT_PERF_RECORD);
                encode_str(w, json);
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
                let name_len = inner.u32("enriched.view_mutation.name.len")? as usize;
                let name_raw = inner.bytes(name_len, "enriched.view_mutation.name")?;
                let component_name = String::from_utf8_lossy(name_raw).into_owned();
                Ok(EnrichedTelemetryEvent::ViewMutation {
                    node_id,
                    native_view_id,
                    parent_id,
                    mutation_kind,
                    frame,
                    source_span,
                    component_name,
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
            EVENT_NETWORK_REQUEST => {
                let request_id = inner.u32("enriched.network_request.id")?;
                let method = decode_str(&mut inner, "enriched.network_request.method")?;
                let url = decode_str(&mut inner, "enriched.network_request.url")?;
                let body = match inner.u8("enriched.network_request.body.present")? {
                    0 => None,
                    _ => Some(decode_str(&mut inner, "enriched.network_request.body")?),
                };
                let capability_id = inner.u32("enriched.network_request.cap")?;
                let source_span = decode_optional_span(&mut inner)?;
                Ok(EnrichedTelemetryEvent::NetworkRequest {
                    request_id,
                    method,
                    url,
                    body,
                    capability_id,
                    source_span,
                })
            }
            EVENT_NETWORK_RESPONSE => {
                let request_id = inner.u32("enriched.network_response.id")?;
                let status_code = inner.u16("enriched.network_response.status")?;
                let latency_ms = inner.u32("enriched.network_response.latency")?;
                let body = match inner.u8("enriched.network_response.body.present")? {
                    0 => None,
                    _ => Some(decode_str(&mut inner, "enriched.network_response.body")?),
                };
                let result_kind = inner.u8("enriched.network_response.result")?;
                let source_span = decode_optional_span(&mut inner)?;
                Ok(EnrichedTelemetryEvent::NetworkResponse {
                    request_id,
                    status_code,
                    latency_ms,
                    body,
                    result_kind,
                    source_span,
                })
            }
            EVENT_PERF_RECORD => {
                let json = decode_str(&mut inner, "enriched.perf_record.json")?;
                Ok(EnrichedTelemetryEvent::PerfRecord { json })
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
            component_name,
        } => EnrichedTelemetryEvent::ViewMutation {
            node_id,
            native_view_id,
            parent_id,
            mutation_kind,
            frame,
            source_span: None,
            component_name,
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
        TelemetryEvent::NetworkRequest {
            request_id,
            method,
            url,
            body,
            capability_id,
        } => EnrichedTelemetryEvent::NetworkRequest {
            request_id,
            method,
            url,
            body,
            capability_id,
            source_span: None,
        },
        TelemetryEvent::NetworkResponse {
            request_id,
            status_code,
            latency_ms,
            body,
            result_kind,
        } => EnrichedTelemetryEvent::NetworkResponse {
            request_id,
            status_code,
            latency_ms,
            body,
            result_kind,
            source_span: None,
        },
        TelemetryEvent::PerfRecord { json } => EnrichedTelemetryEvent::PerfRecord { json },
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
        | EnrichedTelemetryEvent::HandlerInvocation { source_span, .. }
        | EnrichedTelemetryEvent::NetworkRequest { source_span, .. }
        | EnrichedTelemetryEvent::NetworkResponse { source_span, .. } => *source_span = span,
        // `PerfRecord` carries no source span (it is not tied to a `.flux` location),
        // so there is nothing to enrich.
        EnrichedTelemetryEvent::PerfRecord { .. } => {}
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A representative `MetricRecord` JSON (PRD-J / FLUX-059). The DevTools
    /// consumes this verbatim; the exact values are not asserted here — only
    /// that the wire frame preserves the document byte-for-byte.
    const SAMPLE_RECORD_JSON: &str = r#"{"scenario":"ios-imperative-dev","kind":"node-mutation","tree_size":50,"samples":[{"latency":1.2}]}"#;

    #[test]
    fn perf_record_round_trips_on_raw_frame() {
        let event = TelemetryEvent::perf_record(SAMPLE_RECORD_JSON);
        let frame = TelemetryFrame {
            version: PROTOCOL_VERSION,
            event_count: 1,
            events: vec![event],
        };
        let bytes = frame.to_bytes();
        let decoded = TelemetryFrame::from_bytes(&bytes).expect("frame decodes");
        assert_eq!(decoded.events.len(), 1);
        match &decoded.events[0] {
            TelemetryEvent::PerfRecord { json } => assert_eq!(json, SAMPLE_RECORD_JSON),
            other => panic!("expected PerfRecord, got {other:?}"),
        }
    }

    #[test]
    fn perf_record_round_trips_on_enriched_frame() {
        let event = EnrichedTelemetryEvent::PerfRecord {
            json: SAMPLE_RECORD_JSON.to_string(),
        };
        let frame = EnrichedTelemetryFrame {
            version: PROTOCOL_VERSION,
            event_count: 1,
            events: vec![event],
        };
        let bytes = frame.to_bytes();
        let decoded = EnrichedTelemetryFrame::from_bytes(&bytes).expect("frame decodes");
        assert_eq!(decoded.events.len(), 1);
        match &decoded.events[0] {
            EnrichedTelemetryEvent::PerfRecord { json } => assert_eq!(json, SAMPLE_RECORD_JSON),
            other => panic!("expected PerfRecord, got {other:?}"),
        }
    }

    #[test]
    fn perf_record_survives_enrich_pipeline() {
        // The dev server enriches a raw event client-side with no span; the
        // verbatim JSON must be preserved through that path too.
        let raw = TelemetryEvent::perf_record(SAMPLE_RECORD_JSON);
        let enriched = enrich_telemetry(raw);
        match enriched {
            EnrichedTelemetryEvent::PerfRecord { json } => assert_eq!(json, SAMPLE_RECORD_JSON),
            other => panic!("expected PerfRecord after enrich, got {other:?}"),
        }
    }
}
