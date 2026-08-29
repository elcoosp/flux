//! The type checker's internal type representation.
//!
//! [`TypeKind`] from `flux-syntax` is the *serialised* structural type used by
//! the IR, differ and codegen. During checking we need a richer form that can
//! carry unresolved names ([`TcType::Named`]) before we resolve them to ADTs or
//! type constructors, as well as unification variables. [`TcType`] is that form;
//! [`TcType::to_typekind`] converts back to [`TypeKind`] once a type is fully
//! resolved and concrete.

use flux_parser::Decl;
use flux_syntax::{DeclTag, Key, NodeId, NodeTag, Span, TypeKind};
use std::collections::HashSet;
use std::fmt;

/// A type as seen during checking.
///
/// This is a superset of [`TypeKind`]: it adds [`TcType::Named`], an unresolved
/// type-constructor application (`Counter[Int]` before we know whether
/// `Counter` is a component or an ADT), and keeps unification variables
/// explicit.
#[derive(Clone, Debug, PartialEq)]
pub enum TcType {
    /// `Int`.
    Int,
    /// `Float`.
    Float,
    /// `Bool`.
    Bool,
    /// `String`.
    String,
    /// `Unit`.
    Unit,
    /// `List[T]`.
    List(Box<TcType>),
    /// `Map[K, V]`.
    Map(Box<TcType>, Box<TcType>),
    /// `Option[T]`.
    Option(Box<TcType>),
    /// `Fn(A, B) -> R`.
    Fn(Vec<TcType>, Box<TcType>),
    /// Anonymous record with named fields.
    Record(Vec<(String, Box<TcType>)>),
    /// A resolved algebraic-data-type variant applied to payload types.
    Variant(String, Vec<TcType>),
    /// An unresolved type-constructor application, e.g. `Counter[Int]`.
    Named(String, Vec<TcType>),
    /// Unification variable.
    Var(u32),
    /// Unification variable bounded by trait names.
    Constrained(u32, Vec<String>),
}

impl TcType {
    /// Returns `true` when the type contains no unification variable.
    #[must_use]
    pub fn is_concrete(&self) -> bool {
        match self {
            Self::Var(_) | Self::Constrained(_, _) => false,
            Self::Int | Self::Float | Self::Bool | Self::String | Self::Unit => true,
            Self::List(inner) => inner.is_concrete(),
            Self::Option(inner) => inner.is_concrete(),
            Self::Map(key, value) => key.is_concrete() && value.is_concrete(),
            Self::Fn(params, ret) => params.iter().all(Self::is_concrete) && ret.is_concrete(),
            Self::Record(fields) => fields.iter().all(|(_, ty)| ty.is_concrete()),
            Self::Variant(_, payload) => payload.iter().all(Self::is_concrete),
            Self::Named(_, args) => args.iter().all(Self::is_concrete),
        }
    }

    /// The free unification-variable ids contained in this type.
    #[must_use]
    pub fn free_vars(&self) -> HashSet<u32> {
        let mut out = HashSet::new();
        self.collect_vars(&mut out);
        out
    }

    fn collect_vars(&self, out: &mut HashSet<u32>) {
        match self {
            Self::Var(id) | Self::Constrained(id, _) => {
                out.insert(*id);
            }
            _ => self.collect_children(out),
        }
    }

    fn collect_children(&self, out: &mut HashSet<u32>) {
        match self {
            Self::List(inner) | Self::Option(inner) => inner.collect_vars(out),
            Self::Record(fields) => {
                for (_, ty) in fields {
                    ty.collect_vars(out);
                }
            }
            Self::Variant(_, payload) => {
                for ty in payload {
                    ty.collect_vars(out);
                }
            }
            Self::Named(_, args) => {
                for ty in args {
                    ty.collect_vars(out);
                }
            }
            _ => {}
        }
    }

    /// Substitutes each variable in `mapping` throughout this type.
    #[must_use]
    pub fn apply(&self, mapping: &std::collections::HashMap<u32, TcType>) -> Self {
        match self {
            Self::Var(id) => mapping.get(id).cloned().unwrap_or_else(|| self.clone()),
            Self::Constrained(id, traits) => mapping
                .get(id)
                .cloned()
                .unwrap_or_else(|| Self::Constrained(*id, traits.clone())),
            Self::List(inner) => Self::List(Box::new(inner.apply(mapping))),
            Self::Option(inner) => Self::Option(Box::new(inner.apply(mapping))),
            Self::Map(k, v) => Self::Map(Box::new(k.apply(mapping)), Box::new(v.apply(mapping))),
            Self::Fn(params, ret) => Self::Fn(
                params.iter().map(|p| p.apply(mapping)).collect(),
                Box::new(ret.apply(mapping)),
            ),
            Self::Record(fields) => Self::Record(
                fields
                    .iter()
                    .map(|(n, ty)| (n.clone(), Box::new(ty.apply(mapping))))
                    .collect(),
            ),
            Self::Variant(name, payload) => Self::Variant(
                name.clone(),
                payload.iter().map(|t| t.apply(mapping)).collect(),
            ),
            Self::Named(name, args) => Self::Named(
                name.clone(),
                args.iter().map(|t| t.apply(mapping)).collect(),
            ),
            other => other.clone(),
        }
    }

    /// Converts a resolved, concrete type to the serialisable [`TypeKind`].
    ///
    /// Records and variants carry [`flux_syntax::StringId`]s; this crate does
    /// not maintain a string table, so those ids are left at `0`. Unification
    /// variables must already have been resolved — converting an unresolved
    /// type yields `Unit` rather than panicking.
    #[must_use]
    pub fn to_typekind(&self) -> TypeKind {
        match self {
            Self::Int => TypeKind::Int,
            Self::Float => TypeKind::Float,
            Self::Bool => TypeKind::Bool,
            Self::String => TypeKind::String,
            Self::Unit => TypeKind::Unit,
            Self::List(inner) => TypeKind::List(Box::new(inner.to_typekind())),
            Self::Option(inner) => TypeKind::Option(Box::new(inner.to_typekind())),
            Self::Map(k, v) => TypeKind::Map(Box::new(k.to_typekind()), Box::new(v.to_typekind())),
            Self::Fn(params, ret) => TypeKind::Fn(
                params.iter().map(Self::to_typekind).collect(),
                Box::new(ret.to_typekind()),
            ),
            Self::Record(fields) => TypeKind::Record(
                fields
                    .iter()
                    .map(|(_n, ty)| (0, ty.to_typekind()))
                    .collect(),
            ),
            Self::Variant(name, payload) => {
                let _ = name;
                TypeKind::Variant(0, payload.iter().map(Self::to_typekind).collect())
            }
            Self::Named(name, args) => Self::named_to_typekind(name, args),
            Self::Var(_) | Self::Constrained(_, _) => TypeKind::Unit,
        }
    }

    fn named_to_typekind(name: &str, args: &[TcType]) -> TypeKind {
        match (name, args) {
            ("List", [inner]) => TypeKind::List(Box::new(inner.to_typekind())),
            ("Option", [inner]) => TypeKind::Option(Box::new(inner.to_typekind())),
            ("Map", [k, v]) => TypeKind::Map(Box::new(k.to_typekind()), Box::new(v.to_typekind())),
            (other, payload) => {
                let _ = other;
                TypeKind::Variant(0, payload.iter().map(Self::to_typekind).collect())
            }
        }
    }

    /// Builds a [`TcType`] from a surface [`Type`](flux_parser::Type) expression.
    ///
    /// `primitives` lists the built-in scalar names so that a bare `Int` is
    /// recognised as a primitive rather than an unresolved name.
    #[must_use]
    pub fn from_surface(ty: &flux_parser::Type, primitives: &HashSet<String>) -> Self {
        use flux_parser::TypeKindAst;
        match &ty.kind {
            TypeKindAst::Primitive(name) if primitives.contains(name) => match name.as_str() {
                "Int" => Self::Int,
                "Float" => Self::Float,
                "Bool" => Self::Bool,
                "String" => Self::String,
                "Unit" => Self::Unit,
                other => Self::Named(other.to_owned(), Vec::new()),
            },
            TypeKindAst::Primitive(name) => Self::Named(name.clone(), Vec::new()),
            TypeKindAst::Named { name, args } => {
                let converted: Vec<Self> = args
                    .iter()
                    .map(|a| Self::from_surface(a, primitives))
                    .collect();
                // A bare primitive name (`Int`, `Float`, …) may reach here when the
                // surface type was lexed as an identifier rather than a primitive
                // keyword; normalise it to the primitive variant so it unifies with
                // inferred literals.
                if converted.is_empty() && primitives.contains(&name.name) {
                    return match name.name.as_str() {
                        "Int" => Self::Int,
                        "Float" => Self::Float,
                        "Bool" => Self::Bool,
                        "String" => Self::String,
                        "Unit" => Self::Unit,
                        other => Self::Named(other.to_owned(), Vec::new()),
                    };
                }
                Self::app_named(&name.name, converted)
            }
            TypeKindAst::Record(fields) => Self::Record(
                fields
                    .iter()
                    .map(|(n, t)| (n.name.clone(), Box::new(Self::from_surface(t, primitives))))
                    .collect(),
            ),
            TypeKindAst::Fn { params, ret } => Self::Fn(
                params
                    .iter()
                    .map(|p| Self::from_surface(p, primitives))
                    .collect(),
                Box::new(Self::from_surface(ret, primitives)),
            ),
            _ => Self::Named("Unit".to_owned(), Vec::new()),
        }
    }

    /// Applies a recognised built-in type constructor or falls back to [`Self::Named`].
    #[must_use]
    pub fn app_named(name: &str, args: Vec<Self>) -> Self {
        match (name, args.len()) {
            ("List", 1) => Self::List(Box::new(args.into_iter().next().unwrap())),
            ("Option", 1) => Self::Option(Box::new(args.into_iter().next().unwrap())),
            ("Map", 2) => {
                let mut it = args.into_iter();
                let k = it.next().unwrap();
                let v = it.next().unwrap();
                Self::Map(Box::new(k), Box::new(v))
            }
            _ => Self::Named(name.to_owned(), args),
        }
    }
}

impl fmt::Display for TcType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Int => write!(f, "Int"),
            Self::Float => write!(f, "Float"),
            Self::Bool => write!(f, "Bool"),
            Self::String => write!(f, "String"),
            Self::Unit => write!(f, "Unit"),
            Self::List(inner) => write!(f, "List[{inner}]"),
            Self::Option(inner) => write!(f, "Option[{inner}]"),
            Self::Map(k, v) => write!(f, "Map[{k}, {v}]"),
            Self::Fn(params, ret) => {
                let ps: Vec<String> = params.iter().map(|p| p.to_string()).collect();
                write!(f, "Fn({}) -> {ret}", ps.join(", "))
            }
            Self::Record(fields) => {
                let fs: Vec<String> = fields.iter().map(|(n, t)| format!("{n}: {t}")).collect();
                write!(f, "{{ {} }}", fs.join(", "))
            }
            Self::Variant(name, payload) => {
                if payload.is_empty() {
                    write!(f, "{name}")
                } else {
                    let ps: Vec<String> = payload.iter().map(|t| t.to_string()).collect();
                    write!(f, "{name}({})", ps.join(", "))
                }
            }
            Self::Named(name, args) => {
                if args.is_empty() {
                    write!(f, "{name}")
                } else {
                    let ps: Vec<String> = args.iter().map(|t| t.to_string()).collect();
                    write!(f, "{name}[{}]", ps.join(", "))
                }
            }
            Self::Var(id) => write!(f, "?{id}"),
            Self::Constrained(id, traits) => {
                if traits.is_empty() {
                    write!(f, "?{id}")
                } else {
                    write!(f, "?{id}: {}", traits.join(" + "))
                }
            }
        }
    }
}

/// Derives a stable [`NodeId`] from a node's structural position.
///
/// Delegates to the canonical [`flux_syntax::compute_node_id`] (see
/// `docs/adr/ir-node-id-bridge.md`) so the type checker and the IR produce
/// identical IDs for identical source constructs — this is what lets FLUX-018
/// lowering look up inferred types by `NodeId`. The contract (AGENTS.md §3.2)
/// specifies FNV-1a-32 over `(parent_id, kind, span, key)`; the canonical
/// implementation is exactly that (FNV-1a-32 over the canonical little-endian
/// layout, yielding a `u32`).
#[must_use]
pub(crate) fn compute_node_id(
    parent: NodeId,
    tag: impl NodeTag,
    span: Span,
    key: Option<Key>,
) -> NodeId {
    flux_syntax::compute_node_id(parent, tag, span, key)
}

/// Maps a surface declaration to its structural [`DeclTag`], matching the
/// discriminants the type checker has always used (see
/// `crates/flux-ir/src/lower/ids.rs`). The tags are stable across edits and
/// shared with lowering so `TypedAST::types` keys line up with the IR.
///
/// Wrapping the discriminant in [`DeclTag`] (rather than passing a bare `u8`)
/// is what guarantees the compiler rejects an expression tag where a
/// declaration tag is required — that is the whole point of the sealed
/// [`NodeTag`] trait introduced in `flux-syntax`.
#[must_use]
pub(crate) fn decl_tag(decl: &Decl) -> DeclTag {
    match decl {
        Decl::Import(_) => DeclTag(1),
        Decl::Use(_) => DeclTag(2),
        Decl::Component(_) => DeclTag(3),
        Decl::Fn(_) => DeclTag(4),
        Decl::Type(_) => DeclTag(5),
        Decl::Trait(_) => DeclTag(6),
        Decl::Capability(_) => DeclTag(7),
        Decl::Const(_) => DeclTag(8),
        _ => DeclTag(9),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Bridge test (ADR-0027): the type checker's `compute_node_id` must be
    // byte-identical to the canonical `flux_syntax::compute_node_id`, so FLUX-018
    // lowering can look up inferred types by `NodeId`. Historically this crate
    // forked an FNV reduction that omitted `span.file_id`; it now delegates.
    #[test]
    fn matches_canonical_flux_syntax() {
        let parents = [0u32, 1, 7, 4_000_000];
        let tags = [0u8, 1, 3, 9, 255];
        let spans = [
            Span::new(0, 0, 4),
            Span::new(1, 10, 20),
            Span::new(3, 40, 52),
            Span::new(2, 0, 1_000_000),
        ];
        let keys: [Option<Key>; 3] = [None, Some(0), Some(99)];
        for &parent in &parents {
            for &raw in &tags {
                let tag = DeclTag(raw);
                for &span in &spans {
                    for &key in &keys {
                        let our = compute_node_id(parent, tag, span, key);
                        let canonical = flux_syntax::compute_node_id(parent, tag, span, key);
                        assert_eq!(
                            our, canonical,
                            "mismatch for ({parent}, {tag:?}, {span:?}, {key:?})"
                        );
                    }
                }
            }
        }
    }
}
