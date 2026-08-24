//! Shared helpers for lowering pest pairs into the surface syntax tree.

pub(crate) mod decls;
pub(crate) mod exprs;
pub(crate) mod types;

use flux_syntax::{FileId, Span};
use pest::iterators::Pair;

use crate::ast::{Ident, TypeParam};
use crate::error::{Location, ParseError, line_at};
use crate::grammar::Rule;

/// Context threaded through lowering: what file we are in and its text.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Ctx<'a> {
    /// Source text being lowered.
    pub(crate) source: &'a str,
    /// Identity of the source file, used for every span.
    pub(crate) file_id: FileId,
    /// Display path used in diagnostics.
    pub(crate) path: &'a str,
}

impl<'a> Ctx<'a> {
    /// Builds a span from a pest pair.
    pub(crate) fn span(&self, pair: &Pair<'_, Rule>) -> Span {
        let inner = pair.as_span();
        Span::new(self.file_id, inner.start() as u32, inner.end() as u32)
    }

    /// Builds an error anchored at `span` with an optional hint.
    pub(crate) fn error(&self, span: Span, message: String, hint: Option<String>) -> ParseError {
        let offset = span.start as usize;
        ParseError {
            message,
            hint,
            span,
            location: Location::from_offset(self.source, offset),
            path: self.path.to_owned(),
            line_text: line_at(self.source, offset),
        }
    }

    /// Reports a lowering invariant violation: a pair whose shape does not
    /// match the grammar this crate compiled against.
    ///
    /// The grammar and the lowering live in the same crate, so this can only
    /// fire if one was changed without the other; surfacing it as a normal
    /// error keeps the parser panic-free.
    pub(crate) fn malformed(&self, span: Span, expected: &str) -> ParseError {
        self.error(
            span,
            format!("malformed {expected}"),
            Some(format!(
                "the source did not produce a well-formed {expected}; \
                 this indicates a Flux grammar bug — please report it"
            )),
        )
    }
}

/// Result alias for lowering functions.
pub(crate) type Lowered<T> = Result<T, ParseError>;

/// Returns the next inner pair, or a `malformed` error naming `expected`.
pub(crate) fn next_pair<'i, I>(
    ctx: &Ctx<'_>,
    pairs: &mut I,
    span: Span,
    expected: &str,
) -> Lowered<Pair<'i, Rule>>
where
    I: Iterator<Item = Pair<'i, Rule>>,
{
    pairs.next().ok_or_else(|| ctx.malformed(span, expected))
}

/// Lowers an `ident` pair.
pub(crate) fn ident(ctx: &Ctx<'_>, pair: &Pair<'_, Rule>) -> Ident {
    Ident {
        name: pair.as_str().to_owned(),
        span: ctx.span(pair),
    }
}

/// Lowers a `generic_params` pair into its type parameters.
pub(crate) fn generic_params(ctx: &Ctx<'_>, pair: Pair<'_, Rule>) -> Lowered<Vec<TypeParam>> {
    let mut params = Vec::new();
    for param in pair.into_inner() {
        let span = ctx.span(&param);
        let mut inner = param.into_inner();
        let name_pair = next_pair(ctx, &mut inner, span, "generic parameter name")?;
        let name = ident(ctx, &name_pair);
        let bound = inner.next().map(|bound| ident(ctx, &bound));
        params.push(TypeParam { name, bound, span });
    }
    Ok(params)
}

/// Unescapes a `string_lit`'s literal text segments.
///
/// Escapes recognised by Appendix B.1 are `\"`, `\{` and `\\`; any other
/// backslash sequence is preserved verbatim so the type checker can report it.
pub(crate) fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(character) = chars.next() {
        if character != '\\' {
            out.push(character);
            continue;
        }
        match chars.next() {
            Some('"') => out.push('"'),
            Some('{') => out.push('{'),
            Some('}') => out.push('}'),
            Some('\\') => out.push('\\'),
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}
