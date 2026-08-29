//! Structural view-tree model shared by the release codegen and the parity
//! harness (roadmap Phase 4).
//!
//! [`ViewNode`] is the language-neutral structural model both codegen backends'
//! emitted source and the dev-path surface AST reduce to. [`view_tree`] walks the
//! lowered reactive arena (with the ADR-0027 node-ID [`Bridge`]) and produces that
//! tree directly — so parity compares the *same* vocabulary the codegen consumed,
//! deterministically, without re-parsing generated Swift/Kotlin text.
//!
//! Only the facts that must match between dev and release are retained: the view
//! graph, control flow (`if`/`when`/`ForEach`/`match`), and canonical condition /
//! collection / key / prop-value text. `VStack` vs `Column` and `\(x)` vs `${x}`
//! are normalized away by the backends themselves, so no cosmetic drift can
//! produce a false divergence.

use flux_ir::LoweredIr;
use flux_parser::{Arg, BinOp, BlockItem, Expr, ExprKind, MatchPattern, MatchPatternKind, StrPart};
use flux_syntax::{NodeId, NodeKind};

use crate::bridge::{Bridge, expr_id};

/// A single node in the language-neutral structural view tree.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub enum ViewNode {
    /// A top-level component (dev `Component` node, release `struct …: View` /
    /// `@Composable fun …`).
    Component {
        /// Surface component name (e.g. `HelloWorld`).
        name: String,
        /// Structural children.
        children: Vec<ViewNode>,
    },
    /// A leaf or container adapter view (`Text`, `Button`, `Column`, `VStack`,
    /// `Image`, `Row`, `HStack`, …), normalized to the Flux surface spelling.
    Primitive {
        /// Normalized Flux surface name.
        name: String,
        /// Trailing-block prop entries, keyed by name (currently unused for the
        /// lowered walk, retained for vocabulary parity with the AST reducer).
        props: Vec<(String, String)>,
        /// Structural children.
        children: Vec<ViewNode>,
    },
    /// A conditional (`if` or `when … otherwise`).
    If {
        /// The condition, in canonical form.
        cond: String,
        /// Then-branch children.
        then_branch: Vec<ViewNode>,
        /// Else/otherwise children (empty when absent).
        else_branch: Vec<ViewNode>,
    },
    /// A keyed collection repeater. The emitted body is intentionally empty in the
    /// MLP (keyed items are reconciled at runtime by the host, FLUX-014), so the
    /// lowered IR carries an empty splice and both backends render an empty
    /// wrapper — parity asserts the empty body in all three paths.
    ForEach {
        /// The collection expression, in canonical form.
        collection: String,
        /// The stable key extractor, in canonical form.
        key_path: String,
    },
    /// An algebraic-data-type match (`switch`/`when`), one arm per body.
    Match {
        /// The scrutinee expression, in canonical form.
        scrutinee: String,
        /// One entry per arm: its pattern label and its body children.
        arms: Vec<(String, Vec<ViewNode>)>,
    },
    /// A `Router` navigation container.
    Router {
        /// Destination screens.
        children: Vec<ViewNode>,
    },
    /// A `Screen` destination inside a `Router`.
    Screen {
        /// The route string.
        route: String,
        /// Screen body children.
        children: Vec<ViewNode>,
    },
}

impl ViewNode {
    /// Returns the human-readable kind label, used in diagnostics.
    #[must_use]
    pub fn kind_label(&self) -> &'static str {
        match self {
            ViewNode::Component { .. } => "Component",
            ViewNode::Primitive { .. } => "Primitive",
            ViewNode::If { .. } => "If",
            ViewNode::ForEach { .. } => "ForEach",
            ViewNode::Match { .. } => "Match",
            ViewNode::Router { .. } => "Router",
            ViewNode::Screen { .. } => "Screen",
        }
    }
}

/// Normalizes a backend/codegen view name to the common Flux surface spelling so
/// Swift `VStack` and Kotlin `Column` compare equal.
#[must_use]
pub fn normalize_view_name(name: &str) -> String {
    match name {
        "VStack" => "Column",
        "HStack" => "Row",
        "CupertinoButton" | "MaterialButton" => "Button",
        // FLUX-042: both backends emit `withAnimation(...)` for `Animate`.
        "withAnimation" => "Animate",
        // FLUX-043: native theme extension surface names reduce to `Theme`.
        "MaterialTheme" | "FluxTheme" => "Theme",
        other => other,
    }
    .to_owned()
}

/// Returns `true` for layout adapters that carry real structural children.
#[must_use]
pub fn is_container(name: &str) -> bool {
    matches!(
        name,
        "Column" | "Row" | "VStack" | "HStack" | "ZStack" | "Stack" | "Provider"
    )
}

/// Builds the structural view tree from the lowered arena + bridge — the release
/// path's faithful equivalent of the dev-path AST reducer.
///
/// Roots are the component *declarations* in arena order. Each declaration's body
/// is walked through its lowered children, recovering control-flow and view names
/// via the bridge.
#[must_use]
pub fn view_tree(lowered: &LoweredIr, bridge: &Bridge) -> Vec<ViewNode> {
    let mut roots = Vec::new();
    for id in lowered.arena.all_ids() {
        let Some(node) = lowered.arena.get(id) else {
            continue;
        };
        if node.kind() != NodeKind::Component {
            continue;
        }
        if let Some(comp) = bridge.component(id) {
            let name = comp.name.name.clone();
            let children = children_of(lowered, bridge, node);
            roots.push(ViewNode::Component { name, children });
        }
    }
    roots
}

fn children_of(lowered: &LoweredIr, bridge: &Bridge, node: flux_ir::NodeView<'_>) -> Vec<ViewNode> {
    let mut out = Vec::new();
    for child in node.children() {
        for id in child.node_ids() {
            node_from_lowered(lowered, bridge, id, &mut out);
        }
    }
    out
}

fn node_from_lowered(lowered: &LoweredIr, bridge: &Bridge, id: NodeId, out: &mut Vec<ViewNode>) {
    let Some(node) = lowered.arena.get(id) else {
        return;
    };
    match node.kind() {
        NodeKind::Component => {
            if let Some(comp) = bridge.component(id) {
                out.push(ViewNode::Primitive {
                    name: normalize_view_name(&comp.name.name),
                    props: vec![],
                    children: children_of(lowered, bridge, node),
                });
            }
        }
        NodeKind::Primitive => {
            let name = bridge.expr(id).and_then(callee_name).unwrap_or_else(|| {
                lowered
                    .component_names
                    .iter()
                    .find(|(cid, _)| *cid == node.component_id())
                    .map(|(_, n)| n.clone())
                    .unwrap_or_else(|| "<anon>".to_owned())
            });
            let normalized = normalize_view_name(&name);
            match normalized.as_str() {
                "Router" => out.push(ViewNode::Router {
                    children: children_of(lowered, bridge, node),
                }),
                "Screen" => out.push(ViewNode::Screen {
                    route: screen_route(bridge, id),
                    children: children_of(lowered, bridge, node),
                }),
                _ => {
                    let children = if is_container(&normalized) {
                        children_of(lowered, bridge, node)
                    } else {
                        Vec::new()
                    };
                    out.push(ViewNode::Primitive {
                        name: normalized,
                        props: vec![],
                        children,
                    });
                }
            }
        }
        _ => {
            if let Some(expr) = bridge.expr(id) {
                emit_control_flow(lowered, bridge, expr, out);
            }
        }
    }
}

fn emit_control_flow(lowered: &LoweredIr, bridge: &Bridge, expr: &Expr, out: &mut Vec<ViewNode>) {
    match &expr.kind {
        ExprKind::If {
            cond,
            then_block,
            else_branch,
        } => out.push(ViewNode::If {
            cond: render_cond(cond),
            then_branch: reduce_block(lowered, bridge, then_block),
            else_branch: reduce_else(lowered, bridge, else_branch.as_deref()),
        }),
        ExprKind::When {
            cond,
            then_block,
            otherwise,
        } => out.push(ViewNode::If {
            cond: render_cond(cond),
            then_branch: reduce_block(lowered, bridge, then_block),
            else_branch: otherwise
                .as_ref()
                .map(|b| reduce_block(lowered, bridge, b))
                .unwrap_or_default(),
        }),
        ExprKind::ForEach { items, key, .. } => out.push(ViewNode::ForEach {
            collection: canonicalize_expr(&render_expr(items)),
            key_path: render_key(key),
        }),
        ExprKind::Match {
            scrutinee, arms, ..
        } => out.push(ViewNode::Match {
            scrutinee: canonicalize_expr(&render_expr(scrutinee)),
            arms: arms
                .iter()
                .map(|arm| {
                    let mut body = Vec::new();
                    node_from_lowered_arm(lowered, bridge, &arm.body, &mut body);
                    (pattern_label(&arm.pattern), body)
                })
                .collect(),
        }),
        _ => {}
    }
}

fn reduce_block(lowered: &LoweredIr, bridge: &Bridge, block: &flux_parser::Block) -> Vec<ViewNode> {
    let mut out = Vec::new();
    for item in &block.items {
        if let BlockItem::Expr(expr) = item {
            let id = expr_id(expr.span);
            node_from_lowered(lowered, bridge, id, &mut out);
        }
    }
    out
}

fn reduce_else(lowered: &LoweredIr, bridge: &Bridge, else_branch: Option<&Expr>) -> Vec<ViewNode> {
    let mut out = Vec::new();
    if let Some(expr) = else_branch {
        match &expr.kind {
            ExprKind::Lambda { body, .. } => out.extend(reduce_block(lowered, bridge, body)),
            ExprKind::If { .. } => {
                let id = expr_id(expr.span);
                node_from_lowered(lowered, bridge, id, &mut out);
            }
            _ => {}
        }
    }
    out
}

fn node_from_lowered_arm(
    lowered: &LoweredIr,
    bridge: &Bridge,
    expr: &Expr,
    out: &mut Vec<ViewNode>,
) {
    let id = expr_id(expr.span);
    node_from_lowered(lowered, bridge, id, out);
}

fn render_cond(cond: &Expr) -> String {
    canonicalize_expr(&render_expr(cond))
}

fn screen_route(bridge: &Bridge, id: NodeId) -> String {
    let Some(expr) = bridge.expr(id) else {
        return String::new();
    };
    if let ExprKind::Call { args, .. } = &expr.kind {
        for arg in args {
            match arg {
                Arg::Named { name, value } if name.name == "route" => {
                    return canonicalize_expr(&render_expr(value));
                }
                Arg::Positional(value) => return canonicalize_expr(&render_expr(value)),
                _ => {}
            }
        }
    }
    String::new()
}

fn pattern_label(pattern: &MatchPattern) -> String {
    match &pattern.kind {
        MatchPatternKind::Wildcard => "_".to_owned(),
        MatchPatternKind::Variant { name, .. } => name.name.clone(),
        MatchPatternKind::Literal(expr) => canonicalize_expr(&render_expr(expr)),
        MatchPatternKind::Guard { name, .. } => name.name.clone(),
        _ => "<pattern>".to_owned(),
    }
}

/// Recovers the callee name from a call expression.
fn callee_name(expr: &Expr) -> Option<String> {
    match &expr.kind {
        ExprKind::Ident(ident) => Some(ident.name.clone()),
        ExprKind::Field { field, .. } => Some(field.name.clone()),
        _ => None,
    }
}

/// Renders an expression to a canonical, backend-agnostic string so Swift
/// `\(x)` and Kotlin `${x}` compare equal. Mirrors `flux_parity::bridge::canonicalize_expr`.
fn canonicalize_expr(text: &str) -> String {
    // Collapse any spelling of the "unsupported expression" placeholder to a
    // canonical `0` so the dev-path and release-path reduced trees compare
    // equal under JSON parity (mirrors `flux_parity::bridge::canonicalize_expr`).
    if text.to_ascii_lowercase().contains("unsupported") {
        return "0".to_owned();
    }
    let t = text.trim();
    if t.starts_with('"') && t.ends_with('"') && t.len() >= 2 {
        // String literal: normalize interpolation delimiters to `{…}`.
        let inner = &t[1..t.len() - 1];
        let inner = inner.replace("\\(", "{").replace("${", "{");
        return format!("\"{inner}\"");
    }
    // Key paths: `\.self` / `\.id` (Swift) and `{ it }` / `{ it.id }` (Kotlin).
    let t = t
        .replace("\\.self", "key:.self")
        .replace("\\.id", "key:.id");
    t.replace("{ it }", "key:.self")
        .replace("{ it.id }", "key:.id")
}

/// Renders a `ForEach` key extractor to canonical form, matching the dev-path
/// [`flux_parity::bridge::render_key`] so the two reduced trees compare equal.
/// A key lambda `fn(u){u.id}` → `key:.id`; `fn(u){u}` → `key:.self`.
fn render_key(key: &Expr) -> String {
    if let ExprKind::Lambda { params, body } = &key.kind {
        if let Some(param) = params.first() {
            if let Some(flux_parser::BlockItem::Expr(inner)) = body.items.first() {
                if let ExprKind::Field { base, field } = &inner.kind {
                    if let ExprKind::Ident(base_id) = &base.kind {
                        if base_id.name == param.name.name {
                            return format!("key:.{}", field.name);
                        }
                    }
                }
            }
        }
    }
    "key:.self".to_owned()
}

/// Backend-agnostic expression renderer, kept identical to the dev-path
/// [`flux_parity::bridge::render_expr`] so the two reduced trees canonicalize to
/// the same strings and JSON parity holds.
fn render_expr(expr: &Expr) -> String {
    match &expr.kind {
        ExprKind::Int(value) => value.to_string(),
        ExprKind::Float(value) => render_float(*value),
        ExprKind::Bool(value) => value.to_string(),
        ExprKind::Str(parts) => render_string(parts),
        ExprKind::Ident(ident) => ident.name.clone(),
        ExprKind::Binary { op, lhs, rhs, .. } => {
            format!(
                "({} {} {})",
                render_expr(lhs),
                binop_symbol(*op),
                render_expr(rhs)
            )
        }
        ExprKind::Field { base, field, .. } => {
            format!("{}.{}", render_expr(base), field.name)
        }
        ExprKind::List(items) => {
            let rendered: Vec<String> = items.iter().map(render_expr).collect();
            format!("[{}]", rendered.join(", "))
        }
        _ => "/* unsupported expr */ 0".to_owned(),
    }
}

fn render_float(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.1}")
    } else {
        value.to_string()
    }
}

fn render_string(parts: &[StrPart]) -> String {
    let mut body = String::new();
    for part in parts {
        match part {
            StrPart::Text(text) => body.push_str(text),
            StrPart::Interp(expr) => {
                body.push('{');
                body.push_str(&render_expr(expr));
                body.push('}');
            }
            _ => {}
        }
    }
    format!("\"{body}\"")
}

fn binop_symbol(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Rem => "%",
        BinOp::Eq => "==",
        BinOp::Ne => "!=",
        BinOp::Lt => "<",
        BinOp::Gt => ">",
        BinOp::Le => "<=",
        BinOp::Ge => ">=",
        BinOp::And => "&&",
        BinOp::Or => "||",
        _ => "?",
    }
}
