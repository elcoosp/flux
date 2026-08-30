use super::*;
impl Checker {
    pub(crate) fn infer_lambda(
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

    pub(crate) fn infer_call(
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

    pub(crate) fn check_binary(
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
}
