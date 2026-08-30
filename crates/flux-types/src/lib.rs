//! `flux-types` — the Flux bidirectional type checker.
//!
//! This crate consumes the surface [`Ast`] produced by `flux-parser` and the
//! shared [`TypeKind`] vocabulary from `flux-syntax`, and produces a
//! [`TypedAST`]: every node's inferred type plus the list of generic
//! instantiations that lowering must specialise (spec §18.2, §20.2, §20.3).
//!
//! The algorithm is bidirectional with let-polymorphism
//! (`let`-bound names are generalised), Haskell-style type-class resolution for
//! the three prelude traits `Numeric`/`Eq`/`Show`, ADT exhaustiveness checking,
//! and monomorphization tracking. Diagnostics follow `AGENTS.md` §3.7: each
//! [`TypeError`] carries a [`Span`] (where), an expected/actual type (why), and
//! a hint (how). The unified [`FluxError`] umbrella adds a `compile` / `runtime`
//! / `capability` classification with `what`/`where`/`why`/`how` accessors
//! (LANE-I, FLUX-02X).

#![forbid(unsafe_code)]
#![warn(
    missing_docs,
    missing_debug_implementations,
    rust_2018_idioms,
    unreachable_pub
)]

/// The canonical Flux capability surface (spec §24, Appendix E). The single
/// source of truth for capability names, numeric ids, and method ids — shared
/// by the compiler (`flux-ir` `CALL_CAP` emission) and the dev server so the
/// wire ids can never drift from the host registries. Also defines the
/// permission-gating model (`PermissionKind`, `PermissionChecker`,
/// `required_permission`) for capability access control.
pub mod capabilities;
mod checker;
mod env;
mod error;
mod exhaust;
mod kind;
mod prelude;
mod scheme;
mod traits;
mod unify;

pub use capabilities::{
    CAPABILITY_IDL, CapabilityIdl, MethodIdl, PermissionChecker, PermissionKind, is_satisfied,
    required_permission,
};
pub use error::{
    CapabilityError, CompileError, CompilePhase, FluxError, RuntimeError, capability_denied,
    compile_error,
};

pub use checker::{Checker, GenericInstantiation, check_decl, collect_adts};
pub use env::{AdtDef, Binding, CtorKind, Env, PARAM_BASE, TraitInfo, VariantDef};
pub use error::TypeError;
pub use flux_parser::{Ast, Type};
pub use flux_syntax::{NodeId, Span, TypeKind};
pub use kind::TcType;
pub use scheme::{Scheme, Supply, generalise, instantiate};
pub use unify::{UnifyError, unify};

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Resolves a module name (the first `use` segment, e.g. `theme`) to its source
/// text. The dev server wires this to the package root on disk so
/// `use theme` resolves `theme.flux` (or `theme/main.flux`). `Send + Sync` so it
/// can be shared with the sub-checkers the type checker spins up for transitive
/// `use`s.
pub type ModuleLoader = Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;

/// A type-checked syntax tree.
///
/// It owns the original [`Ast`] alongside the inferred [`TypeKind`] for each
/// node (keyed by the node's stable [`NodeId`]) and the list of generic
/// instantiations observed during checking. Lowering (FLUX-018) consumes this
/// to emit specialised bytecode per instantiation.
#[derive(Clone, Debug)]
pub struct TypedAST {
    /// The original (untyped) syntax tree.
    pub ast: Ast,
    /// Inferred types keyed by node id. When a node was never assigned a type
    /// (e.g. a `trait` declaration), it is simply absent.
    pub types: HashMap<NodeId, TypeKind>,
    /// Every generic instantiation discovered while checking, in order.
    pub instantiations: Vec<GenericInstantiation>,
    /// Names of every algebraic-data-type value constructor (variant) in
    /// scope. The handler bytecode compiler uses this set to decide whether a
    /// `Name(args)` call lowers to a value record (`ALLOC_RECORD` + field sets)
    /// or to a capability/method invocation (`CALL_CAP`). Kept as a flat name
    /// set so the compiler can resolve the form from the parse tree alone,
    /// without re-walking the type environment.
    pub constructors: HashSet<String>,
    /// Records the resolved field **position** (0-based, declaration order) for
    /// each `base.field` / `base?.field` expression, keyed by the expression's
    /// `NodeId` (the same `compute_node_id` bridge the rest of lowering uses).
    /// The bytecode emitter reads this to emit `GET_FIELD`/`SET_FIELD` with the
    /// positional index the VM expects (records are stored as positional
    /// `Vec<(PropIdx, Value)>`; FLUX-072).
    pub field_indices: HashMap<NodeId, u16>,
}

impl TypedAST {
    /// Returns the inferred type for `node`, if one was recorded.
    #[must_use]
    pub fn type_of(&self, node: NodeId) -> Option<&TypeKind> {
        self.types.get(&node)
    }

    /// Returns `true` when an instantiation of `name` carrying exactly the given
    /// argument type strings was recorded. Used by tests to assert
    /// monomorphization tracking.
    #[must_use]
    pub fn has_instantiation(&self, name: &str, args: &[&str]) -> bool {
        self.instantiations.iter().any(|inst| {
            inst.name == name
                && inst.generic_args.len() == args.len()
                && inst
                    .generic_args
                    .iter()
                    .zip(args)
                    .all(|(got, want)| got.to_string() == *want)
        })
    }
}

/// Type-checks `ast`, returning a [`TypedAST`] on success.
///
/// `use` directives are rejected (actionable error) because there is no module
/// loader wired in — use [`type_check_with_loader`] when module resolution from
/// the package root is required (the dev server uses that entry point).
pub fn type_check(ast: &Ast) -> Result<TypedAST, TypeError> {
    type_check_with_loader(ast, None)
}

/// Type-checks `ast` with an optional module loader.
///
/// `loader` maps a module name (from `use theme`) to its source text. When `None`,
/// `use` is rejected. The dev server passes a loader that reads `<name>.flux` (or
/// `<name>/main.flux`) from the package root so cross-file `use` resolves
/// end-to-end (FLUX module system).
pub fn type_check_with_loader(
    ast: &Ast,
    loader: Option<ModuleLoader>,
) -> Result<TypedAST, TypeError> {
    let mut checker = match loader {
        Some(loader) => Checker::with_loader(loader),
        None => Checker::new(),
    };
    collect_adts(&mut checker.env, ast);

    let mut types = HashMap::new();
    for decl in &ast.decls {
        let (id, ty) = check_decl(&mut checker, decl)?;
        types.insert(id, ty.to_typekind());
    }

    let types = checker
        .types
        .into_iter()
        .map(|(id, ty)| (id, ty.to_typekind()))
        .collect();

    Ok(TypedAST {
        ast: ast.clone(),
        types,
        instantiations: checker.instantiations,
        constructors: checker.env.variants.keys().cloned().collect(),
        field_indices: checker.field_indices,
    })
}

/// Renders `err` as a human-readable diagnostic string for `source`.
///
/// The `Span` carried by [`TypeError`] is an absolute byte offset; to turn it
/// into the `file:line:col` form required by the spec's diagnostic contract the
/// original source text is needed. Callers that have the source (e.g. the dev
/// server) should render errors with this helper.
///
/// # Examples
///
/// ```rust
/// use flux_types::{type_check, render_diagnostic};
/// use flux_parser::parse;
///
/// let src = "fn inc(x: Int) -> Int { x + 1 }";
/// let ast = parse(src, 0, "f.flux").unwrap();
/// let typed = type_check(&ast).unwrap();
/// let _ = typed; // render_diagnostic used at the error boundary
/// ```
pub fn render_diagnostic(source: &str, err: &TypeError) -> String {
    let (line, col) = crate::error::line_col(source, err.span.start as usize);
    let path = "<source>";
    format!(
        "error: {}\n  --> {}:{}:{}\n   |\n   = hint: {}",
        err.message,
        path,
        line,
        col,
        err.hint.as_deref().unwrap_or("")
    )
}
