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
pub struct Kotlin;

impl Backend for Kotlin {
    const INDENT_UNIT: usize = 4;
    const CHILD_STEP: usize = 1;

    /// Kotlin nests a `Screen` body one level inside its `composable(...) {`.
    const SCREEN_BODY_STEP: usize = 1;

    /// Kotlin's NavHost destinations are embedded as children inside the
    /// NavHost brace, not after a separate destination modifier.
    const ROUTER_CHILDREN_AFTER_CLOSE: bool = false;

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
        // Audit D5/T-403.3: delegate to axis-aware implementation (default vertical).
        Self::container_spacing_axis(gap, "vertical")
    }

    fn container_spacing_axis(gap: &str, axis: &str) -> String {
        // Audit T-403.3: emit arrangement matching the parent container axis.
        if axis == "horizontal" {
            format!(
                "(horizontalAlignment = Alignment.CenterHorizontally, horizontalArrangement = Arrangement.spacedBy({gap}.dp))"
            )
        } else {
            format!(
                "(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy({gap}.dp))"
            )
        }
    }

    fn image_expr(value: &str) -> String {
        format!("Image(painter = painterResource({value}), contentDescription = null)")
    }

    fn router_open(start_destination: &str) -> String {
        // Declare a navController variable so that `Router.navigate(target)`
        // (rendered by router_navigate_expr) can call navController.navigate.
        format!(
            "val navController = rememberNavController()\n    NavHost(\n        navController = navController,\n        startDestination = {}\n    ) {{",
            start_destination
        )
    }

    fn router_close() -> String {
        "}".to_owned()
    }

    fn router_destination_close() -> String {
        // Kotlin's NavHost closes in `router_close`; no separate destination
        // closure to close (NavHost handles destinations via `composable`).
        String::new()
    }

    fn router_navigate_expr(target: &str) -> String {
        // Kotlin's NavHost uses a NavController for programmatic navigation.
        format!("navController.navigate({target})")
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
        // ForEach bodies must be inside a LazyListScope — wrap in LazyColumn
        // (audit H22: items() was emitted directly, only valid in LazyListScope).
        format!("LazyColumn {{\n                items({collection}, key = {key}) {{ {element} ->")
    }

    fn for_each_close() -> String {
        "}\n            }".to_owned()
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
        // The placeholder is already a quoted Kotlin string literal (e.g. `"name"`)
        // from the AST; do not double-quote it. When empty, emit `""`.
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
                "TextField(value = {value}, onValueChange = {{ {on_change} }}, placeholder = {{ {placeholder} }})"
            )
        }
    }

    fn toggle_open(value: &str) -> String {
        // Kotlin uses Switch with checked + onCheckedChange
        format!("Switch(checked = {value}, onCheckedChange = {{ {value} = it }}) {{")
    }

    fn toggle_close() -> String {
        "}".to_string()
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

    fn render_await(expr: &str) -> String {
        // Audit T-403.8: Kotlin launches a coroutine — replace `await expr`
        // with `expr.await()` inside the coroutine.
        format!("{expr}.await()")
    }

    fn list_type(element: &str) -> String {
        format!("List<{element}>")
    }

    fn spacer() -> &'static str {
        "Spacer(\"\")"
    }

    fn unsupported_placeholder() -> String {
        "/* unsupported expr */ 0".to_owned()
    }

    fn native_name(spec: &PrimitiveSpec) -> &'static str {
        spec.kotlin_view
    }

    fn prelude() -> &'static str {
        "package dev.flux.app\n\nimport androidx.compose.animation.core.*\nimport androidx.compose.foundation.Image\nimport androidx.compose.foundation.layout.*\nimport androidx.compose.foundation.lazy.*\nimport androidx.compose.foundation.shape.RoundedCornerShape\nimport androidx.compose.foundation.text.KeyboardActions\nimport androidx.compose.foundation.text.KeyboardOptions\nimport androidx.compose.material3.*\nimport androidx.compose.material3.Button\nimport androidx.compose.material3.Text\nimport androidx.compose.runtime.*\nimport androidx.compose.ui.Alignment\nimport androidx.compose.ui.Modifier\nimport androidx.compose.ui.res.painterResource\nimport androidx.compose.ui.text.input.KeyboardType\nimport androidx.compose.ui.unit.dp\nimport androidx.navigation.NavHostController\nimport androidx.navigation.compose.NavHost\nimport androidx.navigation.compose.composable\nimport androidx.navigation.compose.rememberNavController\nimport kotlinx.coroutines.GlobalScope\nimport kotlinx.coroutines.launch\n\n"
    }

    fn escape_text(s: &str) -> String {
        // Kotlin metacharacters: backslash, double quote, and $ (string templates).
        let mut out = String::with_capacity(s.len() + 8);
        for c in s.chars() {
            match c {
                '\\' => out.push_str("\\\\"),
                '"' => out.push_str("\\\""),
                '$' => out.push_str("\\$"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                other => out.push(other),
            }
        }
        out
    }

    fn animation_spec(curve: &str) -> String {
        // FLUX-042: map the Flux curve name onto a Compose `AnimationSpec`.
        // Returns a bare spec (no `withAnimation` wrapper — Kotlin has no
        // `withAnimation`); the `emit_animate` hook wraps it in
        // `animateFloatAsState(…)`.
        let trimmed = curve.trim().trim_matches('"');
        match trimmed {
            "spring" => "spring()".to_owned(),
            "bouncy" => "spring(dampingRatio = 0.5f)".to_owned(),
            "smooth" => "tween(300)".to_owned(),
            "easeIn" => "tween(easing = FastOutLinearInEasing)".to_owned(),
            "easeOut" => "tween(easing = LinearOutSlowInEasing)".to_owned(),
            "easeInOut" => "tween(easing = FastOutSlowInEasing)".to_owned(),
            "linear" => "tween(easing = LinearEasing)".to_owned(),
            other => {
                if other.is_empty() {
                    "tween()".to_owned()
                } else {
                    other.to_owned()
                }
            }
        }
    }

    fn emit_animate(
        em: &mut Emitter<'_, Self>,
        curve: &str,
        trailing: Option<&flux_parser::Block>,
        node_id: flux_syntax::NodeId,
        indent: usize,
    ) {
        // T-402.3: Kotlin emits `animateFloatAsState` as a state cell, not
        // `withAnimation(…)`. The val declaration lives at `indent`; children
        // follow inside an `AnimatedContent(…)` wrapper so the composable
        // scope is valid and the parity recognizer can fold both backends
        // to the common `Animate` name.
        let spec = Self::animation_spec(curve);
        em.line(
            indent,
            &format!("val anim = animateFloatAsState(targetValue = 0f, animationSpec = {spec})"),
        );
        em.line(indent, &format!("AnimatedContent(targetState = anim) {{"));
        em.emit_trailing_or_children(trailing, node_id, indent + Self::CHILD_STEP);
        em.line(indent, "}");
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
        // Audit T-403.4: parameters separated by ", " (not leading commas).
        let params: Vec<String> = meta
            .props()
            .iter()
            .map(|prop| {
                let ty = native_type::<Self>(&prop.ty, subst);
                format!("{}: {}", prop.name.name, ty)
            })
            .collect();
        let header = if params.is_empty() {
            format!("@Composable fun {name}{generics}(\n) {{")
        } else {
            format!(
                "@Composable fun {name}{generics}(\n     {}\n) {{",
                params.join("\n     ")
            )
        };
        em.append_line(&header);
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
        _has_router: bool,
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
                em.append_line(&format!("    data object {vname} : {name}"));
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

    #[test]
    fn kotlin_escapes_dollar_sign() {
        // T-401: the scaffold fixture `tapped $count times` must produce `\$`
        // in Kotlin output so `$` is not treated as a string-template token.
        let escaped = <Kotlin as Backend>::escape_text("tapped $count times");
        assert!(
            escaped.contains(r"\$"),
            "Kotlin must escape `$`: got {escaped}"
        );
    }

    #[test]
    fn kotlin_escapes_quotes() {
        // `Text("say \"hi\"")` — inner quotes must become Kotlin `\"`.
        let escaped = <Kotlin as Backend>::escape_text("say \"hi\"");
        assert_eq!(escaped, r#"say \"hi\""#);
    }

    #[test]
    fn kotlin_injection_cannot_terminate_literal() {
        let payload = "\"){ evil() }";
        let escaped = <Kotlin as Backend>::escape_text(payload);
        assert!(
            escaped.contains(r#"\""#),
            "Kotlin must escape `\"`: got {escaped}"
        );
        // Wrap in quotes like render_string does; verify no bare `"` in the body.
        let lit = format!("\"{}\"", escaped);
        let inner = &lit[1..lit.len() - 1];
        assert_no_bare_quote(inner);
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
