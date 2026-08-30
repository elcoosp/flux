//! Structured log buffer for the DevTools log viewer (FLUX-060).
//!
//! A bounded, FIFO buffer of [`LogEntry`] records. It is decoupled from gpui and
//! from any global `tracing` subscriber so it is unit-testable without a socket or
//! a running runtime. The dev server already emits `tracing` output (AGENTS.md
//! §3.12); a host/agent installs a subscriber that forwards records into
//! [`crate::state::DevToolsState::ingest_log`] — this module owns the storage and
//! the pure rendering shape.

/// Severity of a log record (mirrors `tracing`'s `Level` without depending on it).
///
/// Ordered by severity (low→high) so a level filter like `Info` keeps every
/// record at `Info` or above and drops `Debug`/`Trace` noise.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    /// Trace-level diagnostics.
    Trace,
    /// Debug-level diagnostics.
    Debug,
    /// Informational.
    Info,
    /// Warnings (recoverable).
    Warn,
    /// Errors (a red banner in the host; see Appendix E §E.6).
    Error,
}

impl LogLevel {
    /// The stable, single-character tag used by the log viewer.
    #[must_use]
    pub fn tag(self) -> &'static str {
        match self {
            Self::Trace => "T",
            Self::Debug => "D",
            Self::Info => "I",
            Self::Warn => "W",
            Self::Error => "E",
        }
    }
}

/// One structured log record.
#[derive(Clone, Debug, PartialEq)]
pub struct LogEntry {
    /// Severity.
    pub level: LogLevel,
    /// The `tracing` target (crate/module), or `"<unknown>"`.
    pub target: String,
    /// The formatted message.
    pub message: String,
}

impl LogEntry {
    /// Builds a record, normalizing an empty target to `"<unknown>"`.
    #[must_use]
    pub fn new(level: LogLevel, target: impl Into<String>, message: impl Into<String>) -> Self {
        let target = target.into();
        Self {
            level,
            target: if target.is_empty() {
                "<unknown>".to_owned()
            } else {
                target
            },
            message: message.into(),
        }
    }

    /// The one-line rendering the log viewer shows: `L target: message`.
    #[must_use]
    pub fn render(&self) -> String {
        format!("{} {}: {}", self.level.tag(), self.target, self.message)
    }
}

/// A bounded FIFO of [`LogEntry`] records (newest at the end).
///
/// Capacity is fixed; when full, the oldest record is evicted (oldest-first),
/// matching the timeline buffer's eviction contract so scrubbing stays cheap.
#[derive(Clone, Debug, PartialEq)]
pub struct LogBuffer {
    entries: Vec<LogEntry>,
    capacity: usize,
}

impl LogBuffer {
    /// Creates an empty buffer holding at most `capacity` records.
    ///
    /// A zero capacity is rejected (a log viewer with no room is a logic error);
    /// callers must supply a positive capacity.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "LogBuffer capacity must be positive");
        Self {
            entries: Vec::with_capacity(capacity),
            capacity,
        }
    }

    /// Appends a record, evicting the oldest when at capacity.
    pub fn push(&mut self, entry: LogEntry) {
        if self.entries.len() == self.capacity {
            self.entries.remove(0);
        }
        self.entries.push(entry);
    }

    /// Number of retained records.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the buffer currently holds no records.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// A snapshot of the retained records in insertion order (oldest first).
    #[must_use]
    pub fn snapshot(&self) -> Vec<LogEntry> {
        self.entries.clone()
    }

    /// Removes every retained record, leaving the buffer empty.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_then_snapshot_is_fifo() {
        let mut buf = LogBuffer::new(8);
        buf.push(LogEntry::new(
            LogLevel::Info,
            "flux-devserver",
            "listening on :7331",
        ));
        buf.push(LogEntry::new(
            LogLevel::Error,
            "flux-host",
            "handshake rejected",
        ));
        assert_eq!(buf.len(), 2);
        let snap = buf.snapshot();
        assert_eq!(snap[0].target, "flux-devserver");
        assert_eq!(snap[1].target, "flux-host");
    }

    #[test]
    fn capacity_evicts_oldest() {
        let mut buf = LogBuffer::new(2);
        buf.push(LogEntry::new(LogLevel::Info, "a", "1"));
        buf.push(LogEntry::new(LogLevel::Info, "b", "2"));
        buf.push(LogEntry::new(LogLevel::Info, "c", "3"));
        assert_eq!(buf.len(), 2);
        let snap = buf.snapshot();
        // "a" was evicted; "b" and "c" remain, in order.
        assert_eq!(snap[0].target, "b");
        assert_eq!(snap[1].target, "c");
    }

    #[test]
    fn empty_target_normalized() {
        let e = LogEntry::new(LogLevel::Warn, "", "no module");
        assert_eq!(e.target, "<unknown>");
        assert_eq!(e.render(), "W <unknown>: no module");
    }

    #[test]
    fn level_tag_is_stable() {
        assert_eq!(LogLevel::Error.tag(), "E");
        assert_eq!(LogLevel::Trace.tag(), "T");
    }
}
