use super::*;
impl Checker {
    pub(crate) fn infer(&mut self, expr: &Expr) -> Result<TcType, TypeError> {
        let ty = self.infer_inner(expr)?;
        self.record(ExprTag(10), expr.span, &ty);
        Ok(ty)
    }

    pub(crate) fn infer_inner(&mut self, expr: &Expr) -> Result<TcType, TypeError> {
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
            ExprKind::Field { base, field } => self.infer_field_access(base, field, expr),
            ExprKind::OptField { base, field } => self.infer_opt_field_access(base, field),
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
}
