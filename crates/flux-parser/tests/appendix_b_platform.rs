//! Appendix B.3.8–B.3.10 examples: platform conditionals, capabilities, refs.
//!
//! These are the FLUX-003 acceptance tests: each example is reproduced
//! verbatim from `/docs/spec/mlp-appendices.md` §B.3 and asserted against the
//! shape the parser produces.

use flux_parser::{Arg, BlockItem, Decl, Expr, ExprKind, TypeKindAst};

mod common;

use common::{component, parse_ok};

#[test]
fn b38_else_block_lowers_to_block_not_elided_call() {
    // Regression: an `else { … }` block must lower to a block-valued
    // expression, never to `Call { callee: Elided, trailing: Some(block) }`
    // (the old degenerate shape that forced every downstream consumer to
    // special-case it). The grammar represents a bare block as a zero-argument
    // lambda, so the else branch must be that — not a `Call`.
    let ast = parse_ok(
        "compo PlatformButton
  if platform() == \"ios\" {
    CupertinoButton(text: \"Tap\", onClick: || { ... })
  } else {
    MaterialButton(text: \"Tap\", onClick: || { ... })
  }
",
    );
    let BlockItem::Expr(Expr {
        kind: ExprKind::If { else_branch, .. },
        ..
    }) = &component(&ast, 0).body.items[0]
    else {
        panic!("expected an `if` expression");
    };
    let Some(branch) = else_branch else {
        panic!("expected an else branch");
    };
    assert!(
        matches!(
            &branch.kind,
            ExprKind::Lambda { params, .. } if params.is_empty()
        ),
        "else branch should be a zero-argument lambda (block), got {:?}",
        branch.kind
    );
    assert!(
        !matches!(&branch.kind, ExprKind::Call { callee, .. } if matches!(callee.kind, ExprKind::Elided)),
        "else branch must not be an `Elided`-callee call"
    );
}

#[test]
fn b39_capability_declarations_list_every_method() {
    let ast = parse_ok(
        r#"capability Camera {
  fn capture() -> Data
  fn startPreview() -> Unit
  fn stopPreview() -> Unit
}

capability Storage {
  fn set(key: String, value: Data) -> Unit
  fn get(key: String) -> Option[Data]
  fn delete(key: String) -> Unit
}"#,
    );

    let Decl::Capability(camera) = &ast.decls[0] else {
        panic!("expected a capability declaration");
    };
    assert_eq!(camera.name.name, "Camera");
    assert_eq!(camera.methods.len(), 3);

    let Decl::Capability(storage) = &ast.decls[1] else {
        panic!("expected a capability declaration");
    };
    let get = &storage.methods[1];
    assert!(matches!(
        get.ret.as_ref().map(|ty| &ty.kind),
        Some(TypeKindAst::Named { name, args })
            if name.name == "Option" && args.len() == 1
    ));
}

#[test]
fn b310_refs_parse_create_ref_with_a_generic_argument() {
    let ast = parse_ok(
        "compo LoginForm
  let emailRef = createRef[TextField]()
  let passwordRef = createRef[TextField]()

  onMount {
    emailRef.focus()
  }

  Column(gap: 12) {
    TextField(ref: emailRef, placeholder: \"Email\")
    TextField(ref: passwordRef, placeholder: \"Password\")
    Button(text: \"Submit\", onClick: || {
      let email = emailRef.text()
      let password = passwordRef.text()
      Auth.login(email, password)
    })
  }
",
    );

    let decl = component(&ast, 0);
    let BlockItem::Expr(Expr {
        kind: ExprKind::Let {
            value: Some(value), ..
        },
        ..
    }) = &decl.body.items[0]
    else {
        panic!("expected a `let` binding");
    };
    let ExprKind::CreateRef { args } = &value.kind else {
        panic!("expected `createRef[…]()`");
    };
    assert!(matches!(
        &args[0].kind,
        TypeKindAst::Named { name, .. } if name.name == "TextField"
    ));
}

#[test]
fn named_arguments_are_distinguished_from_positional_ones() {
    let ast = parse_ok(
        "compo A\n  Button(text: \"Go\", onClick: || { ... })
",
    );
    let BlockItem::Expr(Expr {
        kind: ExprKind::Call { args, .. },
        ..
    }) = &component(&ast, 0).body.items[0]
    else {
        panic!("expected a call expression");
    };
    assert!(args.iter().all(|arg| matches!(arg, Arg::Named { .. })));
}
