//! FLUX-F53: brackets inside `//` comments must not count toward the
//! nesting ceiling.

use flux_parser::parse;

#[test]
fn comment_with_unmatched_braces_does_not_trip_nesting_guard() {
    let braces: String = "{".repeat(20);
    let src = format!("compo C\n  // {braces}\n  Text(\"x\")\n");
    parse(&src, 0, "f.flux").expect("20 unmatched braces in a comment must not reject");
}

#[test]
fn comment_with_unmatched_parens_does_not_trip_nesting_guard() {
    let parens: String = "(".repeat(20);
    let src = format!("compo C\n  // {parens}\n  Text(\"x\")\n");
    parse(&src, 0, "f.flux").expect("20 unmatched parens in a comment must not reject");
}

#[test]
fn comment_with_unmatched_brackets_does_not_trip_nesting_guard() {
    let brackets: String = "[".repeat(20);
    let src = format!("compo C\n  // {brackets}\n  Text(\"x\")\n");
    parse(&src, 0, "f.flux").expect("20 unmatched brackets in a comment must not reject");
}

#[test]
fn comment_with_mixed_unmatched_brackets_does_not_trip_nesting_guard() {
    let mut s = String::from("compo C\n  // ");
    for _ in 0..10 {
        s.push_str("{[(");
    }
    s.push_str("\n  Text(\"x\")\n");
    parse(&s, 0, "f.flux").expect("mixed unmatched brackets in a comment must not reject");
}

/// Negative case: brackets inside actual code still trip the ceiling.
#[test]
fn deeply_nested_code_still_trips_nesting_guard() {
    let mut s = String::from("compo C\n  let x = ");
    for _ in 0..20 {
        s.push('(');
    }
    s.push('0');
    for _ in 0..20 {
        s.push(')');
    }
    s.push('\n');
    let err = parse(&s, 0, "f.flux").expect_err("20 nested parens must reject");
    assert!(err.message.contains("nesting exceeds"));
}
