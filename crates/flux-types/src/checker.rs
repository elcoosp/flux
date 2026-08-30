//! The bidirectional type checker core.
//!
//! [`Checker`] walks the parsed [`Ast`] in two passes: first it collects every
//! top-level `type` declaration (ADTs) and every component/function signature
//! into the environment, then it checks each declaration's body. Expression
//! inference is bidirectional: `infer` synthesises a type for an expression, and
//! `check` verifies an expression against an expected type (used for `let`
//! annotations and calls). `let`-bound names are generalised (let-polymorphism)
//! via [`crate::scheme`].

use crate::ModuleLoader;
use crate::env::{AdtDef, Binding, CtorKind, Env, VariantDef};
use crate::error::TypeError;
use crate::exhaust::check_exhaustive;
use crate::kind::{TcType, compute_node_id, decl_tag};
use crate::prelude::{prelude, primitives};
use crate::scheme::{Supply, generalise, instantiate};
use crate::traits::{admits_arithmetic, admits_equality, check_trait_bound};
use crate::unify::{UnifyError, unify_into};
use flux_parser::{
    Ast, BinOp, Decl, Expr, ExprKind, Ident, LetPattern, Param, Pattern, Type, UseDecl,
};
use flux_syntax::{ExprTag, NodeId, NodeTag, Span};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

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
#[derive(Default)]
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
    /// Optional module loader. When `Some`, a `use theme` directive asks it for
    /// the source of module `theme` (the dev server wires this to the package
    /// root on disk); the loaded module's exports are merged into this environment.
    /// When `None`, `use` is rejected with an actionable error.
    module_loader: Option<ModuleLoader>,
    /// Modules currently being resolved, shared across the whole `use` tree
    /// (the sub-checkers spun up for transitive `use`s hold the same `Arc`) so a
    /// cycle (`a` uses `b` uses `a`) is detected instead of recursing forever.
    modules_loading: Arc<Mutex<std::collections::HashSet<String>>>,
}

impl std::fmt::Debug for Checker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Checker")
            .field("env", &self.env)
            .field("generics", &self.generics)
            .field("modules_loading", &self.modules_loading)
            .field("module_loader", &self.module_loader.is_some())
            .finish_non_exhaustive()
    }
}

impl Checker {
    /// Creates a fresh checker with the prelude environment loaded and no
    /// module loader (so `use` directives are rejected — use
    /// [`Checker::with_loader`] when cross-file resolution is required).
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
            module_loader: None,
            modules_loading: Arc::new(Mutex::new(std::collections::HashSet::new())),
        }
    }

    /// Creates a checker preloaded with the prelude and a module loader.
    ///
    /// The loader maps a module name (the first `use` segment, e.g. `theme`) to
    /// its source text. The dev server wires this to the package root on disk so
    /// `use theme` resolves `theme.flux` (or `theme/main.flux`) from there. The
    /// same loader is shared with sub-checkers so a module's own `use`s resolve
    /// transitively.
    #[must_use]
    pub fn with_loader(loader: ModuleLoader) -> Self {
        let mut checker = Self::new();
        checker.module_loader = Some(loader);
        checker
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
}

pub use generics::collect_adts;
pub use use_resolution::check_decl;

mod apply_callee;
mod call;
mod field;
mod generics;
mod infer;
mod opt_field;
mod scope;
mod use_resolution;

#[cfg(test)]
mod tests;
