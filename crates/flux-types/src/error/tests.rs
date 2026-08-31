use super::*;
use crate::kind::TcType;

#[test]
fn mismatch_render_contains_location_and_hint() {
    let span = flux_syntax::Span::new(0, 10, 14);
    let err = TypeError::mismatch(&TcType::Int, &TcType::String, span);
    let rendered = err.render("compo X\n  count = \"nope\"\n", "main.flux");
    assert!(rendered.contains("main.flux:2:3"), "got: {rendered}");
    assert!(rendered.contains("hint:"), "got: {rendered}");
    assert!(rendered.contains("expected `Int`"), "got: {rendered}");
}

#[test]
fn new_error_without_hint_renders_only_what_where() {
    let span = flux_syntax::Span::new(0, 0, 4);
    let err = TypeError::new("cannot lower construct", span);
    let rendered = err.render("compo X", "x.flux");
    assert!(rendered.contains("x.flux:1:1"));
    assert!(!rendered.contains("hint:"));
}

#[test]
fn flux_error_class_and_accessors() {
    let denied = capability_denied(
        1,
        1,
        Some("Camera".to_owned()),
        Some("take".to_owned()),
        ".camera".to_owned(),
    );
    assert_eq!(denied.class(), "capability");
    assert!(denied.what().contains("Camera"));
    assert!(denied.how().unwrap().contains(".camera"));
    assert!(denied.where_span().is_none());
}
