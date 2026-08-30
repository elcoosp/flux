use super::*;
impl Checker {
    pub(crate) fn resolve_use(
        &mut self,
        use_decl: &UseDecl,
        span: Span,
    ) -> Result<(NodeId, TcType), TypeError> {
        let loader = self.module_loader.as_ref().ok_or_else(|| {
            TypeError::new("module resolution is not available in this context", span).with_hint(
                "this build has no module loader; `use` can only resolve modules \
                 when the dev server (or a build with package-root access) provides one"
                    .to_owned(),
            )
        })?;

        let module_name = use_decl
            .segments
            .first()
            .map(|s| s.name.clone())
            .unwrap_or_default();

        if !self
            .modules_loading
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(module_name.clone())
        {
            return Err(
                TypeError::new(format!("cyclic `use` of module `{module_name}`"), span)
                    .with_hint("modules must not form an import cycle".to_owned()),
            );
        }
        let _guard = ModuleLoadGuard {
            name: module_name.clone(),
            set: Arc::clone(&self.modules_loading),
        };

        let source = loader(&module_name).ok_or_else(|| {
            TypeError::new(format!("cannot resolve module `{module_name}`"), span).with_hint(
                format!(
                    "no module source was found for `{module_name}`; check it exists at the \
             package root (e.g. `{module_name}.flux` or `{module_name}/main.flux`)"
                ),
            )
        })?;

        let ast = flux_parser::parse(&source, 0, &module_name).map_err(|e| {
            TypeError::new(format!("module `{module_name}` failed to parse: {e}"), span)
        })?;

        // Type-check the module in an isolated checker that shares the loader AND
        // the in-progress set so the module's own `use`s resolve transitively and
        // cycles are detected even across the recursion.
        let mut sub = Checker::with_loader(Arc::clone(loader));
        sub.modules_loading = Arc::clone(&self.modules_loading);
        // Collect ADTs first (mirrors `type_check`), then check every declaration.
        collect_adts(&mut sub.env, &ast);
        for decl in &ast.decls {
            check_decl(&mut sub, decl)?;
        }
        // Collect the module's exported bindings: for each top-level declaration that
        // introduces a name (component, fn, record, type, trait, capability, or a
        // module-level const `Color.red`), look it up in the submodule's environment.
        let exports: std::collections::HashMap<String, Binding> = ast
            .decls
            .iter()
            .filter_map(export_name)
            .filter_map(|name| sub.env.lookup(&name).map(|b| (name, b.clone())))
            .collect();

        if use_decl.glob {
            for (name, binding) in &exports {
                self.env.insert(name.clone(), binding.clone());
            }
        } else if use_decl.segments.len() == 2 {
            // `use theme::red` — bring only `red` (the last segment).
            let wanted = &use_decl.segments[1].name;
            let binding = exports
                .get(wanted)
                .or_else(|| exports.get(&format!("{module_name}.{wanted}")))
                .ok_or_else(|| {
                    TypeError::new(
                        format!("module `{module_name}` has no export `{wanted}`"),
                        span,
                    )
                    .with_hint("use `use theme::*` to bring every export into scope".to_owned())
                })?;
            self.env.insert(wanted.clone(), binding.clone());
        } else {
            // `use theme` — bring every export into scope directly, and also record
            // them under the `theme.` namespace so dotted access works.
            for (name, binding) in &exports {
                self.env.insert(name.clone(), binding.clone());
                self.env
                    .insert(format!("{module_name}.{name}"), binding.clone());
            }
        }

        Ok((
            compute_node_id(0, decl_tag(&Decl::Use(use_decl.clone())), span, None),
            TcType::Unit,
        ))
    }
}

/// RAII guard that removes a module from the in-progress set on scope exit, even
/// when resolution returns early via `?`.
struct ModuleLoadGuard {
    name: String,
    set: Arc<Mutex<std::collections::HashSet<String>>>,
}

impl Drop for ModuleLoadGuard {
    fn drop(&mut self) {
        self.set
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.name);
    }
}

/// Returns the export name a top-level declaration introduces into a module's
/// environment, if it is exported by `use`. Components, functions, records,
/// `type` ADTs, traits, and capabilities export their declared name; a
/// module-level const `Color.red` exports its dotted path. `use`/`import`
/// directives and `trait`/`capability` are not re-exported.
fn export_name(decl: &Decl) -> Option<String> {
    match decl {
        Decl::Component(c) => Some(c.name.name.clone()),
        Decl::Fn(f) => Some(f.name.text.clone()),
        Decl::Record(r) => Some(r.name.name.clone()),
        Decl::Type(t) => Some(t.name.name.clone()),
        Decl::Trait(t) => Some(t.name.name.clone()),
        Decl::Capability(c) => Some(c.name.name.clone()),
        Decl::Const(c) => Some(
            c.path
                .iter()
                .map(|id| id.name.clone())
                .collect::<Vec<_>>()
                .join("."),
        ),
        Decl::Use(_) => None,
        _ => None,
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
        Decl::Use(use_decl) => checker.resolve_use(use_decl, span),
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
