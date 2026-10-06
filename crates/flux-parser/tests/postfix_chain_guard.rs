//! Round-11 follow-up: a postfix chain (`.`, `?.`, `(`, `{`) produces one AST
//! level per operator, so an unbounded chain both (a) overflowed a recursive
//! `.clone()` in the loop and (b) built an N-deep tree whose `Drop` recurses
//! N frames. `postfix_expr` now bounds the chain at `MAX_PARSE_DEPTH` and uses
//! `std::mem::replace` so the loop is O(1) per iteration.

use flux_parser::parse;

#[test]
fn long_binop_chain_parses_iteratively() {
    // Binops are already iterative (`add_expr` loops); this pins that they
    // remain iterative and don't trip the postfix limit.
    let n = 10_000;
    let mut src = String::from("compo X\n  Text(text: \"1");
    for _ in 0..n {
        src.push_str("+1");
    }
    src.push_str("\")\n");
    parse(&src, 0, "p.flux").expect("10k-term binop chain must parse");
}

#[test]
fn long_field_chain_is_rejected_not_overflow() {
    // 10k-deep `a.a.a...`. Before the fix, the recursive `.clone()` inside
    // `postfix_expr` overflowed the 8 MiB test thread here.
    let n = 10_000;
    let mut src = String::from("compo X\n  Text(text: a");
    for _ in 0..n {
        src.push_str(".a");
    }
    src.push_str(")\n");
    let err = parse(&src, 0, "p.flux").expect_err("10k-deep field chain must be rejected");
    assert!(
        err.message.contains("postfix chain too long"),
        "expected postfix-chain error, got: {}",
        err.message,
    );
}

#[test]
fn moderate_field_chain_parses() {
    // A realistic chain — 20 fields — is well under the cap and must parse.
    let mut src = String::from("compo X\n  Text(text: a");
    for _ in 0..20 {
        src.push_str(".a");
    }
    src.push_str(")\n");
    parse(&src, 0, "p.flux").expect("20-deep field chain must parse");
}

#[test]
fn long_call_chain_is_rejected_not_overflow() {
    // `a(a(a(...)))` — build via the argument side to keep the top-level chain
    // short, but with a long linear sequence at the same depth.
    let n = 1000;
    let mut src = String::from("compo X\n  Text(text: a");
    for _ in 0..n {
        src.push_str("(0)");
    }
    src.push_str(")\n");
    let err = parse(&src, 0, "p.flux").expect_err("long call chain must be rejected");
    assert!(err.message.contains("postfix chain too long"));
}
