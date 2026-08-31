use crate::ast::*;
/// A brace-delimited block: optional closure parameters plus a body.
#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    /// Closure parameters written as `{ item, index => … }`.
    pub params: Vec<Pattern>,
    /// Body items in source order.
    pub items: Vec<BlockItem>,
    /// Span of the block including both braces.
    pub span: Span,
}

/// One entry in a block body.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum BlockItem {
    /// `state count: Int = 0`.
    State(StateDecl),
    /// `derived double = count * 2` — a computed signal that re-derives from
    /// other signals whenever they change (FLUX-072 #12).
    Derived(DerivedDecl),
    /// `width: size` — a prop entry in a trailing prop block.
    Prop {
        /// Prop name.
        name: Ident,
        /// Prop value.
        value: Expr,
    },
    /// A bare expression.
    Expr(Expr),
}

/// `state count: Int = 0`.
#[derive(Clone, Debug, PartialEq)]
pub struct StateDecl {
    /// State cell name.
    pub name: Ident,
    /// Declared type, absent when it is inferred from the initialiser.
    pub ty: Option<Type>,
    /// Initial value.
    pub init: Expr,
    /// Span of the declaration.
    pub span: Span,
}

/// `derived double = count * 2`.
///
/// A computed signal: it re-evaluates `init` whenever any signal it reads
/// changes, so it never desyncs from its sources (FLUX-072 #12).
#[derive(Clone, Debug, PartialEq)]
pub struct DerivedDecl {
    /// Computed-signal name.
    pub name: Ident,
    /// Declared type, absent when inferred from the body.
    pub ty: Option<Type>,
    /// The expression that derives the value.
    pub init: Expr,
    /// Span of the declaration.
    pub span: Span,
}
