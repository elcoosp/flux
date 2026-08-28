//! Per-node Compose renderers for the [`Emitter`](crate::program::Emitter).
//!
//! Each function renders one lowered node kind into Compose, recovering the
//! originating surface expression from the node-ID bridge. These are kept
//! separate from the structural traversal in `program` to stay within the
//! project's 300-line file budget.

use flux_parser::{Block, Expr, ExprKind};
use flux_syntax::NodeId;

use crate::expressions::render_expr;
use crate::model::composable_name;
use crate::printers::{key_extractor_of, render_inline};
use crate::program::Emitter;

/// Emits a primitive composable (Text, Button, Image, Column, …) with a
/// deterministic modifier chain derived from its props.
pub(crate) fn emit_primitive(em: &mut Emitter<'_>, id: NodeId, indent: usize) {
    let node = em.lowered.arena.get(id).expect("primitive node");
    let Some(expr) = em.bridge.expr(id) else {
        return;
    };
    let ExprKind::Call {
        callee,
        args,
        trailing,
    } = &expr.kind
    else {
        return;
    };
    let name = match &callee.kind {
        ExprKind::Ident(ident) => ident.name.clone(),
        _ => return,
    };
    let kotlin = composable_name(&name);
    let mut props = em.collect_props(args, trailing.as_deref());
    // Positional args (e.g. `Text("…")`, `Image(url)`) carry the primary value.
    let positional: Vec<String> = args
        .iter()
        .filter_map(|a| match a {
            flux_parser::Arg::Positional(value) => Some(render_expr(value)),
            _ => None,
        })
        .collect();

    // `Router`/`Screen` lower as primitives in the arena, so route them to the
    // dedicated NavHost / destination renderers here.
    if name == "Router" {
        emit_router(em, id, indent);
        return;
    }
    if name == "Screen" {
        emit_screen(em, id, indent);
        return;
    }

    let gap = props.remove("gap");
    if name == "Column" || name == "Row" {
        let spacing = gap
            .map(|g| format!("(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy({g}.dp))"))
            .unwrap_or_default();
        em.line(indent, &format!("{kotlin}{spacing} {{"));
        emit_trailing_or_children(em, trailing.as_deref(), node, indent + 1);
        em.line(indent, "}");
        return;
    }

    // Leaf primitives: Text(value), Button(onClick = { … }) { … }, Image(…).
    let primary = props
        .get("text")
        .or_else(|| props.get("url"))
        .cloned()
        .or_else(|| positional.first().cloned());
    match name.as_str() {
        "Text" => {
            let value = primary
                .map(render_inline)
                .unwrap_or_else(|| "\"\"".to_owned());
            em.line(indent, &format!("Text({value})"));
        }
        "Button" => {
            // `text:` (label) and `onClick:` (handler) are passed explicitly; if
            // absent, the trailing block carries the label and/or handler.
            let label = render_button_label(args, trailing.as_deref());
            let handler = collect_handler(args);
            em.line(indent, &format!("Button(onClick = {{ {handler} }}) {{"));
            em.line(indent + 1, &format!("Text({label})"));
            em.line(indent, "}");
        }
        "Image" => {
            let value = primary
                .map(render_inline)
                .unwrap_or_else(|| "\"\"".to_owned());
            em.line(
                indent,
                &format!("Image(painter = painterResource({value}), contentDescription = null)"),
            );
        }
        other => {
            em.line(indent, &format!("{other}()"));
        }
    }
}

/// Emits an `if/else` for a lowered `If` node (covers Flux `if` and
/// `when … otherwise`).
pub(crate) fn emit_if(em: &mut Emitter<'_>, id: NodeId, indent: usize) {
    let Some(expr) = em.bridge.expr(id) else {
        return;
    };
    let (cond, then_children, else_children): (&Expr, Vec<NodeId>, Vec<NodeId>) = match &expr.kind {
        ExprKind::If {
            cond,
            then_block,
            else_branch,
        } => {
            let then = em.block_children(then_block);
            let els = match else_branch {
                Some(b) => em.branch_children(b),
                None => Vec::new(),
            };
            (cond, then, els)
        }
        ExprKind::When {
            cond,
            then_block,
            otherwise,
        } => {
            let then = em.block_children(then_block);
            let els = match otherwise {
                Some(b) => em.block_children(b),
                None => Vec::new(),
            };
            (cond, then, els)
        }
        _ => return,
    };
    let cond_text = render_expr(cond);
    em.line(indent, &format!("if ({cond_text}) {{"));
    em.emit_children(then_children, indent + 1);
    if !else_children.is_empty() {
        em.line(indent, "} else {");
        em.emit_children(else_children, indent + 1);
    }
    em.line(indent, "}");
}

/// Emits a `items(...)` block for a lowered `ForEach` node, with a stable key
/// extractor (per Appendix F / spec FR-011).
pub(crate) fn emit_for_each(em: &mut Emitter<'_>, id: NodeId, indent: usize) {
    let Some(expr) = em.bridge.expr(id) else {
        return;
    };
    let ExprKind::ForEach { items, key, body } = &expr.kind else {
        return;
    };
    let collection = render_expr(items);
    let key_extractor = key_extractor_of(key);
    em.line(
        indent,
        &format!("items({collection}, key = {key_extractor}) {{ item ->"),
    );
    em.emit_block_body(body, indent + 1);
    em.line(indent, "}");
}

/// Emits a `NavHost` for a `Router` node.
pub(crate) fn emit_router(em: &mut Emitter<'_>, id: NodeId, indent: usize) {
    let node = em.lowered.arena.get(id).expect("router node");
    em.line(indent, "NavHost(");
    em.line(indent + 1, "navController = rememberNavController(),");
    em.line(indent + 1, "startDestination = \"home\"");
    em.line(indent, ") {");
    em.emit_children(Emitter::child_ids(node), indent + 1);
    em.line(indent, "}");
}

/// Emits a screen as a `composable` destination inside the NavHost.
pub(crate) fn emit_screen(em: &mut Emitter<'_>, id: NodeId, indent: usize) {
    let Some(expr) = em.bridge.expr(id) else {
        return;
    };
    let ExprKind::Call { args, trailing, .. } = &expr.kind else {
        return;
    };
    let route = args
        .first()
        .map(|a| render_expr(a.value()))
        .unwrap_or_else(|| "\"\"".to_owned());
    em.line(indent, &format!("composable({route}) {{"));
    if let Some(block) = trailing {
        em.emit_block_body(block, indent + 1);
    }
    em.line(indent, "}");
}

/// Emits the body of a trailing block (component children) at `indent`.
fn emit_trailing_or_children(
    em: &mut Emitter<'_>,
    trailing: Option<&Block>,
    node: flux_ir::NodeView<'_>,
    indent: usize,
) {
    if let Some(block) = trailing {
        em.emit_block_body(block, indent);
    } else {
        em.emit_children(Emitter::child_ids(node), indent);
    }
}

/// Renders the button label.
///
/// The label is supplied either as the `text:` named argument
/// (`Button(text: "Increment", …)`) or as a `Text(…)` child of the trailing
/// block (`Button(…) { Text("Increment") }`). When neither is present, fall back
/// to an empty string so the generated `Button` still compiles.
fn render_button_label(args: &[flux_parser::Arg], trailing: Option<&Block>) -> String {
    // Named `text:` argument takes precedence (matches the canonical example).
    for arg in args {
        if let flux_parser::Arg::Named { name, value } = arg {
            if name.name == "text" {
                return render_inline(render_expr(value));
            }
        }
    }
    // Otherwise look for a `Text(…)` child in the trailing block.
    let Some(block) = trailing else {
        return "\"\"".to_owned();
    };
    for item in &block.items {
        let flux_parser::BlockItem::Expr(expr) = item else {
            continue;
        };
        let ExprKind::Call { callee, args, .. } = &expr.kind else {
            continue;
        };
        let ExprKind::Ident(ident) = &callee.kind else {
            continue;
        };
        if ident.name != "Text" {
            continue;
        }
        // `Text("…")` (positional) or `Text(text: "…")` (named).
        if let Some(positional) = args.iter().find_map(|a| match a {
            flux_parser::Arg::Positional(value) => Some(render_inline(render_expr(value))),
            _ => None,
        }) {
            return positional;
        }
        for arg in args {
            if let flux_parser::Arg::Named { name, value } = arg {
                if name.name == "text" {
                    return render_inline(render_expr(value));
                }
            }
        }
    }
    "\"\"".to_owned()
}

/// Finds the `onClick`/`onTap` handler argument and renders its body as Kotlin
/// statements. Returns an empty string when no handler is present, so the
/// lambda is still valid (`Button(onClick = { }) { … }`).
fn collect_handler(args: &[flux_parser::Arg]) -> String {
    for arg in args {
        let flux_parser::Arg::Named { name, value } = arg else {
            continue;
        };
        if name.name != "onClick" && name.name != "onTap" {
            continue;
        }
        if let Some(body) = crate::expressions::render_handler_body(value) {
            return body;
        }
    }
    String::new()
}
