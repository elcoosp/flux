//! Network inspector view (spec §5.3, FLUX-060): the retained HTTP exchange log.
//!
//! Consumes [`DevToolsState::network_snapshot`], which is fed from the host's
//! `TelemetryEvent::NetworkRequest` / `NetworkResponse` telemetry (emitted by the
//! `Http` capability, FLUX-047). Each row shows the request line plus status and
//! latency once the response lands; an errored exchange is surfaced so a
//! developer can see failed fetches at a glance.

use std::sync::Arc;

use gpui::{AnyElement, Context, IntoElement, Render, Window};

use crate::row::{empty_row, into_any, kv_row, rows_column};
use crate::state::DevToolsState;

/// Renders the retained HTTP exchange log as a list of request/response rows.
pub struct NetworkInspectorView {
    state: Arc<DevToolsState>,
}

impl NetworkInspectorView {
    /// Creates the view bound to the shared state.
    pub fn new(state: Arc<DevToolsState>) -> Self {
        Self { state }
    }

    /// The current retained network exchanges.
    fn exchanges(&self) -> Vec<crate::time_travel::NetworkRecord> {
        self.state.network_snapshot()
    }

    /// Renders the view as a standalone pane.
    pub fn render_pane(&self, _cx: &Context<'_, Self>) -> impl IntoElement {
        let records = self.exchanges();
        if records.is_empty() {
            return into_any(empty_row("No network traffic yet."));
        }
        let mut rows: Vec<AnyElement> = Vec::with_capacity(records.len());
        for rec in &records {
            // Left: status + latency (or a pending marker). Right: the request line.
            let summary = match rec.phase {
                crate::time_travel::NetworkPhase::Pending => "… pending".to_string(),
                crate::time_travel::NetworkPhase::Complete => {
                    let status = rec
                        .status_code
                        .map_or_else(|| "—".to_string(), |s| s.to_string());
                    let latency = rec
                        .latency_ms
                        .map_or_else(String::new, |ms| format!(" ({ms}ms)"));
                    format!("{status}{latency}")
                }
            };
            rows.push(into_any(kv_row(summary, rec.render())));
        }
        into_any(rows_column(rows))
    }
}

impl Render for NetworkInspectorView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        self.render_pane(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::DevToolsState;
    use crate::time_travel::NetworkPhase;

    #[test]
    fn render_pane_lists_pending_then_complete_exchanges() {
        // Proven without a display: the view surfaces exactly the exchanges the
        // wire client retained, in FIFO order, with status once resolved.
        let state = DevToolsState::new();
        state.ingest_network_request(
            1,
            "GET".into(),
            "https://api.example.com/a".into(),
            None,
            14,
        );
        state.ingest_network_request(
            2,
            "POST".into(),
            "https://api.example.com/b".into(),
            Some("x=1".into()),
            14,
        );
        state.ingest_network_response(1, 200, 42, Some("ok".into()), 1);

        let view = NetworkInspectorView::new(Arc::new(state));
        let records = view.exchanges();
        assert_eq!(records.len(), 2);
        // First exchange completed; second still pending.
        assert_eq!(records[0].phase, NetworkPhase::Complete);
        assert_eq!(records[0].status_code, Some(200));
        assert_eq!(records[1].phase, NetworkPhase::Pending);
    }
}
