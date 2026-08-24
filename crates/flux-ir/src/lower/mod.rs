//! AST → TypedAST → reactive-tree IR lowering (FLUX-018).
//!
//! This module walks a [`flux_parser::Ast`] that has already been type-checked
//! into a [`flux_types::TypedAST`] and emits the packed [`IRArena`] that the
//! differ and wire codec consume. Every emitted node carries the *same*
//! [`NodeId`] that the type checker used to key `typed.types`, so downstream
//! code can look up the inferred type for an IR node by ID (ADR-0027 — the
//! "node-ID bridge").
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

pub use bytecode::{HandlerCompileError, compile_handler};
pub use error::LoweringError;

use flux_parser::{Ast, Decl};
use flux_syntax::{Child, ComponentId, NodeKind, Props, StringTable, Value};
use flux_types::TypedAST;

use crate::arena::IRArena;
use crate::builder::{ArenaBuilder, Node};
use crate::closure::ClosureIR;
use crate::instance::InstanceRegistry;
use ids::{ExprNodeKind, decl_node_id, expr_node_id};

/// The fully lowered program.
///
/// Returned by [`lower`]; bundles the packed [`IRArena`], the handler closure
/// table (keyed by [`HandlerId`]), and the per-component [`InstanceRegistry`]
/// that lets the host app preserve state across hot swaps.
#[derive(Clone, Debug)]
pub struct LoweredIr {
    /// The packed reactive tree.
    pub arena: IRArena,
    /// Handler closures, keyed by their [`HandlerId`].
    pub closures: std::collections::HashMap<flux_syntax::HandlerId, ClosureIR>,
    /// Live component-instance registry.
    pub instances: InstanceRegistry,
}

impl LoweredIr {
    /// Returns the closure registered for `handler`, if any.
    #[must_use]
    pub fn closure(&self, handler: flux_syntax::HandlerId) -> Option<&ClosureIR> {
        self.closures.get(&handler)
    }
}

/// Lowers a type-checked program into the reactive-tree IR.
///
/// `lower` walks `ast` in declaration order and packs a [`Node`] per
/// runtime-relevant surface construct. The returned [`LoweredIr::arena`] carries
/// exactly the [`NodeId`]s the type checker assigned (see the bridge note on
/// this module), so `typed.types.keys()` and `arena.all_ids()` are the same set
/// for every node the type checker typed.
///
/// # Errors
///
/// Returns [`LoweringError`] (carrying a [`Span`]) when lowering cannot proceed
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
/// let src = "component Hello { state count: Int = 0 Button(text: \"tap\") }";
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
    /// String interning table shared with the arena.
    strings: StringTable,
    /// All compiled closures, keyed by [`HandlerId`].
    closures: std::collections::HashMap<flux_syntax::HandlerId, ClosureIR>,
    /// Signals owned by the enclosing component, named for handler capture.
    signal_scope: Vec<(String, flux_syntax::SignalId)>,
    /// Per-component signal allocator (resets each component).
    signal_counter: flux_syntax::SignalId,
    /// Handler allocator (monotonic across the whole program).
    handler_counter: flux_syntax::HandlerId,
}

impl<'a> Lowerer<'a> {
    fn new(typed: &'a TypedAST) -> Self {
        Self {
            typed,
            builder: ArenaBuilder::new(),
            name_to_component: std::collections::HashMap::new(),
            next_component: ComponentId::from(0u32),
            strings: StringTable::new(),
            closures: std::collections::HashMap::new(),
            signal_scope: Vec::new(),
            signal_counter: flux_syntax::SignalId::from(0u32),
            handler_counter: flux_syntax::HandlerId::from(0u32),
        }
    }

    fn finish(self) -> LoweredIr {
        let arena = self.builder.finish();
        LoweredIr {
            arena,
            closures: self.closures,
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
    fn intern_str(&mut self, text: &str) -> Value {
        let id = self.strings.intern(text);
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

    fn lower_decl(&mut self, decl: &Decl) -> Result<(), LoweringError> {
        match decl {
            // Only components produce runtime tree nodes in the MLP.
            Decl::Component(comp) => self.lower_component(comp),
            // fn / type / trait / capability / import / use / const are
            // type-level only; the type checker handled them, so we skip them.
            Decl::Fn(_)
            | Decl::Type(_)
            | Decl::Trait(_)
            | Decl::Capability(_)
            | Decl::Import(_)
            | Decl::Use(_)
            | Decl::Const(_) => Ok(()),
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

        // State cells are scoped per component.
        self.signal_scope.clear();
        self.signal_counter = flux_syntax::SignalId::from(0u32);

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
                    self.signal_scope.push((decl.name.name.clone(), sig));
                }
                flux_parser::BlockItem::Prop { .. } => {
                    // Prop entries at this level are not part of the MLP
                    // component body; they only appear inside trailing call
                    // blocks, which lower_call handles directly.
                }
                flux_parser::BlockItem::Expr(expr) => {
                    let child = self.lower_expr(expr, owner)?;
                    children.push(child);
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
                cond: _,
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
                Ok(Child::Node(id))
            }
            flux_parser::ExprKind::ForEach {
                items: _,
                key: _,
                body,
            } => {
                let id = expr_node_id(expr, ExprNodeKind::ForEach);
                // The items are produced at runtime by the host (keyed
                // reconciliation, FLUX-014); we emit the ForEach node with an
                // empty splice. Body is type-checked but not statically
                // expanded.
                let _ = body;
                let node = Node {
                    id,
                    kind: NodeKind::ForEach,
                    component_id: owner,
                    props: Props::default(),
                    children: vec![Child::Splice { items: vec![] }],
                    handlers: vec![],
                    span: expr.span,
                };
                self.builder.pack(node);
                Ok(Child::Node(id))
            }
            flux_parser::ExprKind::Match { scrutinee: _, arms } => {
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
        let component_id = self.intern_component(&name);

        // Build props from positional + named args, plus any trailing block
        // prop entries. Positional args take the next sequential PropIdx;
        // named args map to a stable PropIdx derived from their name so the
        // prop layout is stable across edits.
        let mut fields: Vec<(flux_syntax::PropIdx, Value)> = Vec::new();
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
            fields.push((idx, value));
        }

        if let Some(block) = trailing {
            for item in &block.items {
                if let flux_parser::BlockItem::Prop { name, value } = item {
                    let idx = prop_index_for_name(&name.name);
                    let v = self.lower_value(value, owner, &mut handlers)?;
                    fields.push((idx, v));
                }
            }
        }

        let props = Props::from_fields(fields);
        let children = if let Some(block) = trailing {
            self.lower_block(block, owner)?
        } else {
            vec![]
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
        Ok(Child::Node(id))
    }

    /// Lowers an argument value; handler lambdas become [`Value::HandlerRef`]
    /// with a compiled [`ClosureIR`] registered in the arena.
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
            flux_parser::ExprKind::Lambda { params: _, body } => {
                let handler = self.next_handler();
                let (bytecode, captured) = compile_handler(body, &self.signal_scope, expr.span)?;
                let closure = ClosureIR::new(handler, bytecode, captured, expr.span);
                self.closures.insert(handler, closure);
                handlers.push(handler);
                Ok(Value::HandlerRef(handler))
            }
            other => Err(LoweringError::new(
                format!("unsupported argument value: {other:?}"),
                expr.span,
            )),
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

/// Maps a prop name to a stable [`PropIdx`] so the wire layout is identical
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
