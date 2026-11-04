//! Round-21 regressions for the type checker and diagnostics rendering:
//! 1. User-defined generic ADT variant fields now rewrite to `PARAM_BASE`
//!    vars, so matching a specialised variant binds the payload type.
//! 2. `TypeError::render` no longer panics on an inverted (end < start) span.
//! 3. `content_address` tolerates orphan (root-unreachable) nodes.

use flux_parser::parse;
use flux_types::type_check;

fn check(src: &str) -> Result<flux_types::TypedAST, flux_types::TypeError> {
    let ast = parse(src, 0, "t.flux").expect("parse");
    type_check(&ast)
}

/// `type Boxed[T] { Box(T) }` + `fn unbox(b: Boxed[Bool]) -> Bool { match b {
/// Box(v) => v } }` used to fail with "expected Bool, got T" because variant
/// field types kept the unresolved surface name.
#[test]
fn user_generic_adt_variant_fields_are_rewritten() {
    let src = concat!(
        "type Boxed[T] =\n",
        "  | Box(T)\n",
        "fn unbox(b: Boxed[Bool]) -> Bool {\n",
        "  match b {\n",
        "    Box(v) => v\n",
        "  }\n",
        "}\n",
    );
    let typed = check(src).expect("generic ADT variant match must type-check");
    let _ = typed;
}

#[test]
fn user_generic_adt_rejects_wrong_payload_type() {
    let src = concat!(
        "type Boxed[T] =\n",
        "  | Box(T)\n",
        "fn unbox(b: Boxed[Bool]) -> Bool {\n",
        "  match b {\n",
        "    Box(v) => v\n",
        "  }\n",
        "}\n",
        "fn bad(b: Boxed[Bool]) -> Int {\n",
        "  match b {\n",
        "    Box(v) => v\n",
        "  }\n",
        "}\n",
    );
    // Matching Box(v) must bind v: Bool, so returning it as Int is an error.
    assert!(
        check(src).is_err(),
        "Bool payload returned as Int must still be rejected"
    );
}

/// An inverted span (end < start) used to panic the caret renderer with a
/// u32 subtraction underflow.
#[test]
fn render_with_inverted_span_does_not_panic() {
    let err = flux_types::TypeError::new("synthetic mismatch", flux_syntax::Span::new(0, 10, 4));
    let rendered = err.render("fn f() { x }", "t.flux");
    assert!(rendered.contains("synthetic mismatch"));
}
