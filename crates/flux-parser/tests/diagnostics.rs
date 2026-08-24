//! Diagnostics: every parse failure must say what, where, why and how.

use flux_parser::parse;

const FILE_ID: u32 = 11;

fn error(source: &str) -> flux_parser::ParseError {
    parse(source, FILE_ID, "broken.flux").expect_err("source must not parse")
}

#[test]
fn unclosed_brace_reports_end_of_file_with_a_close_brace_hint() {
    let error = error("component Broken {\n  Text(\"hi\")\n");
    assert_eq!(error.message, "unclosed `{`");
    assert!(
        error
            .hint
            .as_deref()
            .is_some_and(|hint| hint.contains("add the matching `}`")),
        "hint was {:?}",
        error.hint
    );
}

#[test]
fn unclosed_brace_points_at_the_opening_brace_not_the_end_of_file() {
    let error = error("component Broken {\n  Text(\"hi\")\n");
    assert_eq!((error.location.line, error.location.column), (1, 18));
}

#[test]
fn bad_interpolation_reports_the_offending_column() {
    let error = error("component A { Text(\"hi {name\") }");
    assert_eq!((error.location.line, error.location.column), (1, 20));
}

#[test]
fn bad_interpolation_renders_a_caret_under_the_string() {
    let rendered = error("component A { Text(\"hi {name\") }").render();
    assert!(rendered.contains("broken.flux:1:20"), "{rendered}");
    assert!(rendered.contains('^'), "{rendered}");
}

#[test]
fn invalid_generic_bound_reports_line_and_column() {
    let error = error("component C[T: 3] { Text(\"x\") }");
    assert_eq!((error.location.line, error.location.column), (1, 16));
}

#[test]
fn invalid_generic_bound_names_a_generic_parameter_as_expected() {
    let error = error("component C[T: 3] { Text(\"x\") }");
    assert!(
        error.hint.as_deref().is_some_and(|hint| hint.contains('`')),
        "hint was {:?}",
        error.hint
    );
}

#[test]
fn every_error_carries_the_file_id_it_was_parsed_with() {
    assert_eq!(error("component").span.file_id, FILE_ID);
}

#[test]
fn error_spans_point_inside_the_source() {
    let source = "component A { let }";
    let error = parse(source, FILE_ID, "broken.flux").expect_err("must not parse");
    assert!(
        (error.span.start as usize) <= source.len(),
        "span {:?} escapes the source",
        error.span
    );
}

#[test]
fn rendered_diagnostic_shows_the_offending_source_line() {
    let rendered = error("component A {\n  state 9 = 1\n}").render();
    assert!(rendered.contains("state 9 = 1"), "{rendered}");
}

#[test]
fn rendered_diagnostic_starts_with_the_error_keyword() {
    assert!(error("type").render().starts_with("error: "));
}

#[test]
fn integer_literal_wider_than_i64_is_rejected_with_an_actionable_message() {
    let error = error("component A { state x: Int = 99999999999999999999 }");
    assert!(
        error.message.contains("does not fit in Int"),
        "message was {:?}",
        error.message
    );
}

#[test]
fn unterminated_string_reports_a_location_on_the_string_line() {
    let error = error("component A {\n  Text(\"unterminated)\n}");
    assert_eq!(error.location.line, 2);
}

#[test]
fn match_without_arms_is_rejected() {
    let error = error("fn f(x: Int) -> Int { match x { } }");
    assert!(error.message.starts_with("unexpected"), "{error:?}");
}

#[test]
fn a_stray_closing_brace_is_reported_at_its_own_column() {
    let error = error("component A { }\n}\n");
    assert_eq!((error.location.line, error.location.column), (2, 1));
}
