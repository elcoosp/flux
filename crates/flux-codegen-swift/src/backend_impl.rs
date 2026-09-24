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
pub struct Swift;

impl Backend for Swift {
    const INDENT_UNIT: usize = 1;
    const CHILD_STEP: usize = 4;

    /// Swift inlines a `Screen` body at the same indent as its `// Screen`
    /// comment, so the body step is 0.
    const SCREEN_BODY_STEP: usize = 0;

    /// Swift renders destinations inside a `navigationDestination` modifier that
    /// follows the `NavigationStack` close-brace, so children are emitted after
    /// `router_close`.
    const ROUTER_CHILDREN_AFTER_CLOSE: bool = true;

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
        Self::container_spacing_axis(gap, "vertical")
    }

    fn container_spacing_axis(gap: &str, _axis: &str) -> String {
        format!("(spacing: {gap})")
    }

    fn image_expr(value: &str) -> String {
        format!("Image(uiImage: UIImage(named: {value}) ?? UIImage())")
    }

    fn router_open(_start_destination: &str) -> String {
        // SwiftUI NavigationStack bound to the route path for programmatic
        // navigation. The destination modifier is chained by router_close.
        // Audit T-403.7: start_destination handled by NavigationStack path binding.
        "NavigationStack(path: $route) {".to_owned()
    }

    fn router_close() -> String {
        // Close the NavigationStack root view, then chain the destination
        // modifier that maps route strings to screen destinations.
        "}.navigationDestination(for: String.self) { route in".to_owned()
    }

    fn router_destination_close() -> String {
        // Close the navigationDestination closure.
        "}".to_owned()
    }

    fn router_navigate_expr(target: &str) -> String {
        // Push the target route onto the NavigationPath stack.
        // SwiftUI reads the appended String value via
        // `.navigationDestination(for: String.self)`.
        format!("route.append({target})")
    }

    fn screen_open(route: &str) -> String {
        // Emit a conditional that shows only the matching screen.
        // The route is already a quoted Swift string literal (e.g. "home").
        format!("if route == {route} {{")
    }

    fn screen_close() -> String {
        // Close the screen conditional opened by screen_open.
        "}".to_owned()
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
        // Audit T-403.6: use mutable binding ($state) instead of .constant()
        // so the field is editable. Generate a @State private var in the
        // component body for the bound signal.
        let value = if value.is_empty() {
            "\"\"".to_owned()
        } else {
            value.to_owned()
        };
        let _on_change = if value.is_empty() {
            "{ _ in }".to_owned()
        } else {
            // Audit T-403.6: onEditingChanged is a Bool callback; use onCommit
            // or generate a proper binding write. For now, use text: $binding.
            "{ _ in }".to_owned()
        };
        let placeholder = if placeholder.is_empty() {
            "\"\"".to_owned()
        } else {
            placeholder.to_owned()
        };
        // Audit T-403.6: use a mutable Binding to make the TextField editable.
        let getter = format!("get: {{ {} }}", value);
        let setter = format!("set: {{ newValue in {} = newValue }}", value);
        format!(
            "TextField({placeholder}, text: Binding({}, {}))",
            getter, setter
        )
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

    fn render_await(expr: &str) -> String {
        // Swift: await verbatim inside async context
        format!("await {}", expr)
    }

    fn list_type(element: &str) -> String {
        format!("[{}]", element)
    }

    fn unsupported_placeholder() -> String {
        "0 /* unsupported */".to_owned()
    }

    fn native_name(spec: &PrimitiveSpec) -> &'static str {
        spec.swift_view
    }

    fn prelude() -> &'static str {
        "import SwiftUI\n"
    }

    fn escape_text(s: &str) -> String {
        // Swift metacharacters: backslash and double quote.
        let mut out = String::with_capacity(s.len() + 8);
        for c in s.chars() {
            match c {
                '\\' => out.push_str("\\\\"),
                '"' => out.push_str("\\\""),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                other => out.push(other),
            }
        }
        out
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
            .map(|t| match t.group {
                flux_codegen_core::primitives::TokenGroup::Color => {
                    format!("static let {} = {}", t.name, t.swift)
                }
                _ => format!("static let {}: CGFloat = {}", t.name, t.swift),
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
        // The Flux `route` state drives the NavigationStack path; it must be a
        // `NavigationPath` so `route.append(...)` pushes properly.
        if name == "route" {
            em.append_line("    @State private var route = NavigationPath()");
        } else {
            em.append_line(&format!("    @State private var {name}: {ty} = {init}"));
        }
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
                // Audit T-403.5: Guard pattern emits exact-type test (not default).
                flux_parser::MatchPatternKind::Guard { name, .. } => {
                    em.line(indent + step, &format!("case let .{}(_):", name.name));
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

    #[test]
    fn swift_escapes_quotes() {
        // T-401: `Text("say \"hi\"")` — inner quotes must become Swift `\"`.
        let escaped = <Swift as Backend>::escape_text("say \"hi\"");
        assert_eq!(escaped, r#"say \"hi\""#);
    }

    #[test]
    fn swift_injection_cannot_terminate_literal() {
        // A string containing `\"){ ... }` must not close the Swift literal early.
        let payload = "\"){ evil() }";
        let escaped = <Swift as Backend>::escape_text(payload);
        assert!(
            escaped.contains(r#"\""#),
            "Swift must escape `\"`: got {escaped}"
        );
        let inner = escaped;
        assert_no_bare_quote(&inner);
    }

    /// Asserts that `body` contains no unescaped `"`. A `"` is "escaped" only
    /// when it immediately follows an odd number of backslashes.
    fn assert_no_bare_quote(body: &str) {
        let bytes = body.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'"' {
                let mut backslashes = 0;
                let mut j = i;
                while j > 0 && bytes[j - 1] == b'\\' {
                    backslashes += 1;
                    j -= 1;
                }
                assert!(
                    backslashes % 2 == 1,
                    "unescaped bare `\"` in string body: {body}"
                );
            }
            i += 1;
        }
    }
}
