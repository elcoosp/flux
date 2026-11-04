//! Round-21 regressions for handler bytecode emission:
//! 1. `list.isEmpty()` used to emit a misaligned instruction stream (the
//!    `LOAD_INT_CONST` opcode byte was consumed as `EQ_I64`'s third operand).
//! 2. Positional variant-constructor args used to overwrite the MATCH_TAG tag
//!    slot (slot 0), making the variant unmatchable.
//! 3. `List + List` used to compile to `ADD_I64`, faulting at runtime with
//!    TypeMismatch instead of concatenating.

use flux_ir::lower::lower;
use flux_parser::parse;
use flux_syntax::opcode::raw;
use flux_syntax::{HandlerId, NodeKind, StringTable, Value};
use flux_types::type_check;
use flux_vm_ref::{InMemorySignals, run};

fn lower_src(src: &str) -> flux_ir::lower::LoweredIr {
    let ast = parse(src, 0, "test.flux").expect("parse");
    let typed = type_check(&ast).expect("type-check");
    lower(&ast, &typed).expect("lower")
}

fn first_handler_bytecode(lowered: &flux_ir::lower::LoweredIr) -> Vec<u8> {
    let (_, closure) = lowered
        .closures
        .iter()
        .find(|(id, _)| **id != HandlerId::from(0u32))
        .or_else(|| lowered.closures.iter().next())
        .map(|(id, c)| (*id, c.clone()))
        .expect("at least one closure");
    closure.bytecode
}

/// Seeds every signal the handler reads with an empty list, so list ops see
/// real `List` values (the bare VM run has no state materialisation step).
fn signals_seed_lists(code: &[u8]) -> InMemorySignals {
    let instrs = flux_vm_ref::decode_program(code).expect("decode");
    let mut seed = Vec::new();
    for instr in &instrs {
        if instr.opcode == flux_syntax::opcode::Opcode::ReadSignal {
            let id = instr.u32(1);
            seed.push((id, Value::List(vec![])));
        }
    }
    InMemorySignals::from_signals(seed)
}

/// `messages.isEmpty()` compiles to an ALIGNED program that runs and produces
/// the right boolean for an empty list.
#[test]
fn list_is_empty_compiles_to_aligned_correct_bytecode() {
    let src = concat!(
        "compo Inbox\n",
        "  state messages: List[String] = []\n",
        "  state empty: Bool = true\n",
        "  Button(text: \"check\", onPress: { empty = messages.isEmpty() })\n",
    );
    let lowered = lower_src(src);
    let code = first_handler_bytecode(&lowered);
    assert!(!code.is_empty(), "handler must compile to real bytecode");

    // The aligned stream must decode cleanly and RUN to HALT.
    let mut signals = signals_seed_lists(&code);
    let outcome = run(&code, &mut signals, &StringTable::new(), Value::Null)
        .expect("isEmpty handler must run to HALT");
    // `messages` is an empty list, so `empty` must be written `true`.
    let wrote_true = outcome
        .signals
        .iter()
        .any(|(_, v)| matches!(v, Value::Bool(true)));
    assert!(wrote_true, "isEmpty on an empty list must write true");
}

/// `shape = Circle(5.0)` inside a handler must keep the variant tag in slot 0
/// so `MATCH_TAG` can still dispatch; the payload goes to slot 1+.
#[test]
fn positional_variant_args_do_not_overwrite_the_tag_slot() {
    let src = concat!(
        "type Shape =\n",
        "  | Circle(Float)\n",
        "compo Board\n",
        "  state shape: Shape = Circle(1.0)\n",
        "  Button(text: \"set\", onPress: { shape = Circle(5.0) })\n",
    );
    let lowered = lower_src(src);
    let code = first_handler_bytecode(&lowered);
    let mut signals = InMemorySignals::default();
    let outcome = run(&code, &mut signals, &StringTable::new(), Value::Null)
        .expect("constructor handler must run");
    // Find the written record: its field 0 must still be the Circle tag int,
    // with the payload at the next slot.
    let record = outcome
        .signals
        .iter()
        .filter_map(|(_, v)| match v {
            Value::Record(fields) => Some(fields),
            _ => None,
        })
        .next()
        .expect("constructor writes a record");
    assert!(
        matches!(record.first().map(|(_, v)| v), Some(Value::Int(t)) if *t != 5 && *t >= 0),
        "slot 0 must hold the variant TAG, not the first positional payload: {record:?}"
    );
    assert!(
        record
            .iter()
            .any(|(_, v)| matches!(v, Value::Float(f) if *f == 5.0)),
        "the 5.0 payload must be present in a later slot: {record:?}"
    );
}

/// `List + List` must compile to LIST_CONCAT and run, not fault with
/// TypeMismatch from ADD_I64.
#[test]
fn list_concat_compiles_and_runs() {
    let src = concat!(
        "compo Feed\n",
        "  state items: List[String] = []\n",
        "  state more: List[String] = []\n",
        "  Button(text: \"m\", onPress: { items = items + more })\n",
    );
    let lowered = lower_src(src);
    let code = first_handler_bytecode(&lowered);
    assert!(
        code.contains(&raw::LIST_CONCAT),
        "List + List must emit LIST_CONCAT, got {code:?}"
    );
    let mut signals = signals_seed_lists(&code);
    run(&code, &mut signals, &StringTable::new(), Value::Null)
        .expect("list concat handler must run to HALT");
}

/// Generic user components lower as `NodeKind::Component` (not `Primitive`) —
/// the host's adapter resolution reads this discriminant.
#[test]
fn generic_user_component_lowers_as_component_node() {
    let src = concat!(
        "compo Boxed[T](value: T)\n",
        "  Text(text: \"v\")\n",
        "compo App\n",
        "  Boxed(value: 1)\n",
    );
    let lowered = lower_src(src);
    let mut kinds = Vec::new();
    for id in lowered.arena.all_ids() {
        if let Some(view) = lowered.arena.get(id) {
            kinds.push(view.kind());
        }
    }
    assert!(
        kinds.iter().any(|k| *k == NodeKind::Component),
        "a generic user compo call must lower as Component, got {kinds:?}"
    );
}

/// `content_address` used to panic on orphan nodes (packed but unreachable
/// from any root) because `final_ids` only visits root-reachable nodes; the
/// same module deliberately tolerates dangling child refs, so orphans must
/// degrade the same way instead of indexing the map.
#[test]
fn content_address_tolerates_orphan_nodes() {
    use flux_ir::{ArenaBuilder, Node};
    use flux_syntax::{ComponentId, NodeId, NodeKind, Props, Span};
    let mut b = ArenaBuilder::new();
    // Rooted node.
    b.pack(Node {
        id: NodeId::from(1u32),
        kind: NodeKind::Primitive,
        component_id: ComponentId::from(0u32),
        props: Props::from_fields(vec![]),
        children: vec![],
        handlers: vec![],
        span: Span::new(0, 0, 4),
    });
    // Orphan: packed, never referenced by any root.
    b.pack(Node {
        id: NodeId::from(2u32),
        kind: NodeKind::Primitive,
        component_id: ComponentId::from(1u32),
        props: Props::from_fields(vec![]),
        children: vec![],
        handlers: vec![],
        span: Span::new(0, 5, 9),
    });
    let mut arena = b.finish();
    // Must not panic; returns the remap table.
    let _ = arena.content_address();
}
