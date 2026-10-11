//! FLUX-F60 regression: `record R { … }` accepts both the comma-separated
//! single-line form and the layout-separated multi-line form.

use flux_parser::parse;

#[test]
fn multi_line_record_without_commas_parses() {
    let src = "record Point {\n  x: Int\n  y: Int\n}\n\ncompo C\n  Text(\"x\")\n";
    parse(src, 0, "f.flux").expect("multi-line record without commas must parse");
}

#[test]
fn single_line_record_with_commas_still_parses() {
    let src = "record Point { x: Int, y: Int }\n\ncompo C\n  Text(\"x\")\n";
    parse(src, 0, "f.flux").expect("single-line record with commas must parse");
}

#[test]
fn multi_line_record_with_commas_still_parses() {
    let src = "record Point {\n  x: Int,\n  y: Int,\n}\n\ncompo C\n  Text(\"x\")\n";
    parse(src, 0, "f.flux").expect("multi-line record with commas must parse");
}

#[test]
fn empty_record_parses() {
    let src = "record Empty {}\n\ncompo C\n  Text(\"x\")\n";
    parse(src, 0, "f.flux").expect("empty record must parse");
}

#[test]
fn single_field_record_parses() {
    let src = "record One { x: Int }\n\ncompo C\n  Text(\"x\")\n";
    parse(src, 0, "f.flux").expect("single-field record must parse");
}

#[test]
fn mixed_comma_and_layout_parses() {
    // Trailing comma after one field, none after another — both accepted.
    let src = "record R {\n  a: Int,\n  b: Int\n  c: Int,\n}\n\ncompo C\n  Text(\"x\")\n";
    parse(src, 0, "f.flux").expect("mixed separators must parse");
}
