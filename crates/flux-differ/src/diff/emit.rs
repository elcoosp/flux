use std::hash::{Hash, Hasher};

use ahash::AHashSet;
use flux_ir::{IRArena, NodeView};
use flux_syntax::{ClosureRef, HandlerId, NodeRef, Patch, SignalId, Span};

pub(crate) fn emit_replace(patches: &mut Vec<Patch>, n: &NodeView<'_>) {
    patches.push(Patch::Replace {
        id: n.id(),
        node: to_ref(n),
    });
}

/// Emits `Handler` patches for the differing handler bodies (state-preserving).
pub(crate) fn emit_handler(
    patches: &mut Vec<Patch>,
    new: &IRArena,
    new_handlers: Vec<HandlerId>,
    old_handlers: Vec<HandlerId>,
) {
    for hid in new_handlers
        .iter()
        .chain(old_handlers.iter())
        .collect::<AHashSet<_>>()
    {
        if !new_handlers.contains(hid) || !old_handlers.contains(hid) {
            continue;
        }
        if let Some(cl) = new.closure(*hid) {
            patches.push(Patch::Handler {
                id: *hid,
                closure: closure_ref(&cl.bytecode, cl.captured_signals.clone(), cl.span),
            });
        }
    }
}

/// Builds a `ClosureRef` from a closure's bytecode. The digest is a content
/// hash; the canonical BLAKE3 form is produced by the serialization crate
/// (FLUX-013). Here a stable `u64` hash suffices for diff identity.
pub(crate) fn closure_ref(bytecode: &[u8], captured: Vec<SignalId>, span: Span) -> ClosureRef {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytecode.hash(&mut hasher);
    ClosureRef {
        hash: hasher.finish(),
        bytecode_offset: 0,
        bytecode_len: bytecode.len() as u16,
        captured_signals: captured,
        span,
        excerpt: None,
    }
}

/// Converts a `NodeView` into a standalone `NodeRef` for embedding in patches.
pub(crate) fn to_ref(v: &NodeView<'_>) -> NodeRef {
    NodeRef {
        id: v.id(),
        kind: v.kind(),
        component_id: v.component_id(),
        props: v.props(),
        children: v.children(),
        handlers: v.handlers(),
        span: v.span(),
    }
}
