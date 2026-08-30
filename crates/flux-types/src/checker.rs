//! The bidirectional type checker core.
//!
//! [`Checker`] walks the parsed [`Ast`] in two passes: first it collects every
//! top-level `type` declaration (ADTs) and every component/function signature
//! into the environment, then it checks each declaration's body. Expression
//! inference is bidirectional: `infer` synthesises a type for an expression, and
//! `check` verifies an expression against an expected type (used for `let`
//! annotations and calls). `let`-bound names are generalised (let-polymorphism)
//! via [`crate::scheme`].

use crate::env::{AdtDef, Binding, CtorKind, Env, VariantDef};
use crate::error::TypeError;
use crate::exhaust::check_exhaustive;
use crate::kind::{TcType, compute_node_id, decl_tag};
use crate::prelude::{prelude, primitives};
use crate::scheme::{Supply, generalise, instantiate};
use crate::traits::{admits_arithmetic, admits_equality, check_trait_bound};
use crate::unify::{UnifyError, unify_into};
use flux_parser::{Ast, BinOp, Decl, Expr, ExprKind, Ident, LetPattern, Param, Pattern, Type};
use flux_syntax::{ExprTag, NodeId, NodeTag, Span};
use std::collections::HashMap;

/// A resolved callee shape, owned so it outlives any `self.env` borrow.
#[derive(Clone)]
enum CalleeShape {
    /// A single-variant ADT constructor, with `field_count` payload fields.
    Adt {
        /// Number of payload fields on the (single) variant.
        field_count: usize,
    },
    /// A component, possibly generic.
    Component {
        /// Whether the component is generic (so an instantiation is recorded).
        generic: bool,
        /// Prop signatures with generic params as `Var(PARAM_BASE + i)`, used to
        /// pin the generic parameters from concrete call-site arguments.
        props: Vec<(String, TcType)>,
    },
    /// A record-type constructor: builds a `TcType::Record` from named fields.
    Record {
        /// Field name → type, in declaration order.
        fields: Vec<(String, TcType)>,
    },
}

/// A record of a generic instantiation discovered during checking.
///
/// Lowering needs this to emit specialised bytecode per instantiation (spec
/// §18.2 / §20.3). For an applied component or ADT constructor such as
/// `Counter[Int]`, `generic_args` holds the concrete argument types.
#[derive(Clone, Debug, PartialEq)]
pub struct GenericInstantiation {
    /// The name of the instantiated generic (component or ADT), e.g. `Counter`.
    pub name: String,
    /// Concrete type arguments, e.g. `[Int, Float]`.
    pub generic_args: Vec<TcType>,
}

/// The type checker state threaded through a single [`type_check`](crate::type_check) run.
#[derive(Debug, Default)]
pub struct Checker {
    pub(crate) env: Env,
    supply: Supply,
    subst: HashMap<u32, TcType>,
    /// Per-expression/declaration inferred types, keyed by derived [`NodeId`].
    pub types: HashMap<NodeId, TcType>,
    /// Every generic instantiation encountered.
    pub instantiations: Vec<GenericInstantiation>,
    /// Resolved field position (0-based, declaration order) for each
    /// `base.field` expression, keyed by the expression's `NodeId`. Mirrors the
    /// `types` map but for the positional index the VM expects when emitting
    /// `GET_FIELD`/`SET_FIELD` over a record stored as a positional
    /// `Vec<(PropIdx, Value)>` (FLUX-072).
    pub field_indices: HashMap<NodeId, u16>,
    /// Primitive scalar names.
    prims: std::collections::HashSet<String>,
    /// Generic parameter names in scope, mapped to their unification variable.
    /// Consulted by [`Self::conv_ty`] so a surface `T` resolves to the bound
    /// variable rather than an unresolved `Named("T", [])`.
    generics: std::collections::HashMap<String, TcType>,
}

impl Checker {
    /// Creates a checker with the prelude preloaded.
    #[must_use]
    pub fn new() -> Self {
        let mut supply = Supply::default();
        let env = prelude(&mut supply);
        let prims = primitives();
        Self {
            env,
            supply,
            subst: HashMap::new(),
            types: HashMap::new(),
            instantiations: Vec::new(),
            field_indices: HashMap::new(),
            prims,
            generics: HashMap::new(),
        }
    }

    /// Records the inferred type for `span` under the standard structural node id.
    fn record(&mut self, tag: impl NodeTag, span: Span, ty: &TcType) -> NodeId {
        let id = compute_node_id(0, tag, span, None);
        self.types.insert(id, ty.clone());
        id
    }

    /// Fully applies the current substitution to `ty`.
    fn resolve(&self, ty: &TcType) -> TcType {
        let mut out = ty.clone();
        // Zeronk repeated passes until stable.
        for _ in 0..4 {
            let next = out.apply(&self.subst);
            if next == out {
                break;
            }
            out = next;
        }
        out
    }

    fn fresh(&mut self) -> u32 {
        self.supply.fresh()
    }

    fn fresh_ty(&mut self) -> TcType {
        TcType::Var(self.fresh())
    }

    /// Unifies `found` against `expected`, reporting a precise mismatch error.
    fn expect(&mut self, expected: &TcType, found: &TcType, span: Span) -> Result<(), TypeError> {
        let expected = self.resolve(expected);
        let found = self.resolve(found);
        let mut subst = self.subst.clone();
        match unify_into(&expected, &found, &mut subst) {
            Ok(()) => {
                self.subst = subst;
                Ok(())
            }
            Err(UnifyError::Mismatch(_, _)) => {
                let (e, f) = (expected.clone(), found.clone());
                let msg = format!("expected `{e}`, got `{f}`");
                Err(TypeError::mismatch(&e, &f, span).with_hint(msg))
            }
            Err(UnifyError::Recursive(_)) => {
                Err(TypeError::new("occurs check failed: infinite type", span))
            }
        }
    }

    /// Internal: resolve a surface [`Type`] into a [`TcType`].
    fn conv_ty(&self, ty: &Type) -> TcType {
        let resolved = TcType::from_surface(ty, &self.prims);
        // Rewrite unresolved `Named(g, [])` that matches an in-scope generic
        // parameter to the bound unification variable.
        if let TcType::Named(name, args) = &resolved {
            if args.is_empty() {
                if let Some(var) = self.generics.get(name) {
                    return var.clone();
                }
            }
        }
        resolved
    }

    /// Infers the type of `expr`, recording it under the expression node id.
    fn infer(&mut self, expr: &Expr) -> Result<TcType, TypeError> {
        let ty = self.infer_inner(expr)?;
        self.record(ExprTag(10), expr.span, &ty);
        Ok(ty)
    }

    fn infer_inner(&mut self, expr: &Expr) -> Result<TcType, TypeError> {
        match &expr.kind {
            ExprKind::Int(_) => Ok(TcType::Int),
            ExprKind::Float(_) => Ok(TcType::Float),
            ExprKind::Bool(_) => Ok(TcType::Bool),
            ExprKind::Str(parts) => {
                for part in parts {
                    if let flux_parser::StrPart::Interp(inner) = part {
                        let ty = self.infer(inner)?;
                        // Unification variables and constrained (generic) types
                        // may satisfy `Show` at a concrete instantiation, so they
                        // are accepted opaquely rather than rejected here.
                        let ok = matches!(&ty, TcType::Var(_) | TcType::Constrained(_, _)) || {
                            self.env.push_scope();
                            let bound = check_trait_bound("Show", &ty, inner.span).is_ok();
                            self.env.pop_scope();
                            bound
                        };
                        if !ok {
                            return Err(TypeError::new(
                                "interpolated value does not implement `Show`",
                                inner.span,
                            )
                            .with_hint(
                                "only Int, Float, Bool, String and Show types may be \
                                 interpolated into a string literal"
                                    .to_owned(),
                            ));
                        }
                    }
                }
                Ok(TcType::String)
            }
            ExprKind::List(items) => {
                let element = self.fresh_ty();
                for item in items {
                    let item_ty = self.infer(item)?;
                    self.expect(&element, &item_ty, item.span)?;
                }
                Ok(TcType::List(Box::new(element)))
            }
            ExprKind::Null => {
                // The `Null` literal (FLUX-053 / ADR-0051) inhabits every
                // `Option[T]`; its element type is left as a fresh variable so it
                // unifies with whatever `Option[...]` the context expects.
                Ok(TcType::Option(Box::new(self.fresh_ty())))
            }
            ExprKind::Ident(ident) => self.lookup_value(&ident.name, ident.span),
            ExprKind::Elided => Ok(TcType::Unit),
            ExprKind::Record { name, fields } => {
                let _ = name;
                let mut field_tys = Vec::with_capacity(fields.len());
                for (fname, fval) in fields {
                    let fty = self.infer(fval)?;
                    field_tys.push((fname.name.clone(), Box::new(fty)));
                }
                Ok(TcType::Record(field_tys))
            }
            ExprKind::Binary { op, lhs, rhs } => {
                let l = self.infer(lhs)?;
                let r = self.infer(rhs)?;
                self.check_binary(*op, &l, &r, expr.span)
            }
            ExprKind::Field { base, field } => {
                // Module-level associated constant access, e.g. `Color.red`:
                // the base is an identifier and the dot-path names a constant
                // registered under `"Color.red"`.
                if let ExprKind::Ident(base_ident) = &base.kind {
                    let const_name = format!("{}.{}", base_ident.name, field.name);
                    if let Some(Binding::Mono(ty)) = self.env.lookup(&const_name) {
                        return Ok(ty.clone());
                    }
                }
                let base_ty = self.infer(base)?;
                let base_ty = self.resolve(&base_ty);
                match &base_ty {
                    TcType::Record(fields) => {
                        if let Some((pos, (_, ty))) = fields
                            .iter()
                            .enumerate()
                            .find(|(_, (n, _))| n == &field.name)
                        {
                            // Record the resolved positional index so the
                            // bytecode emitter can emit GET_FIELD with the slot
                            // the VM expects (records are positional).
                            let fid = compute_node_id(0, ExprTag(10), expr.span, None);
                            self.field_indices.insert(fid, pos as u16);
                            Ok((**ty).clone())
                        } else {
                            Err(TypeError::new(
                                format!("no field `{}` on record", field.name),
                                field.span,
                            )
                            .with_hint(format!(
                                "record has fields: {}",
                                fields
                                    .iter()
                                    .map(|(n, _)| n.as_str())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            )))
                        }
                    }
                    TcType::Named(name, _) => {
                        // A named record type: resolve its fields from the
                        // registered record constructor so field access (and
                        // the bytecode field index) works through the nominal
                        // type, not just the structural `Record` form.
                        if let Some(Binding::Ctor(CtorKind::Record { fields })) =
                            self.env.lookup(name)
                        {
                            if let Some((pos, (_, ty))) = fields
                                .iter()
                                .enumerate()
                                .find(|(_, (n, _))| n == &field.name)
                            {
                                let fid = compute_node_id(0, ExprTag(10), expr.span, None);
                                self.field_indices.insert(fid, pos as u16);
                                Ok(ty.clone())
                            } else {
                                Err(TypeError::new(
                                    format!("no field `{}` on record `{name}`", field.name),
                                    field.span,
                                )
                                .with_hint(format!(
                                    "record has fields: {}",
                                    fields
                                        .iter()
                                        .map(|(n, _)| n.as_str())
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                )))
                            }
                        } else {
                            Ok(self.fresh_ty())
                        }
                    }
                    TcType::Var(_)
                    | TcType::Constrained(_, _)
                    | TcType::Variant(_, _)
                    | TcType::Fn(_, _)
                    | TcType::List(_)
                    | TcType::Option(_)
                    | TcType::Map(_, _) => Ok(self.fresh_ty()),
                    other => Err(TypeError::new(
                        format!("cannot access field `{}` on `{other}`", field.name),
                        field.span,
                    )
                    .with_hint("field access requires a record type".to_owned())),
                }
            }
            ExprKind::OptField { base, field } => {
                // Null-safe access (FLUX-053 / ADR-0051). The base type must be
                // `Option[T]`; the result widens the field's type to
                // `Option[...]` because the chain short-circuits to `Null` when
                // the base is `Null`.
                let base_ty = self.infer(base)?;
                let base_ty = self.resolve(&base_ty);
                match &base_ty {
                    TcType::Option(inner) => {
                        // Access the field on the unwrapped inner type, then
                        // wrap the result back into `Option`.
                        let inner_ty = match &**inner {
                            TcType::Record(fields) => {
                                if let Some((_, ty)) = fields.iter().find(|(n, _)| n == &field.name)
                                {
                                    (**ty).clone()
                                } else {
                                    return Err(TypeError::new(
                                        format!("no field `{}` on record", field.name),
                                        field.span,
                                    )
                                    .with_hint(format!(
                                        "record has fields: {}",
                                        fields
                                            .iter()
                                            .map(|(n, _)| n.as_str())
                                            .collect::<Vec<_>>()
                                            .join(", ")
                                    )));
                                }
                            }
                            // Opaque inner types expose an open surface, like
                            // `Field` above: the accessed type is a fresh var,
                            // widened back into `Option`.
                            TcType::Var(_)
                            | TcType::Constrained(_, _)
                            | TcType::Named(_, _)
                            | TcType::Variant(_, _)
                            | TcType::Fn(_, _)
                            | TcType::List(_)
                            | TcType::Map(_, _) => self.fresh_ty(),
                            other => {
                                return Err(TypeError::new(
                                    format!("cannot access field `{}` on `{}`", field.name, other),
                                    field.span,
                                )
                                .with_hint(
                                    "optional field access requires a record type".to_owned(),
                                ));
                            }
                        };
                        Ok(TcType::Option(Box::new(inner_ty)))
                    }
                    // A base already known to be non-nullable (concrete record
                    // or scalar) cannot be null-safe-chained: `?.` is only
                    // meaningful over `Option`.
                    TcType::Record(_)
                    | TcType::Int
                    | TcType::Bool
                    | TcType::Float
                    | TcType::String => Err(TypeError::new(
                        "`?.` requires an Option base".to_owned(),
                        base.span,
                    )
                    .with_hint(
                        "optional chaining can only be applied to a nullable (Option) value"
                            .to_owned(),
                    )),
                    // Unresolved / opaque non-Option bases: be permissive and
                    // return `Option[fresh]` so adapter-type chains don't reject
                    // otherwise well-formed programs (mirrors `Field`).
                    TcType::Var(_)
                    | TcType::Constrained(_, _)
                    | TcType::Named(_, _)
                    | TcType::Variant(_, _)
                    | TcType::Fn(_, _)
                    | TcType::List(_)
                    | TcType::Map(_, _) => Ok(TcType::Option(Box::new(self.fresh_ty()))),
                    other => Err(TypeError::new(
                        format!("`?.` requires an Option base, found `{}`", other),
                        base.span,
                    )
                    .with_hint(
                        "optional chaining can only be applied to a nullable (Option) value"
                            .to_owned(),
                    )),
                }
            }
            ExprKind::Call {
                callee,
                args,
                trailing,
            } => self.infer_call(callee, args, trailing.as_deref(), expr.span),
            ExprKind::Let { pattern, value } => {
                let value_ty = match value {
                    Some(v) => self.infer(v)?,
                    None => TcType::Unit,
                };
                self.bind_let(pattern, &value_ty)?;
                Ok(TcType::Unit)
            }
            ExprKind::Assign { target, value } => {
                let target_ty = self.infer(target)?;
                let value_ty = self.infer(value)?;
                self.expect(&target_ty, &value_ty, value.span)?;
                Ok(TcType::Unit)
            }
            ExprKind::If {
                cond,
                then_block,
                else_branch,
            } => {
                let cond_ty = self.infer(cond)?;
                self.expect(&TcType::Bool, &cond_ty, cond.span)?;
                let then_ty = self.infer_block(then_block)?;
                match else_branch {
                    Some(other) => {
                        // An `else { block }` is lowered as a zero-argument
                        // lambda — the grammar's "block as expression" form
                        // (see `block_expr` in the parser). Infer its body
                        // block directly so its value unifies with the `then`
                        // branch's block value. A nested `else if …` arrives as
                        // a real `If` expression and takes the normal path.
                        let else_ty = match &other.kind {
                            ExprKind::Lambda { params, body } if params.is_empty() => {
                                self.infer_block(body)?
                            }
                            _ => self.infer(other)?,
                        };
                        self.expect(&then_ty, &else_ty, other.span)?;
                        Ok(then_ty)
                    }
                    None => Ok(TcType::Unit),
                }
            }
            ExprKind::When {
                cond,
                then_block,
                otherwise,
            } => {
                let cond_ty = self.infer(cond)?;
                self.expect(&TcType::Bool, &cond_ty, cond.span)?;
                self.infer_block(then_block)?;
                if let Some(other) = otherwise {
                    self.infer_block(other)?;
                }
                Ok(TcType::Unit)
            }
            ExprKind::Match { scrutinee, arms } => {
                let scr_ty = self.infer(scrutinee)?;
                let scr_ty = self.resolve(&scr_ty);
                // Check exhaustiveness against the ADT before checking bodies.
                if let Err(err) = check_exhaustive(&self.env, &scr_ty, arms) {
                    return Err(TypeError::new(
                        format!(
                            "non-exhaustive match: missing variants {}",
                            err.missing.join(", ")
                        ),
                        err.span,
                    )
                    .with_hint(
                        "add arms for every variant, or a trailing `_` wildcard".to_owned(),
                    ));
                }
                // Infer each arm body once and unify their types.
                let mut result_ty = TcType::Unit;
                let mut first = true;
                for arm in arms {
                    self.env.push_scope();
                    self.bind_pattern_ty(&arm.pattern.kind, &scr_ty, arm.pattern.span)?;
                    let body_ty = self.infer(&arm.body)?;
                    self.env.pop_scope();
                    if first {
                        result_ty = body_ty;
                        first = false;
                    } else {
                        self.expect(&result_ty, &body_ty, arm.body.span)?;
                        result_ty = self.resolve(&result_ty);
                    }
                }
                Ok(result_ty)
            }
            ExprKind::ForEach { items, key, body } => {
                let items_ty = self.infer(items)?;
                let items_ty = self.resolve(&items_ty);
                let element = match &items_ty {
                    TcType::List(inner) => (**inner).clone(),
                    TcType::Var(_) => self.fresh_ty(),
                    other => {
                        return Err(TypeError::new(
                            format!("`ForEach` expects a `List`, got `{other}`"),
                            items.span,
                        )
                        .with_hint("the first argument to ForEach must be a list".to_owned()));
                    }
                };
                // key function: fn(item) -> key
                self.infer(key)?;
                self.env.push_scope();
                // The block's closure parameters bind the element.
                if let Some(param) = body.params.first() {
                    self.bind_simple_pattern(param, &element)?;
                }
                self.infer_block(body)?;
                self.env.pop_scope();
                Ok(TcType::Unit)
            }
            ExprKind::Provide { context, value } => {
                let _ = self.infer(value)?;
                let _ = context;
                Ok(TcType::Unit)
            }
            ExprKind::UseContext(ident) => {
                // `useContext(RouterContext)` yields the context value.
                match self.env.lookup(&ident.name) {
                    Some(_) => Ok(TcType::Named(ident.name.clone(), Vec::new())),
                    None => Err(TypeError::new(
                        format!("unknown context `{}`", ident.name),
                        ident.span,
                    )),
                }
            }
            ExprKind::Lambda { params, body } => self.infer_lambda(params, body),
            ExprKind::Lifecycle { kind, body } => {
                let _ = kind;
                self.infer_block(body)?;
                Ok(TcType::Unit)
            }
            ExprKind::Resource(expr) => {
                let _ = self.infer(expr)?;
                // `resource(fn { ... })` yields a 2-tuple `(value, { refetch })`
                // so `let (users, { refetch }) = ...` destructures by tuple
                // position; `value` is left polymorphic.
                Ok(TcType::Record(vec![
                    ("0".to_owned(), Box::new(self.fresh_ty())),
                    (
                        "1".to_owned(),
                        Box::new(TcType::Record(vec![(
                            "refetch".to_owned(),
                            Box::new(TcType::Fn(vec![], Box::new(TcType::Unit))),
                        )])),
                    ),
                ]))
            }
            ExprKind::CreateRef { args } => {
                let arg = if let Some(first) = args.first() {
                    self.conv_ty(first)
                } else {
                    self.fresh_ty()
                };
                Ok(TcType::Named("Ref".to_owned(), vec![arg]))
            }
            _ => Ok(TcType::Unit),
        }
    }

    fn infer_lambda(
        &mut self,
        params: &[Param],
        body: &flux_parser::Block,
    ) -> Result<TcType, TypeError> {
        self.env.push_scope();
        let mut param_tys = Vec::with_capacity(params.len());
        for param in params {
            let ty = match &param.ty {
                Some(decl_ty) => self.conv_ty(decl_ty),
                None => self.fresh_ty(),
            };
            self.env
                .insert(param.name.name.clone(), Binding::Mono(ty.clone()));
            param_tys.push(ty);
        }
        let ret = self.infer_block(body)?;
        self.env.pop_scope();
        Ok(TcType::Fn(param_tys, Box::new(ret)))
    }

    fn infer_call(
        &mut self,
        callee: &Expr,
        args: &[flux_parser::Arg],
        trailing: Option<&flux_parser::Block>,
        span: Span,
    ) -> Result<TcType, TypeError> {
        // A trailing block (component body) is type-checked for internal
        // errors even though its result does not change the call's type.
        if let Some(block) = trailing {
            let _bt = self.infer_block(block)?;
        }
        // `Numeric.zero()` / `Numeric.one()` — trait method resolution.
        if let ExprKind::Ident(ident) = &callee.kind {
            if ident.name == "Numeric" && !args.is_empty() {
                if let Some(flux_parser::Arg::Named { name, .. }) = args.first() {
                    if name.name == "zero" || name.name == "one" {
                        // The result is the trait's associated `T`. We return a
                        // fresh variable constrained by `Numeric` so that a
                        // later use (e.g. assignment to `Int`) pins it, while an
                        // assignment to a non-`Numeric` type is rejected by
                        // `check_trait_bound`.
                        if !matches!(self.env.lookup("Numeric"), Some(Binding::Trait(_))) {
                            return Err(TypeError::new(
                                "trait `Numeric` is not in scope".to_owned(),
                                ident.span,
                            ));
                        }
                        let id = self.fresh();
                        return Ok(TcType::Constrained(id, vec!["Numeric".to_owned()]));
                    }
                }
            }
            // Plain function / constructor call.
            let callee_ty = self.infer(callee)?;
            return self.apply_callee(&callee_ty, args, trailing, span);
        }
        let callee_ty = self.infer(callee)?;
        self.apply_callee(&callee_ty, args, trailing, span)
    }

    fn apply_callee(
        &mut self,
        callee_ty: &TcType,
        args: &[flux_parser::Arg],
        _trailing: Option<&flux_parser::Block>,
        span: Span,
    ) -> Result<TcType, TypeError> {
        // Resolve the callee shape into owned data first, so no `self.env`
        // borrow is live when we recursively infer argument expressions.
        let shape = match callee_ty {
            TcType::Named(name, inner) if inner.is_empty() => {
                let name = name.clone();
                // A variant constructor is keyed directly by name.
                if let Some((_adt_name, variant)) = self.env.variants.get(&name) {
                    Some(CalleeShape::Adt {
                        field_count: variant.fields.len(),
                    })
                } else {
                    match self.env.lookup(&name) {
                        Some(Binding::Ctor(CtorKind::Adt(adt))) => {
                            let field_count =
                                adt.variants.first().map(|v| v.fields.len()).unwrap_or(0);
                            Some(CalleeShape::Adt { field_count })
                        }
                        Some(Binding::Ctor(CtorKind::Component { params, props })) => {
                            Some(CalleeShape::Component {
                                generic: !params.is_empty(),
                                props: props.clone(),
                            })
                        }
                        Some(Binding::Ctor(CtorKind::Record { fields })) => {
                            Some(CalleeShape::Record {
                                fields: fields.clone(),
                            })
                        }
                        _ => None,
                    }
                }
            }
            _ => None,
        };

        match callee_ty {
            TcType::Named(name, inner) if inner.is_empty() => {
                let name = name.clone();
                match shape {
                    Some(CalleeShape::Adt { field_count }) => {
                        let provided: Vec<TcType> = args
                            .iter()
                            .map(|a| self.infer(a.value()))
                            .collect::<Result<_, _>>()?;
                        if provided.len() != field_count {
                            return Err(TypeError::new(
                                format!(
                                    "constructor `{name}` expects {field_count} argument(s), got {}",
                                    provided.len()
                                ),
                                span,
                            ));
                        }
                        // A variant constructor produces a value of the *ADT*
                        // type. For a non-generic ADT the outer type is opaque
                        // (`Named(adt, [])`); for a generic ADT (e.g. `Result`)
                        // the concrete payload types are recovered at `match`
                        // time via `bind_pattern_ty`'s variant-field binding,
                        // so we do not widen the constructor type here (that
                        // would break multi-variant ADTs whose variants carry
                        // differing field arities, e.g. `Shape`). FLUX-055.
                        let adt_name = self
                            .env
                            .variants
                            .get(&name)
                            .map(|(adt, _)| adt.clone())
                            .unwrap_or_else(|| name.clone());
                        Ok(TcType::Named(adt_name, Vec::new()))
                    }
                    Some(CalleeShape::Component { generic, props }) => {
                        // Determine the generic parameter variables used in the
                        // prop types (they appear as `Var(PARAM_BASE + i)`).
                        let mut param_vars: Vec<u32> = Vec::new();
                        for (_, pty) in &props {
                            collect_param_vars(pty, &mut param_vars);
                        }
                        param_vars.sort_unstable();
                        param_vars.dedup();
                        // Fresh inference variable per generic param, with a
                        // substitution from the PARAM_BASE var to the fresh one.
                        let mut subst: HashMap<u32, TcType> = HashMap::new();
                        let mut tvars: Vec<TcType> = Vec::new();
                        for v in &param_vars {
                            let fresh = self.fresh_ty();
                            subst.insert(*v, fresh.clone());
                            tvars.push(fresh);
                        }
                        // Unify each call argument against its declared prop
                        // type (with generic vars substituted), pinning the
                        // generic parameters to concrete call-site types.
                        for arg in args {
                            let arg_ty = self.infer(arg.value())?;
                            let decl_ty = match arg {
                                flux_parser::Arg::Named { name, .. } => props
                                    .iter()
                                    .find(|(n, _)| n == &name.name)
                                    .map(|(_, t)| t.clone()),
                                flux_parser::Arg::Positional { .. } => None,
                                #[allow(unreachable_patterns)]
                                _ => None,
                            };
                            if let Some(decl_ty) = decl_ty {
                                let resolved = decl_ty.apply(&subst);
                                let _ = self.expect(&resolved, &arg_ty, arg.value().span);
                            }
                        }
                        if generic && !tvars.is_empty() {
                            let generic_args: Vec<TcType> =
                                tvars.iter().map(|t| self.resolve(t)).collect();
                            self.instantiations.push(GenericInstantiation {
                                name: name.clone(),
                                generic_args,
                            });
                        }
                        // Component calls render; they do not produce a value
                        // Expression-level, so they type as `Unit`.
                        Ok(TcType::Unit)
                    }
                    Some(CalleeShape::Record { fields }) => {
                        // Verify every supplied argument names a known field and
                        // matches its declared type, then build the record type.
                        for arg in args {
                            let fname = match arg {
                                flux_parser::Arg::Named { name, .. } => &name.name,
                                flux_parser::Arg::Positional { .. } => {
                                    return Err(TypeError::new(
                                        "record construction requires named fields (`Task(label: …)`)".to_owned(),
                                        span,
                                    ));
                                }
                                #[allow(unreachable_patterns)]
                                _ => {
                                    return Err(TypeError::new(
                                        "record construction requires named fields".to_owned(),
                                        span,
                                    ));
                                }
                            };
                            let decl_ty = fields
                                .iter()
                                .find(|(n, _)| n == fname)
                                .map(|(_, t)| t.clone());
                            let Some(decl_ty) = decl_ty else {
                                return Err(TypeError::new(
                                    format!("`{fname}` is not a field of record `{name}`"),
                                    span,
                                ));
                            };
                            let arg_ty = self.infer(arg.value())?;
                            let _ = self.expect(&decl_ty, &arg_ty, arg.value().span);
                        }
                        Ok(TcType::Named(name.clone(), Vec::new()))
                    }
                    _ => Err(TypeError::new(
                        format!("`{name}` is not a callable constructor"),
                        span,
                    )),
                }
            }
            TcType::Fn(params, ret) => {
                let mut provided = Vec::with_capacity(args.len());
                for arg in args {
                    provided.push(self.infer(arg.value())?);
                }
                if provided.len() != params.len() {
                    return Err(TypeError::new(
                        format!(
                            "function expects {} argument(s), got {}",
                            params.len(),
                            provided.len()
                        ),
                        span,
                    ));
                }
                for (exp, got) in params.iter().zip(&provided) {
                    self.expect(exp, got, span)?;
                }
                Ok((**ret).clone())
            }
            TcType::Var(_) | TcType::Constrained(_, _) => {
                for arg in args {
                    self.infer(arg.value())?;
                }
                Ok(self.fresh_ty())
            }
            other => Err(TypeError::new(
                format!("`{other}` is not a function or constructor"),
                span,
            )
            .with_hint("only functions, components and ADT constructors can be called".to_owned())),
        }
    }

    fn check_binary(
        &mut self,
        op: BinOp,
        l: &TcType,
        r: &TcType,
        span: Span,
    ) -> Result<TcType, TypeError> {
        let l = self.resolve(l);
        let r = self.resolve(r);
        match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem => {
                // Arithmetic on numbers; `+` also serves as list append and
                // string concatenation in the surface language.
                match (&l, &r) {
                    (TcType::List(_), TcType::List(_)) if op == BinOp::Add => Ok(l),
                    (TcType::String, TcType::String) if op == BinOp::Add => Ok(TcType::String),
                    _ => {
                        // Reject non-numeric operands *before* unifying, so a
                        // `Show`-constrained variable used in arithmetic is
                        // reported instead of being silently unified to `Int`.
                        if !admits_arithmetic(&l) || !admits_arithmetic(&r) {
                            return Err(TypeError::new(
                                format!("operator `{op:?}` requires a Numeric type, got `{l}`"),
                                span,
                            )
                            .with_hint(
                                "the operands must be Int or Float (satisfying Numeric)".to_owned(),
                            ));
                        }
                        self.expect(&l, &r, span)?;
                        Ok(l)
                    }
                }
            }
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => {
                self.expect(&l, &r, span)?;
                if !admits_equality(&l) {
                    return Err(TypeError::new(
                        format!("operator `{op:?}` requires an Eq type, got `{l}`"),
                        span,
                    ));
                }
                Ok(TcType::Bool)
            }
            BinOp::And | BinOp::Or => {
                self.expect(&TcType::Bool, &l, span)?;
                self.expect(&TcType::Bool, &r, span)?;
                Ok(TcType::Bool)
            }
            _ => Ok(TcType::Bool),
        }
    }

    fn lookup_value(&mut self, name: &str, span: Span) -> Result<TcType, TypeError> {
        // A `$name` identifier is a two-way binding sigil: it resolves to the
        // underlying signal `name` (the `$` is stripped for type checking;
        // the write-back is emitted by the lowering pass). FLUX-072 #4.
        if let Some(bare) = name.strip_prefix('$') {
            if let Some(ty) = self.try_lookup_value(bare) {
                return Ok(ty);
            }
        }
        match self.env.lookup(name) {
            Some(Binding::Mono(ty)) => Ok(ty.clone()),
            Some(Binding::Poly(scheme)) => {
                let inst = instantiate(scheme, &mut self.supply);
                Ok(inst)
            }
            Some(Binding::Ctor(_)) => {
                // Constructors and components resolve to a nominal type so
                // they can be applied as callees (e.g. `Text(...)`,
                // `Circle(5.0)`). Field/param arity is checked in `apply_callee`.
                Ok(TcType::Named(name.to_owned(), Vec::new()))
            }
            Some(Binding::Trait(_)) => Ok(TcType::Named(name.to_owned(), Vec::new())),
            None => Err(
                TypeError::new(format!("unbound name `{name}`"), span).with_hint(
                    "declare it with `let`, `state`, or bring it into scope via `import`"
                        .to_owned(),
                ),
            ),
        }
    }

    /// Non-failing variant of [`Self::lookup_value`]: returns the type of
    /// `name` if bound, without producing a type error. Used to resolve the
    /// `$name` two-way binding sigil. FLUX-072 #4.
    fn try_lookup_value(&self, name: &str) -> Option<TcType> {
        match self.env.lookup(name) {
            Some(Binding::Mono(ty)) => Some(ty.clone()),
            Some(Binding::Poly(scheme)) => Some(instantiate(scheme, &mut self.supply.clone())),
            _ => None,
        }
    }

    fn bind_let(&mut self, pattern: &LetPattern, value_ty: &TcType) -> Result<(), TypeError> {
        match pattern {
            LetPattern::Ident(ident) => {
                let env_free = self.env.free_vars();
                let scheme = generalise(value_ty, &env_free);
                self.env.insert(ident.name.clone(), Binding::Poly(scheme));
                Ok(())
            }
            LetPattern::Tuple(patterns) => {
                // Tuple types are modelled as records keyed by index.
                self.bind_tuple(patterns, value_ty)
            }
            LetPattern::Record(fields) => self.bind_record_let(fields, value_ty),
            _ => Ok(()),
        }
    }

    fn bind_tuple(&mut self, patterns: &[LetPattern], value_ty: &TcType) -> Result<(), TypeError> {
        let value_ty = self.resolve(value_ty);
        let fields: Vec<(String, Box<TcType>)> = match &value_ty {
            TcType::Record(fs) => fs
                .iter()
                .enumerate()
                .map(|(i, (_, t))| (i.to_string(), t.clone()))
                .collect(),
            _ => {
                return Err(TypeError::new(
                    format!("cannot destructure a non-tuple type `{value_ty}`"),
                    Span::new(0, 0, 0),
                ));
            }
        };
        for (i, pat) in patterns.iter().enumerate() {
            let Some(ty) = fields
                .iter()
                .find(|(k, _)| k == &i.to_string())
                .map(|(_, t)| &**t)
            else {
                return Err(TypeError::new(
                    format!("tuple has no element at index {i}"),
                    Span::new(0, 0, 0),
                ));
            };
            self.bind_let(pat, ty)?;
        }
        Ok(())
    }

    fn bind_record_let(&mut self, fields: &[Ident], value_ty: &TcType) -> Result<(), TypeError> {
        let value_ty = self.resolve(value_ty);
        for field in fields {
            let found = match &value_ty {
                TcType::Record(fs) => fs.iter().find(|(n, _)| n == &field.name).map(|(_, t)| &**t),
                TcType::Named(_, _) => Some(&value_ty),
                _ => None,
            };
            let Some(ty) = found else {
                return Err(TypeError::new(
                    format!("no field `{}` to bind in let", field.name),
                    field.span,
                ));
            };
            let env_free = self.env.free_vars();
            let scheme = generalise(ty, &env_free);
            self.env.insert(field.name.clone(), Binding::Poly(scheme));
        }
        Ok(())
    }

    fn bind_simple_pattern(&mut self, pattern: &Pattern, ty: &TcType) -> Result<(), TypeError> {
        match pattern {
            Pattern::Ident(ident) => {
                let env_free = self.env.free_vars();
                let scheme = generalise(ty, &env_free);
                self.env.insert(ident.name.clone(), Binding::Poly(scheme));
                Ok(())
            }
            Pattern::Wildcard(_) => Ok(()),
            _ => Ok(()),
        }
    }

    fn bind_pattern_ty(
        &mut self,
        kind: &flux_parser::MatchPatternKind,
        scr_ty: &TcType,
        span: Span,
    ) -> Result<(), TypeError> {
        match kind {
            flux_parser::MatchPatternKind::Wildcard => Ok(()),
            flux_parser::MatchPatternKind::Variant { name, fields } => {
                // Find the variant in the ADT of scr_ty.
                let adt_name = match scr_ty {
                    TcType::Variant(n, _) | TcType::Named(n, _) => n.clone(),
                    _ => return Ok(()),
                };
                let Some(Binding::Ctor(CtorKind::Adt(adt))) = self.env.lookup(&adt_name) else {
                    return Ok(());
                };
                let Some(def) = adt.variants.iter().find(|v| v.name == name.name) else {
                    return Err(TypeError::new(
                        format!("variant `{}` does not belong to `{}`", name.name, adt_name),
                        name.span,
                    ));
                };
                if def.fields.len() != fields.len() {
                    return Err(TypeError::new(
                        format!(
                            "variant `{}` expects {} field(s), got {}",
                            name.name,
                            def.fields.len(),
                            fields.len()
                        ),
                        span,
                    ));
                }
                // Clone the field types out of `adt` so the `self.env` borrow
                // ends before we recursively bind sub-patterns.
                let field_tys = def.fields.clone();
                for (field_ty, pat) in field_tys.iter().zip(fields) {
                    self.bind_simple_pattern(pat, field_ty)?;
                }
                Ok(())
            }
            flux_parser::MatchPatternKind::Literal(_) => Ok(()),
            flux_parser::MatchPatternKind::Guard { name, .. } => {
                self.bind_simple_pattern(&Pattern::Ident(name.clone()), scr_ty)
            }
            _ => Ok(()),
        }
    }

    fn infer_block(&mut self, block: &flux_parser::Block) -> Result<TcType, TypeError> {
        let _ = block.params; // closure params handled by callers
        let mut last = TcType::Unit;
        for item in &block.items {
            last = match item {
                flux_parser::BlockItem::State(decl) => {
                    let init_ty = self.infer(&decl.init)?;
                    if let Some(decl_ty) = &decl.ty {
                        let expected = self.conv_ty(decl_ty);
                        self.expect(&expected, &init_ty, decl.init.span)?;
                    }
                    let resolved = init_ty.apply(&self.subst);
                    self.env
                        .insert(decl.name.name.clone(), Binding::Mono(resolved));
                    TcType::Unit
                }
                flux_parser::BlockItem::Derived(decl) => {
                    // A derived signal is a read-only computed binding: it reads
                    // like a signal but re-derives from its sources. Type it as
                    // the inferred body type and bind it into scope (FLUX-072 #12).
                    let init_ty = self.infer(&decl.init)?;
                    if let Some(decl_ty) = &decl.ty {
                        let expected = self.conv_ty(decl_ty);
                        self.expect(&expected, &init_ty, decl.init.span)?;
                    }
                    let resolved = init_ty.apply(&self.subst);
                    self.env
                        .insert(decl.name.name.clone(), Binding::Mono(resolved));
                    TcType::Unit
                }
                flux_parser::BlockItem::Prop { .. } => TcType::Unit,
                flux_parser::BlockItem::Expr(expr) => self.infer(expr)?,
                _ => TcType::Unit,
            };
        }
        Ok(last)
    }
}

/// Collects the `Var(PARAM_BASE + i)` generic-parameter variables referenced
/// inside `ty` into `out` (used to discover a component's generic arity from
/// its prop types).
fn collect_param_vars(ty: &TcType, out: &mut Vec<u32>) {
    match ty {
        TcType::Var(id) | TcType::Constrained(id, _) => {
            if *id >= crate::env::PARAM_BASE {
                out.push(*id);
            }
        }
        TcType::List(inner) => collect_param_vars(inner, out),
        TcType::Option(inner) => collect_param_vars(inner, out),
        TcType::Map(k, v) => {
            collect_param_vars(k, out);
            collect_param_vars(v, out);
        }
        TcType::Fn(params, ret) => {
            for p in params {
                collect_param_vars(p, out);
            }
            collect_param_vars(ret, out);
        }
        TcType::Record(fields) => {
            for (_, f) in fields {
                collect_param_vars(f, out);
            }
        }
        TcType::Variant(_, payload) | TcType::Named(_, payload) => {
            for t in payload {
                collect_param_vars(t, out);
            }
        }
        TcType::Int | TcType::Float | TcType::Bool | TcType::String | TcType::Unit => {}
    }
}

/// Rewrites generic parameter names (`Named("T", [])`) inside `ty` to the
/// corresponding `Var(PARAM_BASE + index)`, so a component's prop types can be
/// unified against call-site arguments to pin the generic parameters.
fn rewrite_generics(ty: &mut TcType, index: &HashMap<String, usize>) {
    match ty {
        TcType::Named(name, args) if args.is_empty() => {
            if let Some(&i) = index.get(name) {
                *ty = TcType::Var(crate::env::PARAM_BASE + i as u32);
            }
        }
        TcType::List(inner) => rewrite_generics(inner, index),
        TcType::Option(inner) => rewrite_generics(inner, index),
        TcType::Map(k, v) => {
            rewrite_generics(k, index);
            rewrite_generics(v, index);
        }
        TcType::Fn(params, ret) => {
            for p in params {
                rewrite_generics(p, index);
            }
            rewrite_generics(ret, index);
        }
        TcType::Record(fields) => {
            for (_, f) in fields {
                rewrite_generics(f, index);
            }
        }
        TcType::Variant(_, payload) | TcType::Named(_, payload) => {
            for t in payload {
                rewrite_generics(t, index);
            }
        }
        TcType::Var(_)
        | TcType::Constrained(_, _)
        | TcType::Int
        | TcType::Float
        | TcType::Bool
        | TcType::String
        | TcType::Unit => {}
    }
}

/// Collects ADT definitions from `type` declarations so they are visible before
/// their uses (mutual references are not required by the B.3 examples).
pub fn collect_adts(env: &mut Env, ast: &Ast) {
    for decl in &ast.decls {
        match decl {
            Decl::Type(type_decl) => {
                let params: Vec<String> = type_decl
                    .generics
                    .iter()
                    .map(|p| p.name.name.clone())
                    .collect();
                let variants: Vec<VariantDef> = type_decl
                    .variants
                    .iter()
                    .map(|v| VariantDef {
                        name: v.name.name.clone(),
                        fields: v
                            .fields
                            .iter()
                            .map(|f| TcType::from_surface(f, &primitives()))
                            .collect(),
                    })
                    .collect();
                env.register_adt(&type_decl.name.name, AdtDef { params, variants });
            }
            Decl::Record(rec) => {
                let fields: Vec<(String, TcType)> = rec
                    .fields
                    .iter()
                    .map(|f| {
                        (
                            f.name.name.clone(),
                            TcType::from_surface(&f.ty, &primitives()),
                        )
                    })
                    .collect();
                env.insert(
                    rec.name.name.clone(),
                    Binding::Ctor(CtorKind::Record { fields }),
                );
            }
            // User-defined components are callable constructors in the same
            // way as the prelude adapters; generic params make them
            // polymorphic (e.g. `Counter[T: Numeric]`). Prop types are lowered
            // with each generic parameter `P_i` rewritten to `Var(PARAM_BASE +
            // i)` so call sites can pin the parameter from a concrete argument.
            Decl::Component(comp) => {
                let param_index: HashMap<String, usize> = comp
                    .generics
                    .iter()
                    .enumerate()
                    .map(|(i, gp)| (gp.name.name.clone(), i))
                    .collect();
                let props: Vec<(String, TcType)> = comp
                    .props
                    .iter()
                    .map(|p| {
                        let mut ty = TcType::from_surface(&p.ty, &primitives());
                        rewrite_generics(&mut ty, &param_index);
                        (p.name.name.clone(), ty)
                    })
                    .collect();
                env.insert(
                    comp.name.name.clone(),
                    Binding::Ctor(CtorKind::Component {
                        params: comp.generics.iter().map(|g| g.name.name.clone()).collect(),
                        props,
                    }),
                );
            }
            Decl::Fn(fn_decl) => {
                // Forward-declare the function name so earlier declarations can
                // call it (mutual recursion among top-level fns is allowed).
                // Generic params become fresh variables; the body is checked
                // later in `check_decl`, which re-binds them in a fresh scope.
                let mut supply = Supply::default();
                let gen_vars: HashMap<String, TcType> = fn_decl
                    .generics
                    .iter()
                    .map(|g| (g.name.name.clone(), TcType::Var(supply.fresh())))
                    .collect();
                let mut param_tys: Vec<TcType> = Vec::with_capacity(fn_decl.params.len());
                for p in &fn_decl.params {
                    let mut ty = match &p.ty {
                        Some(t) => TcType::from_surface(t, &primitives()),
                        None => TcType::Var(supply.fresh()),
                    };
                    for (name, var) in &gen_vars {
                        rewrite_named_to_var(&mut ty, name, var);
                    }
                    param_tys.push(ty);
                }
                let mut ret_ty = match &fn_decl.ret {
                    Some(t) => TcType::from_surface(t, &primitives()),
                    None => TcType::Var(supply.fresh()),
                };
                for (name, var) in &gen_vars {
                    rewrite_named_to_var(&mut ret_ty, name, var);
                }
                env.insert(
                    fn_decl.name.text.clone(),
                    Binding::Mono(TcType::Fn(param_tys, Box::new(ret_ty))),
                );
            }
            _ => {}
        }
    }
}

/// Rewrites a `Named(name, [])` occurrence inside `ty` to `var` (used to map a
/// function's generic-parameter surface names to their inference variables).
fn rewrite_named_to_var(ty: &mut TcType, name: &str, var: &TcType) {
    match ty {
        TcType::Named(n, args) if args.is_empty() && n == name => {
            *ty = var.clone();
        }
        TcType::List(inner) => rewrite_named_to_var(inner, name, var),
        TcType::Option(inner) => rewrite_named_to_var(inner, name, var),
        TcType::Map(k, v) => {
            rewrite_named_to_var(k, name, var);
            rewrite_named_to_var(v, name, var);
        }
        TcType::Fn(params, ret) => {
            for p in params {
                rewrite_named_to_var(p, name, var);
            }
            rewrite_named_to_var(ret, name, var);
        }
        TcType::Record(fields) => {
            for (_, f) in fields {
                rewrite_named_to_var(f, name, var);
            }
        }
        TcType::Variant(_, payload) | TcType::Named(_, payload) => {
            for t in payload {
                rewrite_named_to_var(t, name, var);
            }
        }
        TcType::Var(_)
        | TcType::Constrained(_, _)
        | TcType::Int
        | TcType::Float
        | TcType::Bool
        | TcType::String
        | TcType::Unit => {}
    }
}

/// Checks a single top-level declaration's signature and body.
///
/// Components and functions are checked after ADTs are collected. Returns the
/// recorded node id of the declaration and its inferred type.
pub fn check_decl(checker: &mut Checker, decl: &Decl) -> Result<(NodeId, TcType), TypeError> {
    let span = decl.span();
    match decl {
        Decl::Type(_) => {
            // Already collected.
            Ok((compute_node_id(0, decl_tag(decl), span, None), TcType::Unit))
        }
        Decl::Component(comp) => {
            checker.env.push_scope();
            // Generic params as constrained variables — recorded both in the
            // lexical environment (so bodies see them) and in the `generics`
            // map (so `conv_ty` rewrites a surface `T` to the same variable).
            let mut generic_map: std::collections::HashMap<String, TcType> =
                std::collections::HashMap::new();
            for gp in &comp.generics {
                let id = checker.fresh();
                let var = if let Some(bound) = &gp.bound {
                    TcType::Constrained(id, vec![bound.name.clone()])
                } else {
                    TcType::Var(id)
                };
                checker
                    .env
                    .insert(gp.name.name.clone(), Binding::Mono(var.clone()));
                generic_map.insert(gp.name.name.clone(), var);
            }
            checker.generics = generic_map;
            // Props.
            for prop in &comp.props {
                let ty = checker.conv_ty(&prop.ty);
                checker
                    .env
                    .insert(prop.name.name.clone(), Binding::Mono(ty));
            }
            let body_ty = checker.infer_block(&comp.body)?;
            checker.env.pop_scope();
            checker.generics.clear();
            // NOTE: generic instantiations are recorded only at *call sites*
            // (see `apply_callee`), where concrete type arguments are known.
            // Recording them at the definition site would push never-resolved
            // fresh variables, which lowering would consume as junk.
            let id = checker.record(decl_tag(decl), span, &body_ty);
            Ok((id, body_ty))
        }
        Decl::Fn(fn_decl) => {
            checker.env.push_scope();
            // Generic params as variables, also recorded in the `generics`
            // map so `conv_ty` rewrites surface type names to the same vars.
            let mut generic_map: std::collections::HashMap<String, TcType> =
                std::collections::HashMap::new();
            for gp in &fn_decl.generics {
                let var = TcType::Var(checker.fresh());
                checker
                    .env
                    .insert(gp.name.name.clone(), Binding::Mono(var.clone()));
                generic_map.insert(gp.name.name.clone(), var);
            }
            checker.generics = generic_map;
            // Params.
            let _ = &fn_decl.params;
            for param in &fn_decl.params {
                let ty = match &param.ty {
                    Some(decl_ty) => checker.conv_ty(decl_ty),
                    None => checker.fresh_ty(),
                };
                checker
                    .env
                    .insert(param.name.name.clone(), Binding::Mono(ty));
            }
            let body_ty = checker.infer_block(&fn_decl.body)?;
            let ret_ty = match &fn_decl.ret {
                Some(decl_ty) => {
                    let expected = checker.conv_ty(decl_ty);
                    checker.expect(&expected, &body_ty, fn_decl.body.span)?;
                    expected
                }
                None => body_ty.clone(),
            };
            checker.env.pop_scope();
            checker.generics.clear();
            let id = checker.record(decl_tag(decl), span, &ret_ty);
            Ok((id, ret_ty))
        }
        Decl::Trait(_) | Decl::Capability(_) => {
            Ok((compute_node_id(0, decl_tag(decl), span, None), TcType::Unit))
        }
        Decl::Import(_) | Decl::Use(_) => {
            Ok((compute_node_id(0, decl_tag(decl), span, None), TcType::Unit))
        }
        Decl::Const(const_binding) => {
            // Module-level associated constant, e.g. `Color.red = RGB(1.0, 0.0, 0.0)`.
            // It is stored under its dotted path so that a later `Color.red`
            // field-access can resolve it.
            let value_ty = checker.infer(&const_binding.value)?;
            let full_name = const_binding
                .path
                .iter()
                .map(|id| id.name.clone())
                .collect::<Vec<_>>()
                .join(".");
            checker
                .env
                .insert(full_name, Binding::Mono(value_ty.clone()));
            let id = checker.record(decl_tag(decl), span, &value_ty);
            Ok((id, value_ty))
        }
        _ => Ok((compute_node_id(0, decl_tag(decl), span, None), TcType::Unit)),
    }
}
