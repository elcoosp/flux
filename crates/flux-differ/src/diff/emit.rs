use std::hash::{Hash, Hasher};

use flux_ir::{IRArena, NodeView};
use flux_syntax::{ClosureRef, HandlerId, NodeRef, Patch, SignalId, Span};

pub(crate) fn emit_replace(patches: &mut Vec<Patch>, n: &NodeView<'_>) {
    patches.push(Patch::Replace {
        id: n.id(),
        node: to_ref(n),
    });
}

/// Emits `Handler` patches for handlers in `new_handlers` (added or body
/// changed). Handlers present only in `old_handlers` (removed) are not
/// emitted — the host reconciles by removing any handler not mentioned in
/// the patch set (audit P2.5).
pub(crate) fn emit_handler(
    patches: &mut Vec<Patch>,
    new: &IRArena,
    new_handlers: Vec<HandlerId>,
    old_handlers: Vec<HandlerId>,
) {
    for hid in new_handlers {
        if let Some(cl) = new.closure(hid) {
            // Emit for all handlers in new (added or body changed).
            // Host handles removal of handlers not in this set.
            patches.push(Patch::Handler {
                id: hid,
                closure: closure_ref(&cl.bytecode, cl.captured_signals.clone(), cl.span),
            });
        }
    }
    // Handlers only in old_handlers are implicitly removed — host reconciles
    // by comparing the patch's handler set with the node's current handlers.
    let _ = old_handlers;
}

/// Builds a `ClosureRef` from a closure's bytecode. The digest is a content
/// hash; the canonical BLAKE3 form is produced by the serialization crate
/// (FLUX-013). Here a stable `u64` hash suffices for diff identity.
///
/// The `bytecode_len` field is a `u16` wire prefix. A handler body larger
/// than `u16::MAX` bytes cannot be represented — the previous `as u16` cast
/// silently truncated the reference (audit H14 / ADR-0059 class), so the
/// host sliced the wrong bytes out of the shared closure blob or failed
/// dispatch. The policy here mirrors the encoder's (`frame.rs::write_closures`):
/// log a warning and emit a **zero-length** ref so the failure surfaces as an
/// `InvalidDispatch` on first invocation — a visible diagnostic — rather than
/// silently shipping the wrong byte range. The dev-server encoder independently
/// applies the same rule to its own closure blob, so a handler this large
/// cannot reach a host through either path.
pub(crate) fn closure_ref(bytecode: &[u8], captured: Vec<SignalId>, span: Span) -> ClosureRef {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytecode.hash(&mut hasher);
    let bytecode_len = match u16::try_from(bytecode.len()) {
        Ok(len) => len,
        Err(_) => {
            tracing::warn!(
                bytecode_len = bytecode.len(),
                "handler body exceeds u16 length; emitting a zero-length closure ref"
            );
            0
        }
    };
    ClosureRef {
        hash: hasher.finish(),
        bytecode_offset: 0,
        bytecode_len,
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
