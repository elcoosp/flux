//! Lowering of top-level declarations (Appendix B.2 "Top-level").

use flux_syntax::Span;
use pest::iterators::Pair;

use crate::ast::{
    Annotation, ComponentDecl, ConstBinding, Decl, FnDecl, FnName, ImportDecl, Param, PropDecl,
    UseDecl,
};
use crate::grammar::Rule;
use crate::lower::exprs::{block, call_args, expr};
use crate::lower::types::ty;
use crate::lower::{Ctx, Lowered, generic_params, ident, next_pair, unescape};

mod types;

use types::{capability_decl, trait_decl, type_decl};

/// Lowers a `statement` pair into a declaration.
pub(crate) fn decl(ctx: &Ctx<'_>, pair: Pair<'_, Rule>) -> Lowered<Decl> {
    let span = ctx.span(&pair);
    let mut inner = pair.into_inner();
    let form = next_pair(ctx, &mut inner, span, "declaration")?;
    match form.as_rule() {
        Rule::import_decl => import_decl(ctx, form).map(Decl::Import),
        Rule::use_decl => use_decl(ctx, form).map(Decl::Use),
        Rule::annotated_component => component(ctx, form).map(Decl::Component),
        Rule::fn_decl => fn_decl(ctx, form).map(Decl::Fn),
        Rule::type_decl => type_decl(ctx, form).map(Decl::Type),
        Rule::trait_decl => trait_decl(ctx, form).map(Decl::Trait),
        Rule::capability_decl => capability_decl(ctx, form).map(Decl::Capability),
        Rule::const_binding => const_binding(ctx, form).map(Decl::Const),
        _ => Err(ctx.malformed(span, "declaration")),
    }
}

fn import_decl(ctx: &Ctx<'_>, pair: Pair<'_, Rule>) -> Lowered<ImportDecl> {
    let span = ctx.span(&pair);
    let mut inner = pair.into_inner();
    let name_pair = next_pair(ctx, &mut inner, span, "import name")?;
    let source_pair = next_pair(ctx, &mut inner, span, "import path")?;
    let raw = source_pair.as_str();
    let source = unescape(raw.trim_matches('"'));
    Ok(ImportDecl {
        name: ident(ctx, &name_pair),
        source,
        span,
    })
}

fn use_decl(ctx: &Ctx<'_>, pair: Pair<'_, Rule>) -> Lowered<UseDecl> {
    let span = ctx.span(&pair);
    let glob = pair.as_str().trim_end().ends_with('*');
    let mut inner = pair.into_inner();
    let path_pair = next_pair(ctx, &mut inner, span, "use path")?;
    let segments = path_pair
        .into_inner()
        .map(|segment| ident(ctx, &segment))
        .collect();
    Ok(UseDecl {
        segments,
        glob,
        span,
    })
}

fn const_binding(ctx: &Ctx<'_>, pair: Pair<'_, Rule>) -> Lowered<ConstBinding> {
    let span = ctx.span(&pair);
    let mut inner = pair.into_inner();
    let path_pair = next_pair(ctx, &mut inner, span, "constant path")?;
    let path = path_pair
        .into_inner()
        .map(|segment| ident(ctx, &segment))
        .collect();
    let value_pair = next_pair(ctx, &mut inner, span, "constant value")?;
    Ok(ConstBinding {
        path,
        value: expr(ctx, value_pair)?,
        span,
    })
}

fn component(ctx: &Ctx<'_>, pair: Pair<'_, Rule>) -> Lowered<ComponentDecl> {
    let span = ctx.span(&pair);
    let mut annotations = Vec::new();
    let mut decl = None;
    for part in pair.into_inner() {
        match part.as_rule() {
            Rule::annotation => annotations.push(annotation(ctx, part)?),
            Rule::component_decl => decl = Some(part),
            _ => return Err(ctx.malformed(span, "component declaration")),
        }
    }
    let decl = decl.ok_or_else(|| ctx.malformed(span, "component declaration"))?;
    component_body(ctx, decl, annotations, span)
}

/// Lowers the `component_decl` header and body, given its annotations.
fn component_body(
    ctx: &Ctx<'_>,
    decl: Pair<'_, Rule>,
    annotations: Vec<Annotation>,
    span: Span,
) -> Lowered<ComponentDecl> {
    let decl_span = ctx.span(&decl);
    let mut inner = decl.into_inner();
    let name_pair = next_pair(ctx, &mut inner, decl_span, "component name")?;
    let name = ident(ctx, &name_pair);
    let mut generics = Vec::new();
    let mut props = Vec::new();
    let mut body = None;
    for part in inner {
        match part.as_rule() {
            Rule::generic_params => generics = generic_params(ctx, part)?,
            Rule::props_block => props = props_block(ctx, part)?,
            Rule::block => body = Some(block(ctx, part)?),
            _ => return Err(ctx.malformed(decl_span, "component declaration")),
        }
    }
    let body = body.ok_or_else(|| {
        ctx.error(
            decl_span,
            format!("component `{}` has no body", name.name),
            Some("add a `{ … }` body after the component header".to_owned()),
        )
    })?;
    Ok(ComponentDecl {
        annotations,
        name,
        generics,
        props,
        body,
        span,
    })
}

fn annotation(ctx: &Ctx<'_>, pair: Pair<'_, Rule>) -> Lowered<Annotation> {
    let span = ctx.span(&pair);
    let mut inner = pair.into_inner();
    let name_pair = next_pair(ctx, &mut inner, span, "annotation name")?;
    let name = ident(ctx, &name_pair);
    let args = match inner.next() {
        Some(list) => call_args(ctx, list)?,
        None => Vec::new(),
    };
    Ok(Annotation { name, args, span })
}

fn props_block(ctx: &Ctx<'_>, pair: Pair<'_, Rule>) -> Lowered<Vec<PropDecl>> {
    let mut props = Vec::new();
    for prop in pair.into_inner() {
        let span = ctx.span(&prop);
        let mut inner = prop.into_inner();
        let name_pair = next_pair(ctx, &mut inner, span, "prop name")?;
        let name = ident(ctx, &name_pair);
        let ty_pair = next_pair(ctx, &mut inner, span, "prop type")?;
        let declared = ty(ctx, ty_pair)?;
        let default = match inner.next() {
            Some(value) => Some(expr(ctx, value)?),
            None => None,
        };
        props.push(PropDecl {
            name,
            ty: declared,
            default,
            span,
        });
    }
    Ok(props)
}

fn fn_decl(ctx: &Ctx<'_>, pair: Pair<'_, Rule>) -> Lowered<FnDecl> {
    let span = ctx.span(&pair);
    let mut inner = pair.into_inner();
    let name_pair = next_pair(ctx, &mut inner, span, "function name")?;
    let name = fn_name(ctx, &name_pair)?;
    let mut generics = Vec::new();
    let mut parameters = Vec::new();
    let mut ret = None;
    let mut body = None;
    for part in inner {
        match part.as_rule() {
            Rule::generic_params => generics = generic_params(ctx, part)?,
            Rule::params => parameters = params(ctx, part)?,
            Rule::ret_ty => ret = Some(ret_ty(ctx, part)?),
            Rule::block => body = Some(block(ctx, part)?),
            _ => return Err(ctx.malformed(span, "function declaration")),
        }
    }
    let body = body.ok_or_else(|| {
        ctx.error(
            span,
            format!("function `{}` has no body", name.text),
            Some("add a `{ … }` body after the signature".to_owned()),
        )
    })?;
    Ok(FnDecl {
        name,
        generics,
        params: parameters,
        ret,
        body,
        span,
    })
}

pub(crate) fn ret_ty(ctx: &Ctx<'_>, pair: Pair<'_, Rule>) -> Lowered<crate::ast::Type> {
    let span = ctx.span(&pair);
    let mut inner = pair.into_inner();
    let declared = next_pair(ctx, &mut inner, span, "return type")?;
    ty(ctx, declared)
}

pub(crate) fn fn_name(ctx: &Ctx<'_>, pair: &Pair<'_, Rule>) -> Lowered<FnName> {
    let span = ctx.span(pair);
    let mut inner = pair.clone().into_inner();
    let form = inner
        .next()
        .ok_or_else(|| ctx.malformed(span, "function name"))?;
    Ok(FnName {
        text: form.as_str().trim().to_owned(),
        is_operator: form.as_rule() == Rule::operator,
        span: ctx.span(&form),
    })
}

/// Lowers a `params` pair into parameters; shared with lambda lowering.
pub(crate) fn params(ctx: &Ctx<'_>, pair: Pair<'_, Rule>) -> Lowered<Vec<Param>> {
    let mut parameters = Vec::new();
    for param in pair.into_inner() {
        let span = ctx.span(&param);
        let mut inner = param.into_inner();
        let name_pair = next_pair(ctx, &mut inner, span, "parameter name")?;
        let name = ident(ctx, &name_pair);
        let mut declared = None;
        let mut default = None;
        for part in inner {
            match part.as_rule() {
                Rule::ty => declared = Some(ty(ctx, part)?),
                Rule::expr => default = Some(expr(ctx, part)?),
                _ => return Err(ctx.malformed(span, "parameter")),
            }
        }
        parameters.push(Param {
            name,
            ty: declared,
            default,
            span,
        });
    }
    Ok(parameters)
}

/// Returns the span covering the whole file pair.
pub(crate) fn file_span(ctx: &Ctx<'_>, pair: &Pair<'_, Rule>) -> Span {
    ctx.span(pair)
}
