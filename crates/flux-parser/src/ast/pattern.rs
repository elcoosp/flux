//! Match arms and binding patterns (Appendix B.2 "Patterns").

use flux_syntax::Span;

use crate::ast::{Expr, Ident};

/// One arm of a `match` expression.
#[derive(Clone, Debug, PartialEq)]
pub struct MatchArm {
    /// Pattern matched against the scrutinee.
    pub pattern: MatchPattern,
    /// Expression evaluated when the pattern matches.
    pub body: Expr,
    /// Span of the arm.
    pub span: Span,
}

/// A `match` arm pattern.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum MatchPatternKind {
    /// `_` — matches anything.
    Wildcard,
    /// `Circle(r)` or a bare binding `value`.
    Variant {
        /// Variant or binding name.
        name: Ident,
        /// Sub-patterns, empty when no parentheses were written.
        fields: Vec<Pattern>,
    },
    /// A literal pattern, e.g. `0` or `"home"`.
    Literal(Expr),
    /// `n if n > 0`.
    Guard {
        /// Bound name.
        name: Ident,
        /// Guard condition.
        cond: Expr,
    },
}

/// A `match` arm pattern together with its span.
#[derive(Clone, Debug, PartialEq)]
pub struct MatchPattern {
    /// What kind of pattern this is.
    pub kind: MatchPatternKind,
    /// Span of the pattern.
    pub span: Span,
}

/// A binding position that is either a name or a wildcard.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Pattern {
    /// A named binding.
    Ident(Ident),
    /// `_` — discards the bound value.
    Wildcard(Span),
}

/// The destructuring form on the left of a `let`.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum LetPattern {
    /// `let x = …`.
    Ident(Ident),
    /// `let (a, b) = …`.
    Tuple(Vec<LetPattern>),
    /// `let { refetch } = …`.
    Record(Vec<Ident>),
}
