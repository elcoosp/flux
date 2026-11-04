//! Round-21 adversarial tests for `flux_codegen_core::expressions::render_string`:
//! hostile `StrPart` sequences (the shared string renderer both backends call)
//! against a mock backend. Contract: no panics, escaped text stays inside the
//! quotes, and the interpolation delimiters frame exactly the expression.

use flux_codegen_core::expressions::render_string;
use flux_codegen_core::{
    Backend, Emitter, PrimitiveSpec, model::ComponentMeta, primitives::DesignToken,
};
use flux_parser::{Expr, ExprKind, Ident, RecordDecl, StrPart, TypeDecl};
use flux_syntax::NodeId;
use flux_syntax::Span;
use std::collections::HashMap;

/// Minimal mock backend with Swift-style rules: escape backslash, quote,
/// newline, CR, tab; interpolation delimiters `\(` … `)` (copied from the
/// escaping.rs mock contract so both suites agree on the base rules).
struct MockSwift;

impl Backend for MockSwift {
    const INDENT_UNIT: usize = 1;
    const CHILD_STEP: usize = 4;
    const SCREEN_BODY_STEP: usize = 0;
    const ROUTER_CHILDREN_AFTER_CLOSE: bool = false;
    fn int_type() -> &'static str {
        "Int"
    }
    fn float_type() -> &'static str {
        "Double"
    }
    fn bool_type() -> &'static str {
        "Bool"
    }
    fn string_type() -> &'static str {
        "String"
    }
    fn unit_type() -> &'static str {
        "Void"
    }
    fn any_type() -> &'static str {
        "Any"
    }
    fn record_type(_: &[String]) -> String {
        String::new()
    }
    fn container_spacing(_: &str) -> String {
        String::new()
    }
    fn container_spacing_axis(_: &str, _: &str) -> String {
        String::new()
    }
    fn image_expr(_: &str) -> String {
        String::new()
    }
    fn router_open(_: &str) -> String {
        String::new()
    }
    fn router_close() -> String {
        String::new()
    }
    fn router_destination_close() -> String {
        String::new()
    }
    fn router_navigate_expr(_: &str) -> String {
        String::new()
    }
    fn screen_open(_: &str) -> String {
        String::new()
    }
    fn screen_close() -> String {
        String::new()
    }
    fn if_open(_: &str) -> String {
        String::new()
    }
    fn for_each_open(_: &str, _: &str, _: &str) -> String {
        String::new()
    }
    fn for_each_close() -> String {
        String::new()
    }
    fn button_open(_: &str, _: &str) -> String {
        String::new()
    }
    fn button_style(_: &str) -> &'static str {
        ""
    }
    fn text_field(_: &str, _: &str, _: &str) -> String {
        String::new()
    }
    fn key_extractor(_: &Expr) -> String {
        String::new()
    }
    fn interp_open() -> &'static str {
        "\\("
    }
    fn interp_close() -> &'static str {
        ")"
    }
    fn list_literal(_: &[String]) -> String {
        String::new()
    }
    fn render_await(_: &str) -> String {
        String::new()
    }
    fn unsupported_placeholder() -> String {
        String::new()
    }
    fn native_name(_: &PrimitiveSpec) -> &'static str {
        ""
    }
    fn animation_spec(_: &str) -> String {
        String::new()
    }
    fn theme_extension(_: &[DesignToken]) -> String {
        String::new()
    }
    fn component_body_indent() -> usize {
        0
    }
    fn emit_component_header(
        _: &mut Emitter<'_, Self>,
        _: &str,
        _: &str,
        _: &ComponentMeta<'_>,
        _: &HashMap<String, String>,
    ) {
    }
    fn emit_body_open(_: &mut Emitter<'_, Self>) {}
    fn emit_component_footer(_: &mut Emitter<'_, Self>) {}
    fn emit_placeholder_component(_: &mut Emitter<'_, Self>, _: NodeId) {}
    fn emit_state_cell(
        _: &mut Emitter<'_, Self>,
        _: &str,
        _: &str,
        _: &str,
        _: &HashMap<String, String>,
        _: Option<&str>,
    ) {
    }
    fn emit_sum_type(_: &mut Emitter<'_, Self>, _: &TypeDecl) {}
    fn emit_record_type(_: &mut Emitter<'_, Self>, _: &RecordDecl) {}
    fn emit_match(_: &mut Emitter<'_, Self>, _: NodeId, _: usize) {}

    fn escape_text(s: &str) -> String {
        let mut out = String::with_capacity(s.len() + 8);
        for c in s.chars() {
            match c {
                '\\' => out.push_str("\\\\"),
                '"' => out.push_str("\\\""),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                other => out.push(other),
            }
        }
        out
    }
}

fn text(text: &str) -> StrPart {
    StrPart::Text(text.to_owned())
}

fn interp_ident(name: &str) -> StrPart {
    StrPart::Interp(Expr {
        kind: ExprKind::Ident(Ident {
            name: name.to_owned(),
            span: Span::new(0, 0, 1),
        }),
        span: Span::new(0, 0, 1),
    })
}

/// Every output must be a single `"…"` literal with NO raw newline and NO raw
/// quote in *text* regions. `\(...)` interpolation spans may legally contain
/// nested string literals, so the scanner tracks interpolation nesting with
/// its own inner literal state.
fn assert_single_literal(out: &str, context: &str) {
    assert!(
        out.starts_with('"') && out.ends_with('"') && out.len() >= 2,
        "{context}: output must be a quoted literal, got {out:?}"
    );
    let inner = &out[1..out.len() - 1];
    let bytes: Vec<char> = inner.chars().collect();
    let mut i = 0usize;
    let mut escaped = false;
    while i < bytes.len() {
        let ch = bytes[i];
        if escaped {
            escaped = false;
            i += 1;
            continue;
        }
        match ch {
            '\\' => {
                if i + 1 < bytes.len() && bytes[i + 1] == '(' {
                    // `\(` opens a live-interpolation span: scan to its
                    // matching `)`, honouring nested string literals (which
                    // carry their own escape state) and nested parens.
                    let mut depth = 1usize;
                    i += 2;
                    let mut in_nested = false;
                    let mut nested_esc = false;
                    while i < bytes.len() && depth > 0 {
                        let c = bytes[i];
                        if in_nested {
                            if nested_esc {
                                nested_esc = false;
                            } else if c == '\\' {
                                nested_esc = true;
                            } else if c == '"' {
                                in_nested = false;
                            }
                        } else {
                            match c {
                                '"' => in_nested = true,
                                '(' => depth += 1,
                                ')' => depth -= 1,
                                _ => {}
                            }
                        }
                        i += 1;
                    }
                } else {
                    escaped = true;
                    i += 1;
                }
            }
            '"' => panic!("{context}: raw quote in text region: {out:?}"),
            '\n' | '\r' => panic!("{context}: raw newline in text region: {out:?}"),
            _ => i += 1,
        }
    }
}

#[test]
fn hostile_text_parts_render_escaped() {
    for text_val in [
        "quote \" mid",
        "backslash \\ mid",
        "newline \n mid",
        "cr \r mid",
        "unicode \u{1F600}\u{0301}\u{202E}",
        "",
        "\\(live) ${tpl} {brace}",
        "curve \"); evil(); (\"",
    ] {
        let out = render_string::<MockSwift>(&[text(&text_val)]);
        assert_single_literal(&out, &format!("text {text_val:?}"));
    }
}

#[test]
fn interleaved_text_and_interp_stay_delimited() {
    // An interpolation whose own expression is a string containing
    // quotes/braces must not splice literal-breaking characters around the
    // interpolation delimiters.
    let nested = Expr {
        kind: ExprKind::Str(vec![text("a \" b"), interp_ident("x"), text(" } ")]),
        span: Span::new(0, 0, 1),
    };
    let parts = vec![
        text("before \" "),
        interp_ident("state"),
        StrPart::Interp(nested),
        text(" after"),
    ];
    let out = render_string::<MockSwift>(&parts);
    assert_single_literal(&out, "interleaved");
    assert!(
        out.contains("\\(state"),
        "interp open/close must frame the identifier: {out:?}"
    );
}

#[test]
fn empty_and_alternating_parts_render_without_panic() {
    let out = render_string::<MockSwift>(&[]);
    assert_eq!(out, "\"\"", "empty parts render an empty literal");
    let out = render_string::<MockSwift>(&[text(""), text(""), interp_ident("a"), text("")]);
    assert_single_literal(&out, "alternating");
}
