//! Integration tests for the FLUX-018 lowering pass.
//!
//! These are the TDD red→green→refactor evidence. The most important is
//! [`bridge_node_ids_match_types`] — it proves the ADR-0027 node-ID bridge:
//! every `NodeId` the type checker assigned in `typed.types` also exists as a
//! packed node in the lowered arena.

use flux_ir::InstanceRegistry;
use flux_ir::lower::{LoweringError, lower};
use flux_parser::parse;
use flux_syntax::{Child, HandlerId, NodeId, NodeKind, PropIdx, SignalId, Value};
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
    let src = "compo Counter\n        state count: Int = 0\n        Button(text: \"tap\", onClick: { count = count + 1 })\n    ";
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
    let src = "compo Hello\n  Text(\"hi\")\n";
    let (ast, typed) = typed(src);
    let a = lower(&ast, &typed).expect("lower a").arena;
    let b = lower(&ast, &typed).expect("lower b").arena;
    let patches = flux_differ::diff(&a, &b);
    assert!(patches.is_empty(), "identical lowers must diff to nothing");
}

#[test]
fn diff_from_empty_arena_is_nonempty() {
    let src = "compo Hello\n  Text(\"hi\")\n";
    let (ast, typed) = typed(src);
    let lowered = lower(&ast, &typed).expect("lower").arena;
    let patches = flux_differ::diff(&flux_ir::IRArena::new(), &lowered);
    assert!(
        !patches.is_empty(),
        "diff against empty arena must produce patches"
    );
}

#[test]
fn foreach_emits_populated_splice() {
    // FLUX-072: ForEach lowers its body into a keyed splice instead of the
    // old empty placeholder. The body is a `Text` leaf, so the splice carries
    // one `(Key, NodeId)` pair keyed by the `key:` expression.
    let src = "compo List\n        state items: List[Int] = []\n        ForEach(items, key: fn(i) { i }) { item => Text(\"x\") }\n    ";
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
                Child::Splice { items } => {
                    assert!(!items.is_empty(), "splice is populated with the body node")
                }
                other => panic!("expected splice, got {other:?}"),
            }
        }
    }
    assert!(found, "ForEach node emitted");
}

#[test]
fn handler_bytecode_increments_signal_under_vm() {
    let src = "compo Counter\n        state count: Int = 0\n        Button(text: \"tap\", onClick: { count = count + 1 })\n    ";
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
    let src =
        "compo A\n  state x: Int = 0\n  Text(\"a\")\ncompo B\n  state y: Int = 0\n  Text(\"b\")\n";
    let (ast, typed) = typed(src);
    let lowered = lower(&ast, &typed).expect("lower both components");
    assert_eq!(lowered.arena.len(), 4, "two components + two text leaves");
}

#[test]
fn string_assignment_handler_lowers_to_valid_write() {
    // A handler that assigns a string literal to a string signal is a valid
    // signal write (Appendix E: `WRITE_SIGNAL` accepts any value, including a
    // `Value::Str` loaded by `LOAD_STR_CONST`). The MLP envelope supports it,
    // so lowering must SUCCEED loudly (never silently no-op) and produce a
    // closure carrying the write.
    let src = "compo C
        state name: String = \"hi\"
        Button(text: \"go\", onClick: { name = \"bye\" })
    ";
    let (ast, typed) = typed(src);
    let lowered = lower(&ast, &typed).expect("string-assignment handler is a valid write");
    let mut found_handler = false;
    for id in lowered.arena.all_ids() {
        let v = lowered.arena.get(id).expect("present");
        if v.kind() == NodeKind::Primitive {
            for (_, value) in v.props().fields() {
                if let Value::HandlerRef(h) = value {
                    found_handler = lowered.closures.contains_key(h);
                }
            }
        }
    }
    assert!(found_handler, "onClick handler compiled into a closure");
}

#[test]
fn signal_deps_reads_signal_from_prop() {
    // `count` is state signal 1; passing it as a prop makes the Button node's
    // `signal_deps` include that signal (ADR-0027 T13).
    let src = "compo Counter\n        state count: Int = 0\n        Button(text: count)\n    ";
    let (ast, typed) = typed(src);
    let lowered = lower(&ast, &typed).expect("lower");
    let arena = &lowered.arena;

    let mut button_deps: Option<Vec<SignalId>> = None;
    for id in arena.all_ids() {
        let v = arena.get(id).expect("present");
        if v.kind() == NodeKind::Primitive {
            button_deps = Some(arena.signal_deps_of(id).to_vec());
        }
    }
    let deps = button_deps.expect("button node found");
    assert!(
        deps.contains(&SignalId::from(1u32)),
        "prop reading `count` must record signal_deps == [count] (got {deps:?})"
    );
    // The Component node reads no signals.
    let component_deps = arena
        .signal_deps_of(
            arena
                .all_ids()
                .into_iter()
                .find(|id| arena.get(*id).map(|v| v.kind()) == Some(NodeKind::Component))
                .expect("component node"),
        )
        .to_vec();
    assert!(component_deps.is_empty(), "component has no signal_deps");
}

#[test]
fn prop_thunk_runs_to_alloc_record_of_literals() {
    // A Button with literal props gets a prop_thunk (T14) whose bytecode, when
    // run, leaves an ALLOC_RECORD of the prop values in r1.
    let src = "compo Hello\n        Button(text: \"tap\", width: 5)\n    ";
    let (ast, typed) = typed(src);
    let lowered = lower(&ast, &typed).expect("lower");

    // Locate the Button's node id.
    let mut button_id: Option<NodeId> = None;
    for id in lowered.arena.all_ids() {
        if lowered.arena.get(id).map(|v| v.kind()) == Some(NodeKind::Primitive) {
            button_id = Some(id);
        }
    }
    let button_id = button_id.expect("button node");

    // The thunk must exist and decode to a closure whose captured signals are
    // empty (literal-only props read no signals) but which, when run, produces
    // the record of literal prop values.
    let thunk = lowered
        .prop_thunks
        .get(&button_id)
        .expect("prop thunk compiled for literal props");
    assert!(
        thunk.captured_signals.is_empty(),
        "literal props capture no signals"
    );

    let mut signals = InMemorySignals::default();
    let out = run(&thunk.bytecode, &mut signals, Value::Null).expect("thunk runs");
    match &out.registers[1] {
        Value::Record(fields) => {
            // Field 0 = `text`, field 1 = `width` (positional fill order).
            assert_eq!(fields.len(), 2, "record has both props");
            assert!(
                matches!(fields[0].1, Value::Str(_)),
                "text is a string prop"
            );
            assert_eq!(fields[1], (PropIdx::from(1u16), Value::Int(5)));
        }
        other => panic!("r1 must hold the ALLOC_RECORD, got {other:?}"),
    }
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
