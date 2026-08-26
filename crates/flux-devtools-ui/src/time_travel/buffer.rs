//! Time-travel ring buffer (ADR-0042).
//!
//! The DevTools app keeps a bounded history of [`EnrichedTelemetryEvent`]s so
//! the user can scrub backward and forward through VM execution and signal
//! writes. When the buffer is full the oldest event is evicted, bounding
//! memory regardless of session length.

use std::collections::VecDeque;

use flux_ir_serde::EnrichedTelemetryEvent;

/// Default event capacity for the timeline (ADR-0042).
pub const DEFAULT_CAPACITY: usize = 10_000;

/// A fixed-capacity ring buffer of telemetry events.
///
/// `TimelineBuffer` is the retained history the time-travel scrubber reads
/// from. It is intentionally free of any UI dependency so the scrub/replay
/// algorithm can be unit-tested in isolation (AGENTS.md: verify the core).
#[derive(Clone, Debug)]
pub struct TimelineBuffer {
    events: VecDeque<EnrichedTelemetryEvent>,
    capacity: usize,
}

impl TimelineBuffer {
    /// Creates an empty buffer holding at most `capacity` events.
    ///
    /// # Panics
    ///
    /// Panics if `capacity` is `0`; a zero-capacity timeline cannot retain any
    /// telemetry and is a caller bug. The panic is documented because the
    /// value comes from configuration, not runtime input.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "timeline capacity must be non-zero");
        Self {
            events: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// Appends `event`, evicting the oldest event when at capacity.
    pub fn push(&mut self, event: EnrichedTelemetryEvent) {
        if self.events.len() >= self.capacity {
            self.events.pop_front();
        }
        self.events.push_back(event);
    }

    /// Number of retained events.
    #[must_use]
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Whether the buffer holds no events.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Returns the event at `index` (0 = oldest retained), or `None` if out of
    /// range.
    #[must_use]
    pub fn snapshot_at(&self, index: usize) -> Option<&EnrichedTelemetryEvent> {
        self.events.get(index)
    }

    /// The total capacity (maximum retained events).
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// An iterator over the retained events, oldest first.
    pub fn iter(&self) -> impl Iterator<Item = &EnrichedTelemetryEvent> {
        self.events.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flux_ir_serde::{EnrichedTelemetryEvent, Rect};
    use flux_syntax::{NodeId, Value};

    fn step(offset: u32) -> EnrichedTelemetryEvent {
        EnrichedTelemetryEvent::VmStep {
            bytecode_offset: offset,
            opcode: 0x01,
            registers: Box::new(std::array::from_fn(|_| Value::Null)),
            gas_remaining: 100,
            source_span: None,
        }
    }

    fn view_add(node: u32) -> EnrichedTelemetryEvent {
        EnrichedTelemetryEvent::ViewMutation {
            node_id: NodeId::from(node),
            native_view_id: 0,
            mutation_kind: 0,
            frame: Some(Rect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            }),
            source_span: None,
        }
    }

    #[test]
    fn push_and_index_are_fifo() {
        let mut buf = TimelineBuffer::new(8);
        buf.push(step(0));
        buf.push(view_add(1));
        assert_eq!(buf.len(), 2);
        assert!(matches!(
            buf.snapshot_at(0),
            Some(EnrichedTelemetryEvent::VmStep { .. })
        ));
        assert!(matches!(
            buf.snapshot_at(1),
            Some(EnrichedTelemetryEvent::ViewMutation { .. })
        ));
        assert!(buf.snapshot_at(2).is_none());
    }

    #[test]
    fn capacity_evicts_oldest() {
        let mut buf = TimelineBuffer::new(3);
        for i in 0..5u32 {
            buf.push(step(i));
        }
        assert_eq!(buf.len(), 3);
        // Oldest two (offsets 0, 1) were dropped; retained are 2, 3, 4.
        assert!(matches!(
            buf.snapshot_at(0),
            Some(EnrichedTelemetryEvent::VmStep {
                bytecode_offset: 2,
                ..
            })
        ));
        assert!(matches!(
            buf.snapshot_at(2),
            Some(EnrichedTelemetryEvent::VmStep {
                bytecode_offset: 4,
                ..
            })
        ));
    }

    #[test]
    fn empty_buffer_reports_empty() {
        let buf = TimelineBuffer::new(4);
        assert!(buf.is_empty());
        assert_eq!(buf.len(), 0);
        assert!(buf.snapshot_at(0).is_none());
    }

    #[test]
    fn iter_is_oldest_first() {
        let mut buf = TimelineBuffer::new(8);
        buf.push(step(1));
        buf.push(step(2));
        let offsets: Vec<u32> = buf
            .iter()
            .map(|e| match e {
                EnrichedTelemetryEvent::VmStep {
                    bytecode_offset, ..
                } => *bytecode_offset,
                _ => u32::MAX,
            })
            .collect();
        assert_eq!(offsets, vec![1, 2]);
    }

    #[test]
    #[should_panic(expected = "timeline capacity must be non-zero")]
    fn zero_capacity_panics() {
        let _ = TimelineBuffer::new(0);
    }
}
