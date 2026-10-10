//! Regression (round 32-33 parser work): the spec-documented guard syntax
//! (`s if s > 90 => "A"`, §18.5) must survive a full
//! `parse -> format -> parse` round trip. Without the parser fix, `format_source`
//! (which already knew how to emit `s if cond`) would emit syntax its own
//! parser rejected — a self-inconsistency.

#[test]
fn guard_syntax_round_trips_through_fmt() {
    let src = "compo Score\n  state score: Int = 95\n  Text(text: match score {\n    s if s > 90 => \"A\"\n    s if s > 80 => \"B\"\n    _ => \"C\"\n  })\n";
    let ast = flux_parser::parse(src, 0, "score.flux").expect("parses");
    let formatted = flux_parser::format_source(src, 0, "score.flux").expect("formats");
    // Re-parsing the formatted source must succeed and produce the same AST
    // (checked via Debug; ASTs are not PartialEq).
    let reparsed = flux_parser::parse(&formatted, 0, "score.flux")
        .expect("reparse of formatted source");
    assert_eq!(
        format!("{ast:?}"),
        format!("{reparsed:?}"),
        "guard arm AST must round-trip through `format_source`:\n--- formatted ---\n{formatted}"
    );
}
