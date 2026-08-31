//! The Flux surface parser: `.flux` source text to a typed syntax tree.
//!
//! Parsing is a two-phase, allocation-light pipeline:
//!
//! 1. `lexer::lex` performs a single left-to-right pass over the source,
//!    resolving indentation into `Indent`/`Dedent`/`Newline` layout
//!    tokens so the recursive-descent parser can delimit component bodies and
//!    view-call children without braces.
//! 2. `parser::parse_source` is a hand-written recursive-descent parser that
//!    builds
//!    the [`Ast`] (see [`ast`]). Every parse failure carries a [`ParseError`]
//!    with the what/where/why/how diagnostics required by AGENTS.md §3.11.

pub mod ast;
pub mod error;
pub mod fmt;
pub mod lexer;
pub mod parser;

pub use ast::{
    Annotation, Arg, Ast, BinOp, Block, BlockItem, CapabilityDecl, ComponentDecl, ConstBinding,
    Decl, Expr, ExprKind, FnDecl, FnName, Ident, LetPattern, LifecycleKind, MatchArm, MatchPattern,
    MatchPatternKind, MethodSig, Param, Pattern, PropDecl, RecordDecl, RecordField, StateDecl,
    StrPart, TraitDecl, Type, TypeDecl, TypeKindAst, TypeParam, UseDecl, Variant,
};
pub use error::{Location, ParseError};
pub use lexer::{LexError, Token, TokenKind, keyword_kind};
pub use lexer::{lex, tokenize};

/// Pretty-prints an [`Ast`] to canonical Flux source (FLUX-078 / `flux fmt`).
///
/// The LSP "format on save" and the `flux fmt` CLI both call this so styling
/// decisions live in exactly one place.
pub use fmt::format_ast;
/// Parses `source` and pretty-prints it back to canonical Flux.
pub use fmt::format_source;
/// Convenience [`format_source`] wrapper using a synthetic `file_id` of `0`.
pub use fmt::format_str;

use crate::parser::parse_source;

/// Parses `source` into an [`Ast`].
///
/// `file_id` is a stable content hash (see `flux_ir::compute_file_id`) used to
/// derive node IDs; `path` is only used in diagnostics.
///
/// # Errors
///
/// Returns a [`ParseError`] when `source` is not valid Flux, carrying the
/// what/where/why/how diagnostics required by AGENTS.md §3.7.
pub fn parse(source: &str, file_id: u32, path: &str) -> Result<Ast, ParseError> {
    parse_source(source, file_id, path)
}

/// Parses `source` with a synthetic `file_id` of `0`; convenience for tests and
/// the dev server when content addressing is not yet available.
///
/// # Errors
///
/// See [`parse`].
pub fn parse_str(source: &str, path: &str) -> Result<Ast, ParseError> {
    parse(source, 0, path)
}
