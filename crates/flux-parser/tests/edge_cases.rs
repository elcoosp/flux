//! Edge cases: empty input, boundary values, Unicode, and span fidelity.

use flux_parser::{BlockItem, Decl, Expr, ExprKind, StrPart, parse};

const FILE_ID: u32 = 42;

fn ok(source: &str) -> flux_parser::Ast {
    match parse(source, FILE_ID, "edge.flux") {
        Ok(ast) => ast,
        Err(error) => panic!("{}", error.render()),
    }
}

#[test]
fn empty_source_parses_to_zero_declarations() {
    assert!(ok("").decls.is_empty());
}

#[test]
fn whitespace_and_comment_only_source_parses_to_zero_declarations() {
    assert!(ok("\n  // just a comment\n\n").decls.is_empty());
}

#[test]
fn comments_between_declarations_are_skipped() {
    let ast = ok("// one\ncomponent A { Text(\"a\") }\n// two\ncomponent B { Text(\"b\") }\n");
    assert_eq!(ast.decls.len(), 2);
}

#[test]
fn i64_min_and_max_literals_are_accepted() {
    let source = format!(
        "component A {{ state lo: Int = {} state hi: Int = {} }}",
        i64::MIN + 1,
        i64::MAX
    );
    let ast = ok(&source);
    let Decl::Component(decl) = &ast.decls[0] else {
        panic!("expected a component");
    };
    let values: Vec<i64> = decl
        .body
        .items
        .iter()
        .filter_map(|item| match item {
            BlockItem::State(state) => match state.init.kind {
                ExprKind::Int(value) => Some(value),
                _ => None,
            },
            _ => None,
        })
        .collect();
    assert_eq!(values, vec![i64::MIN + 1, i64::MAX]);
}

#[test]
fn unicode_string_contents_round_trip_their_text() {
    let ast = ok("component Cafe { Text(\"héllo → 世界 🎉\") }");
    let Decl::Component(decl) = &ast.decls[0] else {
        panic!("expected a component");
    };
    assert_eq!(decl.name.name, "Cafe");
    let BlockItem::Expr(Expr {
        kind: ExprKind::Call { args, .. },
        ..
    }) = &decl.body.items[0]
    else {
        panic!("expected a call");
    };
    let ExprKind::Str(parts) = &args[0].value().kind else {
        panic!("expected a string");
    };
    assert!(matches!(&parts[0], StrPart::Text(text) if text == "héllo → 世界 🎉"));
}

#[test]
fn a_span_after_multibyte_text_still_indexes_the_original_bytes() {
    let source = "component A { Text(\"→→→\") }\ncomponent Bee { Text(\"b\") }";
    let ast = ok(source);
    let span = ast.decls[1].span();
    assert_eq!(
        &source[span.start as usize..span.end as usize][..13],
        "component Bee"
    );
}

#[test]
fn every_declaration_span_carries_the_requested_file_id() {
    let ast = ok("component A { Text(\"a\") }\nfn f() -> Int { 1 }");
    assert!(ast.decls.iter().all(|decl| decl.span().file_id == FILE_ID));
}

#[test]
fn escaped_braces_and_quotes_do_not_start_an_interpolation() {
    let ast = ok("component A { Text(\"a \\{ b \\\" c\") }");
    let Decl::Component(decl) = &ast.decls[0] else {
        panic!("expected a component");
    };
    let BlockItem::Expr(Expr {
        kind: ExprKind::Call { args, .. },
        ..
    }) = &decl.body.items[0]
    else {
        panic!("expected a call");
    };
    let ExprKind::Str(parts) = &args[0].value().kind else {
        panic!("expected a string");
    };
    assert_eq!(parts.len(), 1);
    assert!(matches!(&parts[0], StrPart::Text(text) if text == "a { b \" c"));
}

#[test]
fn an_empty_string_literal_has_no_parts() {
    let ast = ok("component A { Text(\"\") }");
    let Decl::Component(decl) = &ast.decls[0] else {
        panic!("expected a component");
    };
    let BlockItem::Expr(Expr {
        kind: ExprKind::Call { args, .. },
        ..
    }) = &decl.body.items[0]
    else {
        panic!("expected a call");
    };
    assert!(matches!(&args[0].value().kind, ExprKind::Str(parts) if parts.is_empty()));
}

#[test]
fn an_empty_list_literal_parses() {
    let ast = ok("component A { state xs: List[Int] = [] }");
    let Decl::Component(decl) = &ast.decls[0] else {
        panic!("expected a component");
    };
    let BlockItem::State(state) = &decl.body.items[0] else {
        panic!("expected a state declaration");
    };
    assert!(matches!(&state.init.kind, ExprKind::List(items) if items.is_empty()));
}

#[test]
fn an_empty_component_body_parses() {
    let ast = ok("component A { }");
    let Decl::Component(decl) = &ast.decls[0] else {
        panic!("expected a component");
    };
    assert!(decl.body.items.is_empty());
}

fn nested(depth: usize) -> String {
    let mut source = String::from("component A { ");
    for _ in 0..depth {
        source.push_str("Column { ");
    }
    source.push_str("Text(\"deep\") ");
    for _ in 0..depth {
        source.push_str("} ");
    }
    source.push('}');
    source
}

#[test]
fn nesting_up_to_the_documented_maximum_parses() {
    // 15 `Column` blocks plus the component brace is exactly the documented
    // limit of 16.
    assert_eq!(ok(&nested(15)).decls.len(), 1);
}

#[test]
fn nesting_past_the_maximum_is_reported_instead_of_overflowing_the_stack() {
    let error = parse(&nested(32), FILE_ID, "edge.flux").expect_err("must be rejected");
    assert!(
        error.message.contains("exceeds the maximum depth"),
        "message was {:?}",
        error.message
    );
}

#[test]
fn the_nesting_limit_error_suggests_extracting_a_component() {
    let error = parse(&nested(32), FILE_ID, "edge.flux").expect_err("must be rejected");
    assert!(
        error
            .hint
            .as_deref()
            .is_some_and(|hint| hint.contains("component")),
        "hint was {:?}",
        error.hint
    );
}

#[test]
fn a_long_operator_chain_is_left_associative() {
    let ast = ok("fn f() -> Int { 1 - 2 - 3 }");
    let Decl::Fn(decl) = &ast.decls[0] else {
        panic!("expected a function");
    };
    let BlockItem::Expr(Expr {
        kind: ExprKind::Binary { lhs, .. },
        ..
    }) = &decl.body.items[0]
    else {
        panic!("expected a binary expression");
    };
    assert!(
        matches!(lhs.kind, ExprKind::Binary { .. }),
        "`1 - 2 - 3` must group as `(1 - 2) - 3`"
    );
}

#[test]
fn multiplication_binds_tighter_than_addition() {
    let ast = ok("fn f() -> Int { 1 + 2 * 3 }");
    let Decl::Fn(decl) = &ast.decls[0] else {
        panic!("expected a function");
    };
    let BlockItem::Expr(Expr {
        kind: ExprKind::Binary { rhs, .. },
        ..
    }) = &decl.body.items[0]
    else {
        panic!("expected a binary expression");
    };
    assert!(
        matches!(rhs.kind, ExprKind::Binary { .. }),
        "`1 + 2 * 3` must group as `1 + (2 * 3)`"
    );
}

#[test]
fn a_windows_line_ending_source_reports_locations_on_the_right_line() {
    let error = parse("component A {\r\n  state 9 = 1\r\n}", FILE_ID, "edge.flux")
        .expect_err("must not parse");
    assert_eq!(error.location.line, 2);
}
