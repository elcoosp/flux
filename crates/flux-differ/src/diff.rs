//! Keyed tree differencing between two lowered arenas (FLUX-014).
//!
//! [`diff`] consumes the old and new [`IRArena`](flux_ir::IRArena)s and produces
//! a [`Patch`](flux_syntax::Patch) vector that the host applies to keep its
//! shadow tree state-preserving: nodes that survive an edit (same construct,
//! parent, and slot) are `Reattach`ed rather than removed and re-inserted, so
//! scroll position, focus, and animation state live across hot reload.

pub use algorithm::diff;

mod algorithm;
mod compare;
mod emit;
mod tree;

mod tests;

#[cfg(test)]
mod emit_tests {
    use crate::diff::emit::{closure_ref, emit_handler};
    use flux_syntax::{HandlerId, Patch, SignalId, Span};

    /// Verifies that `emit_handler` emits a `Patch::Handler` for a handler
    /// present in `new_handlers` but not in `old_handlers` (handler added to
    /// a stable node — audit P2.5).
    #[test]
    fn handler_added_to_stable_node_produces_patch() {
        let mut new = flux_ir::IRArena::new();
        let hid = HandlerId::from(1u32);
        let closure = flux_ir::ClosureIR::new(
            hid,
            vec![0x01, 0x02],
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

    /// Verifies that `handlers_equal` returns `false` when the captured
    /// signal set differs even though the bytecode is identical (audit P2.6).
    #[test]
    fn handlers_equal_differs_on_captured_signals() {
        let hid = HandlerId::from(1u32);

        let mut old = flux_ir::IRArena::new();
        old.add_closure(flux_ir::ClosureIR::new(
            hid,
            vec![0x01, 0x02],
            vec![SignalId::from(10u32)],
            Span::new(0, 0, 0),
        ));

        let mut new = flux_ir::IRArena::new();
        new.add_closure(flux_ir::ClosureIR::new(
            hid,
            vec![0x01, 0x02], // same bytecode
            vec![SignalId::from(20u32)], // different captured signals
            Span::new(0, 0, 0),
        ));

        assert!(
            !crate::diff::compare::handlers_equal(&old, &new, &[hid], &[hid]),
            "same bytecode but different captured_signals must not be equal"
        );
    }
}
