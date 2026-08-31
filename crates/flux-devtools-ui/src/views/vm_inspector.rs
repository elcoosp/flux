//! VM inspector view (spec §5.3): register bank + current instruction.
//!
//! Presents the live VM snapshot as key‑value rows (FLUX-059), with the current
//! opcode rendered as a semantic, coloured [`Badge`] and the remaining gas as a
//! [`Progress`] gauge so a developer can see at a glance how much headroom the
//! running frame has left.

use std::sync::Arc;

use gpui::{div, prelude::*, px, AnyElement, App, Context, IntoElement, Render, Window};
use gpui_component::progress::Progress;
use gpui_component::{badge::Badge, ActiveTheme as _};

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

    /// Heuristic opcode → semantic colour. Low opcodes are the common
    /// load/arithmetic family (green); higher opcodes are control/store/capability
    /// ops (blue). This is a visual hint only — the wire opcode space is not
    /// surfaced as named ISA mnemonics here.
    fn opcode_color(cx: &App, opcode: u8) -> gpui::Hsla {
        if opcode < 0x80 {
            cx.theme().success
        } else {
            cx.theme().info
        }
    }

    /// One row: the opcode as a coloured badge (OP 0xNN) above the register bank.
    fn opcode_badge(vm: &VmState, cx: &App) -> impl IntoElement {
        let (label, color) = match vm.opcode {
            Some(op) => (format!("OP 0x{op:02X}"), Self::opcode_color(cx, op)),
            None => ("OP —".to_string(), cx.theme().muted_foreground),
        };
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .child(Badge::new().color(color).child(label))
    }

    /// One row: remaining gas as a text value plus a horizontal gauge. The gauge
    /// fraction is the gas value against the VM's entry budget (`ENTRY_GAS`,
    /// mirroring the runtime in `flux-vm-ref`/Kotlin), so the bar reads as real
    /// headroom; the raw count is always shown alongside it.
    fn gas_row(vm: &VmState, cx: &App) -> impl IntoElement {
        const ENTRY_GAS: f32 = 100_000.0;
        let (gas_text, pct) = match vm.gas_remaining {
            Some(g) => (
                g.to_string(),
                (g as f32 / ENTRY_GAS * 100.0).clamp(0.0, 100.0),
            ),
            None => ("?".to_string(), 0.0),
        };
        let color = if pct < 20.0 {
            cx.theme().warning
        } else {
            cx.theme().success
        };
        div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(
                div()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("Gas: {gas_text}")),
            )
            .child(Progress::new("vm-gas").value(pct).color(color))
    }

    /// Renders the view as a standalone pane (used by the root layout).
    pub fn render_pane(&self, cx: &App) -> impl IntoElement {
        let vm = self.vm_state();
        let offset = vm
            .bytecode_offset
            .map_or_else(|| "?".into(), |o| format!("0x{o:04X}"));

        let mut rows: Vec<AnyElement> = vec![
            into_any(Self::opcode_badge(&vm, cx).into_any_element()),
            into_any(Self::gas_row(&vm, cx).into_any_element()),
            into_any(kv_row("IP", offset)),
        ];
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
