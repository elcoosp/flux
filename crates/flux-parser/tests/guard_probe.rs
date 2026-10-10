//! Regression test: the spec-documented guard-arm syntax `s if s > 90 => "A"`
//! (§18.5) must parse into `MatchPatternKind::Guard`. Before round 32, the
//! parser returned immediately after the binding identifier, so the `if`
//! keyword tripped the next `eat(FatArrow)` and every guarded match program
//! was rejected at parse time.

#[test]
fn guard_arm_parses_into_guard_pattern() {
    use flux_parser::{ExprKind, MatchPatternKind, parse};
    let src = "compo Score\n  state score: Int = 95\n  let grade = match score {\n    s if s > 90 => \"A\"\n    s if s > 80 => \"B\"\n    _ => \"C\"\n  }\n  Text(text: grade)\n";
    let ast = parse(src, 0, "score.flux").expect("guard syntax must parse");
    // Walk to the `match` expr and inspect the arms.
    let mut found = 0usize;
    for decl in &ast.decls {
        if let flux_parser::ast::Decl::Component(c) = decl {
            for item in &c.body.items {
                if let flux_parser::ast::BlockItem::Expr(e) = item {
                    if let Some(arms) = find_match_arms(e) {
                        for arm in arms {
                            if let MatchPatternKind::Guard { name, .. } = &arm.pattern.kind {
                                assert_eq!(name.name, "s");
                                found += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(found, 2, "both guarded arms must parse as Guard patterns");
}

fn find_match_arms(e: &flux_parser::Expr) -> Option<Vec<flux_parser::ast::MatchArm>> {
    use flux_parser::ExprKind;
    match &e.kind {
        ExprKind::Match { arms, .. } => Some(arms.clone()),
        ExprKind::Let { value: Some(v), .. } => find_match_arms(v),
        _ => None,
    }
}
