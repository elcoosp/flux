use super::*;
use flux_syntax::{DeclTag, Key, Span};

// Bridge test (ADR-0027): the type checker's `compute_node_id` must be
// byte-identical to the canonical `flux_syntax::compute_node_id`, so FLUX-018
// lowering can look up inferred types by `NodeId`. Historically this crate
// forked an FNV reduction that omitted `span.file_id`; it now delegates.
#[test]
fn matches_canonical_flux_syntax() {
    let parents = [0u32, 1, 7, 4_000_000];
    let tags = [0u8, 1, 3, 9, 255];
    let spans = [
        Span::new(0, 0, 4),
        Span::new(1, 10, 20),
        Span::new(3, 40, 52),
        Span::new(2, 0, 1_000_000),
    ];
    let keys: [Option<Key>; 3] = [None, Some(0), Some(99)];
    for &parent in &parents {
        for &raw in &tags {
            let tag = DeclTag(raw);
            for &span in &spans {
                for &key in &keys {
                    let our = compute_node_id(parent, tag, span, key);
                    let canonical = flux_syntax::compute_node_id(parent, tag, span, key);
                    assert_eq!(
                        our, canonical,
                        "mismatch for ({parent}, {tag:?}, {span:?}, {key:?})"
                    );
                }
            }
        }
    }
}
