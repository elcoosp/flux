//! AST → TypedAST → reactive-tree IR lowering (FLUX-018).
//!
//! This module walks a [`flux_parser::Ast`] that has already been type-checked
//! into a [`flux_types::TypedAST`] and emits the packed [`IRArena`] that the
//! differ and wire codec consume. Every emitted node carries the *same*
//! [`flux_syntax::NodeId`] that the type checker used to key `typed.types`, so downstream
//! code can look up the inferred type for an IR node by ID (ADR-0027 — the
//! "node-ID bridge"). The shared identifier type is [`flux_syntax::NodeId`].
//!
//! # The ADR-0027 bridge
//!
//! The type checker records every expression under
//! `compute_node_id(0, 10, span, None)` and every declaration under
//! `compute_node_id(0, decl_tag, span, None)` — always with parent `0` (see
//! `flux-types/src/checker.rs`). Lowering must reproduce that exact
//! `(parent, tag, span, key)` tuple for each node it emits, which is why this
//! crate delegates to the canonical `flux_syntax::compute_node_id` and never
//! re-derives IDs with a local hash. The node's [`NodeKind`] (`Component`,
//! `Primitive`, …) is a *separate* wire discriminant from the structural tag
//! used for the ID.
//!
//! # Handlers
//!
//! Handler bodies are compiled to [`ClosureIR`] bytecode (Appendix E) and
//! registered in the arena's closure table. The wire transport for that
//! bytecode is a follow-up (Gap G1, `flux-ir-serde` second pass); this module
//! ships the `ClosureIR` only — see `handler.rs`.

pub(crate) mod bytecode;
pub(crate) mod error;
pub(crate) mod ids;
pub(crate) mod mono;

pub use bytecode::{HandlerCompileError, compile_handler, compile_handler_with_params};
pub use error::LoweringError;
pub use mono::{Monomorphization, mangle_specialised};

use flux_parser::{Ast, ComponentDecl, Decl, Expr, ExprKind};
use flux_syntax::{Child, ComponentId, Key, NodeId, NodeKind, Props, Span, Value};
use flux_types::TypedAST;

/// Whether `kind` is a UI-producing expression that should become its own
/// child node in the reactive tree.
///
/// `let`, `onMount`/`onCleanup`/`effect`, `provide`, `useContext`, `resource`
/// and `createRef` are not UI producers — they are handled by the codegen layer
/// from the AST and contribute no child node. Skipping them here lets component
/// bodies that bind refs or declare lifecycle hooks still lower.
fn is_ui_expr(kind: &ExprKind) -> bool {
    matches!(
        kind,
        ExprKind::Call { .. }
            | ExprKind::If { .. }
            | ExprKind::When { .. }
            | ExprKind::ForEach { .. }
            | ExprKind::Match { .. }
    )
}

use crate::arena::IRArena;
use crate::builder::{ArenaBuilder, Node};
use crate::closure::ClosureIR;
use crate::instance::InstanceRegistry;
use crate::lower::bytecode::{collect_read_signals, compile_prop_thunk};
use ids::{ExprNodeKind, decl_node_id, expr_node_id};

/// The fully lowered program.
///
/// Returned by [`lower`]; bundles the packed [`IRArena`], the handler closure
/// table (keyed by [`flux_syntax::HandlerId`]), the per-node prop thunks
/// (ADR-0027 T14, keyed by [`flux_syntax::NodeId`]), and the per-component
/// [`InstanceRegistry`] that lets the host app preserve state across hot swaps.
#[derive(Clone, Debug)]
pub struct LoweredIr {
    /// The packed reactive tree.
    pub arena: IRArena,
    /// Handler closures, keyed by their [`flux_syntax::HandlerId`].
    pub closures: std::collections::HashMap<flux_syntax::HandlerId, ClosureIR>,
    /// Prop thunks, keyed by the [`flux_syntax::NodeId`] of the node they
    /// materialise. Each thunk's bytecode is the body of one `ClosureIR`
    /// (reusing the handler-closure machinery) and is referenced from the
    /// node's `prop_thunk` closure reference on the wire.
    pub prop_thunks: std::collections::HashMap<flux_syntax::NodeId, ClosureIR>,
    /// Initial values for state signals, keyed by their allocated id.
    pub state_seed: Vec<(flux_syntax::SignalId, flux_syntax::Value)>,
    /// Component-name interning: `(ComponentId, name)` pairs so the dev server
    /// can ship them in the Init frame's string table (Appendix D §D.9), letting
    /// a host resolve each node's adapter from its `ComponentId`.
    pub component_names: Vec<(flux_syntax::ComponentId, String)>,
    /// Generic instantiations this program specialised, in resolution order
    /// (roadmap Phase 1). Each entry names one monomorphised component the
    /// release backends must emit as its own native type. Empty when the
    /// program uses no generic components.
    pub monomorphizations: Vec<Monomorphization>,
    /// Live component-instance registry.
    pub instances: InstanceRegistry,
}

impl LoweredIr {
    /// Returns the closure registered for `handler`, if any.
    #[must_use]
    pub fn closure(&self, handler: flux_syntax::HandlerId) -> Option<&ClosureIR> {
        self.closures.get(&handler)
    }

    /// Returns `true` when the program contains at least one generic
    /// instantiation the backends must monomorphise.
    #[must_use]
    pub fn requires_monomorph(&self) -> bool {
        !self.monomorphizations.is_empty()
    }

    /// Returns the specialised (mangled) names of every instantiation, in
    /// resolution order.
    #[must_use]
    pub fn specialised_names(&self) -> Vec<&str> {
        self.monomorphizations
            .iter()
            .map(|m| m.mangled.as_str())
            .collect()
    }
}

/// Lowers a type-checked program into the reactive-tree IR.
///
/// `lower` walks `ast` in declaration order and packs a [`Node`] per
/// runtime-relevant surface construct. The returned [`LoweredIr::arena`] carries
/// exactly the [`flux_syntax::NodeId`]s the type checker assigned (see the bridge note on
/// this module), so `typed.types.keys()` and `arena.all_ids()` are the same set
/// for every node the type checker typed.
///
/// # Errors
///
/// Returns [`LoweringError`] (carrying a [`flux_syntax::Span`]) when lowering cannot proceed
/// on well-typed input — for example a component the type checker typed but
/// lowering cannot resolve, or a handler whose body uses an unsupported form.
/// Malformed-*but-typed* input never panics.
///
/// # Examples
///
/// ```rust
/// use flux_ir::lower;
/// use flux_parser::parse;
/// use flux_types::type_check;
///
/// let src = "compo Hello\n  state count: Int = 0\n  Button(text: \"tap\")\n";
/// let ast = parse(src, 0, "hello.flux").unwrap();
/// let typed = type_check(&ast).expect("well-typed");
/// let lowered = lower(&ast, &typed).expect("lowers");
/// assert_eq!(lowered.arena.len(), 2);
/// ```
pub fn lower(ast: &Ast, typed: &TypedAST) -> Result<LoweredIr, LoweringError> {
    let mut lowerer = Lowerer::new(typed);
    for decl in &ast.decls {
        lowerer.lower_decl(decl)?;
    }
    Ok(lowerer.finish())
}

/// State threaded through a single [`lower`] run.
struct Lowerer<'a> {
    typed: &'a TypedAST,
    builder: ArenaBuilder,
    /// Interned component/primitive names → dense [`ComponentId`].
    name_to_component: std::collections::HashMap<String, ComponentId>,
    /// Next [`ComponentId`] to assign.
    next_component: ComponentId,
    /// All compiled closures, keyed by [`HandlerId`].
    closures: std::collections::HashMap<flux_syntax::HandlerId, ClosureIR>,
    /// Compiled prop thunks (ADR-0027 T14), keyed by the node id they
    /// materialise.
    prop_thunks: std::collections::HashMap<flux_syntax::NodeId, ClosureIR>,
    /// Signals owned by the enclosing component, named for handler capture.
    signal_scope: Vec<(String, flux_syntax::SignalId)>,
    /// Initial values for state signals, paired with their allocated id.
    state_seed: Vec<(flux_syntax::SignalId, flux_syntax::Value)>,
    /// Per-component signal allocator (resets each component).
    signal_counter: flux_syntax::SignalId,
    /// Handler allocator (monotonic across the whole program).
    handler_counter: flux_syntax::HandlerId,
    /// Generic-instantiation cursor (roadmap Phase 1).
    mono: mono::MonoTable,
    /// Names of record types declared with `record Name { … }`; calls to these
    /// construct values (FLUX-072), not capability invocations.
    record_ctors: std::collections::HashSet<String>,
    /// Declared component bodies, retained so `lower_call` can inline a
    /// component's body at the call site — binding its `prop`s to the call's
    /// argument signals. This is what lets `TaskRow(task: item, …)` inside a
    /// `ForEach` read the per-row `itemSlot` directly instead of an unseeded
    /// component-global prop signal (the "tasks not rendered" bug, FLUX-072).
    component_decls: std::collections::HashMap<String, flux_parser::ComponentDecl>,
}

impl<'a> Lowerer<'a> {
    fn new(typed: &'a TypedAST) -> Self {
        Self {
            typed,
            builder: ArenaBuilder::new(),
            name_to_component: std::collections::HashMap::new(),
            next_component: ComponentId::from(0u32),
            closures: std::collections::HashMap::new(),
            prop_thunks: std::collections::HashMap::new(),
            signal_scope: Vec::new(),
            state_seed: Vec::new(),
            signal_counter: flux_syntax::SignalId::from(0u32),
            handler_counter: flux_syntax::HandlerId::from(0u32),
            mono: mono::MonoTable::new(typed),
            record_ctors: std::collections::HashSet::new(),
            component_decls: std::collections::HashMap::new(),
        }
    }

    fn finish(self) -> LoweredIr {
        let arena = self.builder.finish();
        let component_names = self
            .name_to_component
            .iter()
            .map(|(name, id)| (*id, name.clone()))
            .collect();
        LoweredIr {
            arena,
            closures: self.closures,
            prop_thunks: self.prop_thunks,
            state_seed: self.state_seed,
            component_names,
            monomorphizations: self.mono.into_resolved(),
            instances: InstanceRegistry::new(),
        }
    }

    /// Interns a component/primitive name, returning a stable [`ComponentId`].
    fn intern_component(&mut self, name: &str) -> ComponentId {
        if let Some(&id) = self.name_to_component.get(name) {
            return id;
        }
        self.next_component = ComponentId::from(self.next_component + 1);
        let id = self.next_component;
        self.name_to_component.insert(name.to_owned(), id);
        id
    }

    /// Interns a string value, returning its [`Value::Str`] handle.
    ///
    /// Interning flows through the [`ArenaBuilder`] so the accumulated table is
    /// carried into the finished arena (Gap G3 — every `Value::Str(id)` stays
    /// resolvable from `arena.string_table()`).
    fn intern_str(&mut self, text: &str) -> Value {
        let id = self.builder.intern_string(text);
        Value::Str(id)
    }

    /// Allocates a fresh [`SignalId`] for a state cell in the current component.
    fn next_signal(&mut self) -> flux_syntax::SignalId {
        self.signal_counter = flux_syntax::SignalId::from(self.signal_counter + 1);
        self.signal_counter
    }

    /// Allocates a fresh [`HandlerId`] for a compiled handler.
    fn next_handler(&mut self) -> flux_syntax::HandlerId {
        self.handler_counter = flux_syntax::HandlerId::from(self.handler_counter + 1);
        self.handler_counter
    }

    /// Emits the ADR-0027 Phase 2/3 signal-graph metadata for `node_id`
    /// (T13 `signal_deps`, T14 `prop_thunk`/`prop_layout`).
    ///
    /// `prop_exprs` are the node's prop value expressions (primitives only);
    /// `control_exprs` are the node's control expressions (cond / collection /
    /// key / scrutinee). The signal-set is the single source of truth derived
    /// from this same walk (never computed twice): when the prop expressions
    /// compile into a thunk, its captured `READ_SIGNAL` set *is* `signal_deps`;
    /// otherwise a plain recursive walk yields the same set as a fallback.
    fn emit_signal_metadata(
        &mut self,
        node_id: NodeId,
        prop_exprs: &[(flux_syntax::PropIdx, &Expr)],
        control_exprs: &[&Expr],
    ) -> Result<(), LoweringError> {
        let scope = &self.signal_scope;
        if !prop_exprs.is_empty() {
            let mut intern = |s: &str| self.builder.intern_string(s);
            match compile_prop_thunk(prop_exprs, &self.typed.field_indices, scope, &mut intern) {
                Ok((bytecode, deps, layout)) => {
                    let thunk_id = self.next_handler();
                    let closure =
                        ClosureIR::new(thunk_id, bytecode, deps.clone(), Span::new(0, 0, 0));
                    self.prop_thunks.insert(node_id, closure);
                    let closure_ref = flux_syntax::ClosureRef {
                        hash: crate::lower::bytecode::hash_closure_placeholder(
                            &self.prop_thunks[&node_id].bytecode,
                            &deps,
                        ),
                        bytecode_offset: 0,
                        bytecode_len: self.prop_thunks[&node_id].bytecode.len() as u16,
                        captured_signals: deps.clone(),
                        span: Span::new(0, 0, 0),
                        excerpt: None,
                    };
                    self.builder
                        .signal_metadata(node_id, deps, Some(closure_ref), layout, None);
                    return Ok(());
                }
                Err(_) => {
                    // Prop form cannot be compiled to the MLP envelope (e.g. a
                    // capability call); fall through to the control-only path so
                    // `signal_deps` still records every read, just without a thunk.
                }
            }
        }
        // Control-only nodes (If/When/ForEach/Match) or thunk-compile failure:
        // collect reads from control + prop exprs and emit no thunk.
        let mut all: Vec<&Expr> = control_exprs.to_vec();
        for (_, expr) in prop_exprs {
            all.push(*expr);
        }
        let deps = collect_read_signals(&all, scope);
        self.builder
            .signal_metadata(node_id, deps, None, Vec::new(), None);
        Ok(())
    }

    fn lower_decl(&mut self, decl: &Decl) -> Result<(), LoweringError> {
        match decl {
            // Only components produce runtime tree nodes in the MLP.
            Decl::Component(comp) => {
                // Retain the declaration so `lower_call` can inline the body
                // at the call site (binding `prop`s to argument signals).
                self.component_decls
                    .insert(comp.name.name.clone(), comp.clone());
                self.lower_component(comp)
            }
            // fn / type / trait / capability / import / use / const are
            // type-level only; the type checker handled them, so we skip them.
            Decl::Fn(_)
            | Decl::Type(_)
            | Decl::Trait(_)
            | Decl::Capability(_)
            | Decl::Use(_)
            | Decl::Const(_) => Ok(()),
            // Record types register their constructor name so handler calls to
            // `Name(field: …)` lower as value construction (FLUX-072).
            Decl::Record(rec) => {
                self.record_ctors.insert(rec.name.name.clone());
                Ok(())
            }
            #[allow(unreachable_patterns)]
            _ => Ok(()),
        }
    }

    fn lower_component(&mut self, comp: &flux_parser::ComponentDecl) -> Result<(), LoweringError> {
        let span = comp.span;
        let id = decl_node_id(&Decl::Component(comp.clone()));
        if !self.typed.types.contains_key(&id) {
            return Err(LoweringError::new(
                format!("no type recorded for component `{}`", comp.name.name),
                span,
            ));
        }
        let component_id = self.intern_component(&comp.name.name);

        // Signal ids are GLOBAL across the program: the VM owns a single signal
        // graph, so every state cell and component prop must get a unique id.
        // Do NOT reset `signal_counter` per component — doing so collides
        // `TodoApp.tasks` (signal 1) with `TaskRow.task` (also signal 1) in the
        // same graph, so a row's `task.label` reads the list and throws
        // `nullDereference` (the "tasks not rendered" bug, FLUX-072 / ADR-0050).
        self.signal_scope.clear();

        // Component props are backed by signals the host writes on each
        // reconcile (props ARE the observable surface per §3.5). Allocating a
        // signal id per prop lets handler bodies read/write `task`/`tasks`
        // directly; an unseeded prop reads as `Null` in the reference oracle
        // and is populated by the host at runtime (FLUX-072 #6 / #9).
        for prop in &comp.props {
            let sig = self.next_signal();
            self.signal_scope.push((prop.name.name.clone(), sig));
        }

        let children = self.lower_block(&comp.body, component_id)?;
        let node = Node {
            id,
            kind: NodeKind::Component,
            component_id,
            props: Props::default(),
            children,
            handlers: vec![],
            span,
        };
        self.builder.pack(node);
        // Components carry no props or control expressions, so their
        // `signal_deps` is empty and they have no prop thunk (ADR-0027 §T13).
        self.emit_signal_metadata(id, &[], &[])?;
        Ok(())
    }

    /// Lowers a block, returning the node-ids of the UI children it produces.
    ///
    /// `owner` is the [`ComponentId`] of the enclosing call site (used so
    /// state-signal IDs inside a child handler resolve against the owner
    /// component's scope). State declarations allocate signal slots; prop
    /// entries are ignored at component-body level (they belong to trailing
    /// call blocks, handled in [`Self::lower_call`]).
    fn lower_block(
        &mut self,
        block: &flux_parser::Block,
        owner: ComponentId,
    ) -> Result<Vec<Child>, LoweringError> {
        let mut children = Vec::with_capacity(block.items.len());
        for item in &block.items {
            match item {
                flux_parser::BlockItem::State(decl) => {
                    let sig = self.next_signal();
                    let mut init_handlers: Vec<flux_syntax::HandlerId> = Vec::new();
                    let init_value = self.lower_value(&decl.init, owner, &mut init_handlers)?;
                    self.state_seed.push((sig, init_value));
                    self.signal_scope.push((decl.name.name.clone(), sig));
                }
                flux_parser::BlockItem::Derived(decl) => {
                    // A derived signal is a computed, read-only signal. For the
                    // reference oracle we seed it with the lowered initial value
                    // of its body so it is usable in prop positions; the host
                    // runtime re-derives it from its sources on each change
                    // (FLUX-072 #12).
                    let sig = self.next_signal();
                    let mut init_handlers: Vec<flux_syntax::HandlerId> = Vec::new();
                    let init_value = self.lower_value(&decl.init, owner, &mut init_handlers)?;
                    self.state_seed.push((sig, init_value));
                    self.signal_scope.push((decl.name.name.clone(), sig));
                }
                flux_parser::BlockItem::Prop { .. } => {
                    // Prop entries at this level are not part of the MLP
                    // component body; they only appear inside trailing call
                    // blocks, which lower_call handles directly.
                }
                flux_parser::BlockItem::Expr(expr) if is_ui_expr(&expr.kind) => {
                    // Only UI-producing expressions become children of the
                    // reactive tree. Non-UI producers (`let`, `onMount`,
                    // `onCleanup`, `effect`, `provide`, `useContext`,
                    // `resource`, `createRef`) bind refs, declare lifecycle
                    // hooks, or surface capabilities and contribute no child
                    // node; codegen reads them from the AST directly.
                    let child = self.lower_expr(expr, owner)?;
                    children.push(child);
                }
                flux_parser::BlockItem::Expr(_) => {
                    // Non-UI expression: no child node (see note above).
                }
                #[allow(unreachable_patterns)]
                _ => {}
            }
        }
        Ok(children)
    }

    /// Lowers an expression into a single [`Child`] slot.
    fn lower_expr(
        &mut self,
        expr: &flux_parser::Expr,
        owner: ComponentId,
    ) -> Result<Child, LoweringError> {
        match &expr.kind {
            flux_parser::ExprKind::Call {
                callee,
                args,
                trailing,
            } => self.lower_call(expr, callee, args, trailing.as_deref(), owner),
            flux_parser::ExprKind::If {
                cond,
                then_block,
                else_branch,
            } => {
                let id = expr_node_id(expr, ExprNodeKind::If);
                let mut children = Vec::with_capacity(2);
                children.extend(self.lower_block(then_block, owner)?);
                if let Some(other) = else_branch {
                    // `else { block }` arrives as a zero-arg lambda per the
                    // parser; unwrap to its body block if so.
                    if let flux_parser::ExprKind::Lambda { params, body } = &other.kind {
                        if params.is_empty() {
                            children.extend(self.lower_block(body, owner)?);
                        } else {
                            children.push(self.lower_expr(other, owner)?);
                        }
                    } else {
                        children.push(self.lower_expr(other, owner)?);
                    }
                }
                let node = Node {
                    id,
                    kind: NodeKind::If,
                    component_id: owner,
                    props: Props::default(),
                    children,
                    handlers: vec![],
                    span: expr.span,
                };
                self.builder.pack(node);
                self.emit_signal_metadata(id, &[], &[cond])?;
                Ok(Child::Node(id))
            }
            flux_parser::ExprKind::ForEach {
                items: items_expr,
                key: key_expr,
                body,
            } => {
                let id = expr_node_id(expr, ExprNodeKind::ForEach);
                // Real ForEach lowering (FLUX-072 / ADR-0050): lower the loop
                // body into the child nodes that the host reconciles per item.
                // The `item` binding is a runtime-scoped variable supplied by the
                // host per iteration. We allocate a dedicated per-ForEach signal
                // slot for `item` and bind it in `signal_scope` while lowering the
                // body, so every row prop thunk reads `itemSlot` (instead of an
                // unresolved free variable, which previously lowered to `Null`).
                // The host, on list-signal change, clones the row template once
                // per element, seeding a fresh per-row signal with `list[i]` and
                // rewriting the row thunk's `READ_SIGNAL itemSlot` to that id.
                let item_slot = self.next_signal();
                // Bind the `ForEach` loop variable to the per-row `itemSlot` so
                // row-body field accesses like `t.label` resolve to the scoped
                // signal instead of the enclosing list signal (FLUX-072 /
                // ADR-0050). The loop variable is declared either as the `key`
                // lambda's parameter (`ForEach(x, key: |t| …) { … t … }`) or as
                // a leading `binding =>` in the body block; fall back to "item"
                // only when neither is present (older `|item|` sources).
                let loop_var = body
                    .params
                    .first()
                    .and_then(|p| match p {
                        flux_parser::Pattern::Ident(ident) => Some(ident.name.clone()),
                        _ => None,
                    })
                    .or_else(|| {
                        if let flux_parser::ExprKind::Lambda { params, .. } = &key_expr.kind {
                            params.first().map(|p| p.name.name.clone())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| "item".to_string());
                self.signal_scope.push((loop_var, item_slot));
                let row_children = self.lower_block(body, owner)?;
                self.signal_scope.pop();
                // `Child::Splice` carries the per-item child node ids as
                // `(Key, NodeId)` pairs (arena.rs encode/decode contract). The
                // key is the row's position in the source body; the host
                // re-binds each row to its list element at reconcile time
                // (FLUX-072 / ADR-0050).
                let items: Vec<(Key, NodeId)> = row_children
                    .into_iter()
                    .enumerate()
                    .flat_map(|(idx, child)| match child {
                        Child::Node(id) => vec![(Key::from(idx as u64), id)],
                        Child::Splice { items } => items,
                        #[allow(unreachable_patterns)]
                        _ => vec![],
                    })
                    .collect();
                let node = Node {
                    id,
                    kind: NodeKind::ForEach,
                    component_id: owner,
                    props: Props::default(),
                    children: vec![Child::Splice { items }],
                    handlers: vec![],
                    span: expr.span,
                };
                self.builder.pack(node);
                // `deps` carries the items-signal (list) so the host knows which
                // signal to watch; `layout` carries `itemSlot` so the host knows
                // which signal each row thunk reads for `item`.
                self.builder.signal_metadata(
                    id,
                    collect_read_signals(&[items_expr], &self.signal_scope),
                    None,
                    Vec::new(),
                    Some(item_slot),
                );
                Ok(Child::Node(id))
            }
            flux_parser::ExprKind::When {
                cond,
                then_block,
                otherwise,
            } => {
                // `when … { … } otherwise { … }` is the Flux conditional; the
                // codegen layer renders it as `if/else` (spec FR-011). Lowering
                // emits an `If` node whose children are the two branches'
                // UI producers.
                let id = expr_node_id(expr, ExprNodeKind::If);
                let mut children = Vec::with_capacity(2);
                children.extend(self.lower_block(then_block, owner)?);
                if let Some(other) = otherwise {
                    children.extend(self.lower_block(other, owner)?);
                }
                let node = Node {
                    id,
                    kind: NodeKind::If,
                    component_id: owner,
                    props: Props::default(),
                    children,
                    handlers: vec![],
                    span: expr.span,
                };
                self.builder.pack(node);
                self.emit_signal_metadata(id, &[], &[cond])?;
                Ok(Child::Node(id))
            }
            flux_parser::ExprKind::Match { scrutinee, arms } => {
                let id = expr_node_id(expr, ExprNodeKind::Match);
                let mut children = Vec::with_capacity(arms.len());
                for arm in arms {
                    children.push(self.lower_expr(&arm.body, owner)?);
                }
                let node = Node {
                    id,
                    kind: NodeKind::Match,
                    component_id: owner,
                    props: Props::default(),
                    children,
                    handlers: vec![],
                    span: expr.span,
                };
                self.builder.pack(node);
                self.emit_signal_metadata(id, &[], &[scrutinee])?;
                Ok(Child::Node(id))
            }
            other => Err(LoweringError::new(
                format!("unsupported expression in UI tree: {other:?}"),
                expr.span,
            )),
        }
    }

    /// Lowers a call `callee(args) { trailing }` into a single [`Child::Node`].
    #[allow(clippy::too_many_lines)]
    fn lower_call(
        &mut self,
        expr: &flux_parser::Expr,
        callee: &flux_parser::Expr,
        args: &[flux_parser::Arg],
        trailing: Option<&flux_parser::Block>,
        owner: ComponentId,
    ) -> Result<Child, LoweringError> {
        let name = match &callee.kind {
            flux_parser::ExprKind::Ident(ident) => ident.name.clone(),
            _ => {
                return Err(LoweringError::new(
                    "call callee must be an identifier".to_owned(),
                    callee.span,
                ));
            }
        };

        let id = expr_node_id(expr, ExprNodeKind::Primitive);
        // A call to a generic component resolves to its *specialised* name
        // (`Counter[Int]` → `Counter_Int`) so each instantiation gets its own
        // `ComponentId` and the release backends emit one native type per
        // instantiation (roadmap Phase 1). Non-generic calls keep their name.
        let interned = self.mono.next_specialised(&name).unwrap_or(name);
        let component_id = self.intern_component(&interned);

        // Inline a declared component's body at the call site when every prop
        // maps to an in-scope identifier (e.g. `TaskRow(task: item, …)` inside a
        // `ForEach`, where `item` resolves to the per-row `itemSlot`). This
        // binds the body's field accesses to the real argument signals instead
        // of an unseeded component-global prop signal — the "tasks not
        // rendered" bug (FLUX-072). Falls back to the normal component-instance
        // path when inlining isn't possible.
        let inline_children = if self.component_decls.contains_key(&interned) {
            let comp = self.component_decls.get(&interned).unwrap().clone();
            self.try_inline_component(&comp, args, trailing, owner)?
        } else {
            None
        };

        // Build props from positional + named args, plus any trailing block
        // prop entries. Positional args take the next sequential PropIdx;
        // named args map to a stable PropIdx derived from their name so the
        // prop layout is stable across edits.
        let mut fields: Vec<(flux_syntax::PropIdx, Value)> = Vec::new();
        // Original prop value expressions, retained for the ADR-0027 T14 prop
        // thunk (the compiled thunk reads the same expressions the props came
        // from — single source of truth with `signal_deps`).
        let mut prop_exprs: Vec<(flux_syntax::PropIdx, &flux_parser::Expr)> = Vec::new();
        let mut next_positional: u16 = 0;
        let mut handlers: Vec<flux_syntax::HandlerId> = Vec::new();

        for arg in args {
            let (idx, value) = match arg {
                flux_parser::Arg::Positional(e) => {
                    let idx = flux_syntax::PropIdx::from(next_positional);
                    next_positional = next_positional.saturating_add(1);
                    (idx, self.lower_value(e, owner, &mut handlers)?)
                }
                flux_parser::Arg::Named {
                    name: arg_name,
                    value,
                } => {
                    let idx = prop_index_for_name(&arg_name.name);
                    (idx, self.lower_value(value, owner, &mut handlers)?)
                }
                #[allow(unreachable_patterns)]
                _ => {
                    return Err(LoweringError::new(
                        "unsupported argument form".to_owned(),
                        expr.span,
                    ));
                }
            };
            prop_exprs.push((idx, arg.value()));
            fields.push((idx, value));
        }

        if let Some(block) = trailing {
            for item in &block.items {
                if let flux_parser::BlockItem::Prop { name, value } = item {
                    let idx = prop_index_for_name(&name.name);
                    let v = self.lower_value(value, owner, &mut handlers)?;
                    prop_exprs.push((idx, value));
                    fields.push((idx, v));
                }
            }
        }

        let props = Props::from_fields(fields);
        let children = match &inline_children {
            Some(c) => c.clone(),
            None => {
                if let Some(block) = trailing {
                    self.lower_block(block, owner)?
                } else {
                    vec![]
                }
            }
        };

        let node = Node {
            id,
            kind: NodeKind::Primitive,
            component_id,
            props,
            children,
            handlers,
            span: expr.span,
        };
        self.builder.pack(node);
        self.emit_signal_metadata(id, &prop_exprs, &[])?;
        Ok(Child::Node(id))
    }

    /// Attempts to inline a component call: lowers the component body with its
    /// `prop`s bound to the call-site argument signals. Returns `Some(children)`
    /// when every prop maps to an in-scope identifier (the common case, e.g.
    /// `TaskRow(task: item, …)` inside a `ForEach` where `item` resolves to the
    /// per-row `itemSlot`); `None` otherwise, in which case the caller falls
    /// back to the regular component-instance path.
    ///
    /// Inlining is what makes a component used inside a `ForEach` read the
    /// row's own values instead of an unseeded component-global prop signal
    /// (the "tasks not rendered" bug, FLUX-072).
    fn try_inline_component(
        &mut self,
        comp: &ComponentDecl,
        args: &[flux_parser::Arg],
        trailing: Option<&flux_parser::Block>,
        owner: ComponentId,
    ) -> Result<Option<Vec<Child>>, LoweringError> {
        // Map prop name → argument expression.
        let mut arg_by_prop: std::collections::HashMap<String, &flux_parser::Expr> =
            std::collections::HashMap::new();
        for arg in args {
            match arg {
                flux_parser::Arg::Named { name, value } => {
                    arg_by_prop.insert(name.name.clone(), value);
                }
                flux_parser::Arg::Positional(_) => return Ok(None),
                _ => return Ok(None),
            }
        }
        if let Some(block) = trailing {
            for item in &block.items {
                if let flux_parser::BlockItem::Prop { name, value } = item {
                    arg_by_prop.insert(name.name.clone(), value);
                }
            }
        }
        // Every prop must map to an in-scope identifier we can resolve to a
        // signal; otherwise inlining isn't safe and we fall back.
        let mut bindings: Vec<(String, flux_syntax::SignalId)> =
            Vec::with_capacity(comp.props.len());
        for prop in &comp.props {
            let Some(expr) = arg_by_prop.get(&prop.name.name) else {
                return Ok(None);
            };
            let flux_parser::ExprKind::Ident(ident) = &expr.kind else {
                return Ok(None);
            };
            let Some(sig) = self
                .signal_scope
                .iter()
                .find(|(n, _)| n == &ident.name)
                .map(|(_, s)| *s)
            else {
                return Ok(None);
            };
            bindings.push((prop.name.name.clone(), sig));
        }
        // Lower the body with the component's props bound to the argument
        // signals (e.g. `task` → `itemSlot`), so field accesses inside the
        // body read the per-row values directly.
        for (name, sig) in &bindings {
            self.signal_scope.push((name.clone(), *sig));
        }
        let children = self.lower_block(&comp.body, owner)?;
        for _ in &bindings {
            self.signal_scope.pop();
        }
        Ok(Some(children))
    }

    /// Lowers an argument value; handler lambdas become [`Value::HandlerRef`]
    /// with a compiled [`ClosureIR`] registered in the arena. Literal values
    /// are stored directly; dynamic expressions (identifiers, calls, records)
    /// are stored as `Null` placeholders because their runtime evaluation is
    /// driven by the codegen layer from the AST (spec FR-011 renders props from
    /// source, not from the lowered literal).
    fn lower_value(
        &mut self,
        expr: &flux_parser::Expr,
        owner: ComponentId,
        handlers: &mut Vec<flux_syntax::HandlerId>,
    ) -> Result<Value, LoweringError> {
        match &expr.kind {
            flux_parser::ExprKind::Int(i) => Ok(Value::Int(*i)),
            flux_parser::ExprKind::Float(f) => Ok(Value::Float(*f)),
            flux_parser::ExprKind::Bool(b) => Ok(Value::Bool(*b)),
            flux_parser::ExprKind::Str(parts) => self.lower_str(parts, owner, handlers),
            flux_parser::ExprKind::Null => Ok(Value::Null),
            flux_parser::ExprKind::Lambda { params, body } => {
                let handler = self.next_handler();
                let mut intern = |s: &str| self.builder.intern_string(s);
                // A `Lambda`'s params are `Param`s; the handler compiler binds
                // `Pattern`s, so project to identifiers (MLP handlers take at
                // most one payload param).
                let pattern_params: Vec<flux_parser::Pattern> = params
                    .iter()
                    .map(|p| flux_parser::Pattern::Ident(p.name.clone()))
                    .collect();
                let (bytecode, captured) = compile_handler_with_params(
                    body,
                    &pattern_params,
                    &self.signal_scope,
                    &self.record_ctors,
                    &self.typed.field_indices,
                    expr.span,
                    &mut intern,
                )?;
                let closure = ClosureIR::new(handler, bytecode, captured, expr.span);
                self.closures.insert(handler, closure);
                handlers.push(handler);
                Ok(Value::HandlerRef(handler))
            }
            flux_parser::ExprKind::List(items) => {
                // A list literal seeds a real `Value::List` so `state`/`derived`
                // signals holding collections (e.g. `state tasks: List[Task] = […]`)
                // materialise as a list, not `null`. Lowering each element
                // recursively handles nested records/strings (FLUX-072 #1).
                let mut lowered: Vec<Value> = Vec::with_capacity(items.len());
                for item in items {
                    lowered.push(self.lower_value(item, owner, handlers)?);
                }
                Ok(Value::List(lowered))
            }
            flux_parser::ExprKind::Record { name: _, fields } => {
                // A record literal (`Task(label: "x", done: false)`) lowers to a
                // `Value::Record` keyed by stable field `PropIdx`s (Appendix C).
                // The record type name is unused on the wire — fields are
                // position-independent, matched by index — so it is dropped here.
                let mut lowered: Vec<(flux_syntax::PropIdx, Value)> =
                    Vec::with_capacity(fields.len());
                for (fname, fexpr) in fields {
                    let idx = prop_index_for_name(&fname.name);
                    lowered.push((idx, self.lower_value(fexpr, owner, handlers)?));
                }
                Ok(Value::Record(lowered))
            }
            flux_parser::ExprKind::Call {
                callee,
                args,
                trailing: _,
            } => {
                // A record-constructor call (`Task(label: "x", done: false)`) is a
                // value literal, not a component call: when its callee names a
                // record type it lowers to a `Value::Record` so `state`/`derived`
                // signals holding records (and lists of records) materialise as
                // real values instead of `null` (FLUX-072 #1). Component calls are
                // UI nodes and are never lowered as `state_seed` literals, so the
                // `record_ctors` guard keeps them out of this path.
                if let flux_parser::ExprKind::Ident(ident) = &callee.kind {
                    if self.record_ctors.contains(&ident.name) {
                        let mut lowered: Vec<(flux_syntax::PropIdx, Value)> =
                            Vec::with_capacity(args.len());
                        let mut next_positional: u16 = 0;
                        for arg in args {
                            let (idx, value) = match arg {
                                flux_parser::Arg::Named { name, value } => (
                                    prop_index_for_name(&name.name),
                                    self.lower_value(value, owner, handlers)?,
                                ),
                                flux_parser::Arg::Positional(e) => {
                                    let idx = flux_syntax::PropIdx::from(next_positional);
                                    next_positional = next_positional.saturating_add(1);
                                    (idx, self.lower_value(e, owner, handlers)?)
                                }
                                #[allow(unreachable_patterns)]
                                _ => continue,
                            };
                            lowered.push((idx, value));
                        }
                        return Ok(Value::Record(lowered));
                    }
                }
                // Non-record calls (component calls, capability invokes, …) cannot
                // be serialised as a static literal for the MLP wire path.
                Ok(Value::Null)
            }
            // Any other `ExprKind` (incl. future non-exhaustive variants) cannot be
            // serialised as a static literal for the MLP wire path. The codegen
            // layer recovers the real expression from the AST, so a `Null`
            // placeholder here keeps lowering total without inventing a value.
            _ => Ok(Value::Null),
        }
    }

    /// Lowers a string literal, interning its text (after simple interpolation
    /// concatenation — interpolations are resolved at runtime, so we structure
    /// the literal text only).
    fn lower_str(
        &mut self,
        parts: &[flux_parser::StrPart],
        _owner: ComponentId,
        _handlers: &mut Vec<flux_syntax::HandlerId>,
    ) -> Result<Value, LoweringError> {
        let mut text = String::new();
        for part in parts {
            match part {
                flux_parser::StrPart::Text(t) => text.push_str(t),
                // Interpolations are rendered as their source span placeholder
                // for the MLP; the runtime re-evaluates them. We keep the text
                // shape faithful by leaving the expression untouched.
                flux_parser::StrPart::Interp(_) => text.push_str("{…}"),
                #[allow(unreachable_patterns)]
                _ => text.push_str("{…}"),
            }
        }
        Ok(self.intern_str(&text))
    }
}

/// Maps a prop name to a stable [`flux_syntax::PropIdx`] so the wire layout is identical
/// across edits (deterministic, not a hash of editable text that could shift).
#[must_use]
pub fn prop_index_for_name(name: &str) -> flux_syntax::PropIdx {
    // Stable, content-independent ordering: assign indices by a fixed digest of
    // the ASCII bytes via FNV-1a, kept in `u16` range. Two props with distinct
    // names get distinct indices; the same name always maps to the same index.
    let mut hash: u32 = 0x811c_9dc5;
    for &byte in name.as_bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    flux_syntax::PropIdx::from((hash & 0xFFFF) as u16)
}
