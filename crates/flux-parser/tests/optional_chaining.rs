//! Optional-chaining (`?.`) grammar tests (FLUX-053 / ADR-0051).
//!
//! Asserts that `?.` lexes and parses into `ExprKind::OptField`, including
//! chained `a?.b?.c` and mixed `a?.b.c` forms.

use flux_parser::{Expr, ExprKind, parse};

/// File id used by these tests.
const FILE_ID: u32 = 7;

/// Parses `source` or panics with the rendered diagnostic.
fn parse_ok(source: &str) -> flux_parser::Ast {
    match parse(source, FILE_ID, "optional.flux") {
        Ok(ast) => ast,
        Err(error) => panic!("{}", error.render()),
    }
}

/// Recursively searches an expression tree for an `OptField` node.
fn contains_opt_field(expr: &Expr) -> bool {
    if matches!(expr.kind, ExprKind::OptField { .. }) {
        return true;
    }
    match &expr.kind {
        ExprKind::OptField { base, .. } => contains_opt_field(base),
        ExprKind::Field { base, .. } => contains_opt_field(base),
        ExprKind::Call { callee, args, .. } => {
            contains_opt_field(callee)
                || args.iter().any(|a| match a {
                    flux_parser::Arg::Positional(e) | flux_parser::Arg::Named { value: e, .. } => {
                        contains_opt_field(e)
                    }
                    _ => false,
                })
        }
        _ => false,
    }
}

/// Finds the first `OptField` ancestor chain length (number of nested
/// `OptField` nodes) within `expr`, used to assert chaining depth.
fn opt_field_depth(expr: &Expr) -> usize {
    match &expr.kind {
        ExprKind::OptField { base, .. } => 1 + opt_field_depth(base),
        ExprKind::Field { base, .. } => opt_field_depth(base),
        ExprKind::Call { callee, args, .. } => {
            let mut d = opt_field_depth(callee);
            for a in args {
                let inner = match a {
                    flux_parser::Arg::Positional(e) | flux_parser::Arg::Named { value: e, .. } => {
                        opt_field_depth(e)
                    }
                    _ => 0,
                };
                d = d.max(inner);
            }
            d
        }
        _ => 0,
    }
}

fn first_opt_field(expr: &Expr) -> &Expr {
    let mut cur = expr;
    loop {
        match &cur.kind {
            ExprKind::OptField { base, .. } => {
                if matches!(base.kind, ExprKind::OptField { .. }) {
                    cur = base;
                } else {
                    return cur;
                }
            }
            ExprKind::Field { base, .. } => cur = base,
            ExprKind::Call { callee, args, .. } => {
                if let Some(f) = first_opt_field_in_call(callee, args) {
                    cur = f;
                } else {
                    return cur;
                }
            }
            _ => return cur,
        }
    }
}

fn first_opt_field_in_call<'a>(
    callee: &'a Expr,
    args: &'a [flux_parser::Arg],
) -> Option<&'a Expr> {
    if let Some(f) = first_opt_field_opt(callee) {
        return Some(f);
    }
    for a in args {
        let e = match a {
            flux_parser::Arg::Positional(e) | flux_parser::Arg::Named { value: e, .. } => e,
            _ => continue,
        };
        if let Some(f) = first_opt_field_opt(e) {
            return Some(f);
        }
    }
    None
}

fn first_opt_field_opt(expr: &Expr) -> Option<&Expr> {
    match &expr.kind {
        ExprKind::OptField { .. } => Some(expr),
        ExprKind::Field { base, .. } => first_opt_field_opt(base),
        ExprKind::Call { callee, args, .. } => first_opt_field_in_call(callee, args),
        _ => None,
    }
}

#[test]
fn single_optional_access_parses() {
    let ast = parse_ok("compo C\n  state user: Option[User] = Null\n  Text(user?.name)\n");
    let comp = match &ast.decls[0] {
        flux_parser::Decl::Component(c) => c,
        other => panic!("expected component, got {other:?}"),
    };
    // The Text argument expression should contain an OptField.
    let mut found = false;
    for item in &comp.body.items {
        if let flux_parser::BlockItem::Expr(e) = item {
            if contains_opt_field(e) {
                found = true;
            }
        }
    }
    assert!(found, "expected an OptField node in the component body");
}

#[test]
fn chained_optional_access_parses() {
    let ast = parse_ok("compo C\n  state user: Option[User] = Null\n  Text(user?.profile?.name)\n");
    let comp = match &ast.decls[0] {
        flux_parser::Decl::Component(c) => c,
        other => panic!("expected component, got {other:?}"),
    };
    let mut depth = 0;
    for item in &comp.body.items {
        if let flux_parser::BlockItem::Expr(e) = item {
            depth = depth.max(opt_field_depth(e));
        }
    }
    // `user?.profile?.name` has two `?.` operators.
    assert_eq!(depth, 2, "expected a chain of two optional accesses");
}

#[test]
fn mixed_dot_and_optional_dot_parses() {
    // `user?.profile.name` — one optional access, then a regular field.
    let ast = parse_ok("compo C\n  state user: Option[User] = Null\n  Text(user?.profile.name)\n");
    let comp = match &ast.decls[0] {
        flux_parser::Decl::Component(c) => c,
        other => panic!("expected component, got {other:?}"),
    };
    let mut opt_count = 0;
    for item in &comp.body.items {
        if let flux_parser::BlockItem::Expr(e) = item {
            let root = first_opt_field(e);
            if matches!(root.kind, ExprKind::OptField { .. }) {
                opt_count += 1;
            }
        }
    }
    assert_eq!(opt_count, 1, "expected exactly one OptField in the chain");
}
