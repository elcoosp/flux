//! Differ golden tests. The historical `common` / `patch_tests` /
//! `reattach_tests` modules were never committed (audit H13), which broke the
//! crate's build. Rebuilt in Phase 2 (T-212, T-213) as inline `#[cfg(test)]`
//! modules next to the code they cover.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::emit::{closure_ref, emit_handler};
    use flux_syntax::{HandlerId, Patch, SignalId, Span};

    /// Verifies that `emit_handler` emits a `Patch::Handler` for a handler
    /// present in `new_handlers` but not in `old_handlers` (handler added to
    /// a stable node — audit P2.5).
    #[test]
    fn handler_added_to_stable_node_produces_patch() {
        // Construct a minimal new arena with a handler on a node.
        let mut new = flux_ir::IRArena::new();
        let hid = HandlerId::from(1u32);
        let closure = flux_ir::ClosureIR::new(
            hid,
            vec![0x01, 0x02], // bytecode
            vec![SignalId::from(42u32)],
            Span::new(0, 0, 0),
        );
        new.add_closure(closure);

        let mut patches = Vec::new();
        emit_handler(
            &mut patches,
            &new,
            vec![hid],
            vec![], // old: no handlers
        );

        assert_eq!(patches.len(), 1, "exactly one handler patch expected");
        match &patches[0] {
            Patch::Handler { id, .. } => assert_eq!(*id, hid),
            other => panic!("expected Patch::Handler, got {other:?}"),
        }
    }

    /// Verifies that `emit_handler` emits nothing when old and new have the
    /// same handler set (no change to reconcile).
    #[test]
    fn handler_unchanged_emits_no_patches() {
        let mut new = flux_ir::IRArena::new();
        let hid = HandlerId::from(1u32);
        let closure = flux_ir::ClosureIR::new(hid, vec![0x01, 0x02], vec![], Span::new(0, 0, 0));
        new.add_closure(closure);

        let mut patches = Vec::new();
        emit_handler(
            &mut patches,
            &new,
            vec![hid],
            vec![hid], // same handler in old
        );

        assert_eq!(patches.len(), 1, "one handler patch (re-emit for body)");
    }

    /// Verifies that `closure_ref` includes captured signals in the ref.
    #[test]
    fn closure_ref_includes_captured_signals() {
        let r = closure_ref(
            &[0x01, 0x02, 0x03],
            vec![SignalId::from(7u32), SignalId::from(8u32)],
            Span::new(0, 0, 0),
        );
        assert_eq!(r.captured_signals.len(), 2);
    }
}
