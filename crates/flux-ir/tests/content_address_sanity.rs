//! Round-23: `content_address` must not panic on a dangling child reference.
//!
//! Two remap helpers in `arena/content_address.rs` did the same job with
//! opposite policies: `remap_children` used `.unwrap_or(cid)` (preserve the
//! original id), while `compute_all_local_ids` used `.expect(...)` (panic).
//! A future arena with a malformed child ref — produced by a bug in a merge
//! or a lower pass, or by manual construction — would have killed the whole
//! dev-server pipeline. This test locks the tolerant behaviour so the two
//! paths stay in agreement.

use flux_ir::{ArenaBuilder, Node};
use flux_syntax::{Child, ComponentId, NodeId, NodeKind, Props, Span};

#[test]
fn content_address_with_dangling_child_does_not_panic() {
    // Parent references child id 999, which is never packed. This is
    // malformed input, but the check must degrade, not panic.
    let mut b = ArenaBuilder::new();
    b.pack(Node {
        id: NodeId::from(1u32),
        kind: NodeKind::Component,
        component_id: ComponentId::from(1u32),
        props: Props::from_fields(vec![]),
        children: vec![Child::Node(NodeId::from(999u32))],
        handlers: vec![],
        span: Span::new(0, 0, 10),
    });
    let mut arena = b.finish();
    // Must not panic.
    let _ = arena.content_address();
}
