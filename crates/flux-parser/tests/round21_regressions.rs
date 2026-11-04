//! Round-21 regressions: trailing-comment layout, `provide … with`, else-if
//! chain depth, nested interpolation depth, `!x` spans, pattern unescaping,
//! and the nesting pre-scan's comment handling.

use flux_parser::{BinOp, BlockItem, Decl, Expr, ExprKind, MatchPatternKind, StrPart, parse};
use flux_syntax::Span;

const PROBE_STACK: usize = 8 * 1024 * 1024;

fn probe_on_small_stack(name: &str, src: String) -> String {
    // 8 MiB — the default test-thread budget this crate's other depth-guard
    // tests use (`deep_nesting_guard.rs` runs its 5000-level pre-scan probe on
    // the default thread for the same reason): MAX_PARSE_DEPTH = 512 frames of
    // recursive-descent needs that much headroom in a debug build before the
    // guard fires. The assertion is that a 5000-link chain produces a clean
    // diagnostic instead of an abort.
    let h = std::thread::Builder::new()
        .name(format!("probe-{name}"))
        .stack_size(PROBE_STACK)
        .spawn(move || match parse(&src, 0, "p.flux") {
            Ok(_) => "OK".to_string(),
            Err(e) => format!("ERR: {}", e.message),
        })
        .expect("spawn");
    h.join()
        .unwrap_or_else(|_| panic!("[{name}] thread aborted (stack overflow)"))
}

fn parse_ok(source: &str) -> flux_parser::Ast {
    match parse(source, 0, "t.flux") {
        Ok(ast) => ast,
        Err(e) => panic!("{}", e.render()),
    }
}

fn exprs_of_block<'a>(block: &'a flux_parser::Block) -> Vec<&'a Expr> {
    block
        .items
        .iter()
        .filter_map(|item| match item {
            BlockItem::Expr(e) => Some(e),
            _ => None,
        })
        .collect()
}

fn calls_named<'a>(exprs: &[&'a Expr], name: &str) -> Vec<&'a Expr> {
    exprs
        .iter()
        .copied()
        .filter(|e| {
            matches!(
                &e.kind,
                ExprKind::Call { callee, .. }
                    if matches!(&callee.kind, ExprKind::Ident(id) if id.name == name)
            )
        })
        .collect()
}

/// A trailing `//` comment used to swallow the line's newline, so the FOLLOWING
/// line lost its Indent (silently re-parenting the node) or its Dedent (a hard
/// parse error). Layout must be comment-transparent (ADR-0046).
///
/// Dream-syntax spaced-prop call (see examples/counter/main.flux): the indented
/// `Text` line is a CHILD of `Column`.
#[test]
fn trailing_comment_preserves_indent_of_next_line() {
    let src = "compo A\n  Column gap: 8.0 // children below\n    Text text: \"x\"\n";
    let ast = parse_ok(src);
    let comp = match &ast.decls[0] {
        Decl::Component(c) => c,
        other => panic!("expected component, got {other:?}"),
    };
    let body = exprs_of_block(&comp.body);
    let cols = calls_named(&body, "Column");
    assert!(!cols.is_empty(), "Column must parse as a body item");
    let col = cols[0];
    let ExprKind::Call { trailing, .. } = &col.kind else {
        panic!("Column is a call");
    };
    let trailing = trailing.as_ref().expect("Column must keep its dream block");
    let inner = exprs_of_block(trailing);
    let texts = calls_named(&inner, "Text");
    assert!(
        !texts.is_empty(),
        "Text must be a CHILD of Column's block, not a sibling (got {:?})",
        inner.iter().map(|e| &e.kind).collect::<Vec<_>>()
    );
}

#[test]
fn trailing_comment_preserves_dedent_to_next_declaration() {
    // Pre-fix this failed with "expected an expression, found `compo`": the
    // comment swallowed the newline and no Dedent was emitted for `compo B`.
    let src = "compo A\n  Text text: \"a\" // note\ncompo B\n  Text text: \"b\"\n";
    let ast = parse_ok(src);
    assert_eq!(ast.decls.len(), 2, "both components must parse");
}

/// Spec grammar (mlp-appendices.md): `provide_expr = "provide" ~ ident ~ "with"
/// ~ expr`. The parser used to demand a `use` token, which no spec-grammar
/// input could satisfy (`with` lexes as an identifier).
#[test]
fn provide_with_parses_per_spec() {
    let src = "compo A\n  Button(text: \"t\", onPress: || { provide Theme with \"dark\" })\n";
    parse_ok(src);
}

#[test]
fn provide_legacy_use_spelling_still_parses() {
    let src = "compo A\n  Button(text: \"t\", onPress: || { provide Theme use \"dark\" })\n";
    parse_ok(src);
}

/// `else if` chains recurse between `expr()` calls, so the depth counter never
/// grew along the chain and a long chain overflowed the stack instead of
/// surfacing the `MAX_PARSE_DEPTH` diagnostic.
#[test]
fn long_else_if_chain_is_rejected_not_overflow() {
    let mut s = String::from("compo X\n  Button(text: \"t\", onPress: || { let v = ");
    for _ in 0..5000 {
        s.push_str("if 1 { 2 } else ");
    }
    s.push_str("if 1 { 2 } else { 3 } })\n");
    let out = probe_on_small_stack("else-if", s);
    assert!(
        out.contains("nesting too deep") || out.contains("too deep"),
        "expected depth-guard error, got: {out}"
    );
}

/// Nested string interpolations spawn sub-parsers whose depth counter used to
/// reset to 0 per level, letting `{ "{ "{ … }" }" }` recurse unbounded. The
/// pre-scan now folds interpolation braces into the same 16-level budget and
/// rejects before any recursion.
#[test]
fn deeply_nested_interpolations_are_rejected_not_overflow() {
    let mut s = String::from("compo X\n  Text text: \"");
    for _ in 0..3000 {
        s.push_str("{ \"");
    }
    for _ in 0..3000 {
        s.push_str("\" }");
    }
    s.push_str("\")\n");
    let out = probe_on_small_stack("interp", s);
    assert!(
        out.contains("nesting exceeds")
            || out.contains("nesting too deep")
            || out.contains("too deep"),
        "expected depth-guard error, got: {out}"
    );
}

/// `!x` used to carry a zero-width span at the operand's end; the desugared
/// `Binary` node must span the whole `!…` form.
#[test]
fn not_expr_span_covers_the_bang() {
    let src = "compo A\n  Button(text: \"t\", onPress: || { let f = !flag })\n";
    let ast = parse_ok(src);
    let mut found: Option<Span> = None;
    fn walk_block(block: &flux_parser::Block, found: &mut Option<Span>) {
        for item in &block.items {
            if let BlockItem::Expr(e) = item {
                walk(e, found);
            }
        }
    }
    fn walk(e: &Expr, found: &mut Option<Span>) {
        match &e.kind {
            ExprKind::Binary { op, lhs, rhs } => {
                if *op == BinOp::Ne {
                    *found = Some(e.span);
                }
                walk(lhs, found);
                walk(rhs, found);
            }
            ExprKind::Lambda { body, .. } => walk_block(body, found),
            ExprKind::Let { value: Some(v), .. } => walk(v, found),
            ExprKind::Call { args, trailing, .. } => {
                for arg in args {
                    walk(arg.value(), found);
                }
                if let Some(block) = trailing.as_ref() {
                    walk_block(block, found);
                }
            }
            _ => {}
        }
    }
    if let Decl::Component(c) = &ast.decls[0] {
        walk_block(&c.body, &mut found);
    }
    let span = found.expect("desugared !expr must exist");
    let bang_off = src.find('!').expect("bang in source") as u32;
    assert!(
        span.start <= bang_off,
        "desugared span must start at/before the `!` (span {span:?}, bang at {bang_off})"
    );
    assert!(span.end > span.start, "span must be non-zero-width");
}

/// `match` pattern string literals used to lose a trailing escaped quote via
/// `trim_matches('"')`: `"a\""` unescaped to `a` instead of `a"`.
#[test]
fn match_pattern_keeps_trailing_escaped_quote() {
    let src = concat!(
        "fn f(x: String) -> Int {\n",
        "  match x {\n",
        "    \"a\\\"\" => 1\n",
        "    _ => 0\n",
        "  }\n",
        "}\n",
    );
    let ast = parse_ok(src);
    let mut value = None;
    fn collect(e: &Expr, out: &mut Option<String>) {
        match &e.kind {
            ExprKind::Match { arms, .. } => {
                for arm in arms {
                    if let MatchPatternKind::Literal(lit) = &arm.pattern.kind {
                        if let ExprKind::Str(parts) = &lit.kind {
                            if let Some(StrPart::Text(s)) = parts.first() {
                                *out = Some(s.clone());
                            }
                        }
                    }
                    collect(&arm.body, out);
                }
            }
            _ => {}
        }
    }
    if let Decl::Fn(f) = &ast.decls[0] {
        for item in exprs_of_block(&f.body) {
            collect(item, &mut value);
        }
    }
    assert_eq!(
        value.as_deref(),
        Some("a\""),
        "pattern literal must unescape to a\" (escaped quote preserved)"
    );
}

/// The nesting pre-scan used to count brackets inside `//` comments; a run of
/// unbalanced openers in commented-out code spuriously rejected valid files.
#[test]
fn brackets_in_comments_do_not_count_toward_nesting() {
    let src = "// (((((((((((((((((((( seventeen open parens in a comment\ncompo A\n  Text text: \"ok\"\n";
    parse_ok(src);
}
