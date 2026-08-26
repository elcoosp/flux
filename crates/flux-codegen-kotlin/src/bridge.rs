//! Node-ID bridge (ADR-0027) from the lowered arena back to the surface AST.
//!
//! [`flux_ir`] lowers every component-body expression under
//! [`EXPR_TAG`] and every component declaration under [`COMPONENT_TAG`], always
//! with parent `0`. Lowered nodes therefore carry the exact [`NodeId`]s the
//! type checker assigned. We rebuild those IDs from the AST so each packed node
//! can recover its originating surface construct (its name, props,
//! interpolations, generics, `@pure` annotation, …) — information the arena
//! deliberately drops to stay compact.
//!
//! The bridge *owns* cloned references to the originating AST nodes (they are
//! cheap, `Clone` types) so it carries no lifetime parameter and can be passed
//! freely through the emitter.

use flux_parser::{Ast, ComponentDecl, Decl, Expr};
use flux_syntax::{DeclTag, ExprTag, NodeId, Span};

/// Structural tag the type checker/lowering assigns to every expression node.
pub(crate) const EXPR_TAG: u8 = 10;
/// Structural tag for a `component` declaration.
pub(crate) const COMPONENT_TAG: u8 = 3;

/// Derives the [`NodeId`] for `span` as an expression-origin node.
#[must_use]
pub(crate) fn expr_id(span: Span) -> NodeId {
    // `ExprTag::into_u8` returns the discriminant unchanged, so this reproduces
    // the exact `NodeId` the type checker/lowering assigned under `EXPR_TAG`
    // (see `flux_ir::lower::ids`); the canonical `compute_node_id` now requires
    // `impl NodeTag` (ADR/issue 3a).
    flux_syntax::compute_node_id(0, ExprTag(EXPR_TAG), span, None)
}

/// Derives the [`NodeId`] for `span` as a component-declaration node.
///
/// The lowerer records every `component` declaration under [`DeclTag`] (see
/// `flux-ir::lower::ids::decl_node_id`), so the bridge must use the same
/// family. `DeclTag` and `ExprTag` map to disjoint byte ranges; using
/// `ExprTag` here silently produces an ID that never matches the lowered node
/// and forces the emitter into its `FluxComponent_<id>` placeholder branch.
#[must_use]
pub(crate) fn component_id(span: Span) -> NodeId {
    flux_syntax::compute_node_id(0, DeclTag(COMPONENT_TAG), span, None)
}

/// Registry of surface constructs keyed by the [`NodeId`] the lowering pass
/// assigned them. Built once per [`crate::codegen`] run from the AST.
#[derive(Debug, Default)]
pub(crate) struct Bridge {
    /// Expression-origin nodes (primitives, `if`/`when`, `ForEach`, `match`).
    exprs: std::collections::HashMap<NodeId, Expr>,
    /// Component declarations.
    components: std::collections::HashMap<NodeId, ComponentDecl>,
}

impl Bridge {
    /// Builds the bridge by walking every declaration and (recursively) every
    /// expression in the AST, recording each under its derived [`NodeId`].
    #[must_use]
    pub(crate) fn build(ast: &Ast) -> Bridge {
        let mut bridge = Bridge::default();
        for decl in &ast.decls {
            if let Decl::Component(comp) = decl {
                let id = component_id(comp.span);
                bridge.components.insert(id, comp.clone());
                walk_block(&comp.body, &mut bridge);
            }
        }
        bridge
    }

    /// Returns the originating expression for `id`, if it was an expression
    /// node in the lowered tree.
    #[must_use]
    pub(crate) fn expr(&self, id: NodeId) -> Option<&Expr> {
        self.exprs.get(&id)
    }

    /// Returns the originating component declaration for `id`.
    #[must_use]
    pub(crate) fn component(&self, id: NodeId) -> Option<&ComponentDecl> {
        self.components.get(&id)
    }
}

/// Records `expr` and all of its sub-expressions under their derived IDs.
fn walk_expr(expr: &Expr, bridge: &mut Bridge) {
    bridge.exprs.insert(expr_id(expr.span), expr.clone());
    use flux_parser::ExprKind;
    match &expr.kind {
        ExprKind::Call {
            callee,
            args,
            trailing,
        } => {
            walk_expr(callee, bridge);
            for arg in args {
                walk_expr(arg.value(), bridge);
            }
            if let Some(block) = trailing {
                walk_block(block, bridge);
            }
        }
        ExprKind::If {
            cond,
            then_block,
            else_branch,
            ..
        } => {
            walk_expr(cond, bridge);
            walk_block(then_block, bridge);
            if let Some(other) = else_branch {
                walk_expr(other, bridge);
            }
        }
        ExprKind::When {
            cond,
            then_block,
            otherwise,
            ..
        } => {
            walk_expr(cond, bridge);
            walk_block(then_block, bridge);
            if let Some(other) = otherwise {
                walk_block(other, bridge);
            }
        }
        ExprKind::Match {
            scrutinee, arms, ..
        } => {
            walk_expr(scrutinee, bridge);
            for arm in arms {
                walk_expr(&arm.body, bridge);
            }
        }
        ExprKind::ForEach {
            items, key, body, ..
        } => {
            walk_expr(items, bridge);
            walk_expr(key, bridge);
            walk_block(body, bridge);
        }
        ExprKind::Binary { lhs, rhs, .. } => {
            walk_expr(lhs, bridge);
            walk_expr(rhs, bridge);
        }
        ExprKind::Field { base, .. } => walk_expr(base, bridge),
        ExprKind::Assign { target, value, .. } => {
            walk_expr(target, bridge);
            walk_expr(value, bridge);
        }
        ExprKind::Lambda { body, .. } => walk_block(body, bridge),
        ExprKind::Let { value: Some(v), .. } => walk_expr(v, bridge),
        ExprKind::Let { value: None, .. } => {}
        ExprKind::Record { fields, .. } => {
            for (_, e) in fields {
                walk_expr(e, bridge);
            }
        }
        ExprKind::List(_) | ExprKind::CreateRef { .. } => {}
        _ => {}
    }
}

/// Records every UI-producing expression in `block`, skipping non-UI forms
/// (`let`, lifecycle, `provide`, `useContext`, `resource`) which contribute no
/// lowered child node and therefore no bridge entry is needed for the tree.
fn walk_block(block: &flux_parser::Block, bridge: &mut Bridge) {
    for item in &block.items {
        if let flux_parser::BlockItem::Expr(expr) = item {
            walk_expr(expr, bridge);
        }
    }
}
