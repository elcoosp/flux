//! Structured log viewer (spec §5.3, FLUX-060): renders the retained log buffer
//! as a gpui-component [`DataTable`] with semantic, level-colored rows.
//!
//! Consumes [`DevToolsState::log_snapshot`] so the same bounded, FIFO buffer the
//! wire client feeds (via `ingest_log`) is what the UI shows — no second copy of
//! the log stream lives in the view. A `Popover` level filter and a "Clear logs"
//! action live in the header (roadmap §5 quick wins).

use std::sync::Arc;

use gpui::{App, Context, Entity, IntoElement, ParentElement, Render, Window, div, prelude::*, px};
use gpui_component::button::Button;
use gpui_component::table::{Column, DataTable, TableDelegate, TableState};
use gpui_component::{ActiveTheme as _, popover::Popover};

use crate::state::DevToolsState;
use crate::time_travel::LogLevel;

/// Table delegate backing the log [`DataTable`]: reads the live, filtered log
/// snapshot straight from the shared state on every render.
struct LogsDelegate {
    state: Arc<DevToolsState>,
    /// Snapshot of the filtered log entries, rebuilt once per render in
    /// `rows_count` and reused by every `render_td` call. The previous
    /// version re-cloned the whole `Vec<LogEntry>` on every cell (3-4 columns
    /// × N rows = thousands of clones per frame, on the UI thread) and re-read
    /// the `RwLock` per cell — so cell content could even observe a different
    /// buffer than `rows_count` if the wire thread pushed mid-frame. This
    /// caches one coherent snapshot for the whole render pass.
    ///
    /// `TableDelegate::rows_count` takes `&self`, but we need to mutate the
    /// cache there; `RefCell` gives interior mutability (safe because gpui
    /// renders on a single thread).
    snapshot: std::cell::RefCell<Vec<crate::time_travel::LogEntry>>,
}

#[allow(refining_impl_trait, elided_lifetimes_in_paths)]
impl TableDelegate for LogsDelegate {
    fn columns_count(&self, _cx: &App) -> usize {
        3
    }

    fn rows_count(&self, _cx: &App) -> usize {
        // Refresh the snapshot exactly once per frame; every subsequent
        // `render_td` reads the cached entries — no lock, no per-cell clone.
        let mut snapshot = self.snapshot.borrow_mut();
        *snapshot = self.state.filtered_log_snapshot();
        snapshot.len()
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
        // Extract the values we need as owned data, then drop the borrow
        // before returning the element. The `impl IntoElement + '_` return
        // type ties the element to `&mut self`, not to the borrow guard, so
        // we must release the RefCell borrow before the return expression
        // is evaluated.
        let (level, tag, target, message) = match self.snapshot.borrow().get(row_ix) {
            Some(e) => (
                e.level,
                e.level.tag().to_string(),
                e.target.clone(),
                e.message.clone(),
            ),
            None => return div().px(px(8.)).into_any_element(),
        };
        match col_ix {
            0 => {
                let color = level_color(level, cx);
                div()
                    .px(px(8.))
                    .text_color(color)
                    .child(tag)
                    .into_any_element()
            }
            1 => div()
                .px(px(8.))
                .text_color(cx.theme().muted_foreground)
                .child(target)
                .into_any_element(),
            _ => div().px(px(8.)).child(message).into_any_element(),
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
        let this = cx.entity();
        let state = self.state.clone();
        let has_logs = !state.log_snapshot().is_empty();
        let filter = state.log_level_filter();

        // Header: a level `Popover` filter and a "Clear logs" button that wipes
        // the retained buffer (roadmap §5 quick wins).
        let filter_label = filter
            .map(|l| l.tag().to_string())
            .unwrap_or_else(|| "All".into());
        let pop_state = state.clone();
        let pop_this = this.clone();
        let filter_popover = Popover::new("log-level-filter")
            .trigger(
                Button::new("log-level-filter-trigger").label(format!("Level: {filter_label}")),
            )
            .content(move |_state, _window, _cx| {
                let state = pop_state.clone();
                let this = pop_this.clone();
                let current = state.log_level_filter();
                let levels = [
                    ("All", None),
                    ("Error", Some(LogLevel::Error)),
                    ("Warn", Some(LogLevel::Warn)),
                    ("Info", Some(LogLevel::Info)),
                    ("Debug", Some(LogLevel::Debug)),
                    ("Trace", Some(LogLevel::Trace)),
                ];
                div()
                    .flex_col()
                    .gap(px(2.))
                    .children(levels.into_iter().map(|(label, level)| {
                        let selected = current == level;
                        Button::new(format!("log-level-{label}"))
                            .label(label)
                            .when(selected, |b| b.outline())
                            .on_click({
                                let state = state.clone();
                                let this = this.clone();
                                move |_event, _window, cx| {
                                    state.set_log_level_filter(level);
                                    this.update(cx, |_, cx| cx.notify());
                                }
                            })
                            .into_any_element()
                    }))
                    .into_any_element()
            });

        let clear_button = Button::new("log-clear")
            .label("Clear")
            .when(has_logs, |b| b.outline())
            .on_click({
                let state = state.clone();
                let this = this.clone();
                move |_event, _window, cx| {
                    state.clear_logs();
                    this.update(cx, |_, cx| cx.notify());
                }
            });

        let header = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .px(px(8.))
            .py(px(4.))
            .gap(px(8.))
            .child(filter_popover)
            .child(clear_button);

        if !has_logs {
            return div()
                .flex()
                .flex_col()
                .flex_1()
                .overflow_hidden()
                .bg(cx.theme().background)
                .child(
                    div().flex().flex_col().size_full().child(header).child(
                        div()
                            .px(px(12.))
                            .py(px(8.))
                            .text_color(cx.theme().muted_foreground)
                            .child("No log output yet."),
                    ),
                )
                .into_any_element();
        }
        if self.table.is_none() {
            let delegate = LogsDelegate {
                state: self.state.clone(),
                // Seeded empty; `rows_count` refreshes it on the first render
                // pass, before any cell is queried.
                snapshot: std::cell::RefCell::new(Vec::new()),
            };
            self.table = Some(cx.new(|table_cx| TableState::new(delegate, window, table_cx)));
        }
        let table = self.table.clone().unwrap();
        div()
            .flex()
            .flex_col()
            .flex_1()
            .overflow_hidden()
            .bg(cx.theme().background)
            .child(header)
            .child(DataTable::new(&table).bordered(true))
            .into_any_element()
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

    #[test]
    fn level_filter_hides_noisier_records() {
        let state = DevToolsState::new();
        state.ingest_log(LogEntry::new(LogLevel::Info, "srv", "up"));
        state.ingest_log(LogEntry::new(LogLevel::Trace, "srv", "tick"));
        state.ingest_log(LogEntry::new(LogLevel::Error, "host", "boom"));
        // No filter: all three.
        assert_eq!(state.filtered_log_snapshot().len(), 3);
        // Filter to Warn+: drops Info/Trace, keeps Error only.
        state.set_log_level_filter(Some(LogLevel::Warn));
        let kept = state.filtered_log_snapshot();
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].level, LogLevel::Error);
        // Clearing the filter restores all records.
        state.set_log_level_filter(None);
        assert_eq!(state.filtered_log_snapshot().len(), 3);
        // Clearing the buffer empties it.
        state.clear_logs();
        assert!(state.log_snapshot().is_empty());
    }
}
