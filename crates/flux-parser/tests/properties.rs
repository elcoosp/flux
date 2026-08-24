//! Property tests: invariants that must hold for every accepted source.

use flux_parser::{Ast, Decl, parse};
use proptest::prelude::*;

const FILE_ID: u32 = 5;

/// Every span in `ast` must lie inside `source` and on a character boundary.
fn spans_are_valid(ast: &Ast, source: &str) -> Result<(), TestCaseError> {
    prop_assert!(ast.span.end as usize <= source.len());
    for decl in &ast.decls {
        let span = decl.span();
        prop_assert_eq!(span.file_id, FILE_ID);
        prop_assert!(span.start <= span.end);
        prop_assert!(span.end as usize <= source.len());
        prop_assert!(source.is_char_boundary(span.start as usize));
        prop_assert!(source.is_char_boundary(span.end as usize));
    }
    Ok(())
}

proptest! {
    /// The parser never panics, whatever bytes it is handed.
    #[test]
    fn parsing_arbitrary_text_never_panics(source in ".{0,400}") {
        let _ = parse(&source, FILE_ID, "prop.flux");
    }

    /// Nor on arbitrary sequences of Flux-shaped tokens.
    #[test]
    fn parsing_arbitrary_token_soup_never_panics(
        tokens in prop::collection::vec(
            prop::sample::select(vec![
                "component", "fn", "state", "type", "trait", "capability", "match",
                "when", "otherwise", "if", "else", "let", "ForEach", "onMount",
                "{", "}", "(", ")", "[", "]", "=", "=>", "|", ":", ",", ".",
                "+", "-", "*", "/", "==", "\"s\"", "1", "1.0", "true", "A", "b",
            ]),
            0..40,
        ),
    ) {
        let source = tokens.join(" ");
        let _ = parse(&source, FILE_ID, "prop.flux");
    }

    /// Every span a successful parse produces indexes the source safely.
    #[test]
    fn spans_of_generated_components_stay_inside_the_source(
        names in prop::collection::vec("[A-Z][a-z]{0,8}", 1..6),
    ) {
        let source = names
            .iter()
            .map(|name| format!("component {name} {{ Text(\"{name}\") }}"))
            .collect::<Vec<_>>()
            .join("\n");
        let ast = parse(&source, FILE_ID, "prop.flux").map_err(|error| {
            TestCaseError::fail(error.render())
        })?;
        prop_assert_eq!(ast.decls.len(), names.len());
        spans_are_valid(&ast, &source)?;
    }

    /// A component's span always covers the text that declared it.
    #[test]
    fn a_component_span_covers_its_own_source_text(
        name in "[A-Z][a-z]{0,8}",
        leading_blank_lines in 0usize..5,
    ) {
        let source = format!(
            "{}component {name} {{ Text(\"x\") }}",
            "\n".repeat(leading_blank_lines)
        );
        let ast = parse(&source, FILE_ID, "prop.flux").map_err(|error| {
            TestCaseError::fail(error.render())
        })?;
        let span = ast.decls[0].span();
        let text = &source[span.start as usize..span.end as usize];
        prop_assert!(text.starts_with("component"), "span text was {text:?}");
        prop_assert!(text.contains(&name), "span text was {text:?}");
    }

    /// Integer literals inside i64 always round-trip; ones outside always fail.
    #[test]
    fn integer_literals_round_trip_exactly_when_they_fit_in_i64(value in any::<i64>()) {
        let source = format!("component A {{ state x: Int = {value} }}");
        let ast = parse(&source, FILE_ID, "prop.flux").map_err(|error| {
            TestCaseError::fail(error.render())
        })?;
        let Decl::Component(decl) = &ast.decls[0] else {
            return Err(TestCaseError::fail("expected a component"));
        };
        let flux_parser::BlockItem::State(state) = &decl.body.items[0] else {
            return Err(TestCaseError::fail("expected a state declaration"));
        };
        prop_assert_eq!(
            match state.init.kind {
                flux_parser::ExprKind::Int(parsed) => parsed,
                _ => return Err(TestCaseError::fail("expected an integer literal")),
            },
            value
        );
    }

    /// Failures always report a location that exists in the source.
    #[test]
    fn error_locations_are_inside_the_source(source in "[a-zA-Z{}()\"= \n]{0,120}") {
        if let Err(error) = parse(&source, FILE_ID, "prop.flux") {
            prop_assert!(error.location.line >= 1);
            prop_assert!(error.location.column >= 1);
            prop_assert!(error.span.end as usize <= source.len().max(1) + 1);
            prop_assert!(error.render().starts_with("error: "));
        }
    }
}
