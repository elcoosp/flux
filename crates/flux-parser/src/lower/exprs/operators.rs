//! Lowering of the binary-operator precedence chain and postfix operations.

use flux_syntax::Span;
use pest::iterators::Pair;

use crate::ast::{BinOp, Expr, ExprKind};
use crate::grammar::Rule;
use crate::lower::exprs::values::call_args;
use crate::lower::exprs::{block, expr};
use crate::lower::{Ctx, Lowered, ident, next_pair};

pub(crate) fn binary_chain(ctx: &Ctx<'_>, pair: Pair<'_, Rule>, span: Span) -> Lowered<Expr> {
    let implied_op = pair_operator(&pair);
    let mut inner = pair.into_inner();
    let first = next_pair(ctx, &mut inner, span, "operand")?;
    let mut lhs = expr(ctx, first)?;
    while let Some(next) = inner.next() {
        let (op_text, rhs_pair) = match next.as_rule() {
            Rule::cmp_op | Rule::add_op | Rule::mul_op => {
                let rhs = next_pair(ctx, &mut inner, span, "right operand")?;
                (next.as_str().to_owned(), rhs)
            }
            // `&&` / `||` are silent in the grammar, so the next pair is the
            // operand and the operator is implied by the enclosing rule.
            _ => (
                match implied_op {
                    Some(text) => text.to_owned(),
                    None => return Err(ctx.malformed(span, "binary expression")),
                },
                next,
            ),
        };
        let op =
            BinOp::from_source(&op_text).ok_or_else(|| ctx.malformed(span, "binary operator"))?;
        let rhs = expr(ctx, rhs_pair)?;
        let joined = lhs.span.join(rhs.span);
        lhs = Expr {
            kind: ExprKind::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
            span: joined,
        };
    }
    Ok(lhs)
}

pub(crate) fn pair_operator(pair: &Pair<'_, Rule>) -> Option<&'static str> {
    match pair.as_rule() {
        Rule::or_expr => Some("||"),
        Rule::and_expr => Some("&&"),
        _ => None,
    }
}

pub(crate) fn postfix(ctx: &Ctx<'_>, pair: Pair<'_, Rule>, span: Span) -> Lowered<Expr> {
    let mut inner = pair.into_inner();
    let base = next_pair(ctx, &mut inner, span, "expression")?;
    let mut current = expr(ctx, base)?;
    for part in inner {
        current = apply_postfix(ctx, current, unwrap_postfix(ctx, part)?)?;
    }
    Ok(current)
}

/// Unwraps the `postfix` grammar wrapper to the operation it contains.
fn unwrap_postfix<'i>(ctx: &Ctx<'_>, part: Pair<'i, Rule>) -> Lowered<Pair<'i, Rule>> {
    if part.as_rule() != Rule::postfix {
        return Ok(part);
    }
    let span = ctx.span(&part);
    let mut nested = part.into_inner();
    next_pair(ctx, &mut nested, span, "postfix operation")
}

/// Applies one postfix operation — field access, call, or trailing block — to
/// the expression accumulated so far.
pub(crate) fn apply_postfix(ctx: &Ctx<'_>, base: Expr, part: Pair<'_, Rule>) -> Lowered<Expr> {
    let part_span = ctx.span(&part);
    let span = base.span.join(part_span);
    let kind = match part.as_rule() {
        Rule::field_access => {
            let mut names = part.into_inner();
            let name = next_pair(ctx, &mut names, part_span, "field name")?;
            ExprKind::Field {
                base: Box::new(base),
                field: ident(ctx, &name),
            }
        }
        Rule::call_args => ExprKind::Call {
            callee: Box::new(base),
            args: match part.into_inner().next() {
                Some(list) => call_args(ctx, list)?,
                None => Vec::new(),
            },
            trailing: None,
        },
        Rule::block => return attach_trailing_block(ctx, base, part, span),
        _ => return Err(ctx.malformed(part_span, "postfix operation")),
    };
    Ok(Expr { kind, span })
}

/// Attaches a trailing block to the call it follows, or wraps a non-call base
/// in a zero-argument call so `Column { … }` and `f() { … }` agree in shape.
pub(crate) fn attach_trailing_block(
    ctx: &Ctx<'_>,
    base: Expr,
    part: Pair<'_, Rule>,
    span: Span,
) -> Lowered<Expr> {
    let trailing = Some(Box::new(block(ctx, part)?));
    let base_span = base.span;
    let kind = match base.kind {
        ExprKind::Call {
            callee,
            args,
            trailing: None,
        } => ExprKind::Call {
            callee,
            args,
            trailing,
        },
        other => ExprKind::Call {
            callee: Box::new(Expr {
                kind: other,
                span: base_span,
            }),
            args: Vec::new(),
            trailing,
        },
    };
    Ok(Expr { kind, span })
}
