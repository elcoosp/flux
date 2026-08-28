//! Integration test for Gap G3 closure: lowered string literals must resolve
//! from the packed arena's own [`flux_syntax::StringTable`].
//!
//! Before the fix, `ArenaBuilder::finish()` dropped the interner, so every
//! `Value::Str(id)` emitted by lowering pointed at an id missing from
//! `arena.string_table()` — the dev server, wire codec and Swift/Kotlin
//! adapters could not recover text. The fix threads the interner into the
//! arena (see `crates/flux-ir/src/builder.rs` and ADR
//! `docs/adr/flux018-string-table-gap.md`).

use flux_ir::lower;
use flux_parser::parse;
use flux_syntax::Value;
use flux_types::type_check;

/// A component with string-literal props must yield a populated arena table,
/// and every `Value::Str` id must resolve against it.
#[test]
fn lowered_string_props_resolve_from_arena_table() {
    let src = "compo Hello\n  state count: Int = 0\n  Text(\"hi\")\n  Button(text: \"inc\")\n";
    let ast = parse(src, 0, "hello.flux").expect("parse");
    let typed = type_check(&ast).expect("well-typed");
    let lowered = lower(&ast, &typed).expect("lowers");
    let arena = &lowered.arena;

    let mut resolved_any = false;
    for id in arena.all_ids() {
        let view = arena.get(id).expect("present");
        for (_, value) in view.props().fields() {
            if let Value::Str(sid) = value {
                let text = arena.string_table().resolve(*sid);
                assert!(
                    text.is_some(),
                    "Value::Str({sid}) must resolve from arena.string_table() (Gap G3)"
                );
                assert_ne!(
                    arena.string_table().len(),
                    0,
                    "arena string table must be populated, not empty"
                );
                resolved_any = true;
            }
        }
    }
    assert!(resolved_any, "expected at least one interned string prop");
}

/// A component with no string-literal props still finishes with a valid empty
/// table (no panic, no dangling interner).
#[test]
fn empty_arena_table_when_no_strings() {
    let src = "compo Box\n  Column()\n  Row()\n";
    let ast = parse(src, 0, "box.flux").expect("parse");
    let typed = type_check(&ast).expect("well-typed");
    let lowered = lower(&ast, &typed).expect("lowers");
    assert_eq!(lowered.arena.string_table().len(), 0);
}
