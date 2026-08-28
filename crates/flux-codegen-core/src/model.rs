//! Surface-level component metadata and native type mapping (shared, FLUX-047).
//!
//! Component name, generics, `@pure`, prop/state types and string interpolations
//! are recovered from the AST via the node-ID bridge; only the type *spelling*
//! differs per backend, so [`native_type`] is parameterised by [`Backend`].

use std::collections::HashMap;

use flux_parser::{Annotation, ComponentDecl, PropDecl, StateDecl, Type};

use crate::backend::Backend;

/// Borrowed metadata about a component, derived from its AST declaration.
#[derive(Debug, Clone, Copy)]
pub struct ComponentMeta<'a> {
    /// The component declaration this metadata describes.
    pub decl: &'a ComponentDecl,
    /// Whether the component is annotated `@pure` (stateless).
    pub is_pure: bool,
}

impl<'a> ComponentMeta<'a> {
    /// Builds metadata from a component declaration.
    #[must_use]
    pub(crate) fn new(decl: &'a ComponentDecl) -> Self {
        let is_pure = decl
            .annotations
            .iter()
            .any(|a: &Annotation| a.name.name == "pure");
        Self { decl, is_pure }
    }

    /// Returns the component's declared props in source order.
    #[must_use]
    pub fn props(&self) -> &[PropDecl] {
        &self.decl.props
    }

    /// Returns the component's `state` declarations in source order.
    #[must_use]
    pub(crate) fn states(&self) -> Vec<&StateDecl> {
        self.decl
            .body
            .items
            .iter()
            .filter_map(|item| match item {
                flux_parser::BlockItem::State(decl) => Some(decl),
                _ => None,
            })
            .collect()
    }

    /// Returns the generic parameter clause, e.g. `<T>` or `""`.
    #[must_use]
    pub(crate) fn generic_clause(&self) -> String {
        if self.decl.generics.is_empty() {
            String::new()
        } else {
            let params: Vec<String> = self
                .decl
                .generics
                .iter()
                .map(|g| g.name.name.clone())
                .collect();
            format!("<{}>", params.join(", "))
        }
    }
}

/// Maps a Flux surface type to the native spelling for backend `B`, applying
/// `subst` (a generic-parameter → concrete-arg replacement) so a specialised
/// monomorphisation (`Counter_Int`) renders concrete prop/state types
/// (`initial: Int`) rather than the generic parameter (`initial: T`).
#[must_use]
pub fn native_type<B: Backend>(ty: &Type, subst: &HashMap<String, String>) -> String {
    match &ty.kind {
        flux_parser::TypeKindAst::Primitive(name) => match name.as_str() {
            "Int" => B::int_type().to_owned(),
            "Float" => B::float_type().to_owned(),
            "Bool" => B::bool_type().to_owned(),
            "String" => B::string_type().to_owned(),
            "Unit" => B::unit_type().to_owned(),
            other => other.to_owned(),
        },
        flux_parser::TypeKindAst::Named { name, args } => {
            if args.is_empty() {
                // A bare type reference: substitute a generic parameter if one
                // matches (e.g. `T` → `Int`), else keep the name.
                if let Some(concrete) = subst.get(&name.name) {
                    return concrete.clone();
                }
                name.name.clone()
            } else {
                let rendered: Vec<String> =
                    args.iter().map(|a| native_type::<B>(a, subst)).collect();
                format!("{}<{}>", name.name, rendered.join(", "))
            }
        }
        flux_parser::TypeKindAst::Record(fields) => {
            let rendered: Vec<String> = fields
                .iter()
                .map(|(n, t)| format!("{}: {}", n.name, native_type::<B>(t, subst)))
                .collect();
            B::record_type(&rendered)
        }
        flux_parser::TypeKindAst::Fn { params, ret } => {
            let rendered: Vec<String> = params.iter().map(|a| native_type::<B>(a, subst)).collect();
            format!(
                "({}) -> {}",
                rendered.join(", "),
                native_type::<B>(ret, subst)
            )
        }
        _ => B::any_type().to_owned(),
    }
}
