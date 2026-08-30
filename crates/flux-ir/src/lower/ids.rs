//! Node-ID helpers for lowering (ADR-0027 bridge).
//!
//! The type checker keys `TypedAST::types` with
//! `compute_node_id(0, tag, span, None)` where `tag` is `10` for every
//! expression and the declaration tag (1–8) for top-level declarations. To join
//! lowering output to those inferred types, every IR node we emit must reuse
//! that exact `(parent, tag, span, key)` tuple. We therefore always pass
//! `parent = 0` and the structural tag, never the [`flux_syntax::NodeKind`]
//! wire discriminant, when deriving IDs.

use flux_parser::{Decl, Expr};
use flux_syntax::{DeclTag, ExprTag, NodeId, compute_node_id};

/// Structural tag the type checker assigns to *every* expression node.
pub(crate) const EXPR_TAG: u8 = 10;

/// Structural tag for a `component` declaration.
pub(crate) const COMPONENT_TAG: u8 = 3;

/// Which wire [`flux_syntax::NodeKind`] an expression-origin IR node represents.
///
/// This is *not* the ID tag — it is the `Node::kind` field. The ID tag is always
/// [`EXPR_TAG`] for expressions so it matches the type checker's `types` keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExprNodeKind {
    /// `Text`, `Button`, `Column`, … — a leaf primitive adapter.
    Primitive,
    /// `if` / `when` conditional subtree.
    If,
    /// `ForEach` keyed list.
    ForEach,
    /// `match` over an ADT.
    Match,
}

/// Derives the [`NodeId`] for a declaration, matching the type checker.
///
/// The tag is wrapped in [`DeclTag`] (not [`ExprTag`]): the node-ID hash folds
/// the tag *family* in, so a declaration ID must use the same family the type
/// checker used when it recorded the inferred type under `TypedAST::types`.
/// Mismatching the family (`ExprTag` vs [`DeclTag`]) silently produces a
/// different ID and breaks the ADR-0027 node-ID bridge.
#[must_use]
pub(crate) fn decl_node_id(decl: &Decl) -> NodeId {
    let tag = match decl {
        Decl::Use(_) => 2,
        Decl::Component(_) => COMPONENT_TAG,
        Decl::Fn(_) => 4,
        Decl::Type(_) => 5,
        Decl::Trait(_) => 6,
        Decl::Capability(_) => 7,
        Decl::Const(_) => 8,
        #[allow(unreachable_patterns)]
        _ => 9,
    };
    compute_node_id(0, DeclTag(tag), decl.span(), None)
}

/// Derives the [`NodeId`] for an expression-origin IR node.
///
/// Always uses [`EXPR_TAG`] and `parent = 0` so it equals the key the type
/// checker stored in `typed.types` for this expression. Keyed `ForEach`
/// children would carry a key, but those are produced at runtime and are not
/// lowered statically.
#[must_use]
pub(crate) fn expr_node_id(expr: &Expr, _kind: ExprNodeKind) -> NodeId {
    compute_node_id(0, ExprTag(EXPR_TAG), expr.span, None)
}
