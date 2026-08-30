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
    div, prelude::*, px, AnyElement, ClickEvent, Context, ElementId, InteractiveElement,
    IntoElement, ParentElement, Render, Styled, Window,
};

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
            return rows_column(vec![into_any(empty_row("no signals yet"))]);
        }

        // Index readers (effect ids) per signal for O(1) lookup on click.
        let readers: std::collections::HashMap<SignalId, Vec<EffectId>> =
            live.signal_edges.iter().cloned().collect();

        let mut rows: Vec<AnyElement> = Vec::new();
        for (id, value) in live.signals.iter() {
            let is_selected = selected == Some(*id);
            let mut row = div()
                .id(ElementId::from(format!("sig-row-{id}")))
                .px(crate::row::ROW_PAD_X)
                .py(crate::row::ROW_PAD_Y)
                .border_b(px(1.0))
                .border_color(gpui::white().opacity(0.08))
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .hover(|s| s.bg(gpui::white().opacity(0.06)))
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
                });
            if is_selected {
                row = row.bg(gpui::rgb(0x2d_6c_df).opacity(0.22));
            }
            rows.push(into_any(
                row.child(format!("sig#{id}"))
                    .child(value_label(value)),
            ));

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
        rows_column(rows)
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
