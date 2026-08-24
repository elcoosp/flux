//! Lowering of literals, string interpolation, records and call arguments.

use flux_syntax::Span;
use pest::iterators::Pair;

use crate::ast::{Arg, Expr, ExprKind, StrPart};
use crate::grammar::Rule;
use crate::lower::exprs::expr;

/// Lowers an `args` pair into call arguments.
use crate::lower::{Ctx, Lowered, ident, next_pair};

pub(crate) fn call_args(ctx: &Ctx<'_>, pair: Pair<'_, Rule>) -> Lowered<Vec<Arg>> {
    let mut args = Vec::new();
    for arg in pair.into_inner() {
        let arg_span = ctx.span(&arg);
        let mut inner = arg.into_inner();
        let form = next_pair(ctx, &mut inner, arg_span, "argument")?;
        match form.as_rule() {
            Rule::named_arg => {
                let named_span = ctx.span(&form);
                let mut parts = form.into_inner();
                let name = next_pair(ctx, &mut parts, named_span, "argument name")?;
                let value = next_pair(ctx, &mut parts, named_span, "argument value")?;
                args.push(Arg::Named {
                    name: ident(ctx, &name),
                    value: expr(ctx, value)?,
                });
            }
            _ => args.push(Arg::Positional(expr(ctx, form)?)),
        }
    }
    Ok(args)
}

pub(crate) fn literal(ctx: &Ctx<'_>, pair: Pair<'_, Rule>, span: Span) -> Lowered<Expr> {
    let mut inner = pair.into_inner();
    let value = next_pair(ctx, &mut inner, span, "literal")?;
    let kind = match value.as_rule() {
        Rule::int_lit => ExprKind::Int(value.as_str().parse().map_err(|_| {
            ctx.error(
                span,
                format!("integer literal `{}` does not fit in Int", value.as_str()),
                Some("Int is a signed 64-bit integer; use Float for larger values".to_owned()),
            )
        })?),
        Rule::float_lit => ExprKind::Float(value.as_str().parse().map_err(|_| {
            ctx.error(
                span,
                format!("float literal `{}` is not representable", value.as_str()),
                Some("Float is an IEEE-754 double".to_owned()),
            )
        })?),
        Rule::bool_lit => ExprKind::Bool(value.as_str().trim() == "true"),
        Rule::string_lit => ExprKind::Str(string_parts(ctx, value)?),
        Rule::list_lit => {
            let mut items = Vec::new();
            for item in value.into_inner() {
                items.push(expr(ctx, item)?);
            }
            ExprKind::List(items)
        }
        _ => return Err(ctx.malformed(span, "literal")),
    };
    Ok(Expr { kind, span })
}

pub(crate) fn string_parts(ctx: &Ctx<'_>, pair: Pair<'_, Rule>) -> Lowered<Vec<StrPart>> {
    let mut parts = Vec::new();
    for part in pair.into_inner() {
        let part_span = ctx.span(&part);
        let mut inner = part.into_inner();
        let form = next_pair(ctx, &mut inner, part_span, "string segment")?;
        match form.as_rule() {
            Rule::str_text => parts.push(StrPart::Text(crate::lower::unescape(form.as_str()))),
            Rule::interp => {
                let interp_span = ctx.span(&form);
                let mut nested = form.into_inner();
                let inner_expr = next_pair(ctx, &mut nested, interp_span, "interpolation")?;
                parts.push(StrPart::Interp(expr(ctx, inner_expr)?));
            }
            _ => return Err(ctx.malformed(part_span, "string segment")),
        }
    }
    Ok(parts)
}

pub(crate) fn record_lit(ctx: &Ctx<'_>, pair: Pair<'_, Rule>, span: Span) -> Lowered<Expr> {
    let mut inner = pair.into_inner();
    let name_pair = next_pair(ctx, &mut inner, span, "record name")?;
    let name = ident(ctx, &name_pair);
    let mut fields = Vec::new();
    for field in inner {
        let field_span = ctx.span(&field);
        let mut parts = field.into_inner();
        let field_name = next_pair(ctx, &mut parts, field_span, "record field name")?;
        let value = next_pair(ctx, &mut parts, field_span, "record field value")?;
        fields.push((ident(ctx, &field_name), expr(ctx, value)?));
    }
    Ok(Expr {
        kind: ExprKind::Record { name, fields },
        span,
    })
}
