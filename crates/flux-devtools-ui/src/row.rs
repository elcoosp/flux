//! Shared building blocks for the DevTools panes (spec §5.3).
//!
//! These keep every pane's key/value rows visually consistent (padding,
//! separators, muted labels, monospaced values) without reaching into
//! gpui-component's `pub(crate)` list primitives, which are not exposed to
//! downstream crates in the current release.

use gpui::{AnyElement, Div, InteractiveElement, IntoElement, ParentElement, Pixels, Styled, px};

/// Standard horizontal padding for pane rows.
pub(crate) const ROW_PAD_X: Pixels = gpui::px(12.0);
/// Standard vertical padding for pane rows.
pub(crate) const ROW_PAD_Y: Pixels = gpui::px(5.0);

/// A single key/value row: `label` on the left (muted), `value` on the right.
/// Used for registers, signals, component frames, and timeline metadata.
///
/// Both cells ellipsize (`…`) instead of overflowing when the pane is too
/// narrow: the label keeps flex priority (so the component name wins), while
/// the value shrinks and truncates first (geometry is secondary info).
pub fn kv_row(label: impl IntoElement, value: impl IntoElement) -> Div {
    gpui::div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .px(ROW_PAD_X)
        .py(ROW_PAD_Y)
        .border_b(px(1.0))
        .border_color(gpui::white().opacity(0.08))
        .child(
            gpui::div()
                .flex_1()
                .min_w(px(0.))
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .child(label),
        )
        .child(
            gpui::div()
                .flex_shrink(1.0)
                .min_w(px(0.))
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .debug_selector(|| "kv-value".to_string())
                .child(value),
        )
}

/// An empty-state row shown when a pane has no data yet, so the surface is
/// never blank (which would read as "nothing rendered").
pub fn empty_row(message: &str) -> Div {
    gpui::div()
        .px(ROW_PAD_X)
        .py(ROW_PAD_Y)
        .text_color(gpui::white().opacity(0.45))
        .child(message.to_string())
}

/// Collect an iterator of rows into a column container ready to drop into a
/// [`gpui_component::scroll::Scrollable`] body.
pub fn rows_column(rows: impl IntoIterator<Item = impl IntoElement>) -> impl IntoElement {
    gpui::div()
        .w_full()
        .min_w(px(0.))
        .flex()
        .flex_col()
        .children(rows.into_iter().map(|r| r.into_any_element()))
}

/// Helper to box a row into an [`AnyElement`] for heterogeneous `Vec` storage.
pub fn into_any(row: impl IntoElement) -> AnyElement {
    row.into_any_element()
}
