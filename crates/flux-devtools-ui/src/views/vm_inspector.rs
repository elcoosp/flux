//! VM inspector view (spec §5.3): register bank + current instruction.

use std::sync::Arc;

use gpui::{AnyElement, Context, IntoElement, Render, Window};

use crate::row::{empty_row, into_any, kv_row, rows_column};
use crate::state::{DevToolsState, VmState};

/// Renders the live VM register bank and instruction pointer.
pub struct VmInspectorView {
    state: Arc<DevToolsState>,
}

impl VmInspectorView {
    /// Creates the view bound to the shared state.
    pub fn new(state: Arc<DevToolsState>) -> Self {
        Self { state }
    }

    /// The current VM snapshot for rendering.
    fn vm_state(&self) -> VmState {
        self.state.vm_state()
    }

    /// Renders the view as a standalone pane (used by the root layout).
    pub fn render_pane(&self, _cx: &Context<'_, Self>) -> impl IntoElement {
        let vm = self.vm_state();
        let offset = vm
            .bytecode_offset
            .map_or_else(|| "?".into(), |o| format!("0x{o:04X}"));
        let gas = vm
            .gas_remaining
            .map_or_else(|| "?".into(), |g| g.to_string());

        let mut rows: Vec<AnyElement> =
            vec![into_any(kv_row("IP", offset)), into_any(kv_row("Gas", gas))];
        if vm.registers.is_empty() {
            rows.push(into_any(empty_row("no registers yet")));
        }
        for (i, val) in vm.registers.iter().enumerate() {
            rows.push(into_any(kv_row(format!("r{i}"), format!("{val:?}"))));
        }
        rows_column(rows)
    }
}

impl Render for VmInspectorView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        self.render_pane(cx)
    }
}
