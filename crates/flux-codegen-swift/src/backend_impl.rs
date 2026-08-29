//! The SwiftUI [`Backend`](flux_codegen_core::Backend) implementation (FLUX-047).
//!
//! Supplies only the syntax that differs from Kotlin: 1-space indentation, the
//! `(spacing: N)` container form, `UIImage(named:)` image binding, the
//! `NavigationStack` navigation API, the scalar spells, and the `struct …: View`
//! / `enum` header forms.

use std::collections::HashMap;

use flux_codegen_core::backend::Backend;
use flux_codegen_core::emitter::Emitter;
use flux_codegen_core::model::{ComponentMeta, native_type};
use flux_codegen_core::primitives::PrimitiveSpec;
use flux_parser::{Expr, ExprKind, TypeDecl};

/// The SwiftUI backend.
pub(crate) struct Swift;

impl Backend for Swift {
    const INDENT_UNIT: usize = 1;
    const CHILD_STEP: usize = 4;

    /// Swift inlines a `Screen` body at the same indent as its `// Screen`
    /// comment, so the body step is 0.
    const SCREEN_BODY_STEP: usize = 0;

    fn int_type() -> &'static str {
        "Int"
    }
    fn float_type() -> &'static str {
        "Double"
    }
    fn bool_type() -> &'static str {
        "Bool"
    }
    fn string_type() -> &'static str {
        "String"
    }
    fn unit_type() -> &'static str {
        "Void"
    }
    fn any_type() -> &'static str {
        "Any"
    }
    fn record_type(fields: &[String]) -> String {
        format!("({})", fields.join(", "))
    }

    fn container_spacing(gap: &str) -> String {
        format!("(spacing: {gap})")
    }

    fn image_expr(value: &str) -> String {
        format!("Image(uiImage: UIImage(named: {value}) ?? UIImage())")
    }

    fn router_open() -> String {
        "NavigationStack {".to_owned()
    }

    fn router_close() -> String {
        "}".to_owned()
    }

    fn screen_open(route: &str) -> String {
        // Swift emits the route as a comment; the destination body needs no brace.
        format!("// Screen route: {route}")
    }

    fn screen_close() -> String {
        String::new()
    }

    fn if_open(cond: &str) -> String {
        format!("if {cond} {{")
    }

    fn for_each_open(collection: &str, key: &str, element: &str) -> String {
        format!("ForEach({collection}, id: {key}) {{ {element} in")
    }

    fn for_each_close() -> String {
        "}".to_owned()
    }

    fn button_open(name: &str, handler: &str) -> String {
        let _ = name;
        // FLUX-064: an async handler (one that `await`s a capability call) must
        // be wrapped in `Task { }` so the release path can suspend. `Task` is
        // part of the Swift concurrency runtime (no extra import needed).
        let inner = if handler.contains("await") {
            format!("Task {{ {handler} }}")
        } else {
            handler.to_owned()
        };
        format!("Button(action: {{ {inner} }}) {{")
    }

    fn button_style(name: &str) -> &'static str {
        match name {
            "CupertinoButton" => ".buttonStyle(.bordered)",
            "MaterialButton" => ".buttonStyle(.borderedProminent)",
            _ => "",
        }
    }

    fn text_field(value: &str, on_change: &str, placeholder: &str) -> String {
        let value = if value.is_empty() {
            "\"\"".to_owned()
        } else {
            value.to_owned()
        };
        let on_change = if on_change.is_empty() {
            "{}".to_owned()
        } else {
            on_change.to_owned()
        };
        let title = if placeholder.is_empty() {
            "\"\"".to_owned()
        } else {
            format!("\"{placeholder}\"")
        };
        format!("TextField({title}, text: .constant({value}), onEditingChanged: {{ {on_change} }})")
    }

    fn key_extractor(key: &Expr) -> String {
        if let ExprKind::Lambda { params, body } = &key.kind {
            if let Some(param) = params.first() {
                if let Some(flux_parser::BlockItem::Expr(inner)) = body.items.first() {
                    if let ExprKind::Field { base, field } = &inner.kind {
                        if let ExprKind::Ident(base_id) = &base.kind {
                            if base_id.name == param.name.name {
                                return format!("\\.{field_name}", field_name = field.name);
                            }
                        }
                    }
                }
            }
        }
        "\\.self".to_owned()
    }

    fn interp_open() -> &'static str {
        "\\("
    }

    fn interp_close() -> &'static str {
        ")"
    }

    fn list_literal(elements: &[String]) -> String {
        format!("[{}]", elements.join(", "))
    }

    fn unsupported_placeholder() -> String {
        "0 /* unsupported */".to_owned()
    }

    fn native_name(spec: &PrimitiveSpec) -> &'static str {
        spec.swift_view
    }

    fn animation_spec(curve: &str) -> String {
        // FLUX-042: map the Flux curve name onto a SwiftUI `Animation` value.
        // Named curves reduce to the standard `Animation.*` spellings; unknown
        // curves fall back to `.default` so the generated source always compiles.
        let trimmed = curve.trim().trim_matches('"');
        let spec = match trimmed {
            "spring" => "Animation.spring()",
            "easeIn" => "Animation.easeIn",
            "easeOut" => "Animation.easeOut",
            "easeInOut" => "Animation.easeInOut",
            "linear" => "Animation.linear",
            "bouncy" => "Animation.bouncy",
            "smooth" => "Animation.smooth",
            other => {
                // A custom spec string (e.g. `Animation.spring(response: …)`) is
                // passed through verbatim; a bare token defaults to `.default`.
                if other.is_empty() {
                    "Animation.default"
                } else {
                    other
                }
            }
        };
        format!("withAnimation({spec})")
    }

    fn theme_extension(tokens: &[flux_codegen_core::primitives::DesignToken]) -> String {
        // FLUX-043: a Swift `enum FluxTheme` exposing every token as a static
        // computed value, so components reference `FluxTheme.colorPrimary` by
        // name. Colors use the SwiftUI `Color` literal; spacing/typography use
        // the raw point/`CGFloat` value (no `.sp`/`.dp` unit in Swift).
        let mut out = String::from("enum FluxTheme {\n");
        // Build the cases from the table.
        let cases: Vec<String> = tokens
            .iter()
            .map(|t| {
                let value = match t.group {
                    flux_codegen_core::primitives::TokenGroup::Color => format!("static let {} = {}", t.name, t.swift),
                    _ => format!("static let {}: CGFloat = {}", t.name, t.swift),
                };
                value
            })
            .collect();
        for case in cases {
            out.push_str(&format!("    {case}\n"));
        }
        out.push_str("}\n");
        out
    }

    fn component_body_indent() -> usize {
        // `struct Name: View {` at 0, props/`@State` at 4, `var body:` at 4,
        // then body children at 2 spaces (level 2).
        2
    }

    fn emit_component_header(
        em: &mut Emitter<'_, Self>,
        name: &str,
        generics: &str,
        meta: &ComponentMeta<'_>,
        subst: &HashMap<String, String>,
    ) {
        em.append_line(&format!("struct {name}{generics}: View {{"));
        for prop in meta.props() {
            let ty = native_type::<Self>(&prop.ty, subst);
            em.append_line(&format!("    let {}: {ty}", prop.name.name));
        }
    }

    fn emit_body_open(em: &mut Emitter<'_, Self>) {
        em.append_line("    var body: some View {");
    }

    fn emit_component_footer(em: &mut Emitter<'_, Self>) {
        em.append_line("    }");
        em.append_line("}");
    }

    fn emit_placeholder_component(em: &mut Emitter<'_, Self>, id: flux_syntax::NodeId) {
        em.append_line(&format!("struct FluxComponent_{id}: View {{"));
        em.append_line("    var body: some View {{ EmptyView() }}");
        em.append_line("}");
    }

    fn emit_state_cell(
        em: &mut Emitter<'_, Self>,
        name: &str,
        ty: &str,
        init: &str,
        _subst: &HashMap<String, String>,
    ) {
        em.append_line(&format!("    @State private var {name}: {ty} = {init}"));
    }

    fn emit_sum_type(em: &mut Emitter<'_, Self>, sum: &TypeDecl) {
        let name = &sum.name.name;
        em.append_line(&format!("enum {name} {{"));
        for variant in &sum.variants {
            let vname = &variant.name.name;
            if variant.fields.is_empty() {
                em.line(1, &format!("case {vname}"));
            } else {
                let params: Vec<String> = variant
                    .fields
                    .iter()
                    .enumerate()
                    .map(|(i, t)| format!("field{i}: {}", native_type::<Self>(t, &HashMap::new())))
                    .collect();
                em.line(1, &format!("case {vname}({})", params.join(", ")));
            }
        }
        em.append_line("}");
    }

    fn emit_match(em: &mut Emitter<'_, Self>, id: flux_syntax::NodeId, indent: usize) {
        // Resolve the match expression up front (borrowing `em` immutably), then
        // release the borrow before we start emitting (which mutates `em`).
        let Some((subject, arms)) = em.lookup_expr(id).and_then(|expr| {
            if let flux_parser::ExprKind::Match { scrutinee, arms } = &expr.kind {
                Some((em.render(scrutinee), arms.clone()))
            } else {
                None
            }
        }) else {
            return;
        };
        em.line(indent, &format!("switch {subject} {{"));
        let step = Self::CHILD_STEP;
        for arm in &arms {
            match &arm.pattern.kind {
                flux_parser::MatchPatternKind::Wildcard => {
                    em.line(indent + step, "default:");
                    em.emit_expr_body(&arm.body, indent + 2 * step);
                }
                flux_parser::MatchPatternKind::Variant { name, fields } => {
                    let binds: Vec<String> = fields
                        .iter()
                        .filter_map(|f| match f {
                            flux_parser::Pattern::Ident(id) => Some(id.name.clone()),
                            _ => None,
                        })
                        .collect();
                    em.line(
                        indent + step,
                        &format!("case let .{}({}):", name.name, binds.join(", ")),
                    );
                    em.emit_expr_body(&arm.body, indent + 2 * step);
                }
                flux_parser::MatchPatternKind::Literal(lit) => {
                    em.line(indent + step, &format!("case {}:", em.render(lit)));
                    em.emit_expr_body(&arm.body, indent + 2 * step);
                }
                _ => {
                    em.line(indent + step, "default:");
                    em.emit_expr_body(&arm.body, indent + 2 * step);
                }
            }
        }
        em.line(indent, "}");
    }
}

#[cfg(test)]
mod tests {
    use crate::backend_impl::Swift;
    use flux_codegen_core::Backend;

    #[test]
    fn sync_handler_stays_plain_closure() {
        let out = <Swift as Backend>::button_open("Button", "taps = (taps + 1)");
        assert!(out.contains("Button(action: { taps = (taps + 1) })"));
        assert!(!out.contains("Task "), "sync handler must not be wrapped");
    }

    #[test]
    fn async_handler_wrapped_in_task() {
        // FLUX-064: an `await`ing handler is wrapped in `Task { }` so the
        // release path can suspend.
        let out = <Swift as Backend>::button_open("Button", "await Auth.login(\"u\")");
        assert!(
            out.contains("Button(action: { Task { await Auth.login(\"u\") } })"),
            "async handler not wrapped in Task: {out}"
        );
    }
}
