//! Central DevTools state (spec §5.2), decoupled from gpui so it is unit
//! testable. The gpui views read from this through a `parking_lot::RwLock`.

use parking_lot::RwLock;

use flux_ir_serde::EnrichedTelemetryEvent;

use crate::time_travel::{
    LogBuffer, LogEntry, LogLevel, ReconstructedState, TimelineBuffer, reconstruct_state,
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
    /// Retained telemetry history (ADR-0042).
    pub timeline: RwLock<TimelineBuffer>,
    /// Reconstructed state at the live (newest) timeline index.
    pub live: RwLock<ReconstructedState>,
    /// Whether the host VM is paused.
    pub is_paused: RwLock<bool>,
    /// Retained structured log stream for the log viewer (FLUX-060). Bounded; the
    /// oldest record is evicted once at capacity, mirroring the timeline buffer.
    pub logs: RwLock<LogBuffer>,
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
        }
    }

    /// Ingests one enriched telemetry event: updates the live reconstructed
    /// state and appends to the timeline.
    pub fn handle_telemetry(&self, event: EnrichedTelemetryEvent) {
        {
            let mut live = self.live.write();
            *live = reconstruct_state(&live, std::slice::from_ref(&event));
        }
        self.timeline.write().push(event);
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
}

impl Default for DevToolsState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
