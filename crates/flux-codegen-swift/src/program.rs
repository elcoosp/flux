//! The SwiftUI emitter: structural traversal of a lowered Flux program.
//!
//! [`Emitter`] walks the lowered reactive tree — whose *structure* (which nodes
//! exist and how they nest) is the source of truth — and renders one Swift
//! `struct …: View` per component. Per-node semantics (names, props,
//! interpolations, generics, `@pure`) are recovered from the AST through the
//! node-ID bridge, captured here as a cheap, borrow-only [`Emitter`].

use flux_ir::LoweredIr;
use flux_parser::{Arg, Block, Expr};
use flux_syntax::NodeId;
use std::collections::HashMap;
use std::fmt::Write;

use crate::bridge::Bridge;
use crate::model::{ComponentMeta, swift_type};

/// State threaded through a single [`Emitter::emit_program`] run.
pub(crate) struct Emitter<'a> {
    pub(crate) lowered: &'a LoweredIr,
    pub(crate) bridge: &'a Bridge,
    /// The accumulated Swift source.
    out: String,
}

impl<'a> Emitter<'a> {
    /// Creates an emitter over `lowered` and its bridge.
    pub(crate) fn new(lowered: &'a LoweredIr, bridge: &'a Bridge) -> Self {
        Self {
            lowered,
            bridge,
            out: String::new(),
        }
    }

    /// Consumes the emitter, returning the generated Swift source.
    #[must_use]
    pub(crate) fn finish(self) -> String {
        self.out
    }

    /// Emits an entire program: one Swift `struct` per component, in the order
    /// the lowering pass packed them.
    pub(crate) fn emit_program(&mut self) {
        let ids: Vec<NodeId> = self.lowered.arena.all_ids().collect();
        let mut first = true;
        for id in ids {
            let Some(node) = self.lowered.arena.get(id) else {
                continue;
            };
            if node.kind() != flux_syntax::NodeKind::Component {
                continue;
            }
            if !first {
                self.out.push('\n');
            }
            first = false;
            self.emit_component(id);
        }
    }

    /// Emits one component as a `struct …: View`.
    fn emit_component(&mut self, id: NodeId) {
        let node = self.lowered.arena.get(id).expect("component node");
        let Some(comp) = self.bridge.component(id) else {
            // No AST component for this node: emit a thin placeholder.
            let _ = writeln!(self.out, "struct FluxComponent_{id}: View {{");
            let _ = writeln!(self.out, "    var body: some View {{ EmptyView() }}");
            let _ = writeln!(self.out, "}}");
            return;
        };
        let meta = ComponentMeta::new(comp);
        let name = &comp.name.name;
        let generics = meta.generic_clause();

        let _ = writeln!(self.out, "struct {name}{generics}: View {{");
        // Props become immutable stored properties (flux_syntax::ComponentKind
        // fully inferred at the call site); @pure components are stateless.
        for prop in meta.props() {
            let ty = swift_type(&prop.ty);
            let _ = writeln!(self.out, "    let {}: {}", prop.name.name, ty);
        }
        if !meta.is_pure {
            self.emit_state(&meta);
        }
        let _ = writeln!(self.out, "    var body: some View {{");
        self.emit_children(Self::child_ids(node), 2);
        let _ = writeln!(self.out, "    }}");
        let _ = writeln!(self.out, "}}");
    }

    /// Emits `@State private var …` for each declared state cell.
    fn emit_state(&mut self, meta: &ComponentMeta<'_>) {
        for state in meta.states() {
            let ty = match &state.ty {
                Some(t) => swift_type(t),
                None => "Any".to_owned(),
            };
            let init = crate::expressions::render_expr(&state.init);
            let _ = writeln!(
                self.out,
                "    @State private var {}: {} = {}",
                state.name.name, ty, init
            );
        }
    }

    /// Emits a list of child nodes at `indent` spaces of indentation.
    pub(crate) fn emit_children(&mut self, ids: Vec<NodeId>, indent: usize) {
        for child in ids {
            self.emit_node(child, indent);
        }
    }

    /// Dispatches a single node to its renderer.
    pub(crate) fn emit_node(&mut self, id: NodeId, indent: usize) {
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
            flux_syntax::NodeKind::Primitive => crate::nodes::emit_primitive(self, id, indent),
            flux_syntax::NodeKind::If => crate::nodes::emit_if(self, id, indent),
            flux_syntax::NodeKind::ForEach => crate::nodes::emit_for_each(self, id, indent),
            flux_syntax::NodeKind::Match => crate::nodes::emit_match(self, id, indent),
            flux_syntax::NodeKind::Router => crate::nodes::emit_router(self, id, indent),
            flux_syntax::NodeKind::Screen => crate::nodes::emit_screen(self, id, indent),
            _ => {}
        }
    }

    /// Writes `text` at `indent` spaces, followed by a newline.
    pub(crate) fn line(&mut self, indent: usize, text: &str) {
        let _ = write!(self.out, "{}", " ".repeat(indent));
        let _ = writeln!(self.out, "{text}");
    }

    /// Returns the flattened child node ids of `node` (resolving splices).
    pub(crate) fn child_ids(node: flux_ir::NodeView<'_>) -> Vec<NodeId> {
        let mut ids = Vec::new();
        for child in node.children() {
            for id in child.node_ids() {
                ids.push(id);
            }
        }
        ids
    }

    /// Emits a block's UI children (the parts that lower to nodes).
    pub(crate) fn emit_block_body(&mut self, block: &Block, indent: usize) {
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
    pub(crate) fn emit_expr_body(&mut self, expr: &Expr, indent: usize) {
        let id = crate::bridge::expr_id(expr.span);
        if self.lowered.arena.get(id).is_some() {
            self.emit_node(id, indent);
        }
    }

    /// Returns the lowered child ids of `block`'s UI-producing expressions.
    pub(crate) fn block_children(&self, block: &Block) -> Vec<NodeId> {
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
    pub(crate) fn branch_children(&self, branch: &Expr) -> Vec<NodeId> {
        match &branch.kind {
            flux_parser::ExprKind::Lambda { params, body } if params.is_empty() => {
                self.block_children(body)
            }
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
    pub(crate) fn collect_props(
        &self,
        args: &[Arg],
        trailing: Option<&Block>,
    ) -> HashMap<String, String> {
        let mut props = HashMap::new();
        for arg in args {
            if let Arg::Named { name, value } = arg {
                props.insert(name.name.clone(), crate::expressions::render_expr(value));
            }
        }
        if let Some(block) = trailing {
            for item in &block.items {
                if let flux_parser::BlockItem::Prop { name, value } = item {
                    props.insert(name.name.clone(), crate::expressions::render_expr(value));
                }
            }
        }
        props
    }
}
