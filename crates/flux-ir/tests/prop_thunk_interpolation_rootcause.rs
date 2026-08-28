//! Root-cause reproduction for the Android dev-mode interpolation regression.
//!
//! Lowers the exact `Counter` example the Android host renders and inspects the
//! `Text` node's compiled prop thunk. A correct interpolation thunk leaves an
//! `ALLOC_RECORD` of the interpolated text in `r1` (no `WRITE_SIGNAL`); a
//! mis-assigned thunk would be the button's increment handler
//! (`READ_SIGNAL` + `ADD_I64` + `WRITE_SIGNAL`).

use flux_ir::lower::lower;
use flux_parser::parse;
use flux_syntax::{NodeId, NodeKind, Value};
use flux_types::type_check;
use flux_vm_ref::{InMemorySignals, run};

fn typed(src: &str) -> (flux_parser::Ast, flux_types::TypedAST) {
    let ast = parse(src, 0, "test.flux").expect("parse");
    let typed = type_check(&ast).expect("type-check");
    (ast, typed)
}

#[test]
fn text_node_prop_thunk_interpolates_count_not_increments() {
    let src = "compo Counter
        state count: Int = 0
        Column {
            Text(text: \"tapped {count} times\")
            Button(text: \"Increment\") { onTap: { count = count + 1 } }
        }
    ";
    let (ast, typed) = typed(src);
    let lowered = lower(&ast, &typed).expect("lower");

    // Find the Text node (the one that owns the interpolated prop) and its
    // prop thunk.
    let mut text_id: Option<NodeId> = None;
    for id in lowered.arena.all_ids() {
        let v = lowered.arena.get(id).expect("present");
        if v.kind() == NodeKind::Primitive && v.props().fields().len() == 1 {
            text_id = Some(id);
        }
    }
    let text_id = text_id.expect("Text node found");

    let thunk = lowered
        .prop_thunks
        .get(&text_id)
        .expect("Text node must have a prop thunk");
    println!("TEXT prop_thunk bytecode = {:02x?}", thunk.bytecode);
    println!("TEXT prop_thunk captured = {:?}", thunk.captured_signals);

    // The interpolation thunk must NOT write a signal.
    assert!(
        !thunk.bytecode.contains(&0x11), // WRITE_SIGNAL opcode
        "Text prop thunk must not contain WRITE_SIGNAL (got increment handler): {:02x?}",
        thunk.bytecode
    );

    // Running it must leave an ALLOC_RECORD (not an Int) in r1.
    let mut signals =
        InMemorySignals::from_signals([(flux_syntax::SignalId::from(1u32), Value::Int(0))]);
    let out = run(&thunk.bytecode, &mut signals, Value::Null).expect("thunk runs");
    match &out.registers[1] {
        Value::Record(fields) => {
            println!("TEXT prop_thunk r1 record fields = {:?}", fields);
            assert!(
                !fields.is_empty(),
                "record holds the interpolated text field"
            );
        }
        other => panic!("r1 must hold the ALLOC_RECORD, got {other:?}"),
    }
}
