//! The shared structural emitter for both Flux release backends (FLUX-047).
//!
//! [`Emitter`] walks the lowered reactive tree — whose *structure* (which nodes
//! exist and how they nest) is the source of truth — and renders native source
//! node by node, recovering per-node semantics (name, props, interpolations)
//! from the AST through the node-ID [`Bridge`]. The language-specific syntax is
//! supplied by the [`Backend`] trait, so this module contains no `if swift`
//! branches and is shared verbatim by `flux-codegen-kotlin` and
//! `flux-codegen-swift`.
//!
//! Indentation is driven by two backend constants: [`Backend::INDENT_UNIT`]
//! (spaces per level) and [`Backend::CHILD_STEP`] (how far a container's
//! children sit inside the container open line). This keeps the Kotlin and
//! Swift indentation tables identical to the pre-refactor output (Kotlin 4-space
//! units with children one level in; Swift 1-space units with children four
//! spaces in).

use flux_ir::LoweredIr;
use flux_parser::{Arg, Block, Expr, ExprKind};
use flux_syntax::NodeId;
use std::collections::HashMap;

use crate::backend::Backend;
use crate::bridge::Bridge;
use crate::expressions::{render_expr, render_handler_body};
use crate::model::{ComponentMeta, native_type};
use crate::primitives::{PrimitiveKind, PrimitiveSpec};

/// State threaded through a single [`Emitter::emit_program`] run.
pub struct Emitter<'a, B: Backend> {
    /// Defensive: the backend is zero-sized, but we keep the param for clarity.
    _backend: std::marker::PhantomData<B>,
    /// The lowered reactive tree (structure).
    lowered: &'a LoweredIr,
    /// The node-ID bridge back to the surface AST (semantics).
    bridge: &'a Bridge,
    /// The accumulated native source.
    out: String,
}

impl<'a, B: Backend> std::fmt::Debug for Emitter<'a, B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Emitter")
            .field("backend", &std::any::type_name::<B>())
            .field("out_len", &self.out.len())
            .finish()
    }
}

impl<'a, B: Backend> Emitter<'a, B> {
    /// Creates an emitter over `lowered` and its bridge.
    pub fn new(lowered: &'a LoweredIr, bridge: &'a Bridge) -> Self {
        Self {
            _backend: std::marker::PhantomData,
            lowered,
            bridge,
            out: String::new(),
        }
    }

    /// Consumes the emitter, returning the generated native source.
    #[must_use]
    pub fn finish(self) -> String {
        self.out
    }

    /// Emits an entire program: algebraic types first, then one component per
    /// lowered `Component` node, in packing order.
    pub fn emit_program(&mut self) {
        self.emit_sum_types();
        let ids: Vec<NodeId> = self.lowered.arena.all_ids().collect();
        let mut first = true;
        for id in ids {
            let Some(node) = self.lowered.arena.get(id) else {
                continue;
            };
            if node.kind() != flux_syntax::NodeKind::Component {
                continue;
            }
            if !(first && self.bridge.types().is_empty()) {
                self.out.push('\n');
            }
            first = false;
            self.emit_component(id);
        }
    }

    /// Emits one component.
    fn emit_component(&mut self, id: NodeId) {
        let node = self.lowered.arena.get(id).expect("component node");
        let Some(comp) = self.bridge.component(id) else {
            B::emit_placeholder_component(self, id);
            return;
        };
        let meta = ComponentMeta::new(comp);
        let name = &comp.name.name;
        let generics = meta.generic_clause();
        B::emit_component_header(self, name, &generics, &meta);
        if !meta.is_pure {
            self.emit_state(&meta);
        }
        B::emit_body_open(self);
        let body_indent = B::component_body_indent();
        self.emit_children(Self::child_ids(node), body_indent);
        B::emit_component_footer(self);
    }

    /// Dispatches a single node to its structural renderer.
    pub fn emit_node(&mut self, id: NodeId, indent: usize) {
        let Some(node) = self.lowered.arena.get(id) else {
            return;
        };
        match node.kind() {
            flux_syntax::NodeKind::Component => {
                if let Some(comp) = self.bridge.component(id) {
                    let name = &comp.name.name;
                    self.line(indent, &format!("{name}()"));
                }
            }
            flux_syntax::NodeKind::Primitive => self.emit_primitive(id, indent),
            flux_syntax::NodeKind::If => self.emit_if(id, indent),
            flux_syntax::NodeKind::ForEach => self.emit_for_each(id, indent),
            flux_syntax::NodeKind::Match => self.emit_match(id, indent),
            _ => {}
        }
    }

    /// Emits a list of child nodes at `indent` levels of indentation.
    pub fn emit_children(&mut self, ids: Vec<NodeId>, indent: usize) {
        for child in ids {
            self.emit_node(child, indent);
        }
    }

    /// Writes `text` at `indent` levels, followed by a newline.
    pub fn line(&mut self, indent: usize, text: &str) {
        self.out.push_str(Self::indent_prefix(indent));
        self.out.push_str(text);
        self.out.push('\n');
    }

    /// Appends a raw string (e.g. a blank separator line) with no indentation.
    pub fn push_raw(&mut self, s: &str) {
        self.out.push_str(s);
    }

    /// Writes a complete line with no indentation (used by backend component/
    /// sum-type header hooks, which manage their own formatting).
    pub fn append_line(&mut self, text: &str) {
        self.out.push_str(text);
        self.out.push('\n');
    }

    /// Renders an expression to native text using this emitter's backend.
    #[must_use]
    pub fn render(&self, expr: &Expr) -> String {
        render_expr::<B>(expr)
    }

    /// Looks up the surface expression recorded for a lowered node id.
    #[must_use]
    pub fn lookup_expr(&self, id: NodeId) -> Option<&Expr> {
        self.bridge.expr(id)
    }

    /// Returns the flattened child node ids of `node` (resolving splices).
    pub fn child_ids(node: flux_ir::NodeView<'_>) -> Vec<NodeId> {
        let mut ids = Vec::new();
        for child in node.children() {
            for id in child.node_ids() {
                ids.push(id);
            }
        }
        ids
    }

    /// Emits a block's UI children (the parts that lower to nodes).
    pub fn emit_block_body(&mut self, block: &Block, indent: usize) {
        for item in &block.items {
            if let flux_parser::BlockItem::Expr(expr) = item {
                let id = crate::bridge::expr_id(expr.span);
                if self.lowered.arena.get(id).is_some() {
                    self.emit_node(id, indent);
                }
            }
        }
    }

    /// Emits the lowered node for a single expression body (used by match arms).
    pub fn emit_expr_body(&mut self, expr: &Expr, indent: usize) {
        let id = crate::bridge::expr_id(expr.span);
        if self.lowered.arena.get(id).is_some() {
            self.emit_node(id, indent);
        }
    }

    /// Returns the lowered child ids of `block`'s UI-producing expressions.
    pub fn block_children(&self, block: &Block) -> Vec<NodeId> {
        let mut ids = Vec::new();
        for item in &block.items {
            if let flux_parser::BlockItem::Expr(expr) = item {
                let id = crate::bridge::expr_id(expr.span);
                if self.lowered.arena.get(id).is_some() {
                    ids.push(id);
                }
            }
        }
        ids
    }

    /// Returns the lowered child ids of an `else`/`otherwise` branch, which may
    /// arrive either as a bare block or as a zero-arg lambda `{ … }`.
    pub fn branch_children(&self, branch: &Expr) -> Vec<NodeId> {
        match &branch.kind {
            ExprKind::Lambda { params, body } if params.is_empty() => self.block_children(body),
            _ => {
                let id = crate::bridge::expr_id(branch.span);
                if self.lowered.arena.get(id).is_some() {
                    vec![id]
                } else {
                    Vec::new()
                }
            }
        }
    }

    /// Collects named + trailing prop values into a name→rendered-expr map.
    pub fn collect_props(&self, args: &[Arg], trailing: Option<&Block>) -> HashMap<String, String> {
        let mut props = HashMap::new();
        for arg in args {
            if let Arg::Named { name, value } = arg {
                props.insert(name.name.clone(), render_expr::<B>(value));
            }
        }
        if let Some(block) = trailing {
            for item in &block.items {
                if let flux_parser::BlockItem::Prop { name, value } = item {
                    props.insert(name.name.clone(), render_expr::<B>(value));
                }
            }
        }
        props
    }

    // ----- Shared structural renderers (language-neutral) -----

    /// Emits every algebraic type preceding the components.
    fn emit_sum_types(&mut self) {
        let mut first = true;
        for sum in self.bridge.types() {
            if !first {
                self.push_raw("\n");
            }
            first = false;
            B::emit_sum_type(self, sum);
        }
    }

    /// Emits `@State`/remember state cells for a component.
    fn emit_state(&mut self, meta: &ComponentMeta<'_>) {
        for state in meta.states() {
            let ty = match &state.ty {
                Some(t) => native_type::<B>(t),
                None => B::any_type().to_owned(),
            };
            let init = render_expr::<B>(&state.init);
            B::emit_state_cell(self, &state.name.name, &ty, &init);
        }
    }

    /// Emits a primitive call as native source, driven by the registry.
    fn emit_primitive(&mut self, id: NodeId, indent: usize) {
        let Some(expr) = self.bridge.expr(id) else {
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
        let Some(spec) = PrimitiveSpec::by_name(&name) else {
            // A user-defined component call (not a built-in primitive): emit it as
            // a native composite `Name(args) { children }`, exactly like the
            // pre-refactor code did (which emitted `Name()`). The arguments are
            // rendered so the generated source stays faithful; the parity reducer
            // treats non-container user calls as childless, which matches the dev
            // path (a component call's own subtree lives in its definition).
            let args = Self::render_args(args);
            self.line(indent, &format!("{name}({args})"));
            return;
        };
        let native = B::native_name(spec);
        let props = self.collect_props(args, trailing.as_deref());

        match spec.kind {
            PrimitiveKind::Container => {
                let gap = props.get("gap").map(String::as_str).unwrap_or("");
                let spacing = if gap.is_empty() {
                    String::new()
                } else {
                    B::container_spacing(gap)
                };
                self.line(indent, &format!("{native}{spacing} {{"));
                self.emit_trailing_or_children(trailing.as_deref(), id, indent + B::CHILD_STEP);
                self.line(indent, "}");
            }
            PrimitiveKind::Leaf => {
                let primary = primary_value::<B>(spec, &props, args);
                let value = primary
                    .map(render_inline)
                    .unwrap_or_else(|| "\"\"".to_owned());
                if spec.flux_name == "Image" {
                    self.line(indent, &B::image_expr(&value));
                } else {
                    self.line(indent, &format!("{native}({value})"));
                }
            }
            PrimitiveKind::Button => {
                let label = Self::render_button_label(args, trailing.as_deref());
                let handler = Self::collect_handler(args);
                self.line(indent, &B::button_open(&handler));
                self.line(indent + B::CHILD_STEP, &format!("Text({label})"));
                self.line(indent, "}");
            }
            PrimitiveKind::Other => {
                // Primitives emitted as a bare call (no special shaping yet):
                // CupertinoButton, MaterialButton, TextField, Provider, When,
                // Switch. This reproduces the pre-refactor `other => "{other}()"`
                // catch-all so the committed parity snapshots stay valid; richer
                // native shaping is future work once their dev-model semantics land.
                self.line(indent, &format!("{name}()"));
            }
            PrimitiveKind::Router | PrimitiveKind::Screen => {
                // `Router`/`Screen` lower as `Primitive` nodes (not dedicated
                // `NodeKind`s), so they reach here by name. Route them to the
                // navigation renderers the same way the pre-refactor code did.
                if spec.flux_name == "Router" {
                    self.emit_router(id, indent);
                } else {
                    self.emit_screen(id, indent);
                }
            }
        }
    }

    /// Emits an `if`/`when` branch as native conditional source.
    fn emit_if(&mut self, id: NodeId, indent: usize) {
        let Some(expr) = self.bridge.expr(id) else {
            return;
        };
        let (cond, then_children, else_children): (&Expr, Vec<NodeId>, Vec<NodeId>) =
            match &expr.kind {
                ExprKind::If {
                    cond,
                    then_block,
                    else_branch,
                } => {
                    let then = self.block_children(then_block);
                    let els = match else_branch {
                        Some(b) => self.branch_children(b),
                        None => Vec::new(),
                    };
                    (cond, then, els)
                }
                ExprKind::When {
                    cond,
                    then_block,
                    otherwise,
                } => {
                    let then = self.block_children(then_block);
                    let els = match otherwise {
                        Some(b) => self.block_children(b),
                        None => Vec::new(),
                    };
                    (cond, then, els)
                }
                _ => return,
            };
        let cond_text = render_expr::<B>(cond);
        self.line(indent, &B::if_open(&cond_text));
        self.emit_children(then_children, indent + B::CHILD_STEP);
        if !else_children.is_empty() {
            self.line(indent, "} else {");
            self.emit_children(else_children, indent + B::CHILD_STEP);
        }
        self.line(indent, "}");
    }

    /// Emits a `Screen` destination. Swift emits a `// Screen route:` comment
    /// and inlines the body at the same indent; Kotlin opens a `composable(...)`
    /// block. The body is emitted by the backend via `screen_open`/`screen_close`.
    fn emit_screen(&mut self, id: NodeId, indent: usize) {
        let Some(expr) = self.bridge.expr(id) else {
            return;
        };
        let ExprKind::Call { args, trailing, .. } = &expr.kind else {
            return;
        };
        let route = args
            .first()
            .map(|a| render_expr::<B>(a.value()))
            .unwrap_or_else(|| "\"\"".to_owned());
        self.line(indent, &B::screen_open(&route));
        if let Some(block) = trailing {
            self.emit_block_body(block, indent + B::SCREEN_BODY_STEP);
        }
        let close = B::screen_close();
        if !close.is_empty() {
            self.line(indent, &close);
        }
    }

    /// Emits a `ForEach` collection view using backend-specific open/close forms.
    fn emit_for_each(&mut self, id: NodeId, indent: usize) {
        let Some(expr) = self.bridge.expr(id) else {
            return;
        };
        let ExprKind::ForEach { items, key, body } = &expr.kind else {
            return;
        };
        let collection = render_expr::<B>(items);
        let key = B::key_extractor(key);
        let element = match body.params.first() {
            Some(flux_parser::Pattern::Ident(id)) => id.name.clone(),
            _ => "item".to_owned(),
        };
        self.line(indent, &B::for_each_open(&collection, &key, &element));
        self.emit_block_body(body, indent + B::CHILD_STEP);
        self.line(indent, &B::for_each_close());
    }

    /// Emits a `Router` navigation container.
    fn emit_router(&mut self, id: NodeId, indent: usize) {
        let _node = self.lowered.arena.get(id);
        self.line(indent, &B::router_open());
        self.emit_children_under_router(id, indent + B::CHILD_STEP);
        self.line(indent, &B::router_close());
    }

    /// Emits the children of a `Router` node (its Screen destinations).
    fn emit_children_under_router(&mut self, id: NodeId, indent: usize) {
        let Some(node) = self.lowered.arena.get(id) else {
            return;
        };
        let ids = Self::child_ids(node);
        self.emit_children(ids, indent);
    }

    /// Emits a `match` over an algebraic data type by delegating to the backend.
    fn emit_match(&mut self, id: NodeId, indent: usize) {
        B::emit_match(self, id, indent);
    }

    /// Emits the body of a trailing block (component children) at `indent`.
    fn emit_trailing_or_children(&mut self, trailing: Option<&Block>, id: NodeId, indent: usize) {
        if let Some(block) = trailing {
            self.emit_block_body(block, indent);
        } else {
            let Some(node) = self.lowered.arena.get(id) else {
                return;
            };
            self.emit_children(Self::child_ids(node), indent);
        }
    }

    /// Renders a call's argument list (`name: value, value, …`) for a user
    /// component invocation. Built-in primitives shape their arguments through
    /// the `Backend` trait instead.
    fn render_args(args: &[Arg]) -> String {
        let rendered: Vec<String> = args
            .iter()
            .map(|arg| match arg {
                Arg::Named { name, value } => {
                    format!("{}: {}", name.name, render_expr::<B>(value))
                }
                Arg::Positional(value) => render_expr::<B>(value),
                _ => String::new(),
            })
            .collect();
        rendered.join(", ")
    }

    /// Renders the button label (named `text:` arg or a `Text(…)` child).
    fn render_button_label(args: &[Arg], trailing: Option<&Block>) -> String {
        for arg in args {
            if let Arg::Named { name, value } = arg {
                if name.name == "text" {
                    return render_inline(render_expr::<B>(value));
                }
            }
        }
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
            if let Some(positional) = args.iter().find_map(|a| match a {
                Arg::Positional(value) => Some(render_inline(render_expr::<B>(value))),
                _ => None,
            }) {
                return positional;
            }
            for arg in args {
                if let Arg::Named { name, value } = arg {
                    if name.name == "text" {
                        return render_inline(render_expr::<B>(value));
                    }
                }
            }
        }
        "\"\"".to_owned()
    }

    /// Finds the `onClick`/`onTap` handler and renders its body as statements.
    fn collect_handler(args: &[Arg]) -> String {
        for arg in args {
            let Arg::Named { name, value } = arg else {
                continue;
            };
            if name.name != "onClick" && name.name != "onTap" {
                continue;
            }
            if let Some(body) = render_handler_body::<B>(value) {
                return body;
            }
        }
        String::new()
    }

    /// Returns the whitespace prefix for `indent` levels (each level =
    /// `INDENT_UNIT` spaces).
    fn indent_prefix(indent: usize) -> &'static str {
        // Per-backend indent tables keep this allocation-free for the common
        // levels; deep nesting allocates a single string (rare).
        const KOTLIN_TABLE: [&str; 17] = [
            "",
            "    ",
            "        ",
            "            ",
            "                ",
            "                    ",
            "                        ",
            "                            ",
            "                                ",
            "                                    ",
            "                                        ",
            "                                            ",
            "                                                ",
            "                                                    ",
            "                                                        ",
            "                                                            ",
            "                                                                ",
        ];
        const SWIFT_TABLE: [&str; 17] = [
            "",
            " ",
            "  ",
            "   ",
            "    ",
            "     ",
            "      ",
            "       ",
            "        ",
            "         ",
            "          ",
            "           ",
            "            ",
            "             ",
            "              ",
            "               ",
            "                ",
        ];
        let table: &[&str] = if B::INDENT_UNIT == 4 {
            &KOTLIN_TABLE
        } else {
            &SWIFT_TABLE
        };
        table
            .get(indent)
            .copied()
            .unwrap_or_else(|| Box::leak(" ".repeat(indent * B::INDENT_UNIT).into_boxed_str()))
    }
}

/// Returns the "primary" value for a leaf primitive: its `primary_prop`, else a
/// positional argument (e.g. `Text("…")`, `Image(url)`).
fn primary_value<B: Backend>(
    spec: &PrimitiveSpec,
    props: &HashMap<String, String>,
    args: &[Arg],
) -> Option<String> {
    if let Some(p) = spec.primary_prop {
        if let Some(v) = props.get(p) {
            return Some(v.clone());
        }
    }
    args.iter().find_map(|a| match a {
        Arg::Positional(value) => Some(render_expr::<B>(value)),
        _ => None,
    })
}

/// Renders a prop value for inline use (Text, Image). Bare literals and
/// interpolations pass through unchanged; this exists so call sites read
/// declaratively.
fn render_inline(value: String) -> String {
    value
}
