//! VM inspector view (spec §5.3): register bank + current instruction.

use gpui::{Context, Entity, IntoElement, ParentElement, Render, Styled, View, Window};

use crate::state::{DevToolsState, VmState};

/// Renders the live VM register bank and instruction pointer.
pub struct VmInspectorView {
    state: Entity<DevToolsState>,
}

impl VmInspectorView {
    /// Creates the view bound to the shared state.
    pub fn new(state: Entity<DevToolsState>) -> Self {
        Self { state }
    }

    /// The current VM snapshot for rendering.
    fn vm_state(&self, cx: &Context<Self>) -> VmState {
        self.state.read(cx).vm_state()
    }

    /// Renders the view as a standalone pane (used by the root layout).
    pub fn render_pane(&self, cx: &Context<Self>) -> impl IntoElement {
        let vm = self.vm_state(cx);
        let offset = vm
            .bytecode_offset
            .map_or_else(|| "?".into(), |o| format!("0x{o:04X}"));
        let gas = vm
            .gas_remaining
            .map_or_else(|| "?".into(), |g| g.to_string());
        gpui::div()
            .flex()
            .flex_col()
            .p_4()
            .child(gpui::div().child(format!("IP: {offset}")))
            .child(gpui::div().child(format!("Gas: {gas}")))
            .children(vm.registers.iter().enumerate().map(|(i, val)| {
                gpui::div()
                    .flex()
                    .justify_between()
                    .child(gpui::div().child(format!("r{i}")))
                    .child(gpui::div().child(format!("{val:?}")))
            }))
    }
}

impl Render for VmInspectorView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_pane(cx)
    }
}
