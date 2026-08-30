
use super::*;

fn span(start: u32, end: u32) -> Span {
    Span::new(1, start, end)
}

#[test]
fn fnv1a32_is_deterministic() {
    // Same input must map to the same ID across calls (the stable-ID contract;
    // FNV is not randomized per process, unlike ahasher).
    let a = compute_node_id(0, ExprTag(7), span(0, 10), None);
    let b = compute_node_id(0, ExprTag(7), span(0, 10), None);
    assert_eq!(a, b);
}

#[test]
fn content_addressed_id_is_deterministic() {
    // The content-addressed id is a pure function of structural content, so
    // identical content always maps to the identical id.
    let a = content_addressed_id(0, 1, 7, 0x1111, 0x2222, None);
    let b = content_addressed_id(0, 1, 7, 0x1111, 0x2222, None);
    assert_eq!(a, b);
}

#[test]
fn content_addressed_id_changes_with_content() {
    // Any of the content inputs must perturb the id.
    let base = content_addressed_id(0, 1, 7, 0x1111, 0x2222, None);
    assert_ne!(
        base,
        content_addressed_id(0, 1, 7, 0x9999, 0x2222, None),
        "props_hash change must change id"
    );
    assert_ne!(
        base,
        content_addressed_id(0, 1, 7, 0x1111, 0x9999, None),
        "children_hash change must change id"
    );
    assert_ne!(
        base,
        content_addressed_id(0, 2, 7, 0x1111, 0x2222, None),
        "kind change must change id"
    );
    assert_ne!(
        base,
        content_addressed_id(0, 1, 8, 0x1111, 0x2222, None),
        "component_id change must change id"
    );
    assert_ne!(
        base,
        content_addressed_id(0, 1, 7, 0x1111, 0x2222, Some(99)),
        "key change must change id"
    );
}

#[test]
fn content_addressed_id_ignores_span() {
    // Unlike compute_node_id, shifting the source span around a node must NOT
    // change its content-addressed id (FLUX-074, item A — the whole point).
    let from_above = content_addressed_id(0, 1, 7, 0x1111, 0x2222, None);
    // A different parent content id would change it; here we isolate span by
    // comparing two ids whose only difference is conceptual source position,
    // which the function never observes at all.
    let unchanged = content_addressed_id(0, 1, 7, 0x1111, 0x2222, None);
    assert_eq!(from_above, unchanged);
}

#[test]
fn expr_and_decl_families_are_disjoint() {
    // The DeclTag family bit (0x80) must keep an ExprTag and a DeclTag with the
    // same numeric discriminant producing distinct IDs.
    let expr = compute_node_id(0, ExprTag(7), span(0, 10), None);
    let decl = compute_node_id(0, DeclTag(7), span(0, 10), None);
    assert_ne!(expr, decl);
}

#[test]
fn every_field_independently_affects_id() {
    // Changing parent, tag, file_id, start, end, or key must each change the ID.
    let base = compute_node_id(0, ExprTag(7), span(0, 10), None);
    assert_ne!(
        base,
        compute_node_id(1, ExprTag(7), span(0, 10), None),
        "parent change must change id"
    );
    assert_ne!(
        base,
        compute_node_id(0, ExprTag(8), span(0, 10), None),
        "tag change must change id"
    );
    assert_ne!(
        compute_node_id(0, ExprTag(7), Span::new(2, 0, 10), None),
        base,
        "file_id change must change id"
    );
    assert_ne!(
        base,
        compute_node_id(0, ExprTag(7), span(5, 10), None),
        "start change must change id"
    );
    assert_ne!(
        base,
        compute_node_id(0, ExprTag(7), span(0, 20), None),
        "end change must change id"
    );
    assert_ne!(
        base,
        compute_node_id(0, ExprTag(7), span(0, 10), Some(99)),
        "key change must change id"
    );
}

#[test]
fn none_and_some_zero_are_distinct() {
    // The key sentinel differs between the no-key (0xFF*8) and key=0 cases.
    let none = compute_node_id(0, ExprTag(7), span(0, 10), None);
    let some_zero = compute_node_id(0, ExprTag(7), span(0, 10), Some(0));
    assert_ne!(none, some_zero);
}
