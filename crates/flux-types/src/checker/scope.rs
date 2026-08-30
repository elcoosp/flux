use super::*;
impl Checker {
    pub(crate) fn lookup_value(&mut self, name: &str, span: Span) -> Result<TcType, TypeError> {
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
    pub(crate) fn try_lookup_value(&self, name: &str) -> Option<TcType> {
        match self.env.lookup(name) {
            Some(Binding::Mono(ty)) => Some(ty.clone()),
            Some(Binding::Poly(scheme)) => Some(instantiate(scheme, &mut self.supply.clone())),
            _ => None,
        }
    }

    pub(crate) fn bind_let(
        &mut self,
        pattern: &LetPattern,
        value_ty: &TcType,
    ) -> Result<(), TypeError> {
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

    pub(crate) fn bind_tuple(
        &mut self,
        patterns: &[LetPattern],
        value_ty: &TcType,
    ) -> Result<(), TypeError> {
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

    pub(crate) fn bind_record_let(
        &mut self,
        fields: &[Ident],
        value_ty: &TcType,
    ) -> Result<(), TypeError> {
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

    pub(crate) fn bind_simple_pattern(
        &mut self,
        pattern: &Pattern,
        ty: &TcType,
    ) -> Result<(), TypeError> {
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

    pub(crate) fn bind_pattern_ty(
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

    pub(crate) fn infer_block(&mut self, block: &flux_parser::Block) -> Result<TcType, TypeError> {
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
