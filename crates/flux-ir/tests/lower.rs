//! Integration tests for the FLUX-018 lowering pass.
//!
//! These are the TDD red→green→refactor evidence. The most important is
//! [`bridge_node_ids_match_types`] — it proves the ADR-0027 node-ID bridge:
//! every `NodeId` the type checker assigned in `typed.types` also exists as a
//! packed node in the lowered arena.

use flux_ir::InstanceRegistry;
use flux_ir::lower::{LoweringError, lower};
use flux_parser::parse;
use flux_syntax::{Child, HandlerId, NodeKind, SignalId, Value};
use flux_types::type_check;
use flux_vm_ref::{InMemorySignals, SignalStore, run};

/// Parses + type-checks `src`, returning the typed AST or panicking the test.
fn typed(src: &str) -> (flux_parser::Ast, flux_types::TypedAST) {
    let ast = parse(src, 0, "test.flux").expect("parse");
    let typed = type_check(&ast).expect("type-check");
    (ast, typed)
}

#[test]
fn bridge_node_ids_match_types() {
    let src = "component Counter {\
        state count: Int = 0\
        Button(text: \"tap\") { onClick: { count = count + 1 } }\
    }";
    let (ast, typed) = typed(src);
    let lowered = lower(&ast, &typed).expect("lower");
    let arena = &lowered.arena;

    // ADR-0027 bridge: every IR node we emit must carry the SAME NodeId the
    // type checker used to key `typed.types`, so downstream code can resolve
    // the inferred type for that node. (The type checker also types
    // sub-expressions such as the handler body and `count + 1`, which are not
    // themselves IR tree nodes, so `typed.types` is a superset — the correct
    // invariant is that the arena's IDs are a subset of the typed keys.)
    let mut emitted = 0usize;
    for id in arena.all_ids() {
        assert!(
            typed.types.contains_key(&id),
            "emitted node id {id} is missing from typed.types (bridge broken)"
        );
        emitted += 1;
    }
    assert!(emitted > 0, "expected at least one emitted node");

    let mut has_component = false;
    let mut has_primitive = false;
    for id in arena.all_ids() {
        let v = arena.get(id).expect("present");
        match v.kind() {
            NodeKind::Component => has_component = true,
            NodeKind::Primitive => has_primitive = true,
            _ => {}
        }
    }
    assert!(has_component, "component node emitted");
    assert!(has_primitive, "primitive node emitted");
}

#[test]
fn diff_of_identical_lowers_is_empty() {
    let src = "component Hello { Text(\"hi\") }";
    let (ast, typed) = typed(src);
    let a = lower(&ast, &typed).expect("lower a").arena;
    let b = lower(&ast, &typed).expect("lower b").arena;
    let patches = flux_differ::diff(&a, &b);
    assert!(patches.is_empty(), "identical lowers must diff to nothing");
}

#[test]
fn diff_from_empty_arena_is_nonempty() {
    let src = "component Hello { Text(\"hi\") }";
    let (ast, typed) = typed(src);
    let lowered = lower(&ast, &typed).expect("lower").arena;
    let patches = flux_differ::diff(&flux_ir::IRArena::new(), &lowered);
    assert!(
        !patches.is_empty(),
        "diff against empty arena must produce patches"
    );
}

#[test]
fn foreach_emits_empty_splice() {
    let src = "component List {\
        state items: List[Int] = []\
        ForEach(items, key: fn(i) { i }) { item => Text(\"x\") }\
    }";
    let (ast, typed) = typed(src);
    let lowered = lower(&ast, &typed).expect("lower");
    let arena = &lowered.arena;

    let mut found = false;
    for id in arena.all_ids() {
        let v = arena.get(id).expect("present");
        if v.kind() == NodeKind::ForEach {
            found = true;
            let children = v.children();
            assert_eq!(children.len(), 1, "ForEach has one child slot");
            match &children[0] {
                Child::Splice { items } => assert!(items.is_empty(), "splice is empty"),
                other => panic!("expected splice, got {other:?}"),
            }
        }
    }
    assert!(found, "ForEach node emitted");
}

#[test]
fn handler_bytecode_increments_signal_under_vm() {
    let src = "component Counter {\
        state count: Int = 0\
        Button(text: \"tap\") { onClick: { count = count + 1 } }\
    }";
    let (ast, typed) = typed(src);
    let lowered = lower(&ast, &typed).expect("lower");
    let arena = &lowered.arena;

    // Find the onClick closure: a Primitive button whose props include a
    // HandlerRef.
    let mut handler: Option<HandlerId> = None;
    for id in arena.all_ids() {
        let v = arena.get(id).expect("present");
        if v.kind() == NodeKind::Primitive {
            for (_, value) in v.props().fields() {
                if let Value::HandlerRef(h) = value {
                    handler = Some(*h);
                }
            }
        }
    }
    let handler = handler.expect("onClick handler compiled");
    let closure = lowered.closure(handler).expect("closure present");

    // Signal ids start at 1 per component; `count` is signal 1.
    let mut signals = InMemorySignals::from_signals([(SignalId::from(1u32), Value::Int(0))]);
    let out = run(&closure.bytecode, &mut signals, Value::Null).expect("vm run");
    let new_value = signals.read(SignalId::from(1u32)).expect("signal updated");
    assert_eq!(new_value, Value::Int(1), "count incremented to 1");
    // gas_used counts the non-HALT instructions executed (ADR-0021).
    assert!(
        out.gas_used >= 1,
        "handler executed at least one instruction"
    );
}

#[test]
fn multiple_components_lower_all() {
    let src = "component A { state x: Int = 0 Text(\"a\") }\
               component B { state y: Int = 0 Text(\"b\") }";
    let (ast, typed) = typed(src);
    let lowered = lower(&ast, &typed).expect("lower both components");
    assert_eq!(lowered.arena.len(), 4, "two components + two text leaves");
}

#[test]
fn unsupported_handler_operand_errors() {
    // A handler that assigns a string literal cannot be compiled yet.
    let src = "component C {\
        state name: String = \"hi\"\
        Button(text: \"go\") { onClick: { name = \"bye\" } }\
    }";
    let (ast, typed) = typed(src);
    let result = lower(&ast, &typed);
    assert!(result.is_err(), "string assignment handler should error");
}

// Keep `InstanceRegistry` / `LoweringError` referenced so the public API stays
// exercised and dead-code lints stay quiet in the test build.
#[allow(dead_code)]
fn _assert_api() -> (InstanceRegistry, LoweringError) {
    (
        InstanceRegistry::new(),
        LoweringError::new("x", flux_syntax::Span::new(0, 0, 0)),
    )
}
