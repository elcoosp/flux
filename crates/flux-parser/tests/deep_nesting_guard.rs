//! Round-11 regression: the parser's bracket pre-scan must reject deeply
//! nested `[]`, `()`, and `{}` input **before** recursion, so a source like
//! `Text(text: [[[[...]]]])` cannot overflow the parse thread's stack.
//!
//! Before the fix, `check_brace_depth` counted only `{}`, and the recursion
//! guard (`MAX_PARSE_DEPTH = 512`) fired far too late — each nesting level
//! walks ~10 stack frames, so the parser overflowed an 8 MiB thread at ~50–100
//! `[` levels. The new pre-scan counts `[]()` alongside `{}` and rejects at
//! `MAX_NESTING_DEPTH = 16`, well before any recursion happens.

use flux_parser::parse;

/// Builds a source with `depth` nested bracket pairs inside a `Text` prop.
fn nested_list(depth: usize) -> String {
    format!(
        "compo X\n  Text(text: {}{})\n",
        "[".repeat(depth),
        "]".repeat(depth),
    )
}

#[test]
fn deeply_nested_list_is_rejected_before_recursion() {
    // 17 > MAX_NESTING_DEPTH (16); must be a clean error, not a stack overflow.
    let src = nested_list(17);
    let err = parse(&src, 0, "p.flux").expect_err("17-deep list must be rejected");
    assert!(
        err.message.contains("nesting exceeds"),
        "error must name the nesting limit, got: {}",
        err.message,
    );
}

#[test]
fn deeply_nested_list_at_5000_is_rejected() {
    // The exact input that overflowed the pre-fix parser. Must now return a
    // clean diagnostic. Running on the default test thread (8 MiB) — if the
    // pre-scan were absent, this test itself would SIGABRT.
    let src = nested_list(5_000);
    let err = parse(&src, 0, "p.flux").expect_err("5000-deep list must be rejected");
    assert!(err.message.contains("nesting exceeds"));
}

#[test]
fn moderately_nested_list_still_parses() {
    // A legitimate, realistic list-of-lists: 4 levels, well under the limit.
    let src = nested_list(4);
    parse(&src, 0, "p.flux").expect("4-deep list must parse");
}

#[test]
fn deeply_nested_braces_still_rejected() {
    // Regression guard for the original `{}`-only behaviour.
    let mut src = String::from("compo X\n  Text(text: \"x\")\n");
    src.push_str(&"{".repeat(17));
    let _ = parse(&src, 0, "p.flux").expect_err("17-deep braces must be rejected");
}

#[test]
fn deeply_nested_parens_are_rejected() {
    // `(((…1…)))` — a paren-nested expression. The parser sees each `(` as a
    // nesting level; 17 must be rejected.
    let depth = 17;
    let src = format!(
        "compo X\n  Text(text: {}{}1{})\n",
        "(".repeat(depth),
        "",
        ")".repeat(depth),
    );
    let err = parse(&src, 0, "p.flux").expect_err("17-deep parens must be rejected");
    assert!(err.message.contains("nesting exceeds"));
}
