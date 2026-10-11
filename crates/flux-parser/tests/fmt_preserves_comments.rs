//! FLUX-F55: `format_source` currently drops every user comment. This
//! test is `#[ignore]`d and pins the destructive behavior. When
//! comment-preserving formatting lands, un-ignore and assert the
//! comments survive.

use flux_parser::format_source;

#[test]
#[ignore = "FLUX-F55: flux fmt drops every comment today"]
fn preserves_line_comment_inside_component() {
    let src = "compo C\n  // important note\n  Text(\"x\")\n";
    let out = format_source(src, 0, "f.flux").expect("fmt");
    assert!(
        out.contains("important note"),
        "fmt must preserve the comment; got:\n{out}"
    );
}

#[test]
#[ignore = "FLUX-F55: flux fmt drops every comment today"]
fn preserves_header_comment() {
    let src = "// file header comment\n\ncompo C\n  Text(\"x\")\n";
    let out = format_source(src, 0, "f.flux").expect("fmt");
    assert!(out.contains("file header comment"), "got:\n{out}");
}

#[test]
#[ignore = "FLUX-F55: flux fmt drops every comment today"]
fn preserves_inline_trailing_comment() {
    let src = "compo C\n  state n: Int = 0  // counter\n  Text(\"{n}\")\n";
    let out = format_source(src, 0, "f.flux").expect("fmt");
    assert!(out.contains("counter"), "got:\n{out}");
}
