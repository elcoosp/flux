//! Lowering of `type`, `trait` and `capability` declarations.

use pest::iterators::Pair;

use crate::ast::{CapabilityDecl, MethodSig, TraitDecl, TypeDecl, Variant};
use crate::grammar::Rule;
use crate::lower::decls::{fn_name, params, ret_ty};
use crate::lower::types::type_list;
use crate::lower::{Ctx, Lowered, generic_params, ident, next_pair};

pub(crate) fn type_decl(ctx: &Ctx<'_>, pair: Pair<'_, Rule>) -> Lowered<TypeDecl> {
    let span = ctx.span(&pair);
    let mut inner = pair.into_inner();
    let name_pair = next_pair(ctx, &mut inner, span, "type name")?;
    let name = ident(ctx, &name_pair);
    let mut generics = Vec::new();
    let mut variants = Vec::new();
    for part in inner {
        match part.as_rule() {
            Rule::generic_params => generics = generic_params(ctx, part)?,
            Rule::variant_body => variants.push(variant(ctx, part)?),
            _ => return Err(ctx.malformed(span, "type declaration")),
        }
    }
    Ok(TypeDecl {
        name,
        generics,
        variants,
        span,
    })
}

pub(crate) fn variant(ctx: &Ctx<'_>, pair: Pair<'_, Rule>) -> Lowered<Variant> {
    let span = ctx.span(&pair);
    let mut inner = pair.into_inner();
    let name_pair = next_pair(ctx, &mut inner, span, "variant name")?;
    let name = ident(ctx, &name_pair);
    let fields = match inner.next() {
        Some(list) => type_list(ctx, list)?,
        None => Vec::new(),
    };
    Ok(Variant { name, fields, span })
}

pub(crate) fn trait_decl(ctx: &Ctx<'_>, pair: Pair<'_, Rule>) -> Lowered<TraitDecl> {
    let span = ctx.span(&pair);
    let mut inner = pair.into_inner();
    let name_pair = next_pair(ctx, &mut inner, span, "trait name")?;
    let name = ident(ctx, &name_pair);
    let mut generics = Vec::new();
    let mut methods = Vec::new();
    for part in inner {
        match part.as_rule() {
            Rule::generic_params => generics = generic_params(ctx, part)?,
            Rule::method_decl => methods.push(method_sig(ctx, part)?),
            _ => return Err(ctx.malformed(span, "trait declaration")),
        }
    }
    Ok(TraitDecl {
        name,
        generics,
        methods,
        span,
    })
}

pub(crate) fn capability_decl(ctx: &Ctx<'_>, pair: Pair<'_, Rule>) -> Lowered<CapabilityDecl> {
    let span = ctx.span(&pair);
    let mut inner = pair.into_inner();
    let name_pair = next_pair(ctx, &mut inner, span, "capability name")?;
    let name = ident(ctx, &name_pair);
    let mut methods = Vec::new();
    for part in inner {
        match part.as_rule() {
            Rule::method_decl => methods.push(method_sig(ctx, part)?),
            _ => return Err(ctx.malformed(span, "capability declaration")),
        }
    }
    Ok(CapabilityDecl {
        name,
        methods,
        span,
    })
}

pub(crate) fn method_sig(ctx: &Ctx<'_>, pair: Pair<'_, Rule>) -> Lowered<MethodSig> {
    let span = ctx.span(&pair);
    let mut inner = pair.into_inner();
    let name_pair = next_pair(ctx, &mut inner, span, "method name")?;
    let name = fn_name(ctx, &name_pair)?;
    let mut generics = Vec::new();
    let mut parameters = Vec::new();
    let mut ret = None;
    for part in inner {
        match part.as_rule() {
            Rule::generic_params => generics = generic_params(ctx, part)?,
            Rule::params => parameters = params(ctx, part)?,
            Rule::ret_ty => ret = Some(ret_ty(ctx, part)?),
            _ => return Err(ctx.malformed(span, "method signature")),
        }
    }
    Ok(MethodSig {
        name,
        generics,
        params: parameters,
        ret,
        span,
    })
}
