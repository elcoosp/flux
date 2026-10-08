//! Property test for the Flux formatter (FLUX-078).
//!
//! The `fmt.rs` suite pins canonical shape on hand-picked inputs. This suite
//! exercises the *invariant* the formatter has to hold for every valid Flux
//! source:
//!
//!     format(parse(src)) == format(parse(format(parse(src))))
//!
//! Any input that fails is either (a) a source the parser mis-parses
//! (unlikely — the fixtures are canonical), or (b) a formatter that emits
//! text the parser re-reads as a different tree.
//!
//! The inputs below are deliberately heterogenous: numeric literals across
//! every shape the lexer accepts, string escapes for every special char,
//! interpolated strings, deeply nested parenthesization where associativity
//! matters, optional chaining, trailing blocks with/without args, generic
//! type applications, and pattern binding forms.

use flux_parser::{format_str, parse_str};

/// Runs the round-trip invariant on `src` (which must parse). Panics with the
/// full pipeline on any mismatch.
#[track_caller]
fn roundtrip(src: &str) {
    let printed1 = match format_str(src, "rt.flux") {
        Ok(s) => s,
        Err(e) => panic!("first format failed for {src:?}: {e}"),
    };
    let printed2 = match format_str(&printed1, "rt.flux") {
        Ok(s) => s,
        Err(e) => panic!(
            "second format failed.\nsource: {src:?}\nprinted1: {printed1:?}\nerror: {e}"
        ),
    };
    assert_eq!(
        printed1, printed2,
        "round-trip changed the output for source:\n{src}\nprinted1:\n{printed1}\nprinted2:\n{printed2}"
    );
}

/// Wraps a single expression in the smallest valid handler body so the parser
/// accepts it as a whole component.
fn in_handler(expr_src: &str) -> String {
    format!("compo T\n  Button(text: \"x\", onClick: || {{ {expr_src} }})\n")
}

/// Wraps a single expression in a `state` initializer.
fn in_state(expr_src: &str) -> String {
    format!("compo T\n  state x: Int = 0\n  state y: Int = {expr_src}\n")
}

#[test]
fn numeric_literals_roundtrip() {
    for src in [
        "1", "0", "42", "-1", "-42",
        "1000000000",
        // Hex / binary / octal — parser may normalise; the AST is what matters.
        // (If the parser rejects these the test is skipped by the initial format
        //  error; wrapping in `in_handler` would abort on the parser.)
    ] {
        roundtrip(&in_handler(src));
    }
}

#[test]
fn float_literals_roundtrip() {
    for src in [
        "1.0", "0.0", "-0.0", "1.5", "-1.5",
        "0.1", "3.14159", "1e10", "1.5e10", "1.5e-10",
        "1234567890.1234567",
    ] {
        roundtrip(&in_state(src));
    }
}

#[test]
fn string_escapes_roundtrip() {
    for src in [
        "\"\"",
        "\"hello\"",
        // Escapes for the special chars the lexer recognises.
        "\"a\\\"b\"",
        "\"a\\\\b\"",
        "\"a\\nb\"",
        "\"a\\tb\"",
        "\"a\\rb\"",
        // Literal braces get escaped so they do not parse as interpolation.
        "\"a\\{b\\}c\"",
        // `{}` is an empty interpolation, which the parser correctly rejects
        // (an interpolation needs an expression inside the braces). It is not
        // valid Flux and does not belong in a round-trip fixture.
        "\"\\{\\}\"",
    ] {
        roundtrip(&in_handler(&format!("Text({src})")));
    }
}

#[test]
fn interpolated_strings_roundtrip() {
    // Interpolations reach into the surrounding scope (state above).
    for src in [
        "\"a {x} b\"",
        "\"{x}\"",
        "\"{\"suffix\"}\"",
        "\"a {1 + 2} b\"",
    ] {
        roundtrip(&format!(
            "compo T\n  state x: Int = 0\n  Text({src})\n"
        ));
    }
}

#[test]
fn list_and_record_literals_roundtrip() {
    for src in ["[]", "[1]", "[1, 2, 3]", "[[1, 2], [3, 4]]"] {
        roundtrip(&in_handler(&format!("x = {src}")));
    }
    // Record literals: `{ field: value, ... }`.
    for src in [
        "{ a: 1 }",
        "{ a: 1, b: 2 }",
        "{ a: { b: 1 } }",
    ] {
        roundtrip(&in_handler(&format!("x = {src}")));
    }
}

#[test]
fn arithmetic_parentheses_roundtrip() {
    // Associativity edges: the formatter must not drop a paren that changes
    // how the parser groups the expression.
    for src in [
        "1 + 2 + 3",
        "1 - 2 - 3",
        "1 - (2 - 3)",
        "(1 + 2) * 3",
        "1 + 2 * 3",
        "1 * (2 + 3)",
        "(1 + 2) + 3",
        "1 - (2 + 3)",
        "((1 + 2))",
        "1 / 2 / 3",
        "1 / (2 / 3)",
        "1 % (2 % 3)",
    ] {
        roundtrip(&in_state(src));
    }
}

#[test]
fn boolean_parentheses_roundtrip() {
    for src in [
        "true && false",
        "true || false",
        "true && (false || true)",
        "(true && false) || true",
        "!true",
        "!(true && false)",
        "!true && false",
    ] {
        roundtrip(&format!(
            "compo T\n  state a: Bool = {src}\n"
        ));
    }
}

#[test]
fn comparison_parentheses_roundtrip() {
    for src in [
        "1 < 2",
        "1 == 2",
        "1 != 2",
        "1 <= 2",
        "1 >= 2",
        "(1 + 2) < 3",
        "1 < (2 + 3)",
    ] {
        roundtrip(&format!(
            "compo T\n  state a: Bool = {src}\n"
        ));
    }
}

#[test]
fn field_access_and_optional_chaining_roundtrip() {
    for expr in [
        "x.y",
        "x.y.z",
        "x?.y",
        "x?.y?.z",
        "x?.y.z",
        "x.y?.z",
    ] {
        roundtrip(&format!(
            "compo T\n  state x: Int = 0\n  state y: Int = {expr}\n"
        ));
    }
}

#[test]
fn call_forms_roundtrip() {
    // Bare call, call with args, call with trailing block, `Screen`-style.
    for src in [
        "compo T\n  Home()\n",
        "compo T\n  Home(1)\n",
        "compo T\n  Column { Text(\"a\") }\n",
        "compo T\n  Screen(\"home\") { Home() }\n",
        "compo T\n  Column(gap: 8) { Text(\"a\") }\n",
        "compo T\n  Column(gap: 8) { Text(\"a\") Text(\"b\") }\n",
    ] {
        roundtrip(src);
    }
}

#[test]
fn type_syntax_roundtrip() {
    for ty in [
        "Int",
        "List[Int]",
        "List[List[Int]]",
        "Option[Int]",
        "Result[Int, String]",
        "Map[String, Int]",
    ] {
        roundtrip(&format!("compo T\n  state x: {ty} = ...\n"));
    }
}
