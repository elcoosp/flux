//! Lowering of blocks, expressions and patterns (Appendix B.2).

use flux_syntax::Span;
use pest::iterators::Pair;

mod control;
mod operators;
mod values;

use crate::ast::{Block, BlockItem, Expr, ExprKind, LifecycleKind, StateDecl};

pub(crate) use control::pat_ident;
pub(crate) use values::call_args;

use crate::grammar::Rule;
use crate::lower::types::{generic_args, ty};
use crate::lower::{Ctx, Lowered, ident, next_pair};
use control::{
    assign_expr, for_expr, if_expr, lambda, let_expr, lifecycle, match_expr, provide_expr,
    when_expr,
};
use operators::{binary_chain, postfix};
use values::{literal, record_lit};

/// Lowers a `block` pair.
pub(crate) fn block(ctx: &Ctx<'_>, pair: Pair<'_, Rule>) -> Lowered<Block> {
    let span = ctx.span(&pair);
    let mut params = Vec::new();
    let mut items = Vec::new();
    for part in pair.into_inner() {
        match part.as_rule() {
            Rule::block_params => {
                for name in part.into_inner() {
                    params.push(pat_ident(ctx, &name));
                }
            }
            Rule::block_item => items.push(block_item(ctx, part)?),
            _ => return Err(ctx.malformed(span, "block")),
        }
    }
    Ok(Block {
        params,
        items,
        span,
    })
}

fn block_item(ctx: &Ctx<'_>, pair: Pair<'_, Rule>) -> Lowered<BlockItem> {
    let span = ctx.span(&pair);
    let mut inner = pair.into_inner();
    let item = next_pair(ctx, &mut inner, span, "block item")?;
    match item.as_rule() {
        Rule::state_decl => Ok(BlockItem::State(state_decl(ctx, item)?)),
        Rule::prop_entry => {
            let entry_span = ctx.span(&item);
            let mut parts = item.into_inner();
            let name_pair = next_pair(ctx, &mut parts, entry_span, "prop entry name")?;
            let name = ident(ctx, &name_pair);
            let value_pair = next_pair(ctx, &mut parts, entry_span, "prop entry value")?;
            Ok(BlockItem::Prop {
                name,
                value: expr(ctx, value_pair)?,
            })
        }
        Rule::expr => Ok(BlockItem::Expr(expr(ctx, item)?)),
        _ => Err(ctx.malformed(span, "block item")),
    }
}

pub(crate) fn state_decl(ctx: &Ctx<'_>, pair: Pair<'_, Rule>) -> Lowered<StateDecl> {
    let span = ctx.span(&pair);
    let mut inner = pair.into_inner();
    let name_pair = next_pair(ctx, &mut inner, span, "state name")?;
    let name = ident(ctx, &name_pair);
    let mut declared = None;
    let mut init = None;
    for part in inner {
        match part.as_rule() {
            Rule::ty => declared = Some(ty(ctx, part)?),
            Rule::expr => init = Some(expr(ctx, part)?),
            _ => return Err(ctx.malformed(span, "state declaration")),
        }
    }
    let init = init.ok_or_else(|| {
        ctx.error(
            span,
            format!("state `{}` has no initial value", name.name),
            Some("every `state` declaration needs `= <expr>`".to_owned()),
        )
    })?;
    Ok(StateDecl {
        name,
        ty: declared,
        init,
        span,
    })
}

/// Lowers an `expr` (or `cond_expr`) pair.
pub(crate) fn expr(ctx: &Ctx<'_>, pair: Pair<'_, Rule>) -> Lowered<Expr> {
    let span = ctx.span(&pair);
    match pair.as_rule() {
        // Transparent wrappers: unwrap to the single expression inside.
        Rule::expr | Rule::cond_expr | Rule::primary | Rule::primary_nb => {
            unwrap_single(ctx, pair, span, "expression")
        }
        Rule::paren_expr => unwrap_single(ctx, pair, span, "parenthesised expression"),
        Rule::lifecycle_expr => unwrap_single(ctx, pair, span, "lifecycle expression"),

        // Precedence chain: each level folds a left-associative operator run.
        Rule::or_expr
        | Rule::and_expr
        | Rule::cmp_expr
        | Rule::add_expr
        | Rule::mul_expr
        | Rule::cmp_nb
        | Rule::add_nb
        | Rule::mul_nb => binary_chain(ctx, pair, span),
        Rule::postfix_expr | Rule::postfix_nb => postfix(ctx, pair, span),

        Rule::let_expr => let_expr(ctx, pair, span),
        Rule::assign_expr => assign_expr(ctx, pair, span),
        Rule::literal => literal(ctx, pair, span),
        Rule::record_lit => record_lit(ctx, pair, span),
        Rule::if_expr => if_expr(ctx, pair, span),
        Rule::when_expr => when_expr(ctx, pair, span),
        Rule::match_expr => match_expr(ctx, pair, span),
        Rule::for_expr => for_expr(ctx, pair, span),
        Rule::provide_expr => provide_expr(ctx, pair, span),
        Rule::lambda => lambda(ctx, pair, span),

        Rule::ident => Ok(Expr {
            kind: ExprKind::Ident(ident(ctx, &pair)),
            span,
        }),
        Rule::ellipsis => Ok(Expr {
            kind: ExprKind::Elided,
            span,
        }),

        _ => reactive_expr(ctx, pair, span),
    }
}

/// Unwraps a grammar rule that wraps exactly one expression.
fn unwrap_single(ctx: &Ctx<'_>, pair: Pair<'_, Rule>, span: Span, expected: &str) -> Lowered<Expr> {
    let mut inner = pair.into_inner();
    let only = next_pair(ctx, &mut inner, span, expected)?;
    expr(ctx, only)
}

/// Lowers the reactive and context forms: lifecycle blocks, `resource`,
/// `useContext`, `createRef` and a bare block used as a closure.
fn reactive_expr(ctx: &Ctx<'_>, pair: Pair<'_, Rule>, span: Span) -> Lowered<Expr> {
    let kind = match pair.as_rule() {
        Rule::on_mount_expr => return lifecycle(ctx, pair, span, LifecycleKind::OnMount),
        Rule::on_cleanup_expr => return lifecycle(ctx, pair, span, LifecycleKind::OnCleanup),
        Rule::effect_expr => return lifecycle(ctx, pair, span, LifecycleKind::Effect),
        Rule::derived_expr => return lifecycle(ctx, pair, span, LifecycleKind::Derived),
        Rule::batch_expr => return lifecycle(ctx, pair, span, LifecycleKind::Batch),
        Rule::untrack_expr => return lifecycle(ctx, pair, span, LifecycleKind::Untrack),
        Rule::use_context_expr => {
            let mut inner = pair.into_inner();
            let name = next_pair(ctx, &mut inner, span, "context name")?;
            ExprKind::UseContext(ident(ctx, &name))
        }
        Rule::block_expr => {
            let mut inner = pair.into_inner();
            let body = next_pair(ctx, &mut inner, span, "closure body")?;
            ExprKind::Lambda {
                params: Vec::new(),
                body: Box::new(block(ctx, body)?),
            }
        }
        Rule::resource_expr => {
            let mut inner = pair.into_inner();
            let arg = next_pair(ctx, &mut inner, span, "resource argument")?;
            ExprKind::Resource(Box::new(expr(ctx, arg)?))
        }
        Rule::create_ref_expr => ExprKind::CreateRef {
            args: create_ref_args(ctx, pair)?,
        },
        _ => return Err(ctx.malformed(span, "expression")),
    };
    Ok(Expr { kind, span })
}

/// Extracts the optional generic argument list of a `createRef[T]()` call.
fn create_ref_args(ctx: &Ctx<'_>, pair: Pair<'_, Rule>) -> Lowered<Vec<crate::ast::Type>> {
    for part in pair.into_inner() {
        if part.as_rule() == Rule::generic_args {
            return generic_args(ctx, part);
        }
    }
    Ok(Vec::new())
}
