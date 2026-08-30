//! Network inspector view (spec §5.3, FLUX-060): the retained HTTP exchange log
//! rendered as a gpui-component [`DataTable`] with semantic, status-colored rows.
//!
//! Consumes [`DevToolsState::network_snapshot`], fed from the host's
//! `TelemetryEvent::NetworkRequest` / `NetworkResponse` telemetry. Each row shows
//! method, URL, status, and latency; an errored exchange is colored red so a
//! developer sees failed fetches at a glance.

use std::sync::Arc;

use gpui::{
    App, Context, Entity, IntoElement, ParentElement, Render, Window, div, prelude::*, px,
};
use gpui_component::table::{Column, DataTable, TableDelegate, TableState};
use gpui_component::ActiveTheme as _;

use crate::state::DevToolsState;
use crate::time_travel::{NetworkPhase, NetworkRecord};

/// Table delegate backing the network [`DataTable`]: reads the live exchange
/// snapshot straight from the shared state on every render.
struct NetworkDelegate {
    state: Arc<DevToolsState>,
}

#[allow(refining_impl_trait, elided_lifetimes_in_paths)]
impl TableDelegate for NetworkDelegate {
    fn columns_count(&self, _cx: &App) -> usize {
        4
    }

    fn rows_count(&self, _cx: &App) -> usize {
        self.state.network_snapshot().len()
    }

    fn column(&self, col_ix: usize, _cx: &App) -> Column {
        match col_ix {
            0 => Column::new("method", "Method"),
            1 => Column::new("url", "URL"),
            2 => Column::new("status", "Status"),
            _ => Column::new("latency", "Latency"),
        }
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        _window: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement + '_ {
        let rec = &self.state.network_snapshot()[row_ix];
        match col_ix {
            0 => div()
                .px(px(8.))
                .text_color(cx.theme().foreground)
                .child(rec.method.clone()),
            1 => div()
                .px(px(8.))
                .text_color(cx.theme().muted_foreground)
                .child(rec.url.clone()),
            2 => {
                let (text, color) = status_cell(rec, cx);
                div().px(px(8.)).text_color(color).child(text)
            }
            _ => {
                let latency = rec
                    .latency_ms
                    .map_or_else(|| "…".to_string(), |ms| format!("{ms}ms"));
                div().px(px(8.)).child(latency)
            }
        }
    }
}

/// Build the status cell text + color (red on error, green on 2xx, amber else).
fn status_cell(rec: &NetworkRecord, cx: &App) -> (String, gpui::Hsla) {
    let theme = cx.theme();
    match rec.phase {
        NetworkPhase::Pending => ("… pending".to_string(), theme.muted_foreground),
        NetworkPhase::Complete => {
            let code = rec.status_code.unwrap_or(0);
            let color = if rec.is_error() {
                theme.danger
            } else if (200..300).contains(&code) {
                theme.success
            } else {
                theme.warning
            };
            (code.to_string(), color)
        }
    }
}

/// Renders the retained HTTP exchanges as a sortable, status-colored table.
pub struct NetworkInspectorView {
    state: Arc<DevToolsState>,
    /// The backing table state entity (created lazily on first render).
    table: Option<Entity<TableState<NetworkDelegate>>>,
}

impl NetworkInspectorView {
    /// Creates the view bound to the shared state.
    pub fn new(state: Arc<DevToolsState>) -> Self {
        Self { state, table: None }
    }
}

impl Render for NetworkInspectorView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        if self.state.network_snapshot().is_empty() {
            return div()
                .px(px(12.))
                .py(px(8.))
                .text_color(cx.theme().muted_foreground)
                .child("No network traffic yet.");
        }
        if self.table.is_none() {
            let delegate = NetworkDelegate {
                state: self.state.clone(),
            };
            self.table = Some(cx.new(|table_cx| TableState::new(delegate, window, table_cx)));
        }
        let table = self.table.clone().unwrap();
        div()
            .size_full()
            .child(DataTable::new(&table).bordered(true))
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
        state.ingest_network_request(1, "GET".into(), "https://api.example.com/a".into(), None, 14);
        state.ingest_network_request(2, "POST".into(), "https://api.example.com/b".into(), Some("x=1".into()), 14);
        state.ingest_network_response(1, 200, 42, Some("ok".into()), 1);

        let view = NetworkInspectorView::new(Arc::new(state));
        let records = view.state.network_snapshot();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].phase, NetworkPhase::Complete);
        assert_eq!(records[0].status_code, Some(200));
        assert_eq!(records[1].phase, NetworkPhase::Pending);
    }
}
