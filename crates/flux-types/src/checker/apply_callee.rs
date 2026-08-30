use super::generics::collect_param_vars;
use super::*;
impl Checker {
    pub(crate) fn apply_callee(
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
}
