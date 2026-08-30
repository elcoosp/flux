//! Structured log viewer (spec §5.3, FLUX-060): renders the retained log buffer.
//!
//! Consumes [`DevToolsState::log_snapshot`] so the same bounded, FIFO buffer the
//! wire client feeds (via `ingest_log`) is what the UI shows — no second copy of
//! the log stream lives in the view.

use std::sync::Arc;

use gpui::{AnyElement, Context, IntoElement, Render, Window};

use crate::row::{empty_row, into_any, kv_row, rows_column};
use crate::state::DevToolsState;

/// Renders the structured log stream as a scrollable list of `L target: message`
/// lines, newest at the bottom, with severity shown left (muted, colored by level).
pub struct LogViewerView {
    state: Arc<DevToolsState>,
}

impl LogViewerView {
    /// Creates the view bound to the shared state.
    pub fn new(state: Arc<DevToolsState>) -> Self {
        Self { state }
    }

    /// The current retained log buffer.
    fn logs(&self) -> Vec<crate::time_travel::LogEntry> {
        self.state.log_snapshot()
    }

    /// Renders the view as a standalone pane.
    pub fn render_pane(&self, _cx: &Context<'_, Self>) -> impl IntoElement {
        let entries = self.logs();
        if entries.is_empty() {
            return into_any(empty_row("No log output yet."));
        }
        let mut rows: Vec<AnyElement> = Vec::with_capacity(entries.len());
        for entry in &entries {
            rows.push(into_any(kv_row(entry.level.tag(), entry.render())));
        }
        into_any(rows_column(rows))
    }
}

impl Render for LogViewerView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        self.render_pane(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::DevToolsState;
    use crate::time_travel::{LogEntry, LogLevel};

    #[test]
    fn render_pane_lists_ingested_logs() {
        // The view must surface whatever `ingest_log` retained — proven without a
        // display by checking the snapshot it renders from.
        let state = DevToolsState::new();
        state.ingest_log(LogEntry::new(LogLevel::Info, "flux-devserver", "listening"));
        state.ingest_log(LogEntry::new(LogLevel::Error, "flux-host", "boom"));
        let view = LogViewerView::new(Arc::new(state));
        let entries = view.logs();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].target, "flux-devserver");
        assert_eq!(entries[1].render(), "E flux-host: boom");
    }
}
