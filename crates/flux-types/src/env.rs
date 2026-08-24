//! The name-resolution environment and the type-constructor table.

use crate::kind::TcType;
use crate::scheme::Scheme;
use std::collections::{HashMap, HashSet};

/// Marker offset for unification variables that stand for an ADT's generic
/// parameters. Inference variables are issued from `0`, so this range never
/// collides with them.
pub const PARAM_BASE: u32 = 0x7000_0000;

/// A single binding in a scope.
#[derive(Clone, Debug)]
pub enum Binding {
    /// A monomorphic value with a known type.
    Mono(TcType),
    /// A let-polymorphic value (generalised over some variables).
    Poly(Scheme),
    /// A type constructor: an algebraic data type or a component.
    Ctor(CtorKind),
    /// A type class (Haskell-style trait).
    Trait(TraitInfo),
}

/// The two kinds of type constructor the checker knows about.
#[derive(Clone, Debug)]
pub enum CtorKind {
    /// An algebraic data type with named variants.
    Adt(AdtDef),
    /// A component; its prop type list is recorded for instantiation tracking.
    Component {
        /// Generic parameter names, empty when not generic.
        params: Vec<String>,
        /// Prop name → type, with generic parameters as `Var(PARAM_BASE + i)`.
        /// Empty when prop types are unknown (prelude adapters). Used at call
        /// sites to pin generic parameters and to record resolved instantiation
        /// types.
        props: Vec<(String, TcType)>,
    },
}

/// An algebraic data type definition.
#[derive(Clone, Debug)]
pub struct AdtDef {
    /// Generic parameter names, empty when not generic.
    pub params: Vec<String>,
    /// The variants in declaration order.
    pub variants: Vec<VariantDef>,
}

/// One variant of an ADT, with its payload field types.
///
/// Field types reference the ADT's generic parameters as variables at
/// [`PARAM_BASE`] + parameter-index, so they can be substituted when the ADT is
/// applied to concrete arguments.
#[derive(Clone, Debug)]
pub struct VariantDef {
    /// Variant name, e.g. `Circle`.
    pub name: String,
    /// Payload field types (may contain [`PARAM_BASE`] variables).
    pub fields: Vec<TcType>,
}

/// A type-class (trait) definition.
#[derive(Clone, Debug)]
pub struct TraitInfo {
    /// The trait's own generic parameter names.
    pub params: Vec<String>,
    /// Trait method names, for resolution of `Trait.method()` calls.
    pub methods: Vec<String>,
}

/// The lexical environment: a stack of scopes plus a global variant lookup.
#[derive(Clone, Debug, Default)]
pub struct Env {
    /// Inner scopes; index `0` is the outermost.
    scopes: Vec<HashMap<String, Binding>>,
    /// Maps a variant name to its parent ADT and definition.
    pub variants: HashMap<String, (String, VariantDef)>,
}

impl Env {
    /// Creates an empty environment.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Opens a new (inner) scope.
    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    /// Closes the innermost scope.
    ///
    /// # Panics
    ///
    /// Panics (debug-only, checked invariant) if called with no open scope,
    /// which can only happen on a malformed scope stack.
    pub fn pop_scope(&mut self) {
        assert!(!self.scopes.is_empty(), "pop_scope with no open scope");
        self.scopes.pop();
    }

    /// Inserts `name` into the current (innermost) scope.
    pub fn insert(&mut self, name: impl Into<String>, binding: Binding) {
        let scope = self.scopes.last_mut().expect("insert with no open scope");
        scope.insert(name.into(), binding);
    }

    /// Looks up `name` from the innermost scope outward.
    #[must_use]
    pub fn lookup(&self, name: &str) -> Option<&Binding> {
        self.scopes.iter().rev().find_map(|s| s.get(name))
    }

    /// Registers an ADT and its variants for later lookup.
    pub fn register_adt(&mut self, name: &str, def: AdtDef) {
        for variant in &def.variants {
            self.variants
                .insert(variant.name.clone(), (name.to_owned(), variant.clone()));
            // Each variant is a callable constructor in its own right.
            self.insert(
                variant.name.clone(),
                Binding::Ctor(CtorKind::Adt(def.clone())),
            );
        }
        self.insert(name, Binding::Ctor(CtorKind::Adt(def)));
    }

    /// The free unification variables of every binding in the environment.
    #[must_use]
    pub fn free_vars(&self) -> HashSet<u32> {
        let mut out = HashSet::new();
        for scope in &self.scopes {
            for binding in scope.values() {
                match binding {
                    Binding::Mono(ty) => out.extend(ty.free_vars()),
                    Binding::Poly(scheme) => out.extend(scheme.ty.free_vars()),
                    Binding::Ctor(_) | Binding::Trait(_) => {}
                }
            }
        }
        out.retain(|v| *v < PARAM_BASE);
        out
    }
}
