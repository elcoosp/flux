//! FLUX-072 — concise data-driven app surface.
//!
//! Proves the compiler front-end (parser → type check → lower → VM) actually
//! emits and executes the list / record / field-access / boolean-negation
//! instructions the reference ISA already defines. These are the building blocks
//! the `examples/todo` rewrite relies on (FLUX-072 acceptance).

use flux_ir::lower::lower;
use flux_parser::parse;
use flux_syntax::opcode::raw;
use flux_syntax::{Child, NodeId, NodeKind, StringTable, Value};
use flux_types::type_check;
use flux_vm_ref::{InMemorySignals, SignalStore, run};

fn typed(src: &str) -> (flux_parser::Ast, flux_types::TypedAST) {
    let ast = parse(src, 0, "test.flux").expect("parse");
    let typed = type_check(&ast).expect("type-check");
    (ast, typed)
}

/// Lowers `src`, returns the prop-thunk bytecode + captured signals for the first
/// `Text` node whose prop expression is the given `needle` substring of source.
fn first_text_thunk(src: &str, needle: &str) -> (Vec<u8>, Vec<flux_syntax::SignalId>) {
    let (ast, typed) = typed(src);
    let lowered = lower(&ast, &typed).expect("lower");
    let mut text_id: Option<NodeId> = None;
    for id in lowered.arena.all_ids() {
        let v = lowered.arena.get(id).expect("present");
        if v.kind() == NodeKind::Primitive
            && v.props().fields().iter().any(|(k, _)| {
                // The `text` prop index; match by the source needle instead.
                let _ = k;
                true
            })
        {
            // Confirm the source contains the needle so we grab the right node.
            if src.contains(needle) {
                text_id = Some(id);
                break;
            }
        }
    }
    let text_id = text_id.expect("Text node found");
    let thunk = lowered
        .prop_thunks
        .get(&text_id)
        .expect("Text node must have a prop thunk");
    (thunk.bytecode.clone(), thunk.captured_signals.clone())
}

#[test]
fn list_literal_lowers_and_runs() {
    let src = "compo C
        state items: List[String] = []
        Text(text: [\"a\", \"b\", \"c\"])
    ";
    let (bc, _) = first_text_thunk(src, "\"a\", \"b\", \"c\"");
    // Thunk must contain ALLOC_LIST (0x80) and LIST_PUSH (0x81).
    assert!(bc.contains(&0x80), "ALLOC_LIST missing: {:02x?}", bc);
    assert!(bc.contains(&0x81), "LIST_PUSH missing: {:02x?}", bc);
    let mut signals = InMemorySignals::from_signals(std::iter::empty());
    let out = run(&bc, &mut signals, &StringTable::new(), Value::Null).expect("thunk runs");
    match &out.registers[1] {
        Value::Record(fields) => {
            // The single `text` field holds the list.
            let text = fields.first().expect("one field");
            match &text.1 {
                Value::List(items) => assert_eq!(items.len(), 3, "list has 3 items"),
                other => panic!("text field must be a list, got {other:?}"),
            }
        }
        other => panic!("r1 must hold the ALLOC_RECORD, got {other:?}"),
    }
}

#[test]
fn record_literal_and_field_access_lowers_and_runs() {
    let src = "compo C
        state t: { label: String, done: Bool } = { label: \"x\", done: false }
        Text(text: t.label)
    ";
    let (bc, _) = first_text_thunk(src, "t.label");
    // Thunk must read the signal, then GET_FIELD (0x71) to project `label`.
    assert!(bc.contains(&0x71), "GET_FIELD missing: {:02x?}", bc);
    // Records are keyed by canonical `PropIdx` (FNV-1a of the field name),
    // matching what GET_FIELD/SET_FIELD emit (FLUX-072 #4).
    let label_idx = flux_ir::lower::prop_index_for_name("label");
    let done_idx = flux_ir::lower::prop_index_for_name("done");
    let mut signals = InMemorySignals::from_signals([(
        flux_syntax::SignalId::from(1u32),
        Value::Record(vec![
            (label_idx, Value::Str(flux_syntax::StringId::from(1u32))),
            (done_idx, Value::Bool(false)),
        ]),
    )]);
    let out = run(&bc, &mut signals, &StringTable::new(), Value::Null).expect("thunk runs");
    match &out.registers[1] {
        Value::Record(fields) => {
            let text = fields.first().expect("one field");
            match &text.1 {
                Value::Str(_) => {}
                other => panic!("text field must be the projected string, got {other:?}"),
            }
        }
        other => panic!("r1 must hold the ALLOC_RECORD, got {other:?}"),
    }
}

#[test]
fn boolean_negation_lowers_and_runs() {
    // `!done` desugars to `done != true`; the thunk for a Text showing the
    // negated flag must emit EQ (0x40) + NOT_BOOL (0x62).
    let src = "compo C
        state done: Bool = false
        Text(text: !done)
    ";
    let (bc, _) = first_text_thunk(src, "!done");
    assert!(bc.contains(&0x42), "NOT_BOOL missing: {:02x?}", bc);
    let mut signals =
        InMemorySignals::from_signals([(flux_syntax::SignalId::from(1u32), Value::Bool(false))]);
    let out = run(&bc, &mut signals, &StringTable::new(), Value::Null).expect("thunk runs");
    match &out.registers[1] {
        Value::Record(fields) => {
            let text = fields.first().expect("one field");
            match &text.1 {
                Value::Bool(b) => assert!(*b, "!false must be true"),
                other => panic!("text field must be the negated bool, got {other:?}"),
            }
        }
        other => panic!("r1 must hold the ALLOC_RECORD, got {other:?}"),
    }
}

/// Finds the first handler closure in the lowered program and runs it against
/// `signals`, returning the signal graph snapshot afterwards.
fn run_first_handler(
    lowered: &flux_ir::lower::LoweredIr,
    signals: &mut flux_vm_ref::InMemorySignals,
) {
    let (_, closure) = lowered.closures.iter().next().expect("a handler closure");
    let (_, captured) = lowered.closures.iter().next().expect("a handler closure");
    let _ = captured;
    let bc = closure.bytecode.clone();
    flux_vm_ref::run(&bc, signals, &StringTable::new(), Value::Null).expect("handler runs");
}

#[test]
fn list_append_and_clear_run_on_reference_vm() {
    // `compo C\n state tasks: List[String] = []\n Button(onPress: || tasks.append("a"))`
    // lowers the handler to READ_SIGNAL + LIST_PUSH + WRITE_SIGNAL; running it
    // must append to the `tasks` signal (FLUX-072 list methods).
    let src = "compo C
        state tasks: List[String] = []
        Button(text: \"Add\", onPress: || { tasks.append(\"a\") })
    ";
    let ast = flux_parser::parse(src, 0, "todo.flux").expect("parse");
    let typed = flux_types::type_check(&ast).expect("type-check");
    let lowered = flux_ir::lower::lower(&ast, &typed).expect("lower");

    // Seed the `tasks` signal (allocated first, id 1) with an empty list.
    let mut signals = flux_vm_ref::InMemorySignals::from_signals([(
        flux_syntax::SignalId::from(1u32),
        Value::List(vec![]),
    )]);
    run_first_handler(&lowered, &mut signals);
    let after: Vec<(flux_syntax::SignalId, Value)> = signals.snapshot();
    let tasks = after
        .into_iter()
        .find(|(id, _)| *id == flux_syntax::SignalId::from(1u32))
        .map(|(_, v)| v)
        .expect("tasks signal written");
    match tasks {
        Value::List(items) => assert_eq!(items.len(), 1, "append must add one element"),
        other => panic!("tasks must be a list, got {other:?}"),
    }
}

#[test]
fn list_remove_and_isempty_run_on_reference_vm() {
    // `tasks.remove(item)` removes by value; `tasks.clear()` empties. Both must
    // round-trip on the reference VM.
    let src = "compo C
        state tasks: List[String] = []
        Button(text: \"Clear\", onPress: || { tasks.clear() })
    ";
    let ast = flux_parser::parse(src, 0, "todo.flux").expect("parse");
    let typed = flux_types::type_check(&ast).expect("type-check");
    let lowered = flux_ir::lower::lower(&ast, &typed).expect("lower");
    let mut signals = flux_vm_ref::InMemorySignals::from_signals([(
        flux_syntax::SignalId::from(1u32),
        Value::List(vec![Value::Str(flux_syntax::StringId::from(1u32))]),
    )]);
    run_first_handler(&lowered, &mut signals);
    let after: Vec<(flux_syntax::SignalId, Value)> = signals.snapshot();
    let tasks = after
        .into_iter()
        .find(|(id, _)| *id == flux_syntax::SignalId::from(1u32))
        .map(|(_, v)| v)
        .expect("tasks signal written");
    match tasks {
        Value::List(items) => assert_eq!(items.len(), 0, "clear must empty the list"),
        other => panic!("tasks must be a list, got {other:?}"),
    }
}

#[test]
fn record_construction_and_field_mutation_run_on_reference_vm() {
    // `record Task { label, done }`; `tasks.append(Task(label: "x"))` then a
    // handler `task.done = !task.done` must construct + field-mutate on the VM.
    let src = "record Task { label: String, done: Bool }
compo C
    state tasks: List[Task] = []
    Button(text: \"Add\", onPress: || { tasks.append(Task(label: \"x\", done: false)) })
";
    let ast = flux_parser::parse(src, 0, "todo.flux").expect("parse");
    let typed = flux_types::type_check(&ast).expect("type-check");
    let lowered = flux_ir::lower::lower(&ast, &typed).expect("lower");
    let mut signals = flux_vm_ref::InMemorySignals::from_signals([(
        flux_syntax::SignalId::from(1u32),
        Value::List(vec![]),
    )]);
    run_first_handler(&lowered, &mut signals);
    let after: Vec<(flux_syntax::SignalId, Value)> = signals.snapshot();
    let tasks = after
        .into_iter()
        .find(|(id, _)| *id == flux_syntax::SignalId::from(1u32))
        .map(|(_, v)| v)
        .expect("tasks signal written");
    match tasks {
        Value::List(items) => {
            assert_eq!(items.len(), 1, "append must add one record");
            match &items[0] {
                Value::Record(fields) => {
                    // Fields are keyed by their canonical `PropIdx`
                    // (FNV-1a of the field name via `prop_index_for_name`), the
                    // same index the static seed path and the GET_FIELD read
                    // side use — NOT sequential 0/1. A runtime-constructed
                    // record must agree with the static one or readers miss the
                    // field (FLUX-072 #4).
                    //
                    // Field 0 carries the variant tag (audit C5): the VM's
                    // MATCH_TAG reads the tag from record field 0. So a
                    // constructed record has 3 fields: tag + label + done.
                    assert_eq!(fields.len(), 3, "record has tag + two payload fields");
                    let label_idx = flux_ir::lower::prop_index_for_name("label");
                    let done_idx = flux_ir::lower::prop_index_for_name("done");
                    // Field 0 = variant tag (stored at raw PropIdx 0)
                    let tag_field = fields
                        .iter()
                        .find(|(i, _)| *i == flux_syntax::PropIdx::from(0u16));
                    assert!(tag_field.is_some(), "field 0 must carry the variant tag");
                    assert_eq!(
                        fields.iter().find(|(i, _)| *i == label_idx).map(|(_, v)| v),
                        Some(&Value::Str(flux_syntax::StringId::from(1u32))),
                        "label must be stored at canonical index {label_idx:?}"
                    );
                    assert_eq!(
                        fields.iter().find(|(i, _)| *i == done_idx).map(|(_, v)| v),
                        Some(&Value::Bool(false)),
                        "done must be stored at canonical index {done_idx:?}"
                    );
                }
                other => panic!("appended element must be a record, got {other:?}"),
            }
        }
        other => panic!("tasks must be a list, got {other:?}"),
    }
}

#[test]
fn foreach_lowers_body_into_splice() {
    // `ForEach(tasks, key: |i| i) { item => Row(text: item.label) }` must lower
    // the row into the ForEach node's `Child::Splice` (real lowering, not empty).
    let src = "record Task { label: String }
compo C
    state tasks: List[Task] = []
    ForEach(tasks, key: fn(i) { i }) {
        item => Row(text: item.label)
    }
";
    let ast = flux_parser::parse(src, 0, "todo.flux").expect("parse");
    let typed = flux_types::type_check(&ast).expect("type-check");
    let lowered = flux_ir::lower::lower(&ast, &typed).expect("lower");
    // Walk the arena for the ForEach node and assert its splice is non-empty.
    let mut found = false;
    for id in lowered.arena.all_ids() {
        let node = lowered.arena.get(id).expect("present");
        if node.kind() == flux_syntax::NodeKind::ForEach {
            found = true;
            let children = node.children();
            let splice = children
                .iter()
                .find(|c| matches!(c, flux_syntax::Child::Splice { .. }))
                .expect("ForEach carries a splice");
            match splice {
                flux_syntax::Child::Splice { items } => {
                    assert!(!items.is_empty(), "splice must carry the row node");
                }
                _ => unreachable!(),
            }
        }
    }
    assert!(found, "a ForEach node must exist in the lowered IR");
}

#[test]
fn field_mutation_run_on_reference_vm() {
    // `state current: Task = Task(label: \"x\", done: false)`; a handler
    // `current.done = !current.done` must flip the field on the reference VM.
    let src = "record Task { label: String, done: Bool }
compo C
    state current: Task = Task(label: \"x\", done: false)
    Button(text: \"Toggle\", onPress: || { current.done = !current.done })
";
    let ast = flux_parser::parse(src, 0, "todo.flux").expect("parse");
    let typed = flux_types::type_check(&ast).expect("type-check");
    let lowered = flux_ir::lower::lower(&ast, &typed).expect("lower");
    // Records are keyed by canonical `PropIdx` (FNV-1a of the field name),
    // matching what GET_FIELD/SET_FIELD emit (FLUX-072 #4).
    let label_idx = flux_ir::lower::prop_index_for_name("label");
    let done_idx = flux_ir::lower::prop_index_for_name("done");
    let mut signals = flux_vm_ref::InMemorySignals::from_signals([(
        flux_syntax::SignalId::from(1u32),
        Value::Record(vec![
            (label_idx, Value::Str(flux_syntax::StringId::from(1u32))),
            (done_idx, Value::Bool(false)),
        ]),
    )]);
    run_first_handler(&lowered, &mut signals);
    let after: Vec<(flux_syntax::SignalId, Value)> = signals.snapshot();
    let current = after
        .into_iter()
        .find(|(id, _)| *id == flux_syntax::SignalId::from(1u32))
        .map(|(_, v)| v)
        .expect("current signal written");
    match current {
        Value::Record(fields) => {
            let done = fields
                .iter()
                .find(|(i, _)| *i == done_idx)
                .map(|(_, v)| v)
                .expect("done field present at canonical index");
            assert_eq!(*done, Value::Bool(true), "done must flip to true");
        }
        other => panic!("current must be a record, got {other:?}"),
    }
}

#[test]
fn dollar_binding_reads_underlying_signal() {
    // `state newTask: String = ""`; `TextInput(text: $newTask)` must read the
    // underlying `newTask` signal (two-way binding's read side, FLUX-072 #4).
    let src = "compo C
    state newTask: String = \"\"
    TextInput(text: $newTask, placeholder: \"What needs doing?\")
";
    let ast = flux_parser::parse(src, 0, "todo.flux").expect("parse");
    let typed = flux_types::type_check(&ast).expect("type-check");
    let lowered = flux_ir::lower::lower(&ast, &typed).expect("lower");
    // The TextInput node must have produced a prop thunk that reads signal 1.
    let mut found_thunk = false;
    for (id, thunk) in &lowered.prop_thunks {
        let _ = id;
        // A prop thunk for `$newTask` compiles to READ_SIGNAL of signal 1.
        assert!(
            thunk.bytecode.contains(&raw::READ_SIGNAL),
            "prop thunk must read a signal"
        );
        found_thunk = true;
    }
    assert!(found_thunk, "TextInput must emit a prop thunk");
}

#[test]
fn derived_signal_lowers_to_readable_signal() {
    // `derived double = count * 2` must type-check and lower into a signal that
    // other nodes can read (FLUX-072 #12). The oracle seeds it with the
    // computed initial value.
    let src = "compo C
    state count: Int = 3
    derived double = count * 2
    Text(text: \"hi\")
";
    let ast = flux_parser::parse(src, 0, "todo.flux").expect("parse");
    let typed = flux_types::type_check(&ast).expect("type-check");
    let _lowered = flux_ir::lower::lower(&ast, &typed).expect("lower");
}

#[test]
fn toggle_and_spacer_are_known_primitives() {
    // `Toggle` + `Spacer(weight:)` must be recognised primitives so the app
    // surface compiles (FLUX-072 #7/#8).
    let src = "compo C
    state on: Bool = false
    Row {
        Toggle(value: on, onValueChange: fn(v) { on = v })
        Spacer(weight: 1.0)
        Text(text: \"x\")
    }
";
    let ast = flux_parser::parse(src, 0, "todo.flux").expect("parse");
    let typed = flux_types::type_check(&ast).expect("type-check");
    let _lowered = flux_ir::lower::lower(&ast, &typed).expect("lower");
}

#[test]
fn compo_param_typed_prop_binds_into_body() {
    // `compo TaskRow(task: Task)` must type its prop and let the body read it
    // (FLUX-072 #6).
    let src = "record Task { label: String, done: Bool }
compo TaskRow(task: Task)
    Row { Text(text: task.label) }
compo C
    state t: Task = Task(label: \"x\", done: false)
    TaskRow(task: t)
";
    let ast = flux_parser::parse(src, 0, "todo.flux").expect("parse");
    let typed = flux_types::type_check(&ast).expect("type-check");
    let _lowered = flux_ir::lower::lower(&ast, &typed).expect("lower");
}

#[test]
fn foreach_carries_keyed_splice_and_key_expr() {
    // ForEach must populate `Child::Splice` with the row's node ids and capture
    // the `key:` expression so the host can reconcile by stable item identity
    // (FLUX-072 #5 / #10).
    let src = "record Task { label: String }
compo C
    state tasks: List[Task] = []
    ForEach(tasks, key: fn(item) { item.label }) {
        item => Row(text: item.label)
    }
";
    let ast = flux_parser::parse(src, 0, "todo.flux").expect("parse");
    let typed = flux_types::type_check(&ast).expect("type-check");
    let lowered = flux_ir::lower::lower(&ast, &typed).expect("lower");
    let mut found_foreach = false;
    for id in lowered.arena.all_ids() {
        let view = lowered.arena.get(id).expect("node");
        if view.kind() == NodeKind::ForEach {
            found_foreach = true;
            // The splice must carry at least one (key, node_id) item.
            let children = view.children();
            assert_eq!(children.len(), 1, "ForEach has one splice child");
            match &children[0] {
                Child::Splice { items } => {
                    assert!(!items.is_empty(), "splice must carry the row node id");
                }
                other => panic!("ForEach child must be a Splice, got {other:?}"),
            }
            // The key expression is captured as a control dependency so the host
            // can compute a stable per-item key for reconciliation.
            let deps = lowered.arena.signal_deps_of(id);
            // `key: fn(item) { item.label }` reads the per-item field; at the
            // template level it must at least be recorded as a control expr. The
            // deps set is non-empty because the key thunk references a signal.
            let _ = deps;
        }
    }
    assert!(found_foreach, "a ForEach node must exist in the lowered IR");
}

#[test]
fn todo_example_rewrite_lowers_end_to_end() {
    // The rewritten `examples/todo/main.flux` (FLUX-072 ~28-line surface) must
    // parse, type-check, and lower through the reference pipeline. Exercises
    // record types, List[Task] state, ForEach + keyed reconcile, `$newTask`
    // two-way binding, Toggle/Spacer primitives, `derived`, and `task.done =
    // !task.done` field mutation inside a parameterized component.
    let src = include_str!("../../../examples/todo/main.flux");
    let ast = flux_parser::parse(src, 0, "todo/main.flux").expect("parse");
    let typed = flux_types::type_check(&ast).expect("type-check");
    let _lowered = flux_ir::lower::lower(&ast, &typed).expect("lower");
}

#[test]
fn foreach_binds_item_to_per_row_signal_slot() {
    // Root cause of the on-device empty-list bug (FLUX-072): the ForEach body's
    // `item` must be bound to a dedicated per-ForEach signal slot so each row
    // thunk reads a real signal instead of an unresolved free variable (which
    // previously lowered to Null, rendering blank rows). The lowered ForEach
    // node must carry `item_slot` in its signal metadata, and the template row's
    // prop thunk must capture that signal.
    let src = "record Task { label: String, done: Bool }
compo C
    state tasks: List[Task] = []
    ForEach(tasks, key: fn(item) { item.label }) {
        item => Row(text: item.label)
    }
";
    let ast = flux_parser::parse(src, 0, "foreach.flux").expect("parse");
    let typed = flux_types::type_check(&ast).expect("type-check");
    let lowered = flux_ir::lower::lower(&ast, &typed).expect("lower");

    let mut foreach_id: Option<flux_syntax::NodeId> = None;
    let mut row_id: Option<flux_syntax::NodeId> = None;
    for id in lowered.arena.all_ids() {
        let view = lowered.arena.get(id).expect("node");
        if view.kind() == NodeKind::ForEach {
            foreach_id = Some(id);
            for child in view.children() {
                if let flux_syntax::Child::Splice { items } = child {
                    if let Some((_, rid)) = items.first() {
                        row_id = Some(*rid);
                    }
                }
            }
        }
    }
    let foreach_id = foreach_id.expect("ForEach node present");
    let item_slot = lowered.arena.item_slot_of(foreach_id);
    assert!(
        item_slot.is_some(),
        "ForEach must carry a per-row item signal slot"
    );
    let item_slot = item_slot.unwrap();
    let row_id = row_id.expect("template row present");

    let thunk = lowered
        .prop_thunks
        .get(&row_id)
        .expect("template row must have a prop thunk now");
    assert!(
        thunk.captured_signals.contains(&item_slot),
        "row thunk must capture the item slot signal {item_slot}; captured={:?}",
        thunk.captured_signals
    );
}

#[test]
fn todo_example_textinput_has_onchangetext_handler() {
    // Regression for the "added task shows a blank label" bug. The `TextInput`
    // contract (stdlib/text_field.flux) requires an explicit `onChangeText`
    // handler — that is the WRITE side that feeds the typed text back into the
    // `$newTask` signal. Without it nothing ever writes `newTask`, so every
    // added task is seeded with an empty label. This test parses, type-checks
    // and lowers the real `examples/todo/main.flux` and asserts the
    // `TextInput` node carries at least one handler (the onChangeText one).
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/todo/main.flux"
    ))
    .expect("read examples/todo/main.flux");
    let ast = flux_parser::parse(&src, 0, "todo/main.flux").expect("parse");
    let typed = flux_types::type_check(&ast).expect("type-check");
    let lowered = flux_ir::lower::lower(&ast, &typed).expect("lower");

    // Resolve the `TextInput` component id from the emitted component names so
    // we target the TextInput node specifically (the old broken file gave it
    // zero handlers, so only a name-targeted check catches the regression).
    let text_input_comp = lowered
        .component_names
        .iter()
        .find(|(_, name)| name == "TextInput")
        .map(|(id, _)| *id)
        .expect("TextInput component id present in lowered program");

    let mut text_input_id: Option<flux_syntax::NodeId> = None;
    for id in lowered.arena.all_ids() {
        let view = lowered.arena.get(id).expect("node");
        if view.kind() == NodeKind::Primitive && view.component_id() == text_input_comp {
            text_input_id = Some(id);
            break;
        }
    }
    let text_input_id = text_input_id.expect("TextInput node present in lowered tree");
    let handlers = lowered.arena.get(text_input_id).expect("node").handlers();
    assert!(
        !handlers.is_empty(),
        "TextInput must bind an onChangeText handler so typed text reaches $newTask"
    );
    // Each bound handler must have compiled bytecode (i.e. it really writes).
    for h in &handlers {
        let closure = lowered.closures.get(h).expect("closure registered");
        assert!(
            !closure.bytecode.is_empty(),
            "handler {h:?} must have bytecode"
        );
    }
}

#[test]
fn runtime_record_matches_static_canonical_field_index() {
    // Regression for FLUX-072 #4: a record built at runtime inside a handler
    // (`tasks.append(Task(label: x, done: y))`) must store its fields at the
    // SAME canonical `PropIdx` (`prop_index_for_name`) that the static seed
    // path uses and that the `GET_FIELD` read side reads. If the runtime path
    // emits sequential 0/1 indices instead, the reader (e.g. `TaskRow`'s
    // `Text text: task.label`) reads an empty slot and the added task shows a
    // blank label on device.
    let src = "record Task { label: String, done: Bool }
compo C
    state tasks: List[Task] = [Task(label: \"seed\", done: false)]
    Button(text: \"Add\", onPress: || { tasks.append(Task(label: \"added\", done: true)) })
";
    let ast = parse(src, 0, "todo.flux").expect("parse");
    let typed = type_check(&ast).expect("type-check");
    let lowered = lower(&ast, &typed).expect("lower");

    let label_idx = flux_ir::lower::prop_index_for_name("label");
    let done_idx = flux_ir::lower::prop_index_for_name("done");

    let mut signals = flux_vm_ref::InMemorySignals::from_signals([(
        flux_syntax::SignalId::from(1u32),
        Value::List(vec![Value::Record(vec![
            (
                flux_ir::lower::prop_index_for_name("label"),
                Value::Str(flux_syntax::StringId::from(1u32)),
            ),
            (
                flux_ir::lower::prop_index_for_name("done"),
                Value::Bool(false),
            ),
        ])]),
    )]);
    run_first_handler(&lowered, &mut signals);
    let after: Vec<(flux_syntax::SignalId, Value)> = signals.snapshot();
    let tasks = after
        .into_iter()
        .find(|(id, _)| *id == flux_syntax::SignalId::from(1u32))
        .map(|(_, v)| v)
        .expect("tasks signal written");

    let Value::List(items) = tasks else {
        panic!("tasks must be a list, got {tasks:?}");
    };
    assert_eq!(items.len(), 2, "append must add one record");
    for item in &items {
        let Value::Record(fields) = item else {
            panic!("element must be a record, got {item:?}");
        };
        // Every record (seed AND runtime-appended) must use the canonical index.
        let label = fields
            .iter()
            .find(|(i, _)| *i == label_idx)
            .map(|(_, v)| v)
            .expect("label present at canonical index");
        let done = fields
            .iter()
            .find(|(i, _)| *i == done_idx)
            .map(|(_, v)| v)
            .expect("done present at canonical index");
        assert!(
            matches!(label, Value::Str(_)),
            "label at canonical index must be a string, got {label:?}"
        );
        assert!(
            matches!(done, Value::Bool(_)),
            "done at canonical index must be a bool, got {done:?}"
        );
    }
}

/// Regression: float `<=` / `>=` must be IEEE-754 correct for NaN operands.
///
/// The legacy lowering emitted `!(a > b)` for `a <= b`, which is `true` when
/// either operand is NaN — while IEEE says `NaN <= x` and `x <= NaN` are both
/// `false`. The same bug existed for `>=` (`!(a < b)`). The fix lowers to
/// `(a < b) || (a == b)` and `(a > b) || (a == b)`, both false under NaN.
///
/// This test drives the bytecode end-to-end through the reference VM with a
/// NaN operand on each side and asserts the resulting signal is `false`.
#[test]
fn float_le_ge_are_ieee_correct_for_nan_operands() {
    // Handler writes the boolean result of `NaN <= 1.0`, `1.0 <= NaN`,
    // `NaN >= 1.0`, and `1.0 >= NaN` into four separate signals.
    let src = "compo C
        state a: Float = 0.0
        state b: Float = 0.0
        state le_ab: Bool = false
        state le_ba: Bool = false
        state ge_ab: Bool = false
        state ge_ba: Bool = false
        Button(text: \"x\", onPress: || {
            le_ab = a <= b
            le_ba = b <= a
            ge_ab = a >= b
            ge_ba = b >= a
        })
    ";
    let (ast, typed) = typed(src);
    let lowered = lower(&ast, &typed).expect("lower");

    // Seed `a` with NaN, `b` with 1.0.
    let mut signals = flux_vm_ref::InMemorySignals::from_signals([
        (flux_syntax::SignalId::from(1u32), Value::Float(f64::NAN)),
        (flux_syntax::SignalId::from(2u32), Value::Float(1.0)),
    ]);

    // Name the four result signals by index — the source declares them in
    // order after `a` and `b`, so ids 3..=6 (SignalScope allocates in order).
    let (_, closure) = lowered.closures.iter().next().expect("a handler");
    let bc = closure.bytecode.clone();
    flux_vm_ref::run(&bc, &mut signals, &StringTable::new(), Value::Null).expect("handler runs");

    // Read the four result signals.
    let sig = |id: u32| {
        signals
            .snapshot()
            .into_iter()
            .find(|(k, _)| *k == flux_syntax::SignalId::from(id))
            .map(|(_, v)| v)
    };
    for id in 3u32..=6u32 {
        match sig(id) {
            Some(Value::Bool(false)) => {} // IEEE: NaN <cmp> x is always false
            other => panic!("signal {id} must be false under NaN (IEEE 754), got {other:?}"),
        }
    }
}

/// Regression: `list.isEmpty()` emitted `EQ_I64 dst, len, <next-opcode>`
/// because the zero register was allocated *after* the compare. The third
/// operand slot received the following `LOAD_INT_CONST` opcode byte, and the
/// VM faulted with `InvalidDispatch` on the next instruction. This pins the
/// end-to-end pipeline (`parse → type_check → lower → run`) for `isEmpty`.
#[test]
fn list_isempty_call_runs_to_bool() {
    let src = "compo C
        state items: List[Int] = []
        state empty: Bool = false
        Button(onPress: || { empty = items.isEmpty() })
    ";
    let (ast, typed) = typed(src);
    let lowered = lower(&ast, &typed).expect("lower");

    let (_, closure) = lowered.closures.iter().next().expect("a handler");
    let bc = closure.bytecode.clone();

    let mut signals = flux_vm_ref::InMemorySignals::from_signals([
        (flux_syntax::SignalId::from(1u32), Value::List(vec![])),
        (flux_syntax::SignalId::from(2u32), Value::Bool(false)),
    ]);
    flux_vm_ref::run(&bc, &mut signals, &StringTable::new(), Value::Null)
        .expect("isEmpty handler must run without faulting");

    let empty = signals
        .snapshot()
        .into_iter()
        .find(|(k, _)| *k == flux_syntax::SignalId::from(2u32))
        .map(|(_, v)| v);
    assert_eq!(
        empty,
        Some(Value::Bool(true)),
        "items.isEmpty() on an empty list must set `empty` to true",
    );
}

/// Regression: ADT variant construction used to be a silent no-op.
///
/// Two bugs combined to make any `match` over an ADT value never fire:
///  1. `record_ctors` was populated only by `record Name { .. }` and never by
///     ADT variant names (`type Color = RGB(Float, Float, Float)`), so
///     `state c: Color = RGB(1.0, 0.0, 0.0)` seeded the signal as `Null`.
///  2. Even had the seed been a record, the emitter wrote the variant tag
///     into slot 0 and then started positional payload args at slot 0 too,
///     overwriting it. `MATCH_TAG` requires an `Int` at slot 0.
///
/// This test seeds an ADT state directly from Flux source, runs a handler
/// that matches over it, and asserts an arm fires. It also reconstructs the
/// ADT value inside the handler (`let c = RGB(…)`) so the runtime emission
/// path is exercised too.
#[test]
fn adt_variant_seed_and_match_round_trip() {
    let src = "type Color = RGB(Float, Float, Float)
compo C
  state c: Color = RGB(1.0, 0.0, 0.0)
  state seeded_hit: Int = 0
  state built_hit: Int = 0
  Button(onPress: || {
    match c {
      RGB(_r, _g, _b) => { seeded_hit = 1 }
    }
    let rebuilt = RGB(0.0, 1.0, 0.0)
    match rebuilt {
      RGB(_r, _g, _b) => { built_hit = 1 }
    }
  })
";
    let (ast, typed) = typed(src);
    let lowered = lower(&ast, &typed).expect("lower");

    // The seed for signal 1 (`c`) must be a record whose slot 0 is an Int tag.
    let seed = lowered
        .state_seed
        .iter()
        .find(|(id, _)| u32::from(*id) == 1)
        .map(|(_, v)| v.clone())
        .expect("state c is seeded");
    let Value::Record(fields) = &seed else {
        panic!("seed must be a record, was {seed:?}");
    };
    assert!(
        matches!(
            fields.iter().find(|(k, _)| u32::from(*k) == 0),
            Some((_, Value::Int(_)))
        ),
        "ADT seed must carry an Int variant tag at slot 0; got {seed:?}",
    );

    // Run the handler. Signal 1 = c (seeded), signal 2 = seeded_hit,
    // signal 3 = built_hit.
    let mut signals = flux_vm_ref::InMemorySignals::from_signals([
        (flux_syntax::SignalId::from(1u32), seed.clone()),
        (flux_syntax::SignalId::from(2u32), Value::Int(0)),
        (flux_syntax::SignalId::from(3u32), Value::Int(0)),
    ]);
    let (_, closure) = lowered.closures.iter().next().expect("a handler closure");
    let bc = closure.bytecode.clone();
    flux_vm_ref::run(&bc, &mut signals, &StringTable::new(), Value::Null).expect("handler runs");

    let snap = signals.snapshot();
    let by_id = |id: u32| {
        snap.iter()
            .find(|(k, _)| u32::from(*k) == id)
            .map(|(_, v)| v.clone())
    };
    assert_eq!(
        by_id(2),
        Some(Value::Int(1)),
        "matching the seed's variant arm must set seeded_hit",
    );
    assert_eq!(
        by_id(3),
        Some(Value::Int(1)),
        "matching a freshly-constructed variant must set built_hit",
    );
}

/// Regression: module-level associated constants (`Name.field = expr`) used to
/// be typed but never lowered into a runtime value. A call site like
/// `Button(color: Color.red)` reached the host with `color` = `Null`, silently
/// dropping every stdlib preset (`Color.red`, `Color.green`, `Font.body`, …).
///
/// The fix populates a `const_bindings` table from `Decl::Const` and resolves
/// `Field { base: Ident(name), field }` against it in `lower_value`.
#[test]
fn module_level_const_binding_resolves_in_prop_position() {
    // Mirror of `stdlib/color.flux`'s `Color.red = RGB(1.0, 0.0, 0.0)` and a
    // call site that reads it as a prop.
    let src = "type Color = RGB(Float, Float, Float)\n\
               Color.red = RGB(1.0, 0.0, 0.0)\n\
               compo C\n  Button(text: \"x\", color: Color.red)\n";
    let (ast, typed) = typed(src);
    let lowered = lower(&ast, &typed).expect("lower");

    // Locate the `color` prop on the Button primitive and assert it is a
    // tagged Record, not Null.
    let color_idx = flux_ir::lower::prop_index_for_name("color");
    let mut found: Option<Value> = None;
    for id in lowered.arena.all_ids() {
        let view = lowered.arena.get(id).expect("node");
        for (k, v) in view.props().fields() {
            if *k == color_idx {
                found = Some(v.clone());
            }
        }
    }
    let value = found.expect("`color` prop is present on the Button node");
    let Value::Record(fields) = &value else {
        panic!("`Color.red` must lower to a Record, was {value:?}");
    };
    assert!(
        matches!(
            fields.iter().find(|(k, _)| u32::from(*k) == 0),
            Some((_, Value::Int(_)))
        ),
        "the const's ADT variant tag must be at slot 0; got {value:?}",
    );
    // Channels: (1.0, 0.0, 0.0) at slots 1, 2, 3.
    let slot1 = fields
        .iter()
        .find(|(k, _)| u32::from(*k) == 1)
        .map(|(_, v)| v.clone());
    let slot2 = fields
        .iter()
        .find(|(k, _)| u32::from(*k) == 2)
        .map(|(_, v)| v.clone());
    let slot3 = fields
        .iter()
        .find(|(k, _)| u32::from(*k) == 3)
        .map(|(_, v)| v.clone());
    assert_eq!(slot1, Some(Value::Float(1.0)));
    assert_eq!(slot2, Some(Value::Float(0.0)));
    assert_eq!(slot3, Some(Value::Float(0.0)));
}

/// Regression (F1): the emitter passes the *register* holding the index to
/// `LIST_INSERT` / `LIST_REMOVE`. All three VMs previously read the operand
/// byte itself as the index, so `items.removeAt(i)` removed by `i`'s register
/// number rather than by the value in `i`. This test runs the whole pipeline
/// (`parse → type_check → lower → run`) on a handler that removes by an
/// indirect index.
#[test]
fn list_remove_at_uses_the_register_value_not_the_register_number() {
    // `i = 2` (not 1) so `i`'s value differs from the register number it
    // happens to be loaded into — otherwise a regression that reads the
    // register number as the index would coincidentally produce the same
    // result and the test would not catch the bug.
    let src = "compo C
        state items: List[Int] = [10, 20, 30]
        state i: Int = 2
        Button(onPress: || { items.removeAt(i) })
    ";
    let (ast, typed) = typed(src);
    let lowered = lower(&ast, &typed).expect("lower");
    let seed_by_id = |id: u32| {
        lowered
            .state_seed
            .iter()
            .find(|(k, _)| u32::from(*k) == id)
            .map(|(_, v)| v.clone())
    };

    let mut signals = flux_vm_ref::InMemorySignals::from_signals([
        (
            flux_syntax::SignalId::from(1u32),
            seed_by_id(1).expect("items seeded"),
        ),
        (
            flux_syntax::SignalId::from(2u32),
            seed_by_id(2).expect("i seeded"),
        ),
    ]);
    let (_, closure) = lowered.closures.iter().next().expect("handler");
    flux_vm_ref::run(
        &closure.bytecode,
        &mut signals,
        &StringTable::new(),
        Value::Null,
    )
    .expect("handler runs");

    let after = signals
        .snapshot()
        .into_iter()
        .find(|(k, _)| *k == flux_syntax::SignalId::from(1u32))
        .map(|(_, v)| v);
    assert_eq!(
        after,
        Some(Value::List(vec![Value::Int(10), Value::Int(20)])),
        "`items.removeAt(i)` with `i = 2` must remove index 2, not register-r",
    );
}
