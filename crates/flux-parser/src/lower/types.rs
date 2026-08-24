//! Lowering of type expressions (Appendix B.2 "Type Expressions").

use pest::iterators::Pair;

use crate::ast::{Ident, Type, TypeKindAst};
use crate::grammar::Rule;
use crate::lower::{Ctx, Lowered, ident, next_pair};

/// Lowers a `ty` pair.
pub(crate) fn ty(ctx: &Ctx<'_>, pair: Pair<'_, Rule>) -> Lowered<Type> {
    let span = ctx.span(&pair);
    let mut inner = pair.into_inner();
    let concrete = next_pair(ctx, &mut inner, span, "type")?;
    match concrete.as_rule() {
        Rule::primitive => Ok(Type {
            kind: TypeKindAst::Primitive(concrete.as_str().trim().to_owned()),
            span,
        }),
        Rule::type_app => type_app(ctx, concrete, span),
        Rule::record_type => record_type(ctx, concrete, span),
        Rule::fn_type => fn_type(ctx, concrete, span),
        _ => Err(ctx.malformed(span, "type")),
    }
}

fn type_app(ctx: &Ctx<'_>, pair: Pair<'_, Rule>, span: flux_syntax::Span) -> Lowered<Type> {
    let mut inner = pair.into_inner();
    let name_pair = next_pair(ctx, &mut inner, span, "type name")?;
    let name: Ident = ident(ctx, &name_pair);
    let mut args = Vec::new();
    if let Some(generic_args) = inner.next() {
        for arg in generic_args.into_inner() {
            args.push(ty(ctx, arg)?);
        }
    }
    Ok(Type {
        kind: TypeKindAst::Named { name, args },
        span,
    })
}

fn record_type(ctx: &Ctx<'_>, pair: Pair<'_, Rule>, span: flux_syntax::Span) -> Lowered<Type> {
    let mut fields = Vec::new();
    for field in pair.into_inner() {
        let field_span = ctx.span(&field);
        let mut parts = field.into_inner();
        let name_pair = next_pair(ctx, &mut parts, field_span, "record field name")?;
        let name = ident(ctx, &name_pair);
        let field_ty = next_pair(ctx, &mut parts, field_span, "record field type")?;
        fields.push((name, ty(ctx, field_ty)?));
    }
    Ok(Type {
        kind: TypeKindAst::Record(fields),
        span,
    })
}

fn fn_type(ctx: &Ctx<'_>, pair: Pair<'_, Rule>, span: flux_syntax::Span) -> Lowered<Type> {
    let mut params = Vec::new();
    let mut ret = None;
    for part in pair.into_inner() {
        match part.as_rule() {
            Rule::type_list => {
                for item in part.into_inner() {
                    params.push(ty(ctx, item)?);
                }
            }
            Rule::ty => ret = Some(ty(ctx, part)?),
            _ => return Err(ctx.malformed(span, "function type")),
        }
    }
    let ret = ret.ok_or_else(|| ctx.malformed(span, "function type return"))?;
    Ok(Type {
        kind: TypeKindAst::Fn {
            params,
            ret: Box::new(ret),
        },
        span,
    })
}

/// Lowers a `type_list` pair into its element types.
pub(crate) fn type_list(ctx: &Ctx<'_>, pair: Pair<'_, Rule>) -> Lowered<Vec<Type>> {
    pair.into_inner().map(|item| ty(ctx, item)).collect()
}

/// Lowers a `generic_args` pair into its argument types.
pub(crate) fn generic_args(ctx: &Ctx<'_>, pair: Pair<'_, Rule>) -> Lowered<Vec<Type>> {
    pair.into_inner().map(|item| ty(ctx, item)).collect()
}
