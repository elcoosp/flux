//! The Flux surface parser: `.flux` source text to a typed syntax tree.
//!
//! The grammar in `src/flux.pest` is derived from Appendix B of
//! `/docs/spec/mlp-appendices.md`; productions Appendix B leaves implicit but
//! that its own B.3 examples require are catalogued in
//! `/docs/adr/parser-grammar-extensions.md`.
//!
//! # Examples
//!
//! ```
//! use flux_parser::{Decl, parse};
//!
//! let ast = parse("component Hello { Text(\"hi\") }", 0, "hello.flux")?;
//! assert!(matches!(ast.decls.as_slice(), [Decl::Component(_)]));
//! # Ok::<(), flux_parser::ParseError>(())
//! ```

#![forbid(unsafe_code)]
#![warn(
    missing_docs,
    missing_debug_implementations,
    rust_2018_idioms,
    unreachable_pub
)]

mod ast;
mod error;
mod grammar;
mod lower;
mod prescan;

pub use ast::{
    Annotation, Arg, Ast, BinOp, Block, BlockItem, CapabilityDecl, ComponentDecl, ConstBinding,
    Decl, Expr, ExprKind, FnDecl, FnName, Ident, ImportDecl, LetPattern, LifecycleKind, MatchArm,
    MatchPattern, MatchPatternKind, MethodSig, Param, Pattern, PropDecl, StateDecl, StrPart,
    TraitDecl, Type, TypeDecl, TypeKindAst, TypeParam, UseDecl, Variant,
};
pub use error::{Location, ParseError};

use flux_syntax::{FileId, Span};
use pest::Parser as _;

use crate::error::line_at;
use crate::grammar::{FluxGrammar, Rule};
use crate::lower::Ctx;

/// Parses `source` into an [`Ast`].
///
/// `file_id` identifies the file in every produced [`Span`]; `path` is the
/// display path used in diagnostics.
///
/// # Errors
///
/// Returns a [`ParseError`] when `source` does not match the Flux grammar, or
/// when a literal in it is not representable (for example an integer wider
/// than 64 bits). The error renders as a Rust-style diagnostic via
/// [`ParseError::render`].
///
/// # Examples
///
/// ```
/// use flux_parser::parse;
///
/// let ast = parse("type Shape = | Circle(Float)", 7, "shape.flux")?;
/// assert_eq!(ast.decls.len(), 1);
/// assert_eq!(ast.span.file_id, 7);
/// # Ok::<(), flux_parser::ParseError>(())
/// ```
pub fn parse(source: &str, file_id: FileId, path: &str) -> Result<Ast, ParseError> {
    let ctx = Ctx {
        source,
        file_id,
        path,
    };
    if let Some(error) = prescan::check_depth(&ctx) {
        return Err(error);
    }
    let mut files = FluxGrammar::parse(Rule::file, source)
        .map_err(|error| prescan::prescan(&ctx).unwrap_or_else(|| syntax_error(&ctx, &error)))?;
    let file = files.next().ok_or_else(|| {
        ctx.error(
            Span::new(file_id, 0, 0),
            "empty parse result".to_owned(),
            Some("the Flux grammar failed to produce a file node".to_owned()),
        )
    })?;
    let span = lower::decls::file_span(&ctx, &file);
    let mut decls = Vec::new();
    for statement in file.into_inner() {
        if statement.as_rule() == Rule::EOI {
            continue;
        }
        decls.push(lower::decls::decl(&ctx, statement)?);
    }
    Ok(Ast { decls, span })
}

/// Converts a pest error into a Flux diagnostic with what/where/why/how.
fn syntax_error(ctx: &Ctx<'_>, error: &pest::error::Error<Rule>) -> ParseError {
    let offset = match error.location {
        pest::error::InputLocation::Pos(pos) => pos,
        pest::error::InputLocation::Span((start, _)) => start,
    };
    let end = match error.location {
        pest::error::InputLocation::Pos(pos) => pos.saturating_add(1),
        pest::error::InputLocation::Span((_, end)) => end,
    };
    let span = Span::new(ctx.file_id, offset as u32, end as u32);
    let (message, hint) = describe(ctx.source, offset, error);
    ParseError {
        message,
        hint,
        span,
        location: Location::from_offset(ctx.source, offset),
        path: ctx.path.to_owned(),
        line_text: line_at(ctx.source, offset),
    }
}

/// Produces the `what` and `how` halves of a syntax diagnostic.
fn describe(
    source: &str,
    offset: usize,
    error: &pest::error::Error<Rule>,
) -> (String, Option<String>) {
    let unclosed = source[..offset.min(source.len())]
        .chars()
        .fold(0i64, |depth, character| match character {
            '{' => depth + 1,
            '}' => depth - 1,
            _ => depth,
        });
    if offset >= source.len() && unclosed > 0 {
        return (
            "unexpected end of file: unclosed `{`".to_owned(),
            Some(format!(
                "{unclosed} block{} left open — add the matching `}}`",
                if unclosed == 1 { " is" } else { "s are" }
            )),
        );
    }
    let expected = expected_rules(error);
    let found = source[offset.min(source.len())..]
        .chars()
        .next()
        .map_or_else(|| "end of file".to_owned(), |ch| format!("`{ch}`"));
    let message = format!("unexpected {found}");
    let hint = expected.map(|rules| format!("expected {rules} here"));
    (message, hint)
}

/// Renders pest's positive expectation set as a human-readable list.
fn expected_rules(error: &pest::error::Error<Rule>) -> Option<String> {
    let pest::error::ErrorVariant::ParsingError { positives, .. } = &error.variant else {
        return None;
    };
    if positives.is_empty() {
        return None;
    }
    let mut names: Vec<String> = positives
        .iter()
        .map(|rule| format!("`{}`", rule_name(*rule)))
        .collect();
    names.sort_unstable();
    names.dedup();
    names.truncate(6);
    Some(names.join(", "))
}

/// Maps a grammar rule to the phrase used in diagnostics.
fn rule_name(rule: Rule) -> &'static str {
    match rule {
        Rule::ident => "identifier",
        Rule::expr | Rule::cond_expr => "expression",
        Rule::block => "block",
        Rule::ty => "type",
        Rule::statement => "declaration",
        Rule::literal => "literal",
        Rule::string_lit => "string literal",
        Rule::type_param => "generic parameter",
        Rule::generic_params => "generic parameter list",
        Rule::match_arm => "match arm",
        Rule::pattern => "pattern",
        Rule::EOI => "end of file",
        other => rule_fallback(other),
    }
}

/// Fallback name for rules without a dedicated phrase: the rule's own spelling.
fn rule_fallback(rule: Rule) -> &'static str {
    match rule {
        Rule::variant => "variant",
        Rule::prop_decl => "prop declaration",
        Rule::param => "parameter",
        Rule::args => "arguments",
        Rule::method_decl => "method signature",
        Rule::interp => "interpolation",
        _ => "different syntax",
    }
}
