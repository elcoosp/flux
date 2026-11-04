//! Round-21 adversarial tests: hostile source text thrown at the public
//! `flux_parser::parse`. Complements `properties.rs` (proptest soup ≤400
//! chars) with hand-picked *structured* hostility: unicode identifier abuse,
//! broken control chars, unterminated constructs, 10k identifiers, number
//! edge text, `$` sigil abuse and nonsense match patterns.
//!
//! Contract under test: `parse` returns Ok **or** Err — never panics, never
//! hangs — and every error span (plus every accepted decl span) lies within
//! the source bounds (`start <= end <= source.len()`).

use flux_parser::parse;

/// Asserts the Ok/Err contract and that all exposed spans stay in bounds.
fn assert_sane(source: &str) {
    let result = parse(source, 7, "adv.flux");
    match result {
        Ok(ast) => {
            assert!(
                ast.span.end as usize <= source.len(),
                "ast span {} exceeds source len {}",
                ast.span.end,
                source.len()
            );
            for decl in &ast.decls {
                let span = decl.span();
                assert!(span.start <= span.end, "inverted decl span {span:?}");
                assert!(
                    span.end as usize <= source.len(),
                    "decl span end {} exceeds source len {} for {source:?}",
                    span.end,
                    source.len()
                );
            }
        }
        Err(err) => {
            assert!(
                err.span.start <= err.span.end,
                "inverted error span {err:?}"
            );
            assert!(
                err.span.end as usize <= source.len(),
                "error span end {} exceeds source len {} for {source:?} ({})",
                err.span.end,
                source.len(),
                err.message
            );
            assert!(!err.message.is_empty(), "error must carry a message");
        }
    }
}

#[test]
fn unicode_exotic_inputs_never_panic() {
    // Emoji, ZWJ sequences, combining marks, RTL override, variation selector,
    // BOM, NUL — as sources and inside identifiers/strings/interpolations.
    let cases: Vec<String> = vec![
        "\u{FEFF}compo X\n  Text(\"a\")\n".to_owned(), // BOM prefix
        "compo X\n  Text(\"\u{1F600}\u{200D}\u{1F600}\")\n".to_owned(), // emoji + ZWJ
        "compo X\n  Text(\"\u{0301}\u{0301}\")\n".to_owned(), // bare combining marks
        "compo X\n  Text(\"\u{202E}rtl\u{202C}\")\n".to_owned(), // RTL override
        "compo X\n  Text(\"\u{FE0F}\")\n".to_owned(),  // variation selector
        "compo X\n  Text(\"\u{0}\")\n".to_owned(),     // NUL in string
        "compo X\n  Text(\u{1F600})\n".to_owned(),     // emoji as ident
        "compo X\n  Text(text: \"\u{1F600}{x}\")\n".to_owned(), // interp after emoji
        "fn \u{1F9D1}\u{200D}\u{1F4BB}() -> Int { 1 }\n".to_owned(), // ZWJ fn name
        "compo X\n  Text(\"\\u{1F600}\")\n".to_owned(), // unicode escape
        "compo X\n  Text(\"\\u{110000}\")\n".to_owned(), // out-of-range escape
        "compo X\n  Text(\"\\u{D800}\")\n".to_owned(), // surrogate escape
        "compo X\n  Text(\"\\q\")\n".to_owned(),       // unknown escape
    ];
    for src in &cases {
        assert_sane(src);
    }
}

#[test]
fn hostile_whitespace_and_line_endings_never_panic() {
    let cases: Vec<String> = vec![
        "compo\rX\r  Text(\"a\")\r".to_owned(),    // CR-only endings
        "compo X\r\n  Text(\"a\")\r\n".to_owned(), // CRLF (supported)
        "compo X\n\t\tText(\"a\")\n \t \n".to_owned(), // tabs mixed with spaces
        "compo X\n\x0CText(\"a\")\n".to_owned(),   // form feed in body
        "compo X\n  Text(\"a\")\n   \x0B\n".to_owned(), // vertical tab
        "\n\n\n\n".to_owned(),                     // newlines only
        "compo X\n  Text(\"a\")".to_owned(),       // no trailing newline
        "\t\t\t".to_owned(),                       // tabs only
    ];
    for src in &cases {
        assert_sane(src);
    }
}

#[test]
fn unterminated_constructs_error_with_in_bounds_spans() {
    let cases: Vec<String> = vec![
        "compo X\n  Text(\"unterminated".to_owned(), // string
        "compo X\n  Text(\"esc\\".to_owned(),        // trailing backslash
        "/* never closed".to_owned(),                // block comment
        "compo X\n  Text(\"a{b".to_owned(),          // interpolation
        "compo X\n  Text(\"a{b{c}\"\"".to_owned(),   // nested interp
        "compo X\n  Text(text: [1, 2".to_owned(),    // list
        "compo X\n  Text(text: f(1, 2".to_owned(),   // call args
        "fn f() -> Int {".to_owned(),                // fn body
        "type T =\n  | A(".to_owned(),               // variant
    ];
    for src in &cases {
        assert_sane(src);
    }
    // Extra: escaped-quote immediately before EOF ( reconstructed separately
    // to keep the table rows one-line each).
    assert_sane("compo X\n  Text(\"esc\\\"");
}

#[test]
fn ten_thousand_char_identifiers_and_strings_are_bounded() {
    let long_ident = "a".repeat(10_000);
    assert_sane(&format!("compo X\n  Text(text: {long_ident})\n"));
    assert_sane(&format!("fn {long_ident}() -> Int {{ 1 }}\n"));
    let long_str = "\u{1F600}\u{0301}".repeat(5_000); // 20k bytes of multibyte
    assert_sane(&format!("compo X\n  Text(\"{long_str}\")\n"));
}

#[test]
fn number_edge_text_never_panics() {
    let cases: Vec<String> = vec![
        "fn f() -> Int { 0x }".to_owned(),
        "fn f() -> Int { 0xG }".to_owned(),
        "fn f() -> Int { 0b }".to_owned(),
        "fn f() -> Int { 0b2 }".to_owned(),
        "fn f() -> Int { 1e }".to_owned(),
        "fn f() -> Int { 1e+ }".to_owned(),
        "fn f() -> Int { 1.2.3 }".to_owned(),
        "fn f() -> Int { .5 }".to_owned(),
        "fn f() -> Int { 5. }".to_owned(),
        "fn f() -> Int { 9223372036854775807 }".to_owned(),
        "fn f() -> Int { -9223372036854775808 }".to_owned(),
        "fn f() -> Int { 9223372036854775808 }".to_owned(), // i64::MAX + 1
        "fn f() -> Int { -9223372036854775809 }".to_owned(), // i64::MIN - 1
        "fn f() -> Int { 0x7FFFFFFFFFFFFFFF }".to_owned(),
        "fn f() -> Int { 1_000_000 }".to_owned(),
        "fn f() -> Int { 007 }".to_owned(),
        "fn f() -> Int { 1e400 }".to_owned(),
        "fn f() -> Int { 0.000000000000000000001 }".to_owned(),
    ];
    for src in &cases {
        assert_sane(src);
    }
}

#[test]
fn dollar_sigil_abuse_never_panics() {
    let cases: Vec<String> = vec![
        "compo X\n  state $ = 1\n".to_owned(),    // bare sigil
        "compo X\n  state $$ = 1\n".to_owned(),   // double sigil
        "compo X\n  state $1 = 1\n".to_owned(),   // digit after sigil
        "compo X\n  state $$$x = 1\n".to_owned(), // triple sigil
        "compo X\n  Text(text: $)\n".to_owned(),  // sigil alone in expr
        "compo X\n  state x = 1\n  Text(text: $x)\n".to_owned(), // valid sigil read
        "compo X\n  Text(text: $missing)\n".to_owned(), // unbound sigil
        "compo X\n  Text(text: $x.y)\n".to_owned(), // sigil field access
    ];
    for src in &cases {
        assert_sane(src);
    }
}

#[test]
fn weird_match_and_pattern_text_never_panics() {
    let cases: Vec<String> = vec![
        "fn f(x: Int) -> Int { match x { => 1 } }".to_owned(), // no pattern
        "fn f(x: Int) -> Int { match { 1 => 2 } }".to_owned(), // no scrutinee
        "fn f(x: Int) -> Int { match x { _ => } }".to_owned(), // no body
        "fn f(x: Int) -> Int { match x { A(,) => 1 } }".to_owned(), // empty fields
        "fn f(x: Int) -> Int { match x { A(B(C( => 1 } }".to_owned(), // broken nest
        "fn f(x: Int) -> Int { match x { 1 => 2, } }".to_owned(), // trailing comma
        "fn f(x: Int) -> Int { match x { | => 1 } }".to_owned(), // bare bar
        "fn f(x: Int) -> Int { match x { _ if => 1 } }".to_owned(), // dangling guard
        "fn f(x: Int) -> Int { match x { a if a > => 1 } }".to_owned(),
    ];
    for src in &cases {
        assert_sane(src);
    }
}

#[test]
fn unbalanced_everything_never_panics() {
    let cases: Vec<String> = vec![
        "}}}}".to_owned(),
        "))))".to_owned(),
        "]]]]".to_owned(),
        "{{{".to_owned(),
        "((([".to_owned(),
        "}{".to_owned(),
        ")(".to_owned(),
        "compo X\n}}}\n  Text(\"a\")\n".to_owned(),
        "fn f() -> Int { 1 }}".to_owned(),
    ];
    for src in &cases {
        assert_sane(src);
    }
}
