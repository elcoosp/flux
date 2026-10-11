//! FLUX-F54: a nested string inside `{…}` interpolation cannot contain
//! `{` or `}`. The parser's interp scan treats the inner `}` as the
//! interpolation's own close.

use flux_parser::parse;

fn assert_current_failure(name: &str, src: &str) {
    let err = match parse(src, 0, "f.flux") {
        Ok(_) => panic!("{name}: documented current behavior is a failure; if this now parses, flip the test"),
        Err(e) => e,
    };
    assert!(
        err.message.contains("unterminated string"),
        "{name}: expected 'unterminated string', got: {}",
        err.message,
    );
}

#[test]
fn nested_string_with_close_brace_fails() {
    let src = r#"compo C
  state s: String = "x"
  Text("{ "}" }")"#;
    assert_current_failure("nested string containing `}`", src);
}

#[test]
fn nested_string_with_open_brace_fails() {
    let src = r#"compo C
  state s: String = "x"
  Text("{ "{" }")"#;
    assert_current_failure("nested string containing `{`", src);
}

/// Positive controls: these shapes DO parse today and must keep parsing.
#[test]
fn nested_string_without_braces_still_parses() {
    let src = r#"compo C
  state s: String = "x"
  Text("{ "hi" }")"#;
    parse(src, 0, "f.flux").expect("a nested string without braces must parse");
}

#[test]
fn empty_nested_string_still_parses() {
    let src = r#"compo C
  state s: String = "x"
  Text("{ "" }")"#;
    parse(src, 0, "f.flux").expect("an empty nested string must parse");
}

#[test]
fn simple_interp_still_parses() {
    let src = r#"compo C
  state n: Int = 0
  Text("{n}")"#;
    parse(src, 0, "f.flux").expect("a simple interpolation must parse");
}
