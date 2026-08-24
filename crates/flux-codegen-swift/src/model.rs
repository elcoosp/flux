//! Surface-level metadata for a component, extracted from its AST declaration.
//!
//! The lowered arena stores only numeric [`flux_syntax::ComponentId`]s and
//! drops the rich surface information (generics, `@pure`, prop/state types,
//! interpolations). The codegen recovers that from the [`ComponentDecl`] via
//! the node-ID bridge, captured here as a cheap, borrow-only [`ComponentMeta`].

use flux_parser::{Annotation, ComponentDecl, PropDecl, StateDecl, Type};

/// Borrowed metadata about a component, derived from its AST declaration.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ComponentMeta<'a> {
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
    pub(crate) fn props(&self) -> &[PropDecl] {
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

    /// Returns the Swift generic parameter clause, e.g. `<T>` or `""`.
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

/// Maps a Flux surface type to its Swift spelling.
#[must_use]
pub(crate) fn swift_type(ty: &Type) -> String {
    match &ty.kind {
        flux_parser::TypeKindAst::Primitive(name) => match name.as_str() {
            "Int" => "Int".to_owned(),
            "Float" => "Double".to_owned(),
            "Bool" => "Bool".to_owned(),
            "String" => "String".to_owned(),
            "Unit" => "Void".to_owned(),
            other => other.to_owned(),
        },
        flux_parser::TypeKindAst::Named { name, args } => {
            if args.is_empty() {
                name.name.clone()
            } else {
                let rendered: Vec<String> = args.iter().map(swift_type).collect();
                format!("{}<{}>", name.name, rendered.join(", "))
            }
        }
        flux_parser::TypeKindAst::Record(fields) => {
            let rendered: Vec<String> = fields
                .iter()
                .map(|(n, t)| format!("{}: {}", n.name, swift_type(t)))
                .collect();
            format!("({})", rendered.join(", "))
        }
        flux_parser::TypeKindAst::Fn { params, ret } => {
            let rendered: Vec<String> = params.iter().map(swift_type).collect();
            format!("({}) -> {}", rendered.join(", "), swift_type(ret))
        }
        _ => "Any".to_owned(),
    }
}

/// Maps a Flux adapter/component name to its SwiftUI view name.
///
/// Layout containers map to their SwiftUI equivalents; everything else keeps
/// its Flux name (which is already Swift-identifier-shaped for the B.3
/// examples).
#[must_use]
pub(crate) fn view_name(name: &str) -> String {
    match name {
        "Column" => "VStack".to_owned(),
        "Row" => "HStack".to_owned(),
        other => other.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::{ComponentMeta, view_name};
    use flux_parser::{Decl, parse};

    fn component_named(src: &str) -> flux_parser::ComponentDecl {
        let ast = parse(src, 0, "t.flux").expect("parse");
        for decl in &ast.decls {
            if let Decl::Component(c) = decl {
                return c.clone();
            }
        }
        panic!("no component in {src}");
    }

    #[test]
    fn view_name_maps_layout_containers() {
        assert_eq!(view_name("Column"), "VStack");
        assert_eq!(view_name("Row"), "HStack");
        assert_eq!(view_name("Text"), "Text");
    }

    #[test]
    fn generic_component_emits_clause() {
        let comp = component_named("component List[T] { prop items: List[T] Text(\"\") }");
        let meta = ComponentMeta::new(&comp);
        assert!(!meta.decl.generics.is_empty());
        assert_eq!(meta.generic_clause(), "<T>");
    }

    #[test]
    fn pure_component_has_no_state() {
        let comp = component_named("component Stateless { Text(\"\") }");
        let meta = ComponentMeta::new(&comp);
        assert!(meta.states().is_empty());
    }
}
