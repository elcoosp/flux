//! Every grammar example from Appendix B.3 must parse.
//!
//! These are the FLUX-003 acceptance tests: each example is reproduced from
//! `/docs/spec/mlp-appendices.md` §B.3 and asserted against the shape the
//! parser produces. The surface syntax is the indentation-delimited "dream"
//! form (see `docs/counter-syntax-dream.md`): `compo`, the `$` state sigil, and
//! spaced-prop view calls. Legacy braced view-call shapes (`Name(args) { … }`)
//! remain valid so existing source keeps parsing.

use flux_parser::{BlockItem, Decl, Expr, ExprKind, MatchPatternKind, StrPart, TypeKindAst};

mod common;

use common::{component, parse_ok};

/// Appendix B.3 source for `b32_generic_component_with_trait_bound_records_the_bound`.
const B32_SOURCE: &str = r#"trait Numeric[T] {
  fn zero() -> T
  fn one() -> T
  fn +(a: T, b: T) -> T
  fn -(a: T, b: T) -> T
}

compo Counter[T: Numeric]
  state count: T = Numeric.zero()

  Column(gap: 8) {
    Text("Count: {count}")
    Button(text: "+", onClick: || { count = count + Numeric.one() })
    Button(text: "-", onClick: || { count = count - Numeric.one() })
  }
"#;

/// Appendix B.3 source for `b33_adt_and_pattern_matching_binds_every_arm`.
const B33_SOURCE: &str = r#"type Shape =
  | Circle(Float)
  | Rectangle(Float, Float)
  | Triangle(Float, Float, Float)

fn area(shape: Shape) -> Float {
  match shape {
    Circle(r) => 3.14159 * r * r
    Rectangle(w, h) => w * h
    Triangle(b, h, _) => 0.5 * b * h
  }
}

compo ShapeDisplay
  state shape: Shape = Circle(5.0)

  Column {
    Text("Area: {area(shape)}")
    Button(text: "Make Square", onClick: || {
      shape = Rectangle(4.0, 4.0)
    })
  }
"#;

#[test]
fn b31_simple_component_declares_state_and_a_column_tree() {
    let ast = parse_ok(
        "compo HelloWorld\n  state count: Int = 0\n\n  Column(gap: 12) {\n    Text(\"Count: {count}\")\n    Button(text: \"Increment\", onClick: || {\n      count = count + 1\n    })\n  }\n",
    );

    let decl = component(&ast, 0);
    assert_eq!(decl.name.name, "HelloWorld");
    let BlockItem::State(state) = &decl.body.items[0] else {
        panic!("expected a state declaration first");
    };
    assert_eq!(state.name.name, "count");
    let ty_name = match state.ty.as_ref().map(|ty| &ty.kind) {
        Some(TypeKindAst::Primitive(name)) => name.as_str(),
        Some(TypeKindAst::Named { name, .. }) => name.name.as_str(),
        other => panic!("expected Int type, got {:?}", other),
    };
    assert_eq!(ty_name, "Int");
}

#[test]
fn b31_string_interpolation_yields_an_interpolated_expression() {
    let ast = parse_ok("compo A\n  Text(\"Count: {count}\")\n");
    let BlockItem::Expr(Expr {
        kind: ExprKind::Call { args, .. },
        ..
    }) = &component(&ast, 0).body.items[0]
    else {
        panic!("expected a call expression");
    };
    let ExprKind::Str(parts) = &args[0].value().kind else {
        panic!("expected a string literal argument");
    };
    assert_eq!(parts.len(), 2);
    assert!(matches!(&parts[0], StrPart::Text(text) if text == "Count: "));
    assert!(matches!(
        &parts[1],
        StrPart::Interp(Expr {
            kind: ExprKind::Ident(name),
            ..
        }) if name.name == "count"
    ));
}

#[test]
fn b32_generic_component_with_trait_bound_records_the_bound() {
    let ast = parse_ok(B32_SOURCE);

    let Decl::Trait(numeric) = &ast.decls[0] else {
        panic!("expected a trait declaration");
    };
    assert_eq!(numeric.generics[0].name.name, "T");
    let operators: Vec<&str> = numeric
        .methods
        .iter()
        .filter(|method| method.name.is_operator)
        .map(|method| method.name.text.as_str())
        .collect();
    assert_eq!(operators, vec!["+", "-"]);

    let counter = component(&ast, 1);
    assert_eq!(
        counter.generics[0]
            .bound
            .as_ref()
            .map(|bound| bound.name.as_str()),
        Some("Numeric")
    );
}

#[test]
fn b33_adt_and_pattern_matching_binds_every_arm() {
    let ast = parse_ok(B33_SOURCE);

    let Decl::Type(shape) = &ast.decls[0] else {
        panic!("expected a type declaration");
    };
    assert_eq!(shape.variants.len(), 3);
    assert_eq!(shape.variants[2].fields.len(), 3);

    let Decl::Fn(area) = &ast.decls[1] else {
        panic!("expected a function declaration");
    };
    let BlockItem::Expr(Expr {
        kind: ExprKind::Match { arms, .. },
        ..
    }) = &area.body.items[0]
    else {
        panic!("expected a match expression");
    };
    assert_eq!(arms.len(), 3);
    let MatchPatternKind::Variant { name, fields } = &arms[2].pattern.kind else {
        panic!("expected a variant pattern");
    };
    assert_eq!(name.name, "Triangle");
    assert_eq!(fields.len(), 3);
}
