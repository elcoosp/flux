use super::*;
/// Collects the `Var(PARAM_BASE + i)` generic-parameter variables referenced
/// inside `ty` into `out` (used to discover a component's generic arity from
/// its prop types).
pub(crate) fn collect_param_vars(ty: &TcType, out: &mut Vec<u32>) {
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
pub(crate) fn rewrite_generics(ty: &mut TcType, index: &HashMap<String, usize>) {
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
pub(crate) fn rewrite_named_to_var(ty: &mut TcType, name: &str, var: &TcType) {
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
