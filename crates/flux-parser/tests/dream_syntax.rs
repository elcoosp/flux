//! Acceptance tests for the indentation-delimited "dream" surface syntax
//! (Appendix B as revised by the FLUX-00X syntax ADR): `compo`, the `$` state
//! sigil, spaced-prop view calls, and `||` lambdas.

use flux_parser::{Arg, Ast, BlockItem, Decl, Expr, ExprKind, StateDecl, Type, TypeKindAst, parse};

const FILE_ID: u32 = 9;

fn parse_ok(source: &str) -> Ast {
    match parse(source, FILE_ID, "dream.flux") {
        Ok(ast) => ast,
        Err(error) => panic!("{}", error.render()),
    }
}

#[test]
fn dream_counter_parses() {
    let source = "compo Counter\n    $count: Int = 0\n\n    Column gap: 8.0\n        Text text: \"tapped {count} times\"\n        Button text: \"Increment\", onClick: || { count = count + 1 }\n";
    let ast = parse_ok(source);
    let Decl::Component(comp) = &ast.decls[0] else {
        panic!("expected a component");
    };
    assert_eq!(comp.name.name, "Counter");

    // First body item is the `$count` state declaration.
    let BlockItem::State(StateDecl { name, ty, .. }) = &comp.body.items[0] else {
        panic!("expected state declaration, got {:?}", comp.body.items[0]);
    };
    assert_eq!(name.name, "count");
    assert!(ty.is_some(), "state should carry its `Int` type");

    // Second body item is the `Column` view call with a spaced `gap` prop and
    // two indented children.
    let BlockItem::Expr(Expr {
        kind: ExprKind::Call {
            callee,
            args,
            trailing,
        },
        ..
    }) = &comp.body.items[1]
    else {
        panic!("expected a Column call, got {:?}", comp.body.items[1]);
    };
    let ExprKind::Ident(ident) = &callee.kind else {
        panic!("callee should be an identifier");
    };
    assert_eq!(ident.name, "Column");
    assert_eq!(args.len(), 1, "Column should have one named `gap` arg");
    let Arg::Named { name, .. } = &args[0] else {
        panic!("gap should be a named arg");
    };
    assert_eq!(name.name, "gap");
    let Some(trailing) = trailing else {
        panic!("Column should own an indented child block");
    };
    assert_eq!(
        trailing.items.len(),
        2,
        "Column has Text and Button children"
    );

    // The `Text` child carries a string interpolation.
    let BlockItem::Expr(Expr {
        kind:
            ExprKind::Call {
                callee: text_callee,
                args: text_args,
                ..
            },
        ..
    }) = &trailing.items[0]
    else {
        panic!("expected Text call");
    };
    let ExprKind::Ident(ti) = &text_callee.kind else {
        panic!("Text callee");
    };
    assert_eq!(ti.name, "Text");
    let Arg::Named { name, value } = &text_args[0] else {
        panic!("text should be a named arg");
    };
    assert_eq!(name.name, "text");
    let ExprKind::Str(parts) = &value.kind else {
        panic!("text value should be a string");
    };
    assert_eq!(
        parts.len(),
        3,
        "string splits into text + interpolation + text"
    );

    // The `Button` child has an `onClick` prop whose value is a `||` lambda
    // assigning `count = count + 1`.
    let BlockItem::Expr(Expr {
        kind:
            ExprKind::Call {
                callee: btn_callee,
                args: btn_args,
                ..
            },
        ..
    }) = &trailing.items[1]
    else {
        panic!("expected Button call");
    };
    let ExprKind::Ident(bi) = &btn_callee.kind else {
        panic!("Button callee");
    };
    assert_eq!(bi.name, "Button");
    let Arg::Named { name, value } = &btn_args[1] else {
        panic!("onClick should be a named arg");
    };
    assert_eq!(name.name, "onClick");
    let ExprKind::Lambda { params, body } = &value.kind else {
        panic!("onClick value should be a lambda, got {:?}", value.kind);
    };
    assert!(params.is_empty(), "|| lambda takes no parameters");
    assert_eq!(body.items.len(), 1, "lambda body is one assignment");
}

#[test]
fn state_sigil_and_legacy_state_keyword_both_parse() {
    // The `$` sigil and the legacy `state` keyword must lower to the same
    // state declaration. Spans differ (the sigil is one char; `state ` is
    // five) so we compare semantic content, not the Debug string.
    let with_sigil = parse_ok("compo A\n    $n: Int = 1\n");
    let with_keyword = parse_ok("compo A\n    state n: Int = 1\n");
    let Decl::Component(sigil) = &with_sigil.decls[0] else {
        panic!("expected component");
    };
    let Decl::Component(keyword) = &with_keyword.decls[0] else {
        panic!("expected component");
    };
    let BlockItem::State(sigil_state) = &sigil.body.items[0] else {
        panic!("expected state declaration");
    };
    let BlockItem::State(keyword_state) = &keyword.body.items[0] else {
        panic!("expected state declaration");
    };
    assert_eq!(sigil_state.name.name, keyword_state.name.name);
    // Type and init carry source spans that legitimately differ between the
    // `$n` and `state n` forms, so compare their structural content only.
    let ty_name = |t: &Option<Type>| -> String {
        match t {
            Some(Type {
                kind: TypeKindAst::Primitive(name),
                ..
            }) => name.clone(),
            Some(Type {
                kind: TypeKindAst::Named { name, .. },
                ..
            }) => name.name.clone(),
            other => format!("{:?}", other),
        }
    };
    let init_int = |e: &Expr| -> i64 {
        match &e.kind {
            ExprKind::Int(v) => *v,
            other => panic!("expected Int init, got {:?}", other),
        }
    };
    assert_eq!(ty_name(&sigil_state.ty), ty_name(&keyword_state.ty));
    assert_eq!(init_int(&sigil_state.init), init_int(&keyword_state.init));
}

#[test]
fn trailing_brace_block_still_parses() {
    let ast = parse_ok("compo A\n    Column(gap: 8.0) {\n        Text(text: \"hi\")\n    }\n");
    let Decl::Component(comp) = &ast.decls[0] else {
        panic!("expected component");
    };
    let BlockItem::Expr(Expr {
        kind: ExprKind::Call { trailing, .. },
        ..
    }) = &comp.body.items[0]
    else {
        panic!("expected Column call");
    };
    assert!(trailing.is_some());
}
