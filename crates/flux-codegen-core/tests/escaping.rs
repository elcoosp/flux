//! T-401: String escaping for generated code.
//!
//! `render_string` delegates text escaping to `Backend::escape_text`
//! (audit H21). The implementations live in each backend crate:
//!   - Kotlin: `crates/flux-codegen-kotlin/src/backend_impl.rs`
//!   - Swift:  `crates/flux-codegen-swift/src/backend_impl.rs`
//!
//! This test verifies the `render_string` → `escape_text` contract using
//! two mock `Backend` types that bake in the correct escaping rules,
//! without requiring a dev-dependency on the backend crates.

use flux_codegen_core::expressions::render_string;
use flux_codegen_core::{
    Backend, Emitter, PrimitiveSpec, model::ComponentMeta, primitives::DesignToken,
};
use flux_parser::{Expr, StrPart, TypeDecl};
use flux_syntax::NodeId;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Swift mock backend
// ---------------------------------------------------------------------------

/// A mock backend whose `escape_text` implements Swift rules:
/// backslash, double-quote, newline, CR, tab.  Does NOT escape `$`
/// (Swift uses `\()` for interpolation, not `$`).
struct SwiftBackend;

impl Backend for SwiftBackend {
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
        _: bool,
    ) {
    }
    fn emit_sum_type(_: &mut Emitter<'_, Self>, _: &TypeDecl) {}
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

// ---------------------------------------------------------------------------
// Kotlin mock backend
// ---------------------------------------------------------------------------

/// A mock backend whose `escape_text` implements Kotlin rules:
/// backslash, double-quote, `$` (string templates), newline, CR, tab.
struct KotlinBackend;

impl Backend for KotlinBackend {
    const INDENT_UNIT: usize = 4;
    const CHILD_STEP: usize = 1;
    const SCREEN_BODY_STEP: usize = 1;
    const ROUTER_CHILDREN_AFTER_CLOSE: bool = false;
    fn int_type() -> &'static str {
        "Int"
    }
    fn float_type() -> &'static str {
        "Double"
    }
    fn bool_type() -> &'static str {
        "Boolean"
    }
    fn string_type() -> &'static str {
        "String"
    }
    fn unit_type() -> &'static str {
        "Unit"
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
        "${"
    }
    fn interp_close() -> &'static str {
        "}"
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
        _: bool,
    ) {
    }
    fn emit_sum_type(_: &mut Emitter<'_, Self>, _: &TypeDecl) {}
    fn emit_match(_: &mut Emitter<'_, Self>, _: NodeId, _: usize) {}

    fn escape_text(s: &str) -> String {
        let mut out = String::with_capacity(s.len() + 8);
        for c in s.chars() {
            match c {
                '\\' => out.push_str("\\\\"),
                '"' => out.push_str("\\\""),
                '$' => out.push_str("\\$"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                other => out.push(other),
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Helpers and tests
// ---------------------------------------------------------------------------

/// Renders `text` as a string literal via `render_string` using backend `B`.
fn render<B: Backend>(text: &str) -> String {
    let parts = vec![StrPart::Text(text.to_string())];
    render_string::<B>(&parts)
}

/// Asserts that `body` contains no unescaped `"`. A `"` is "escaped" only
/// when it immediately follows a `\` that is not itself escaped.
fn assert_no_bare_quote(body: &str) {
    let bytes = body.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            // Count preceding backslashes.
            let mut backslashes = 0;
            let mut j = i;
            while j > 0 && bytes[j - 1] == b'\\' {
                backslashes += 1;
                j -= 1;
            }
            // Odd count ⇒ the `"` is escaped; even count ⇒ bare.
            assert!(
                backslashes % 2 == 1,
                "unescaped bare `\"` in string body: {body}"
            );
        }
        i += 1;
    }
}

#[test]
fn swift_inner_quotes_are_escaped() {
    // T-401: `Text("say \"hi\"")` must not emit an unescaped `"`.
    let out = render::<SwiftBackend>("say \"hi\"");
    // The escaped body must contain `\"` for each original quote.
    assert!(
        out.contains(r#"say \"hi\"""#),
        "inner quotes must be backslash-escaped: {out}"
    );
    // No bare (unescaped) `"` in the body between the wrapping quotes.
    let inner = &out[1..out.len() - 1];
    assert_no_bare_quote(inner);
}

#[test]
fn swift_backslash_is_escaped() {
    let out = render::<SwiftBackend>("path\\to\\file");
    assert_eq!(out, "\"path\\\\to\\\\file\"");
}

#[test]
fn swift_newlines_are_escaped() {
    let out = render::<SwiftBackend>("a\nb");
    assert_eq!(out, "\"a\\nb\"");
}

#[test]
fn swift_dollar_passes_through() {
    // Swift does not use `$` for interpolation — it is NOT escaped.
    let out = render::<SwiftBackend>("cost $5");
    assert!(out.contains('$'), "Swift must not escape `$`: {out}");
}

#[test]
fn kotlin_dollar_sign_is_escaped() {
    // The scaffold fixture `tapped $count times` must produce `\$` in Kotlin
    // output so `${...}` is not a template interpolation.
    let out = render::<KotlinBackend>("tapped $count times");
    assert!(out.contains(r"\$"), "Kotlin must escape `$`: {out}");
}

#[test]
fn kotlin_inner_quotes_are_escaped() {
    let out = render::<KotlinBackend>("say \"hi\"");
    assert!(
        out.contains(r#"say \"hi\"""#),
        "inner quotes must be backslash-escaped: {out}"
    );
    let inner = &out[1..out.len() - 1];
    assert_no_bare_quote(inner);
}

#[test]
fn kotlin_backslash_is_escaped() {
    let out = render::<KotlinBackend>("a\\b");
    assert_eq!(out, "\"a\\\\b\"");
}

#[test]
fn injection_cannot_terminate_literal_swift() {
    // A string like `"){ evil()` must not close the literal early.
    let out = render::<SwiftBackend>("\"){ evil() }");
    let inner = &out[1..out.len() - 1];
    assert_no_bare_quote(inner);
}

#[test]
fn injection_cannot_terminate_literal_kotlin() {
    let out = render::<KotlinBackend>("\"){ evil() }");
    let inner = &out[1..out.len() - 1];
    assert_no_bare_quote(inner);
}

#[test]
fn render_string_wraps_empty_in_empty_quotes() {
    let out = render::<SwiftBackend>("");
    assert_eq!(out, "\"\"");
}
