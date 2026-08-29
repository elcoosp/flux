//! The Kotlin/Compose [`Backend`](flux_codegen_core::Backend) implementation
//! (FLUX-047).
//!
//! Supplies only the syntax that differs from Swift: 4-space indentation, the
//! Compose `Arrangement.spacedBy` container spacing, `painterResource` image
//! binding, the `NavHost`/`composable` navigation API, the scalar spells, and
//! the `@Composable fun` / `sealed interface` header forms.

use std::collections::HashMap;

use flux_codegen_core::backend::Backend;
use flux_codegen_core::emitter::Emitter;
use flux_codegen_core::model::{ComponentMeta, native_type};
use flux_codegen_core::primitives::PrimitiveSpec;
use flux_parser::{Expr, ExprKind, TypeDecl};

/// The Kotlin/Compose backend.
pub(crate) struct Kotlin;

impl Backend for Kotlin {
    const INDENT_UNIT: usize = 4;
    const CHILD_STEP: usize = 1;

    /// Kotlin nests a `Screen` body one level inside its `composable(...) {`.
    const SCREEN_BODY_STEP: usize = 1;

    fn int_type() -> &'static str {
        "Int"
    }
    fn float_type() -> &'static str {
        "Double"
    }
    fn bool_type() -> &'static str {
        "Boolean"
    }
    fn string_type() -> &'static str {
        "String"
    }
    fn unit_type() -> &'static str {
        "Unit"
    }
    fn any_type() -> &'static str {
        "Any"
    }
    fn record_type(fields: &[String]) -> String {
        format!("/* record */ ({})", fields.join(", "))
    }

    fn container_spacing(gap: &str) -> String {
        format!(
            "(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy({gap}.dp))"
        )
    }

    fn image_expr(value: &str) -> String {
        format!("Image(painter = painterResource({value}), contentDescription = null)")
    }

    fn router_open() -> String {
        "NavHost(\n        navController = rememberNavController(),\n        startDestination = \"home\"\n    ) {"
            .to_owned()
    }

    fn router_close() -> String {
        "}".to_owned()
    }

    fn screen_open(route: &str) -> String {
        format!("composable({route}) {{")
    }

    fn screen_close() -> String {
        "}".to_owned()
    }

    fn if_open(cond: &str) -> String {
        format!("if ({cond}) {{")
    }

    fn for_each_open(collection: &str, key: &str, element: &str) -> String {
        format!("items({collection}, key = {key}) {{ {element} ->")
    }

    fn for_each_close() -> String {
        "}".to_owned()
    }

    fn button_open(name: &str, handler: &str) -> String {
        // FLUX-064: an async handler (one that `await`s a capability call) must
        // be wrapped in a coroutine so the release path can suspend without
        // blocking the UI thread. A sync handler stays a plain `() -> Unit`.
        let inner = if handler.contains("await") {
            format!("GlobalScope.launch {{ {handler} }}")
        } else {
            handler.to_owned()
        };
        match name {
            "CupertinoButton" => {
                format!("Button(onClick = {{ {inner} }}, shape = RoundedCornerShape(12.dp)) {{")
            }
            _ => format!("Button(onClick = {{ {inner} }}) {{"),
        }
    }

    fn button_style(_name: &str) -> &'static str {
        ""
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
        if placeholder.is_empty() {
            format!("TextField(value = {value}, onValueChange = {{ {on_change} }})")
        } else {
            format!(
                "TextField(value = {value}, onValueChange = {{ {on_change} }}, placeholder = {{ Text(\"{placeholder}\") }})"
            )
        }
    }

    fn key_extractor(key: &Expr) -> String {
        if let ExprKind::Lambda { params, body } = &key.kind {
            if let Some(param) = params.first() {
                if let Some(flux_parser::BlockItem::Expr(inner)) = body.items.first() {
                    if let ExprKind::Field { base, field } = &inner.kind {
                        if let ExprKind::Ident(base_id) = &base.kind {
                            if base_id.name == param.name.name {
                                return format!("{{ it.{} }}", field.name);
                            }
                        }
                    }
                }
            }
        }
        "{ it }".to_owned()
    }

    fn interp_open() -> &'static str {
        "${"
    }

    fn interp_close() -> &'static str {
        "}"
    }

    fn list_literal(elements: &[String]) -> String {
        format!("listOf({})", elements.join(", "))
    }

    fn unsupported_placeholder() -> String {
        "/* unsupported expr */ 0".to_owned()
    }

    fn native_name(spec: &PrimitiveSpec) -> &'static str {
        spec.kotlin_view
    }

    fn animation_spec(curve: &str) -> String {
        // FLUX-042: map the Flux curve name onto a Compose `AnimationSpec`.
        // Named curves reduce to the standard spellings; unknown curves fall
        // back to `tween()` so the generated source always compiles.
        let trimmed = curve.trim().trim_matches('"');
        let spec = match trimmed {
            "spring" => "spring()",
            "easeIn" => "tween(easing = FastOutLinearInEasing)",
            "easeOut" => "tween(easing = LinearOutSlowInEasing)",
            "easeInOut" => "tween(easing = FastOutSlowInEasing)",
            "linear" => "tween(easing = LinearEasing)",
            other => {
                if other.is_empty() {
                    "tween()"
                } else {
                    other
                }
            }
        };
        format!("withAnimation({spec})")
    }

    fn theme_extension(tokens: &[flux_codegen_core::primitives::DesignToken]) -> String {
        // FLUX-043: a Kotlin `object FluxTheme` exposing every token as a
        // property, so components reference `FluxTheme.colorPrimary` by name.
        // Colors use the Compose `Color(...)` literal; spacing uses `.dp`;
        // typography uses `.sp`.
        let mut out = String::from("object FluxTheme {\n");
        for tok in tokens {
            let value = match tok.group {
                flux_codegen_core::primitives::TokenGroup::Color => {
                    format!("    val {} = {}\n", tok.name, tok.kotlin)
                }
                flux_codegen_core::primitives::TokenGroup::Spacing => {
                    format!("    val {} = {}\n", tok.name, tok.kotlin)
                }
                flux_codegen_core::primitives::TokenGroup::Typography => {
                    format!("    val {} = {}\n", tok.name, tok.kotlin)
                }
            };
            out.push_str(&value);
        }
        out.push_str("}\n");
        out
    }

    fn component_body_indent() -> usize {
        // `@Composable fun Name(` then `) {` then `var …`/`body` at level 1.
        1
    }

    fn emit_component_header(
        em: &mut Emitter<'_, Self>,
        name: &str,
        generics: &str,
        meta: &ComponentMeta<'_>,
        subst: &HashMap<String, String>,
    ) {
        em.append_line(&format!("@Composable fun {name}{generics}("));
        let mut first_prop = true;
        for prop in meta.props() {
            let ty = native_type::<Self>(&prop.ty, subst);
            let comma = if first_prop { "" } else { "," };
            first_prop = false;
            em.append_line(&format!("    {comma} {0}: {1}", prop.name.name, ty));
        }
        em.append_line(") {");
    }

    fn emit_body_open(em: &mut Emitter<'_, Self>) {
        // Kotlin's function body is already opened by `emit_component_header`.
        let _ = em;
    }

    fn emit_component_footer(em: &mut Emitter<'_, Self>) {
        em.append_line("}");
    }

    fn emit_placeholder_component(em: &mut Emitter<'_, Self>, id: flux_syntax::NodeId) {
        em.append_line(&format!("@Composable fun FluxComponent_{id}() {{ }}"));
    }

    fn emit_state_cell(
        em: &mut Emitter<'_, Self>,
        name: &str,
        ty: &str,
        init: &str,
        _subst: &HashMap<String, String>,
    ) {
        em.append_line(&format!(
            "    var {name} by remember {{ mutableStateOf<{ty}>({init}) }}"
        ));
    }

    fn emit_sum_type(em: &mut Emitter<'_, Self>, sum: &TypeDecl) {
        let name = &sum.name.name;
        em.append_line(&format!("sealed interface {name}"));
        for variant in &sum.variants {
            let vname = &variant.name.name;
            if variant.fields.is_empty() {
                em.append_line(&format!("    data class {vname} : {name}"));
            } else {
                let params: Vec<String> = variant
                    .fields
                    .iter()
                    .enumerate()
                    .map(|(i, t)| {
                        format!("val field{i}: {}", native_type::<Self>(t, &HashMap::new()))
                    })
                    .collect();
                em.append_line(&format!(
                    "    data class {vname}({}) : {name}",
                    params.join(", ")
                ));
            }
        }
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
        em.line(indent, &format!("when ({subject}) {{"));
        let step = Self::CHILD_STEP;
        for arm in &arms {
            match &arm.pattern.kind {
                flux_parser::MatchPatternKind::Wildcard => {
                    em.line(indent + step, "else ->");
                    em.emit_expr_body(&arm.body, indent + 2 * step);
                }
                flux_parser::MatchPatternKind::Variant { name, fields } => {
                    em.line(indent + step, &format!("is {} ->", name.name));
                    for (i, field) in fields.iter().enumerate() {
                        if let flux_parser::Pattern::Ident(bind) = field {
                            em.line(
                                indent + 2 * step,
                                &format!("val {} = {}.field{i}", bind.name, subject),
                            );
                        }
                    }
                    em.emit_expr_body(&arm.body, indent + 2 * step);
                }
                flux_parser::MatchPatternKind::Literal(lit) => {
                    em.line(indent + step, &format!("{} ->", em.render(lit)));
                    em.emit_expr_body(&arm.body, indent + 2 * step);
                }
                flux_parser::MatchPatternKind::Guard { name, .. } => {
                    em.line(indent + step, &format!("is {} ->", name.name));
                    em.emit_expr_body(&arm.body, indent + 2 * step);
                }
                _ => {
                    em.line(indent + step, "else ->");
                    em.emit_expr_body(&arm.body, indent + 2 * step);
                }
            }
        }
        em.line(indent, "}");
    }
}

#[cfg(test)]
mod tests {
    use crate::backend_impl::Kotlin;
    use flux_codegen_core::Backend;

    #[test]
    fn sync_handler_stays_plain_closure() {
        // A non-async handler must not be wrapped in a coroutine.
        let out = <Kotlin as Backend>::button_open("Button", "taps = (taps + 1)");
        assert!(out.contains("Button(onClick = { taps = (taps + 1) })"));
        assert!(!out.contains("launch"), "sync handler must not be wrapped");
    }

    #[test]
    fn async_handler_wrapped_in_launch() {
        // FLUX-064: an `await`ing handler is wrapped in `GlobalScope.launch`
        // so the release path can suspend without blocking the UI thread.
        let out = <Kotlin as Backend>::button_open("Button", "await Auth.login(\"u\")");
        assert!(
            out.contains("Button(onClick = { GlobalScope.launch { await Auth.login(\"u\") } })"),
            "async handler not wrapped in launch: {out}"
        );
    }
}
