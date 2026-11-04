use super::index::*;
use crate::util::span_to_range;

use flux_parser::parse;

/// Returns the byte offset of the first byte of `needle` within `text`.
fn cursor_at(text: &str, needle: &str) -> u32 {
    text.find(needle).expect("needle present in fixture") as u32
}

#[test]
fn resolves_component_name_to_its_declaration_span() {
    let src = "compo Counter\n  Button(text: \"tap\")\n  Counter()\n";
    let ast = parse(src, 0, "f.flux").expect("parses");
    let idx = DefIndex::build(&ast);
    let span = idx
        .resolve(src, cursor_at(src, "Counter()"))
        .expect("found definition");
    // The declaration `Counter` is the first occurrence; the name is 7 bytes.
    let decl = cursor_at(src, "Counter");
    assert_eq!(span.start, decl);
    assert_eq!(span.end, decl + 7);
}

#[test]
fn resolves_local_state_binding() {
    let src = "compo C\n  state count: Int = 0\n  Button(text: count)\n";
    let ast = parse(src, 0, "f.flux").expect("parses");
    let idx = DefIndex::build(&ast);
    let use_col = cursor_at(src, "count)");
    let span = idx.resolve(src, use_col).expect("found binding");
    let decl = cursor_at(src, "state count") + 6;
    assert_eq!(span.start, decl);
}

#[test]
fn no_definition_for_unknown_position() {
    let src = "compo C\n  Button(text: \"x\")\n";
    let ast = parse(src, 0, "f.flux").expect("parses");
    let idx = DefIndex::build(&ast);
    let cursor = cursor_at(src, "\"x\"") + 1;
    assert!(idx.resolve(src, cursor).is_none());
}

#[test]
fn inner_binding_shadows_outer() {
    let src = "compo C\n  state count: Int = 0\n  Button(onPress: fn(delta) { delta })\n";
    let ast = parse(src, 0, "f.flux").expect("parses");
    let idx = DefIndex::build(&ast);
    // The `delta` inside the closure body refers to the lambda parameter,
    // not the outer `count` — assert it lands on the lambda parameter.
    let cursor = src.rfind("delta").expect("inner delta present") as u32;
    let span = idx
        .resolve(src, cursor)
        .expect("found lambda param `delta`");
    let decl = cursor_at(src, "fn(delta)") + 3;
    assert_eq!(span.start, decl);
    assert_eq!(span.end, decl + 5);
}

#[test]
fn resolve_produces_an_lsp_range() {
    let src = "compo Counter\n  Button(text: \"tap\")\n  Counter()\n";
    let ast = parse(src, 0, "f.flux").expect("parses");
    let idx = DefIndex::build(&ast);
    let span = idx
        .resolve(src, cursor_at(src, "Counter()"))
        .expect("found");
    let _ = span_to_range(src, span); // must not panic
}
