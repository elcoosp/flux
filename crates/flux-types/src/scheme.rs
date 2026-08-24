//! Type schemes and the let-polymorphism generalisation/instantiation steps.

use crate::kind::TcType;
use std::collections::{HashMap, HashSet};

/// A type scheme: a type together with the unification variables it
/// generalises over. `let`-bound names carry a scheme so they can be used at
/// multiple, distinct concrete types (let-polymorphism).
#[derive(Clone, Debug)]
pub struct Scheme {
    /// Unification variables bound by this scheme.
    pub vars: Vec<u32>,
    /// The polymorphic type, with its bound variables still present as `Var`s.
    pub ty: TcType,
}

/// A fresh-variable supply, kept in the checker for the whole run.
#[derive(Clone, Debug, Default)]
pub struct Supply {
    next: u32,
}

impl Supply {
    /// Returns a variable id not previously issued by this supply.
    #[must_use]
    pub fn fresh(&mut self) -> u32 {
        let id = self.next;
        self.next = self.next.wrapping_add(1);
        id
    }
}

/// Generalises `ty` over the variables that are free in it but not free in the
/// surrounding environment — the classic HM generalisation step.
#[must_use]
pub fn generalise(ty: &TcType, env_free: &HashSet<u32>) -> Scheme {
    let vars: Vec<u32> = ty
        .free_vars()
        .into_iter()
        .filter(|v| !env_free.contains(v) && *v < crate::env::PARAM_BASE)
        .collect();
    Scheme {
        vars,
        ty: ty.clone(),
    }
}

/// Instantiates a scheme with fresh variables, so each use sees independent
/// type variables.
#[must_use]
pub fn instantiate(scheme: &Scheme, supply: &mut Supply) -> TcType {
    let mut mapping = HashMap::new();
    for var in &scheme.vars {
        mapping.insert(*var, TcType::Var(supply.fresh()));
    }
    scheme.ty.apply(&mapping)
}
