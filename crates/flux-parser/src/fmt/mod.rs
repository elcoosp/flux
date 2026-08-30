//! A deterministic pretty-printer for the Flux surface AST.
//!
//! `flux fmt` is a *printer over the existing AST*, never a whitespace
//! heuristic: it re-emits canonical source from [`crate::ast::Ast`] so output
//! stays correct as the grammar grows (FLUX-078 / ADR-0029). The contract that
//! makes this useful is **determinism** — printing the same AST twice yields
//! byte-identical text, and `parse(print(parse(src)))` reproduces the original
//! AST.
//!
//! Canonical rules (v1):
//!
//! * 2-space indentation; layout (`Indent`/`Dedent`) is re-derived from the
//!   parsed tree, never from the original whitespace.
//! * One blank line between top-level declarations.
//! * Trailing whitespace trimmed; a single trailing newline at EOF.
//! * Source-order prop/field order is preserved (never reordered).
//! * Operator precedence is preserved with explicit parentheses where needed.

pub(crate) mod decl;
pub(crate) mod expr;
pub(crate) mod ty;

use crate::ast::Ast;
use crate::error::ParseError;
use crate::parse;

/// The canonical indentation unit: two spaces.
const INDENT_UNIT: &str = "  ";

/// Returns `indent` levels of two-space indentation.
fn indent_str(indent: usize) -> String {
    INDENT_UNIT.repeat(indent)
}

/// Pretty-prints an already-parsed [`Ast`] into canonical Flux source.
///
/// The output is deterministic: the same `Ast` always yields byte-identical
/// text, and `parse(format_ast(parse(src)))` reproduces `parse(src)`.
#[must_use]
pub fn format_ast(ast: &Ast) -> String {
    let mut out = String::new();
    for (index, decl) in ast.decls.iter().enumerate() {
        if index > 0 {
            // Exactly one blank line separates top-level declarations.
            out.push_str("\n\n");
        }
        decl::write_decl(&mut out, decl, 0);
    }
    // Exactly one trailing newline at EOF: the indented body already ends in a
    // newline, so trim any trailing whitespace first, then append a single `\n`.
    while out.ends_with('\n') {
        out.pop();
    }
    out.push('\n');
    out
}

/// Parses `source` and pretty-prints it back to canonical Flux.
///
/// # Errors
///
/// Returns the [`ParseError`] from the parse phase unchanged; the caller is
/// expected to surface it (e.g. `flux fmt` skips the file and reports it).
pub fn format_source(source: &str, file_id: u32, path: &str) -> Result<String, ParseError> {
    let ast = parse(source, file_id, path)?;
    Ok(format_ast(&ast))
}

/// Convenience wrapper used by tests and the CLI when no stable `file_id` exists.
///
/// # Errors
///
/// See [`format_source`].
pub fn format_str(source: &str, path: &str) -> Result<String, ParseError> {
    format_source(source, 0, path)
}
