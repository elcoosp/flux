//! Lowering of control-flow and binding expressions (Appendix B.2).

use flux_syntax::Span;
use pest::iterators::Pair;

use crate::ast::{
    Expr, ExprKind, Ident, LetPattern, LifecycleKind, MatchArm, MatchPattern, MatchPatternKind,
    Pattern,
};
use crate::grammar::Rule;
use crate::lower::exprs::{block, expr};
use crate::lower::{Ctx, Lowered, ident, next_pair};

pub(crate) fn let_expr(ctx: &Ctx<'_>, pair: Pair<'_, Rule>, span: Span) -> Lowered<Expr> {
    let mut inner = pair.into_inner();
    let pattern_pair = next_pair(ctx, &mut inner, span, "let pattern")?;
    let pattern = let_pattern(ctx, pattern_pair)?;
    let value = match inner.next() {
        Some(value) => Some(Box::new(expr(ctx, value)?)),
        None => None,
    };
    Ok(Expr {
        kind: ExprKind::Let { pattern, value },
        span,
    })
}

pub(crate) fn let_pattern(ctx: &Ctx<'_>, pair: Pair<'_, Rule>) -> Lowered<LetPattern> {
    let span = ctx.span(&pair);
    match pair.as_rule() {
        Rule::let_pattern => {
            let mut inner = pair.into_inner();
            let form = next_pair(ctx, &mut inner, span, "let pattern")?;
            let_pattern(ctx, form)
        }
        Rule::ident => Ok(LetPattern::Ident(ident(ctx, &pair))),
        Rule::tuple_pattern => {
            let mut items = Vec::new();
            for item in pair.into_inner() {
                items.push(let_pattern(ctx, item)?);
            }
            Ok(LetPattern::Tuple(items))
        }
        Rule::record_pattern => Ok(LetPattern::Record(
            pair.into_inner().map(|name| ident(ctx, &name)).collect(),
        )),
        _ => Err(ctx.malformed(span, "let pattern")),
    }
}

pub(crate) fn assign_expr(ctx: &Ctx<'_>, pair: Pair<'_, Rule>, span: Span) -> Lowered<Expr> {
    let mut inner = pair.into_inner();
    let target_pair = next_pair(ctx, &mut inner, span, "assignment target")?;
    let target = expr(ctx, target_pair)?;
    let value_pair = next_pair(ctx, &mut inner, span, "assigned value")?;
    Ok(Expr {
        kind: ExprKind::Assign {
            target: Box::new(target),
            value: Box::new(expr(ctx, value_pair)?),
        },
        span,
    })
}

pub(crate) fn if_expr(ctx: &Ctx<'_>, pair: Pair<'_, Rule>, span: Span) -> Lowered<Expr> {
    let mut inner = pair.into_inner();
    let cond_pair = next_pair(ctx, &mut inner, span, "if condition")?;
    let cond = Box::new(expr(ctx, cond_pair)?);
    let then_pair = next_pair(ctx, &mut inner, span, "if body")?;
    let then_block = Box::new(block(ctx, then_pair)?);
    let else_branch = match inner.next() {
        Some(branch) => {
            let branch_span = ctx.span(&branch);
            // `else { block }` lowers to the block directly; `else if …`
            // lowers to the nested `if` expression. The previous lowering
            // wrapped a bare block in a `Call { callee: Elided, … }`, which
            // is not a real call and forced downstream consumers to special-
            // case it.
            let kind = match branch.as_rule() {
                Rule::block => ExprKind::Lambda {
                    params: Vec::new(),
                    body: Box::new(block(ctx, branch)?),
                },
                _ => expr(ctx, branch)?.kind,
            };
            Some(Box::new(Expr {
                kind,
                span: branch_span,
            }))
        }
        None => None,
    };
    Ok(Expr {
        kind: ExprKind::If {
            cond,
            then_block,
            else_branch,
        },
        span,
    })
}

pub(crate) fn when_expr(ctx: &Ctx<'_>, pair: Pair<'_, Rule>, span: Span) -> Lowered<Expr> {
    let mut inner = pair.into_inner();
    let cond_pair = next_pair(ctx, &mut inner, span, "when condition")?;
    let cond = Box::new(expr(ctx, cond_pair)?);
    let then_pair = next_pair(ctx, &mut inner, span, "when body")?;
    let then_block = Box::new(block(ctx, then_pair)?);
    let otherwise = match inner.next() {
        Some(other) => Some(Box::new(block(ctx, other)?)),
        None => None,
    };
    Ok(Expr {
        kind: ExprKind::When {
            cond,
            then_block,
            otherwise,
        },
        span,
    })
}

pub(crate) fn match_expr(ctx: &Ctx<'_>, pair: Pair<'_, Rule>, span: Span) -> Lowered<Expr> {
    let mut inner = pair.into_inner();
    let scrutinee_pair = next_pair(ctx, &mut inner, span, "match scrutinee")?;
    let scrutinee = Box::new(expr(ctx, scrutinee_pair)?);
    let mut arms = Vec::new();
    for arm in inner {
        let arm_span = ctx.span(&arm);
        let mut parts = arm.into_inner();
        let pattern_pair = next_pair(ctx, &mut parts, arm_span, "match pattern")?;
        let pattern = match_pattern(ctx, pattern_pair)?;
        let body_pair = next_pair(ctx, &mut parts, arm_span, "match arm body")?;
        arms.push(MatchArm {
            pattern,
            body: expr(ctx, body_pair)?,
            span: arm_span,
        });
    }
    Ok(Expr {
        kind: ExprKind::Match { scrutinee, arms },
        span,
    })
}

pub(crate) fn match_pattern(ctx: &Ctx<'_>, pair: Pair<'_, Rule>) -> Lowered<MatchPattern> {
    let span = ctx.span(&pair);
    let mut inner = pair.into_inner();
    let form = next_pair(ctx, &mut inner, span, "match pattern")?;
    let kind = match form.as_rule() {
        Rule::wildcard => MatchPatternKind::Wildcard,
        Rule::literal => MatchPatternKind::Literal(expr(ctx, form)?),
        Rule::variant_pattern => {
            let variant_span = ctx.span(&form);
            let mut parts = form.into_inner();
            let name_pair = next_pair(ctx, &mut parts, variant_span, "variant name")?;
            let name = ident(ctx, &name_pair);
            let mut fields = Vec::new();
            if let Some(list) = parts.next() {
                for field in list.into_inner() {
                    fields.push(pat_ident(ctx, &field));
                }
            }
            MatchPatternKind::Variant { name, fields }
        }
        Rule::guard_pattern => {
            let guard_span = ctx.span(&form);
            let mut parts = form.into_inner();
            let name_pair = next_pair(ctx, &mut parts, guard_span, "guard binding")?;
            let cond_pair = next_pair(ctx, &mut parts, guard_span, "guard condition")?;
            MatchPatternKind::Guard {
                name: ident(ctx, &name_pair),
                cond: expr(ctx, cond_pair)?,
            }
        }
        _ => return Err(ctx.malformed(span, "match pattern")),
    };
    Ok(MatchPattern { kind, span })
}

pub(crate) fn pat_ident(ctx: &Ctx<'_>, pair: &Pair<'_, Rule>) -> Pattern {
    let span = ctx.span(pair);
    if pair.as_str().trim() == "_" {
        return Pattern::Wildcard(span);
    }
    Pattern::Ident(Ident {
        name: pair.as_str().trim().to_owned(),
        span,
    })
}

pub(crate) fn for_expr(ctx: &Ctx<'_>, pair: Pair<'_, Rule>, span: Span) -> Lowered<Expr> {
    let mut inner = pair.into_inner();
    let items_pair = next_pair(ctx, &mut inner, span, "ForEach collection")?;
    let items = Box::new(expr(ctx, items_pair)?);
    let key_pair = next_pair(ctx, &mut inner, span, "ForEach key function")?;
    let key = Box::new(expr(ctx, key_pair)?);
    let body_pair = next_pair(ctx, &mut inner, span, "ForEach body")?;
    Ok(Expr {
        kind: ExprKind::ForEach {
            items,
            key,
            body: Box::new(block(ctx, body_pair)?),
        },
        span,
    })
}

pub(crate) fn provide_expr(ctx: &Ctx<'_>, pair: Pair<'_, Rule>, span: Span) -> Lowered<Expr> {
    let mut inner = pair.into_inner();
    let name_pair = next_pair(ctx, &mut inner, span, "context name")?;
    let value_pair = next_pair(ctx, &mut inner, span, "provided value")?;
    Ok(Expr {
        kind: ExprKind::Provide {
            context: ident(ctx, &name_pair),
            value: Box::new(expr(ctx, value_pair)?),
        },
        span,
    })
}

pub(crate) fn lambda(ctx: &Ctx<'_>, pair: Pair<'_, Rule>, span: Span) -> Lowered<Expr> {
    let mut params = Vec::new();
    let mut body = None;
    for part in pair.into_inner() {
        match part.as_rule() {
            Rule::params => params = crate::lower::decls::params(ctx, part)?,
            Rule::block => body = Some(Box::new(block(ctx, part)?)),
            _ => return Err(ctx.malformed(span, "lambda")),
        }
    }
    let body = body.ok_or_else(|| ctx.malformed(span, "lambda body"))?;
    Ok(Expr {
        kind: ExprKind::Lambda { params, body },
        span,
    })
}

pub(crate) fn lifecycle(
    ctx: &Ctx<'_>,
    pair: Pair<'_, Rule>,
    span: Span,
    kind: LifecycleKind,
) -> Lowered<Expr> {
    let mut inner = pair.into_inner();
    let body_pair = next_pair(ctx, &mut inner, span, "lifecycle body")?;
    Ok(Expr {
        kind: ExprKind::Lifecycle {
            kind,
            body: Box::new(block(ctx, body_pair)?),
        },
        span,
    })
}
