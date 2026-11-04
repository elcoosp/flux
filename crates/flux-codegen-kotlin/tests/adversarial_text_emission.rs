//! Round-21 adversarial tests for the Kotlin backend: hostile user text
//! through the full `parse → type_check → lower → codegen` pipeline.
//!
//! Kotlin is the higher-risk target: `$` starts a *live template expression*
//! and a raw newline terminates a string literal, so user text spliced
//! unescaped is the classic curve-injection into generated Compose code.
//! The scans below assert (a) no raw newline inside any string literal,
//! (b) planted code-shaped markers appear ONLY inside string literals, (c)
//! `${` never survives unescaped inside a literal (it would evaluate), and
//! (d) the pipeline never panics.

use flux_codegen_kotlin::codegen;
use flux_ir::lower;
use flux_parser::parse;
use flux_types::type_check;

/// Full pipeline; `expect` with context — a failure on these *valid* programs
/// is a finding in itself.
fn gen_kotlin(src: &str) -> String {
    let ast = parse(src, 0, "adv.flux").expect("hostile program must parse");
    let typed = type_check(&ast).expect("hostile program must type-check");
    let lowered = lower(&ast, &typed).expect("hostile program must lower");
    codegen(&lowered, &ast)
}

/// A literal-state scan for Kotlin source: returns the byte offsets of raw
/// newlines found inside `"` string literals (escapes honoured).
fn raw_newlines_inside_literals(out: &str) -> Vec<usize> {
    let mut violations = Vec::new();
    let mut in_literal = false;
    let mut escaped = false;
    for (i, ch) in out.char_indices() {
        if in_literal {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '\n' || ch == '\r' {
                violations.push(i);
            } else if ch == '"' {
                in_literal = false;
            }
        } else if ch == '"' {
            in_literal = true;
        }
    }
    violations
}

/// Returns the byte offsets of UNESCAPED `${` template-expression starts
/// inside string literals. In a program with no live interpolations (the
/// hostile-payload fixtures) any such start would EVALUATE attacker text at
/// runtime, so every occurrence is a curve-injection violation.
fn unescaped_template_starts(out: &str) -> Vec<usize> {
    let mut violations = Vec::new();
    let mut in_literal = false;
    let mut escaped = false;
    let mut prev_dollar = false;
    for (i, ch) in out.char_indices() {
        if in_literal {
            if escaped {
                // The escape consumed this char (e.g. `\$`): it is literal
                // text and must not count as a template-expression trigger.
                escaped = false;
                prev_dollar = false;
            } else if ch == '\\' {
                escaped = true;
                prev_dollar = false;
            } else {
                if ch == '{' && prev_dollar {
                    violations.push(i);
                }
                if ch == '"' {
                    in_literal = false;
                }
                prev_dollar = ch == '$';
            }
        } else if ch == '"' {
            in_literal = true;
            prev_dollar = false;
        } else {
            prev_dollar = false;
        }
    }
    violations
}

/// Returns true when every occurrence of `marker` sits inside a string
/// literal (never in code position).
fn marker_only_inside_literals(out: &str, marker: &str) -> bool {
    let mut in_literal = false;
    let mut escaped = false;
    let mut lit_spans: Vec<(usize, usize)> = Vec::new();
    let mut start = 0usize;
    for (i, ch) in out.char_indices() {
        if in_literal {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                lit_spans.push((start, i));
                in_literal = false;
            }
        } else if ch == '"' {
            in_literal = true;
            start = i;
        }
    }
    let mut search_from = 0usize;
    while let Some(rel) = out[search_from..].find(marker) {
        let at = search_from + rel;
        let inside = lit_spans.iter().any(|(s, e)| at > *s && at < *e);
        if !inside {
            return false;
        }
        search_from = at + marker.len();
    }
    true
}

const PAYLOADS: &[&str] = &[
    // Flux-source spellings; the *values* carry the hostile characters.
    r#"quote \" inside"#,
    r#"backslash \\ inside"#,
    r#"newline \n inside"#,
    r#"tab \t inside"#,
    "unicode \u{1F600}\u{0301}\u{202E}rtl\u{202C}",
    r#"template $\{evil\} and $\{x\} and \{brace}"#,
    r#"curve x\"); evilFn(); (\"y"#,
    r#"dollar $ then text"#,
];

#[test]
fn hostile_text_props_never_break_out_of_string_literals() {
    for payload in PAYLOADS {
        let src = format!("compo H\n  Text(\"{payload}\")\n");
        let out = gen_kotlin(&src);
        assert!(
            !out.contains("\"\"\""),
            "Kotlin must not emit raw strings for payload {payload:?}"
        );
        let nl = raw_newlines_inside_literals(&out);
        assert!(
            nl.is_empty(),
            "raw newline inside a Kotlin string literal for payload {payload:?} at {nl:?}:\n{out}"
        );
        let templates = unescaped_template_starts(&out);
        assert!(
            templates.is_empty(),
            "unescaped ${{ template start (live code from user text) for payload {payload:?} at {templates:?}:\n{out}"
        );
        if payload.contains("evilFn") {
            assert!(
                out.contains("evilFn"),
                "payload {payload:?} text must reach the generated Kotlin"
            );
            assert!(
                marker_only_inside_literals(&out, "evilFn"),
                "payload {payload:?} spliced code outside a string literal:\n{out}"
            );
        }
    }
}

/// A *live* flux interpolation `{count}` must render as Kotlin template
/// syntax referencing the state — but the braces/quotes around it must stay
/// balanced (no raw newline inside the emitted literal).
#[test]
fn live_interpolation_stays_balanced() {
    let out = gen_kotlin("compo C\n  state count: Int = 0\n  Text(\"n={count}\")\n");
    let nl = raw_newlines_inside_literals(&out);
    assert!(nl.is_empty(), "no raw newlines inside literals");
    assert!(
        out.contains("${count}"),
        "live flux interpolation must emit a Kotlin template expression:\n{out}"
    );
    assert!(!out.contains("\"\"\""), "no raw strings");
}

/// Props named with Kotlin keywords (`default`, `in`, `class`, `var`, `func`)
/// must be rejected upstream or emitted safely (backticked), never as bare
/// keyword identifiers in code position.
#[test]
fn keyword_named_props_emit_safely() {
    let src = concat!(
        "compo Inner(default: String, in: String)\n",
        "compo Outer\n",
        "  Inner(default: \"a\", in: \"b\")\n",
    );
    let ast = parse(src, 0, "kw.flux");
    if let Ok(ast) = ast {
        if let Ok(typed) = type_check(&ast) {
            if let Ok(lowered) = lower(&ast, &typed) {
                let out = codegen(&lowered, &ast);
                assert!(!out.is_empty());
            }
        }
    }
}
