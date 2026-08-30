//! Structured log viewer (spec §5.3, FLUX-060): renders the retained log buffer
//! as a gpui-component [`DataTable`] with semantic, level-colored rows.
//!
//! Consumes [`DevToolsState::log_snapshot`] so the same bounded, FIFO buffer the
//! wire client feeds (via `ingest_log`) is what the UI shows — no second copy of
//! the log stream lives in the view.

use std::sync::Arc;

use gpui::{
    App, Context, Entity, IntoElement, ParentElement, Render, Window, div, prelude::*, px,
};
use gpui_component::table::{Column, DataTable, TableDelegate, TableState};
use gpui_component::ActiveTheme as _;

use crate::state::DevToolsState;
use crate::time_travel::LogLevel;

/// Table delegate backing the log [`DataTable`]: reads the live log snapshot
/// straight from the shared state on every render.
struct LogsDelegate {
    state: Arc<DevToolsState>,
}

#[allow(refining_impl_trait, elided_lifetimes_in_paths)]
impl TableDelegate for LogsDelegate {
    fn columns_count(&self, _cx: &App) -> usize {
        3
    }

    fn rows_count(&self, _cx: &App) -> usize {
        self.state.log_snapshot().len()
    }

    fn column(&self, col_ix: usize, _cx: &App) -> Column {
        match col_ix {
            0 => Column::new("level", "Level"),
            1 => Column::new("target", "Target"),
            _ => Column::new("message", "Message"),
        }
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        _window: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement + '_ {
        let entry = &self.state.log_snapshot()[row_ix];
        match col_ix {
            0 => {
                let color = level_color(entry.level, cx);
                div()
                    .px(px(8.))
                    .text_color(color)
                    .child(entry.level.tag().to_string())
            }
            1 => div()
                .px(px(8.))
                .text_color(cx.theme().muted_foreground)
                .child(entry.target.clone()),
            _ => div()
                .px(px(8.))
                .child(entry.message.clone()),
        }
    }
}

/// Map a [`LogLevel`] to a semantic theme token color.
fn level_color(level: LogLevel, cx: &App) -> gpui::Hsla {
    let theme = cx.theme();
    match level {
        LogLevel::Error => theme.danger,
        LogLevel::Warn => theme.warning,
        LogLevel::Info => theme.foreground,
        LogLevel::Debug => theme.muted_foreground,
        LogLevel::Trace => theme.muted_foreground,
    }
}

/// Renders the structured log stream as a sortable, color-coded table.
pub struct LogViewerView {
    state: Arc<DevToolsState>,
    /// The backing table state entity (created lazily on first render).
    table: Option<Entity<TableState<LogsDelegate>>>,
}

impl LogViewerView {
    /// Creates the view bound to the shared state.
    pub fn new(state: Arc<DevToolsState>) -> Self {
        Self { state, table: None }
    }
}

impl Render for LogViewerView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        if self.state.log_snapshot().is_empty() {
            return div()
                .px(px(12.))
                .py(px(8.))
                .text_color(cx.theme().muted_foreground)
                .child("No log output yet.");
        }
        if self.table.is_none() {
            let delegate = LogsDelegate {
                state: self.state.clone(),
            };
            self.table = Some(cx.new(|table_cx| TableState::new(delegate, window, table_cx)));
        }
        let table = self.table.clone().unwrap();
        div().size_full().child(DataTable::new(&table).bordered(true))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::DevToolsState;
    use crate::time_travel::LogEntry;

    #[test]
    fn render_pane_lists_ingested_logs_in_order() {
        // The view must surface whatever `ingest_log` retained — proven without a
        // display by checking the snapshot it renders from.
        let state = DevToolsState::new();
        state.ingest_log(LogEntry::new(LogLevel::Info, "flux-devserver", "listening"));
        state.ingest_log(LogEntry::new(LogLevel::Error, "flux-host", "boom"));
        let view = LogViewerView::new(Arc::new(state));
        let entries = view.state.log_snapshot();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].target, "flux-devserver");
        assert_eq!(entries[1].render(), "E flux-host: boom");
    }
}
