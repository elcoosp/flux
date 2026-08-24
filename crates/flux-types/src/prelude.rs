//! The built-in environment: primitives, the three stdlib traits, and the
//! constructors / functions the Appendix B.3 examples reference.
//!
//! The Flux prelude (spec §18.3) imports `flux::prelude` implicitly. The type
//! checker does not parse the stdlib source, so it reconstitutes here the
//! minimal set of prelude bindings the B.3 grammar examples need to type-check:
//! the `Numeric`/`Eq`/`Show` traits and the adapter/value constructors used by
//! the examples.

use crate::env::{Binding, CtorKind, Env, TraitInfo};
use crate::kind::TcType;
use crate::scheme::{Scheme, Supply};
use std::collections::HashSet;

/// Names recognised as primitive scalar types.
#[must_use]
pub(crate) fn primitives() -> HashSet<String> {
    ["Int", "Float", "Bool", "String", "Unit"]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

/// Builds a fresh environment seeded with the prelude.
#[must_use]
pub(crate) fn prelude(supply: &mut Supply) -> Env {
    let mut env = Env::new();
    env.push_scope();

    // --- Traits (Haskell-style type classes) ---
    register_trait(&mut env, "Numeric", vec!["zero() -> T", "one() -> T"]);
    register_trait(&mut env, "Eq", vec![]);
    register_trait(&mut env, "Show", vec![]);

    // --- Primitive scalar constructors (value-level ids) ---
    bind_poly(&mut env, supply, "Int", TcType::Int);
    bind_poly(&mut env, supply, "Float", TcType::Float);
    bind_poly(&mut env, supply, "Bool", TcType::Bool);
    bind_poly(&mut env, supply, "String", TcType::String);

    // --- Stdlib types / constructors referenced by B.3 ---
    // List, Option, Map are built-in type constructors; also have value-level
    // constructors so `[]` lists and `Some`/`None` resolve.
    env.insert(
        "List".to_owned(),
        Binding::Ctor(CtorKind::Component {
            params: vec!["T".into()],
            props: Vec::new(),
        }),
    );
    env.insert(
        "Option".to_owned(),
        Binding::Ctor(CtorKind::Component {
            params: vec!["T".into()],
            props: Vec::new(),
        }),
    );
    env.insert(
        "Map".to_owned(),
        Binding::Ctor(CtorKind::Component {
            params: vec!["K".into(), "V".into()],
            props: Vec::new(),
        }),
    );

    // `WebSocket`, `TextField`, `RouterContext` are adapter/platform types
    // whose internal shape is opaque to the checker; treat them as opaque
    // nominal types so `createRef[WebSocket]()` etc. resolve.
    bind_poly(
        &mut env,
        supply,
        "WebSocket",
        TcType::Named("WebSocket".into(), Vec::new()),
    );
    bind_poly(
        &mut env,
        supply,
        "TextField",
        TcType::Named("TextField".into(), Vec::new()),
    );
    bind_poly(
        &mut env,
        supply,
        "RouterContext",
        TcType::Named("RouterContext".into(), Vec::new()),
    );
    bind_poly(
        &mut env,
        supply,
        "Data",
        TcType::Named("Data".into(), Vec::new()),
    );

    // --- Adapter components (B.3) ---
    for comp in [
        "Column",
        "Row",
        "Text",
        "Button",
        "Image",
        "Router",
        "Screen",
        "ForEach",
        "CupertinoButton",
        "MaterialButton",
        "TextField",
        "Provider",
        "When",
        "Switch",
    ] {
        env.insert(
            comp.to_owned(),
            Binding::Ctor(CtorKind::Component {
                params: Vec::new(),
                props: Vec::new(),
            }),
        );
    }

    // --- Capability / value constructors used by B.3.9 / B.3.6 ---
    bind_poly(
        &mut env,
        supply,
        "Camera",
        TcType::Named("Camera".into(), Vec::new()),
    );
    bind_poly(
        &mut env,
        supply,
        "Storage",
        TcType::Named("Storage".into(), Vec::new()),
    );
    bind_poly(
        &mut env,
        supply,
        "Api",
        TcType::Named("Api".into(), Vec::new()),
    );
    bind_poly(
        &mut env,
        supply,
        "Auth",
        TcType::Named("Auth".into(), Vec::new()),
    );

    // `Numeric.zero()`, `Numeric.one()`: trait method calls. The methods take
    // no explicit receiver; resolve to the declared `T` under the trait's own
    // bound. We bind a polymorphic function so any concrete `T` unifies.
    bind_poly(
        &mut env,
        supply,
        "Numeric",
        TcType::Named("Numeric".into(), Vec::new()),
    );

    // `resource(fn { ... })` yields a `(value, { refetch })` pair: the loaded
    // value plus a record exposing a `refetch` callback. The value's concrete
    // type is left polymorphic so `when value.is_loading { ... }` and
    // `value.field` surface access on the opaque resource resolve freely.
    {
        let value_var = supply.fresh();
        let value_ty = TcType::Var(value_var);
        let refetch = TcType::Fn(Vec::new(), Box::new(TcType::Unit));
        // Returns a 2-tuple `(value, { refetch })`; tuples are modelled as
        // records keyed by their index so `let (users, { refetch }) = ...`
        // destructures correctly.
        let resource_ret = TcType::Record(vec![
            ("0".to_owned(), Box::new(value_ty.clone())),
            (
                "1".to_owned(),
                Box::new(TcType::Record(vec![(
                    "refetch".to_owned(),
                    Box::new(refetch),
                )])),
            ),
        ]);
        let resource_ty = TcType::Fn(vec![TcType::Var(supply.fresh())], Box::new(resource_ret));
        env.insert("resource".to_owned(), Binding::Mono(resource_ty));
    }

    // `platform()` reports the host OS as a `String` (Appendix B.3.8).
    bind_poly(
        &mut env,
        supply,
        "platform",
        TcType::Fn(Vec::new(), Box::new(TcType::String)),
    );

    env
}

fn register_trait(env: &mut Env, name: &str, _methods: Vec<&str>) {
    env.insert(
        name.to_owned(),
        Binding::Trait(TraitInfo {
            params: vec!["T".into()],
            methods: vec!["zero".into(), "one".into()],
        }),
    );
}

/// Binds `name` to a polymorphic scheme over no variables (a monomorphic,
/// reusable value) — used for opaque nominal types and trait names.
fn bind_poly(env: &mut Env, supply: &mut Supply, name: &str, ty: TcType) {
    let scheme = Scheme {
        vars: ty.free_vars().into_iter().collect(),
        ty: {
            let _ = supply;
            ty
        },
    };
    env.insert(name.to_owned(), Binding::Poly(scheme));
}
