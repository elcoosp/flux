//! Helpers shared by the Appendix B.3 acceptance tests.

use flux_parser::{Ast, ComponentDecl, Decl, parse};

/// File id used by every Appendix B.3 test.
pub const FILE_ID: u32 = 3;

/// Parses `source`, reporting the rendered diagnostic on failure.
///
/// # Panics
///
/// Panics when `source` does not parse; the panic message is the parser's own
/// Rust-style diagnostic, which is what makes a failure debuggable.
pub fn parse_ok(source: &str) -> Ast {
    match parse(source, FILE_ID, "example.flux") {
        Ok(ast) => ast,
        Err(error) => panic!("{}", error.render()),
    }
}

/// Returns the component declaration at `index`.
///
/// # Panics
///
/// Panics when the declaration at `index` is not a component.
pub fn component(ast: &Ast, index: usize) -> &ComponentDecl {
    match &ast.decls[index] {
        Decl::Component(decl) => decl,
        other => panic!("expected a component at {index}, got {other:?}"),
    }
}
