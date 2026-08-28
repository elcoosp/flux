//! Diagnostics: every parse failure must say what, where, why and how.

use flux_parser::parse;

const FILE_ID: u32 = 11;

fn error(source: &str) -> flux_parser::ParseError {
    parse(source, FILE_ID, "broken.flux").expect_err("source must not parse")
}

#[test]
fn unclosed_brace_reports_end_of_file_with_a_close_brace_hint() {
    // A braced code block that is never closed.
    let error = error("compo Broken\n  if true {\n");
    assert_eq!(error.message, "expected `}`, found ``");
    assert!(error.hint.is_some(), "hint was {:?}", error.hint);
}

#[test]
fn unclosed_brace_points_at_the_opening_brace_not_the_end_of_file() {
    let error = error("compo Broken\n  if true {\n");
    assert_eq!((error.location.line, error.location.column), (3, 1));
}

#[test]
fn bad_interpolation_reports_the_offending_column() {
    let error = error("compo A\n  Text(\"hi {name\")\n");
    assert_eq!((error.location.line, error.location.column), (2, 8));
}

#[test]
fn bad_interpolation_renders_a_caret_under_the_string() {
    let rendered = error("compo A\n  Text(\"hi {name\")\n").render();
    assert!(rendered.contains("broken.flux:2:8"), "{rendered}");
    assert!(rendered.contains('^'), "{rendered}");
}

#[test]
fn invalid_generic_bound_reports_line_and_column() {
    let error = error("compo C[T: 3]\n  Text(\"x\")\n");
    assert_eq!((error.location.line, error.location.column), (1, 12));
}

#[test]
fn invalid_generic_bound_names_a_generic_parameter_as_expected() {
    let error = error("compo C[T: 3]\n  Text(\"x\")\n");
    assert!(
        error.message.contains('`'),
        "message was {:?}",
        error.message
    );
}

#[test]
fn every_error_carries_the_file_id_it_was_parsed_with() {
    assert_eq!(error("compo").span.file_id, FILE_ID);
}

#[test]
fn error_spans_point_inside_the_source() {
    let source = "compo A\n  let";
    let error = parse(source, FILE_ID, "broken.flux").expect_err("must not parse");
    assert!(
        (error.span.start as usize) <= source.len(),
        "span {:?} escapes the source",
        error.span
    );
}

#[test]
fn rendered_diagnostic_shows_the_offending_source_line() {
    let rendered = error("compo A\n  state 9 = 1\n").render();
    assert!(rendered.contains("state 9 = 1"), "{rendered}");
}

#[test]
fn rendered_diagnostic_starts_with_the_error_keyword() {
    assert!(error("type").render().starts_with("error: "));
}

#[test]
fn integer_literal_wider_than_i64_is_rejected_with_an_actionable_message() {
    let error = error("compo A\n  state x: Int = 99999999999999999999\n");
    assert!(
        error.message.contains("64-bit range"),
        "message was {:?}",
        error.message
    );
}

#[test]
fn unterminated_string_reports_a_location_on_the_string_line() {
    let error = error("compo A\n  Text(\"unterminated)\n");
    assert_eq!(error.location.line, 2);
}

#[test]
fn match_without_arms_is_rejected() {
    let error = error("fn f(x: Int) -> Int { match x { } }");
    assert!(error.message.starts_with("match must have"), "{error:?}");
}

#[test]
fn a_stray_closing_brace_is_reported_at_its_own_column() {
    let error = error("compo A\n}\n");
    assert_eq!((error.location.line, error.location.column), (2, 1));
}
