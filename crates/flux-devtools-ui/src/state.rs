//! Central DevTools state (spec §5.2), decoupled from gpui so it is unit
//! testable. The gpui views read from this through a `parking_lot::RwLock`.

use parking_lot::RwLock;
use std::collections::BTreeMap;

use flux_ir_serde::EnrichedTelemetryEvent;
use flux_perf_harness::MetricRecord;
use flux_syntax::SignalId;

use crate::time_travel::{
    LogBuffer, LogEntry, NetworkLog, NetworkRecord, ReconstructedState, TimelineBuffer, ViewFrame,
    reconstruct_state,
};

/// Snapshot of the VM register/instruction view.
#[derive(Clone, Debug, PartialEq)]
pub struct VmState {
    /// VM instruction pointer (bytecode offset).
    pub bytecode_offset: Option<u32>,
    /// Opcode at the instruction pointer.
    pub opcode: Option<u8>,
    /// Register bank r0–r15.
    pub registers: Box<[flux_syntax::Value; 16]>,
    /// Remaining gas.
    pub gas_remaining: Option<u32>,
    /// `.flux` source span of the current instruction, if resolvable.
    pub source_span: Option<flux_syntax::Span>,
}

/// Identity of the host device currently streaming telemetry, learned from the
/// dev server's `HostAnnounce` frame (which the server derives from the host's
/// `Hello` handshake). Lets the DevTools UI show *which* device is being
/// inspected (e.g. an iOS Simulator vs an Android phone).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostInfo {
    /// Host platform, e.g. `"ios"` or `"android"`.
    pub platform: String,
    /// Device model string (e.g. `iPhone17,1` / `UIDevice.model`).
    pub device: String,
    /// Capabilities the host advertised at handshake (`(name, version, features)`).
    pub capabilities: Vec<(String, u32, Vec<String>)>,
}

impl HostInfo {
    /// A short, human-readable label for the host, e.g. `iOS · iPhone17,1`.
    #[must_use]
    pub fn label(&self) -> String {
        let lowered = self.platform.to_ascii_lowercase();
        let platform = match lowered.as_str() {
            "ios" => "iOS",
            "android" => "Android",
            other => other,
        };
        format!("{platform} · {}", self.device)
    }
}

/// A stable key identifying one connected host within the DevTools session map.
///
/// Derived from the `HostAnnounce` identity (platform + device): a given
/// physical device announces the same key on every reconnect, so its session
/// survives reconnects. When the wire protocol gains a stable per-host id
/// (ADR-0039 extension), that id should take precedence here.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct HostKey {
    /// Lowercased platform, e.g. `ios` / `android`.
    pub platform: String,
    /// Device model string.
    pub device: String,
}

impl HostKey {
    /// Builds a key from a [`HostInfo`], normalizing the platform to lowercase.
    #[must_use]
    pub fn from_host(host: &HostInfo) -> Self {
        Self {
            platform: host.platform.to_ascii_lowercase(),
            device: host.device.clone(),
        }
    }

    /// The synthetic key used before any host has announced (single-connection
    /// legacy mode). Keeps the public `timeline_len`/`vm_state` API stable when
    /// no `HostAnnounce` has arrived yet.
    #[must_use]
    pub fn anonymous() -> Self {
        Self {
            platform: String::new(),
            device: String::new(),
        }
    }
}

/// One host's reconstructed DevTools session: its own live state plus its own
/// retained timeline (so two simultaneous hosts can be scrubbed independently).
#[derive(Clone, Debug)]
pub struct DeviceSession {
    /// The host identity this session belongs to.
    pub host: HostInfo,
    /// Reconstructed state at the live (newest) timeline index.
    pub live: ReconstructedState,
    /// Retained telemetry history for this host (ADR-0042).
    pub timeline: TimelineBuffer,
}

impl DeviceSession {
    /// Creates an empty session for `host`.
    #[must_use]
    pub fn new(host: HostInfo) -> Self {
        Self {
            host,
            live: ReconstructedState::base(),
            timeline: TimelineBuffer::new(crate::time_travel::DEFAULT_CAPACITY),
        }
    }

    /// Ingests one enriched telemetry event into this session.
    pub fn handle_telemetry(&mut self, event: &EnrichedTelemetryEvent) {
        self.live = reconstruct_state(&self.live, std::slice::from_ref(event));
        self.timeline.push(event.clone());
    }

    /// Reconstructs the full state at timeline `index` by replaying from base.
    #[must_use]
    pub fn state_at(&self, index: usize) -> Option<ReconstructedState> {
        let mut state = ReconstructedState::base();
        for i in 0..=index {
            let event = self.timeline.snapshot_at(i)?;
            state = reconstruct_state(&state, std::slice::from_ref(event));
        }
        Some(state)
    }

    /// Number of retained timeline events for this host.
    #[must_use]
    pub fn timeline_len(&self) -> usize {
        self.timeline.len()
    }

    /// A view of this session's live VM state (cheap clone for rendering).
    #[must_use]
    pub fn vm_state(&self) -> VmState {
        VmState {
            bytecode_offset: self.live.bytecode_offset,
            opcode: self.live.opcode,
            registers: self.live.registers.clone(),
            gas_remaining: self.live.gas_remaining,
            source_span: None,
        }
    }
}

/// The DevTools central state: the live timeline plus the reconstructed view.
///
/// The gpui app layer (`run_app`) owns this behind a shared lock; views read it on
/// every frame and the wire client writes into it as telemetry arrives. All
/// mutation goes through [`DevToolsState::handle_telemetry`], which also pushes
/// into the [`TimelineBuffer`] for time-travel.
// `parking_lot::RwLock` is not `Debug`, so the struct cannot derive it; this is
// intentional (the state is shared via `Arc`/entities, not printed).
#[allow(missing_debug_implementations)]
pub struct DevToolsState {
    /// Retained telemetry history (ADR-0042). Kept as the active session's mirror
    /// so the legacy single-host [`timeline_len`](Self::timeline_len) /
    /// [`vm_state`](Self::vm_state) / [`state_at`](Self::state_at) API stays
    /// stable; per-host history lives in [`sessions`](Self::sessions).
    pub timeline: RwLock<TimelineBuffer>,
    /// Reconstructed state at the live (newest) timeline index.
    pub live: RwLock<ReconstructedState>,
    /// Whether the host VM is paused.
    pub is_paused: RwLock<bool>,
    /// Retained structured log stream for the log viewer (FLUX-060). Bounded; the
    /// oldest record is evicted once at capacity, mirroring the timeline buffer.
    pub logs: RwLock<LogBuffer>,
    /// Retained HTTP exchange log for the network inspector (FLUX-060). Bounded;
    /// the oldest exchange is evicted once at capacity. Fed from the host's
    /// `TelemetryEvent::NetworkRequest` / `NetworkResponse` telemetry.
    pub net: RwLock<NetworkLog>,
    /// The host currently streaming telemetry, if any. `None` until the first
    /// `HostAnnounce` arrives (the dev server sends one per host connection).
    pub host: RwLock<Option<HostInfo>>,
    /// Per-host reconstructed sessions, keyed by [`HostKey`] (FLUX-061). Lets
    /// DevTools connect to more than one host at once and scrub each independently.
    pub sessions: RwLock<BTreeMap<HostKey, DeviceSession>>,
    /// The host whose telemetry the [`handle_telemetry`](Self::handle_telemetry)
    /// calls currently route to (the most recent `HostAnnounce` on this
    /// connection). `None` until a host announces, then the anonymous key.
    pub active: RwLock<Option<HostKey>>,
    /// Render-perf harness [`MetricRecord`]s (PRD-J / FLUX-059) ingested from
    /// `PerfRecord` telemetry events, in arrival order. This is the backing
    /// store the timeline/flamegraph view renders. Bounded like the timeline.
    pub perf_records: RwLock<Vec<MetricRecord>>,
    /// The timeline index the user is currently scrubbing to via the time-travel
    /// slider (`None` = live edge). Shared so other panes can reflect the
    /// scrubbed state (FLUX-062 time-travel UX).
    pub scrub_index: RwLock<Option<usize>>,
    /// The signal node currently selected in the signal graph (None = none).
    /// Selecting reveals the effect ids that re-run when the signal changes.
    pub selected_signal: RwLock<Option<SignalId>>,
}

impl DevToolsState {
    /// Creates an empty state with the default timeline capacity.
    #[must_use]
    pub fn new() -> Self {
        Self {
            timeline: RwLock::new(TimelineBuffer::new(crate::time_travel::DEFAULT_CAPACITY)),
            live: RwLock::new(ReconstructedState::base()),
            is_paused: RwLock::new(false),
            logs: RwLock::new(LogBuffer::new(512)),
            net: RwLock::new(NetworkLog::new(512)),
            host: RwLock::new(None),
            sessions: RwLock::new(BTreeMap::new()),
            active: RwLock::new(None),
            perf_records: RwLock::new(Vec::new()),
            scrub_index: RwLock::new(None),
            selected_signal: RwLock::new(None),
        }
    }

    /// The timeline index the time-travel slider is currently scrubbed to, or
    /// `None` when following the live edge.
    #[must_use]
    pub fn scrub_index(&self) -> Option<usize> {
        *self.scrub_index.read()
    }

    /// The signal node currently selected in the signal graph (`None` = none).
    #[must_use]
    pub fn selected_signal(&self) -> Option<SignalId> {
        *self.selected_signal.read()
    }

    /// Sets the time-travel scrub index (`None` returns to the live edge) and
    /// notifies so every pane repaints to the scrubbed state.
    pub fn set_scrub_index(&self, index: Option<usize>) {
        *self.scrub_index.write() = index;
    }

    /// Toggles the selected signal node in the signal graph. Clicking a signal
    /// selects it (revealing its reader effects); clicking it again deselects.
    /// Selecting a different signal switches the selection.
    pub fn toggle_signal_selection(&self, id: SignalId) {
        let mut selected = self.selected_signal.write();
        if *selected == Some(id) {
            *selected = None;
        } else {
            *selected = Some(id);
        }
    }

    /// Records the identity of the host now streaming telemetry.
    ///
    /// Inserts/updates this host's [`DeviceSession`] in the multi-device map
    /// (FLUX-061) and marks it the active session that subsequent
    /// [`handle_telemetry`](Self::handle_telemetry) calls route to.
    pub fn set_host(&self, host: HostInfo) {
        let key = HostKey::from_host(&host);
        {
            let mut sessions = self.sessions.write();
            sessions
                .entry(key.clone())
                .or_insert_with(|| DeviceSession::new(host.clone()));
        }
        *self.host.write() = Some(host);
        *self.active.write() = Some(key);
    }

    /// The key of the host that incoming telemetry currently routes to, or the
    /// [`anonymous`](HostKey::anonymous) key when no host has announced.
    #[must_use]
    pub fn active_host_key(&self) -> HostKey {
        self.active
            .read()
            .clone()
            .unwrap_or_else(HostKey::anonymous)
    }

    /// The keys of all known host sessions (FLUX-061 multi-device).
    #[must_use]
    pub fn session_keys(&self) -> Vec<HostKey> {
        self.sessions.read().keys().cloned().collect()
    }

    /// Number of distinct host sessions currently held (FLUX-061).
    #[must_use]
    pub fn session_count(&self) -> usize {
        self.sessions.read().len()
    }

    /// A snapshot of one host session's reconstructed state (FLUX-061), if the
    /// key is known.
    #[must_use]
    pub fn session_state(&self, key: &HostKey) -> Option<DeviceSession> {
        self.sessions.read().get(key).cloned()
    }

    /// Ingests one enriched telemetry event into the active host session.
    ///
    /// The event is also mirrored into the legacy single-host [`timeline`]/[`live`]
    /// fields so the original public API keeps working; true per-event
    /// attribution across *simultaneously* streaming hosts requires the wire
    /// protocol to tag each event with its source host (ADR-0039 extension) —
    /// until then, events route to whatever host announced most recently.
    pub fn handle_telemetry(&self, event: EnrichedTelemetryEvent) {
        let key = self.active_host_key();
        {
            let mut sessions = self.sessions.write();
            let session = sessions.entry(key.clone()).or_insert_with(|| {
                DeviceSession::new(HostInfo {
                    platform: key.platform.clone(),
                    device: key.device.clone(),
                    capabilities: Vec::new(),
                })
            });
            session.handle_telemetry(&event);
        }
        // Mirror into the legacy single-host fields for backward-compatible reads.
        {
            let mut live = self.live.write();
            *live = reconstruct_state(&live, std::slice::from_ref(&event));
        }
        self.timeline.write().push(event.clone());

        // Feed the network inspector (FLUX-060) from the HTTP capability telemetry.
        match &event {
            EnrichedTelemetryEvent::NetworkRequest {
                request_id,
                method,
                url,
                body,
                capability_id,
                ..
            } => self.ingest_network_request(
                *request_id,
                method.clone(),
                url.clone(),
                body.clone(),
                *capability_id,
            ),
            EnrichedTelemetryEvent::NetworkResponse {
                request_id,
                status_code,
                latency_ms,
                body,
                result_kind,
                ..
            } => self.ingest_network_response(
                *request_id,
                *status_code,
                *latency_ms,
                body.clone(),
                *result_kind,
            ),
            EnrichedTelemetryEvent::PerfRecord { json } => {
                self.ingest_perf_record(json);
            }
            _ => {}
        }
    }

    /// Ingests a render-perf harness [`MetricRecord`] (PRD-J / FLUX-059) emitted
    /// as a `PerfRecord` telemetry event. Records whose JSON fails to parse are
    /// dropped with a warning rather than crashing the client (AGENTS.md: never
    /// panic in prod). Returns `true` if the record was accepted.
    pub fn ingest_perf_record(&self, json: &str) -> bool {
        match MetricRecord::from_json(json) {
            Ok(record) => {
                let mut records = self.perf_records.write();
                // Bound the retained records like the timeline buffer so a
                // long-running session cannot grow without limit.
                const MAX_PERF_RECORDS: usize = 1024;
                if records.len() >= MAX_PERF_RECORDS {
                    records.remove(0);
                }
                records.push(record);
                true
            }
            Err(e) => {
                tracing::warn!(%e, "dropping unparseable PerfRecord JSON");
                false
            }
        }
    }

    /// A snapshot of the retained render-perf [`MetricRecord`]s in arrival order
    /// (FLUX-059). Empty until a `PerfRecord` telemetry event arrives.
    #[must_use]
    pub fn perf_records(&self) -> Vec<MetricRecord> {
        self.perf_records.read().clone()
    }

    /// Number of retained render-perf [`MetricRecord`]s (FLUX-059).
    #[must_use]
    pub fn perf_record_count(&self) -> usize {
        self.perf_records.read().len()
    }

    /// Appends a reconstructed view frame to the live component tree directly
    /// (used by tests and any caller that already holds a [`ViewFrame`]).
    pub fn push_view_frame(&self, frame: ViewFrame) {
        self.live.write().view_frames.push(frame);
    }

    /// The current host identity, if known.
    #[must_use]
    pub fn host_info(&self) -> Option<HostInfo> {
        self.host.read().clone()
    }

    /// Reconstructs the full state at timeline `index` by replaying from the
    /// base snapshot to that point.
    ///
    /// Returns `None` if `index` is past the retained history.
    #[must_use]
    pub fn state_at(&self, index: usize) -> Option<ReconstructedState> {
        let timeline = self.timeline.read();
        let base = ReconstructedState::base();
        let mut state = base;
        for i in 0..=index {
            let event = timeline.snapshot_at(i)?;
            state = reconstruct_state(&state, std::slice::from_ref(event));
        }
        Some(state)
    }

    /// Number of retained timeline events.
    #[must_use]
    pub fn timeline_len(&self) -> usize {
        self.timeline.read().len()
    }

    /// A view of the live VM state (cheap clone for rendering).
    #[must_use]
    pub fn vm_state(&self) -> VmState {
        let live = self.live.read();
        VmState {
            bytecode_offset: live.bytecode_offset,
            opcode: live.opcode,
            registers: live.registers.clone(),
            gas_remaining: live.gas_remaining,
            source_span: None,
        }
    }

    /// Appends a structured log record to the retained log buffer (FLUX-060).
    ///
    /// The dev server already emits `tracing` output (AGENTS.md §3.12); a
    /// subscriber forwards records here. This is the single ingest point so the
    /// log viewer reads a consistent, bounded buffer.
    pub fn ingest_log(&self, entry: LogEntry) {
        self.logs.write().push(entry);
    }

    /// A snapshot of the retained log records (oldest first).
    #[must_use]
    pub fn log_snapshot(&self) -> Vec<LogEntry> {
        self.logs.read().snapshot()
    }

    /// Records an outbound HTTP request into the network inspector log (FLUX-060).
    ///
    /// The host emits `TelemetryEvent::NetworkRequest` when the `Http` capability
    /// (FLUX-047) issues a fetch; this starts the exchange `Pending`. This is the
    /// single ingest point so the network inspector reads a consistent, bounded
    /// buffer.
    pub fn ingest_network_request(
        &self,
        request_id: u32,
        method: String,
        url: String,
        body: Option<String>,
        capability_id: u32,
    ) {
        self.net.write().push_request(NetworkRecord::from_request(
            request_id,
            method,
            url,
            body,
            capability_id,
        ));
    }

    /// Records a resolved HTTP response into the network inspector log (FLUX-060),
    /// pairing it with the pending request by `request_id`.
    ///
    /// A response whose request was never retained (evicted or never seen) is
    /// dropped silently — the inspector only shows complete exchanges it can
    /// attribute to a request.
    pub fn ingest_network_response(
        &self,
        request_id: u32,
        status_code: u16,
        latency_ms: u32,
        body: Option<String>,
        result_kind: u8,
    ) {
        self.net
            .write()
            .push_response(request_id, status_code, latency_ms, body, result_kind);
    }

    /// A snapshot of the retained HTTP exchanges (oldest first).
    #[must_use]
    pub fn network_snapshot(&self) -> Vec<NetworkRecord> {
        self.net.read().snapshot()
    }
}

impl Default for DevToolsState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time_travel::LogLevel;
    use flux_ir_serde::EnrichedTelemetryEvent;
    use flux_syntax::Value;

    fn step(offset: u32) -> EnrichedTelemetryEvent {
        EnrichedTelemetryEvent::VmStep {
            bytecode_offset: offset,
            opcode: 0x03,
            registers: Box::new(std::array::from_fn(|_| Value::Null)),
            gas_remaining: 10,
            source_span: None,
        }
    }

    #[test]
    fn toggle_signal_selection_switches_and_deselects() {
        let state = DevToolsState::new();
        // Selecting a signal reveals it as the active selection.
        state.toggle_signal_selection(7);
        assert_eq!(state.selected_signal(), Some(7));
        // Selecting a different signal switches the selection.
        state.toggle_signal_selection(12);
        assert_eq!(state.selected_signal(), Some(12));
        // Selecting the same signal again deselects it.
        state.toggle_signal_selection(12);
        assert_eq!(state.selected_signal(), None);
    }

    #[test]
    fn handle_telemetry_updates_live_and_timeline() {
        let state = DevToolsState::new();
        state.handle_telemetry(step(4));
        state.handle_telemetry(step(8));
        assert_eq!(state.timeline_len(), 2);
        assert_eq!(state.vm_state().bytecode_offset, Some(8));
    }

    #[test]
    fn state_at_replays_prefix() {
        let state = DevToolsState::new();
        state.handle_telemetry(step(4));
        state.handle_telemetry(step(8));
        // Index 0 must reconstruct to offset 4, not the live offset 8.
        let at_zero = state.state_at(0).expect("index 0 present");
        assert_eq!(at_zero.bytecode_offset, Some(4));
        let at_one = state.state_at(1).expect("index 1 present");
        assert_eq!(at_one.bytecode_offset, Some(8));
        assert!(state.state_at(2).is_none());
    }

    #[test]
    fn ingest_log_appends_to_retained_buffer() {
        let state = DevToolsState::new();
        state.ingest_log(LogEntry::new(
            LogLevel::Info,
            "flux-devserver",
            "listening on :7331",
        ));
        state.ingest_log(LogEntry::new(LogLevel::Error, "flux-host", "boom"));
        let logs = state.log_snapshot();
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0].target, "flux-devserver");
        assert_eq!(logs[1].level, LogLevel::Error);
        assert_eq!(logs[1].render(), "E flux-host: boom");
    }

    #[test]
    fn ingest_network_pairs_request_and_response() {
        // FLUX-060 network inspector: a request starts the exchange pending, a
        // response completes it, and a response with no retained request is
        // dropped (no fabricated row).
        let state = DevToolsState::new();
        state.ingest_network_request(
            1,
            "GET".into(),
            "https://api.example.com/users".into(),
            None,
            14,
        );
        assert_eq!(state.network_snapshot().len(), 1);
        assert_eq!(
            state.network_snapshot()[0].phase,
            crate::time_travel::NetworkPhase::Pending
        );

        state.ingest_network_response(1, 200, 42, Some("{\"ok\":true}".into()), 1);
        let snap = state.network_snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].phase, crate::time_travel::NetworkPhase::Complete);
        assert_eq!(snap[0].status_code, Some(200));
        assert_eq!(snap[0].latency_ms, Some(42));
        assert!(!snap[0].is_error());

        // A response whose request was never retained must not create a record.
        state.ingest_network_response(99, 200, 1, None, 1);
        assert_eq!(state.network_snapshot().len(), 1);
    }

    #[test]
    fn handle_telemetry_feeds_network_log() {
        // The HTTP-capability telemetry must reach the network log through the
        // single `handle_telemetry` ingest path (not a separate call site).
        let state = DevToolsState::new();
        state.handle_telemetry(EnrichedTelemetryEvent::NetworkRequest {
            request_id: 5,
            method: "POST".into(),
            url: "https://api.example.com/login".into(),
            body: Some("u=1".into()),
            capability_id: 14,
            source_span: None,
        });
        state.handle_telemetry(EnrichedTelemetryEvent::NetworkResponse {
            request_id: 5,
            status_code: 201,
            latency_ms: 17,
            body: None,
            result_kind: 1,
            source_span: None,
        });
        let snap = state.network_snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].method, "POST");
        assert_eq!(snap[0].status_code, Some(201));
    }

    #[test]
    fn ingest_perf_record_stores_parsed_metric_record() {
        // A `PerfRecord` telemetry event carries the verbatim `MetricRecord`
        // JSON; ingestion must parse it and expose it for the flamegraph.
        use flux_perf_harness::{LatencyMs, MetricKind, MetricRecord, MetricSample, Scenario};
        let rec = MetricRecord::new(
            Scenario::AndroidDeclarativeDev,
            MetricKind::NodeMutation,
            50,
            vec![MetricSample::latency(LatencyMs::from_raw(1.5))],
        );
        let json = rec.to_json().expect("serialize record");

        let state = DevToolsState::new();
        assert!(state.ingest_perf_record(&json));
        assert_eq!(state.perf_record_count(), 1);
        let stored = &state.perf_records()[0];
        assert_eq!(stored.scenario, Scenario::AndroidDeclarativeDev);
        assert_eq!(stored.kind, MetricKind::NodeMutation);
        assert_eq!(stored.p95().map(|l| l.as_f64()), Some(1.5));
    }

    #[test]
    fn ingest_perf_record_rejects_malformed_json() {
        let state = DevToolsState::new();
        assert!(!state.ingest_perf_record("not valid json"));
        assert_eq!(state.perf_record_count(), 0);
    }

    #[test]
    fn handle_telemetry_perf_record_event_feeds_flamegraph() {
        // End-to-end through the public handle_telemetry path (the wire client
        // routes every event here). A PerfRecord event must populate perf_records.
        use flux_perf_harness::{LatencyMs, MetricKind, MetricRecord, MetricSample, Scenario};
        let rec = MetricRecord::new(
            Scenario::LoopbackE2e,
            MetricKind::SaveToPhoton,
            50,
            vec![MetricSample::latency(LatencyMs::from_raw(42.0))],
        );
        let json = rec.to_json().expect("serialize record");
        let event = EnrichedTelemetryEvent::PerfRecord { json };
        let state = DevToolsState::new();
        state.handle_telemetry(event);
        assert_eq!(state.perf_record_count(), 1);
    }

    #[test]
    fn two_hosts_make_two_sessions() {
        // No source discriminator exists on the wire yet (ADR-0039 extension),
        // so events route to the most-recently-announced host. Announcing A,
        // feeding A-events, then announcing B and feeding B-events must yield
        // two independent sessions with their own timelines.
        let state = DevToolsState::new();
        state.set_host(HostInfo {
            platform: "ios".into(),
            device: "iPhone17,1".into(),
            capabilities: Vec::new(),
        });
        state.handle_telemetry(step(4));
        state.handle_telemetry(step(8));

        state.set_host(HostInfo {
            platform: "android".into(),
            device: "Pixel 8".into(),
            capabilities: Vec::new(),
        });
        state.handle_telemetry(step(20));

        assert_eq!(state.session_count(), 2);
        let keys = state.session_keys();
        assert!(keys.contains(&HostKey::from_host(&HostInfo {
            platform: "ios".into(),
            device: "iPhone17,1".into(),
            capabilities: Vec::new(),
        })));
        assert!(keys.contains(&HostKey::from_host(&HostInfo {
            platform: "android".into(),
            device: "Pixel 8".into(),
            capabilities: Vec::new(),
        })));

        let ios_key = HostKey::from_host(&HostInfo {
            platform: "ios".into(),
            device: "iPhone17,1".into(),
            capabilities: Vec::new(),
        });
        let ios = state.session_state(&ios_key).expect("ios session present");
        assert_eq!(ios.timeline_len(), 2);
        assert_eq!(ios.vm_state().bytecode_offset, Some(8));

        let android_key = HostKey::from_host(&HostInfo {
            platform: "android".into(),
            device: "Pixel 8".into(),
            capabilities: Vec::new(),
        });
        let android = state
            .session_state(&android_key)
            .expect("android session present");
        assert_eq!(android.timeline_len(), 1);
        assert_eq!(android.vm_state().bytecode_offset, Some(20));
    }
}
