//! Signal graph view (spec §5.3): the reactive signal cell values as a
//! clickable dependency DAG.
//!
//! Each signal is a node. Clicking a node selects it and reveals the effects
//! that re-run when it changes (its dependency edges, PRD-P user story 2 — "what
//! reads" a signal). Selected nodes are tinted with the theme's `accent` so the
//! active subtree reads at a glance. Hover tints a row with `muted`.

use std::sync::Arc;

use flux_syntax::{EffectId, SignalId, Value};
use gpui::{
    AnyElement, ClickEvent, Context, ElementId, InteractiveElement, IntoElement, ParentElement,
    Render, Styled, Window, div, prelude::*, px,
};
use gpui_component::ActiveTheme as _;
use gpui_component::menu::ContextMenuExt as _;
use gpui_component::menu::PopupMenu;

use crate::row::{empty_row, into_any, kv_row, rows_column};
use crate::state::DevToolsState;
use crate::time_travel::ReconstructedState;

/// Renders the live signal graph as a clickable dependency DAG.
pub struct SignalGraphView {
    state: Arc<DevToolsState>,
}

impl SignalGraphView {
    /// Creates the view bound to the shared state.
    pub fn new(state: Arc<DevToolsState>) -> Self {
        Self { state }
    }

    /// The current reconstructed signal state.
    fn live(&self) -> ReconstructedState {
        self.state.live.read().clone()
    }

    /// Renders the view as a standalone pane.
    pub fn render_pane(&mut self, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let this = cx.entity();
        let live = self.live();
        let selected = self.state.selected_signal();
        if live.signals.is_empty() && live.signal_edges.is_empty() {
            return div()
                .flex()
                .flex_col()
                .flex_1()
                .overflow_hidden()
                .bg(cx.theme().background)
                .child(rows_column(vec![into_any(empty_row("no signals yet"))]))
                .into_any_element();
        }

        // Index readers (effect ids) per signal for O(1) lookup on click.
        let readers: std::collections::HashMap<SignalId, Vec<EffectId>> =
            live.signal_edges.iter().cloned().collect();

        let colors = cx.theme();
        let mut rows: Vec<AnyElement> = Vec::new();
        for (id, value) in live.signals.iter() {
            let is_selected = selected == Some(*id);
            let sig_id = *id;
            let mut row = div()
                .id(ElementId::from(format!("sig-row-{id}")))
                .px(crate::row::ROW_PAD_X)
                .py(crate::row::ROW_PAD_Y)
                .border_b(px(1.0))
                .border_color(colors.border)
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .hover(|s| s.bg(colors.accent.opacity(0.25)))
                .cursor_pointer()
                .on_click({
                    let id = *id;
                    let this = this.clone();
                    let state = self.state.clone();
                    move |_event: &ClickEvent, _window, cx| {
                        this.update(cx, |_, cx| {
                            state.toggle_signal_selection(id);
                            cx.notify();
                        });
                    }
                })
                .child(format!("sig#{id}"))
                .child(value_label(value));
            if is_selected {
                row = row.bg(colors.primary.opacity(0.22));
            }
            let row = row.context_menu(move |menu: PopupMenu, _window, _cx| {
                menu.menu(
                    format!("Inspect signal #{sig_id}"),
                    Box::new(crate::app::InspectSignal { id: sig_id }),
                )
            });
            rows.push(into_any(row));

            // When selected, render the dependency edges as indented readers.
            if is_selected {
                let fx: Vec<EffectId> = readers.get(id).cloned().unwrap_or_default();
                if fx.is_empty() {
                    rows.push(into_any(kv_row("  → readers", "∅")));
                } else {
                    rows.push(into_any(kv_row(
                        "  → readers",
                        fx.iter()
                            .map(|e| format!("fx#{e}"))
                            .collect::<Vec<_>>()
                            .join(", "),
                    )));
                }
            }
        }
        // Background color and overflow clipping for clean pane isolation.
        gpui::div()
            .flex()
            .flex_col()
            .flex_1()
            .overflow_hidden()
            .bg(cx.theme().background)
            .child(rows_column(rows))
            .into_any_element()
    }
}

/// Render a signal value compactly (ints inline; strings by id; null as —).
fn value_label(value: &Value) -> AnyElement {
    let text = match value {
        Value::Int(i) => format!("{i}"),
        Value::Float(f) => format!("{f}"),
        Value::Bool(b) => format!("{b}"),
        Value::Str(s) => format!("str#{s}"),
        Value::Null => "—".to_string(),
        other => format!("{other:?}"),
    };
    div()
        .text_color(gpui::white().opacity(0.7))
        .child(text)
        .into_any_element()
}

impl Render for SignalGraphView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        self.render_pane(cx)
    }
}
