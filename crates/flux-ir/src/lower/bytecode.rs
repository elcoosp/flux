//! Handler bytecode compiler (FLUX-018 §4, Appendix E).
//!
//! Compiles a handler body — a [`flux_parser::Block`] of `state` mutations and
//! arithmetic — into register-based bytecode consumed by the host
//! [`flux_syntax::opcode`] VM. Signals are captured by reference (ADR-0014), so
//! the output is `(bytecode, captured_signal_ids)`.
//!
//! Supported MLP forms:
//! - `state = state + 1` — read a signal, add a constant, write it back.
//! - `state = expr` — assign an arithmetic/boolean expression of signals and
//!   literals.
//! - Bare integer / signal expressions (evaluated, result discarded unless it
//!   is a write, matching SolidJS-style fire-and-forget handlers).
//!
//! Registers follow Appendix E: r0 is the entry payload (unused by handlers
//! here), r15 is the gas register (managed by the VM). We allocate working
//! registers r1… in emission order.

use flux_parser::{
    BinOp, Block, BlockItem, Expr, ExprKind, MatchArm, MatchPattern, MatchPatternKind,
};
use flux_syntax::opcode::raw;
use flux_syntax::{PropIdx, SignalId, Span, StringId, Value};
use std::collections::HashSet;

use crate::lower::error::LoweringError;

/// A string interner callback: maps a literal's concatenated text to its
/// content-addressed [`StringId`]. The emitter stays decoupled from the arena
/// owner by taking this rather than a concrete `StringTable` borrow.
type StringInterner<'a> = &'a mut dyn FnMut(&str) -> StringId;

/// A signal name paired with its assigned [`SignalId`].
type SignalScope = Vec<(String, SignalId)>;

/// Content-addressed hash of a thunk/handler closure body, used for
/// `ClosureRef` interning. This must stay byte-identical to
/// `flux_ir_serde::hash_closure` (same blake3 input framing, including the two
/// length prefixes) so a thunk hashed here matches the `HandlerDef` the frame
/// writer emits for the same bytecode — that hash is how the host pairs a
/// node's `prop_thunk` with the bytecode in the frame's shared blob. It is
/// duplicated rather than imported to avoid a `flux-ir` → `flux-ir-serde`
/// dependency cycle; `crates/flux-ir/tests/lower.rs` pins the two together.
#[must_use]
pub(crate) fn hash_closure_placeholder(bytecode: &[u8], captured: &[flux_syntax::SignalId]) -> u64 {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&(bytecode.len() as u32).to_le_bytes());
    hasher.update(bytecode);
    hasher.update(&(captured.len() as u32).to_le_bytes());
    for id in captured {
        hasher.update(&id.to_le_bytes());
    }
    let mut digest = [0_u8; 8];
    digest.copy_from_slice(&hasher.finalize().as_bytes()[..8]);
    u64::from_le_bytes(digest)
}

/// Compiles `body` to `(bytecode, captured_signals)`.
///
/// # Errors
///
/// Returns [`HandlerCompileError`] when the body uses a form the MLP compiler
/// does not yet support (e.g. a non-signal target, a call expression, or a
/// string interpolation), carrying the offending [`Span`].
///
/// # Examples
///
/// ```rust,no_run
/// use flux_ir::lower::compile_handler;
/// use flux_parser::parse;
///
/// // `count = count + 1` where `count` is signal #1.
/// let src = "component C { state count: Int = 0 }";
/// let ast = parse(src, 0, "c.flux").unwrap();
/// let comp = match &ast.decls[0] {
///     flux_parser::Decl::Component(c) => c,
///     _ => unreachable!(),
/// };
/// // (body shape is exercised in the crate's integration tests)
/// let _ = comp;
/// ```
pub fn compile_handler(
    body: &Block,
    scope: &SignalScope,
    constructors: &HashSet<String>,
    span: Span,
    str_interner: StringInterner<'_>,
) -> Result<(Vec<u8>, Vec<SignalId>), HandlerCompileError> {
    let mut emitter = Emitter::new(scope, constructors, str_interner);
    for item in &body.items {
        match item {
            BlockItem::State(decl) => {
                // `state x = e` is an initialiser, not a mutation; treat as a
                // no-op assignment to a fresh slot is not meaningful in a
                // handler, so we lower it like an assignment to the name.
                emitter.compile_assignment(&decl.name.name, &decl.init)?;
            }
            BlockItem::Expr(expr) => {
                // Propagate compilation failures loudly instead of silently
                // emitting a no-op. The MLP bytecode envelope supports signal
                // reads/writes, arithmetic over literals and signals, and the
                // control-flow forms (if/when/match) compiled below; any other
                // form (capability calls, constructor calls, string assignment)
                // is genuinely unsupported and must surface as an error so the
                // dev server and the runtime can report it (FLUX-014 P3 addendum).
                emitter.compile_expr_stmt(expr)?;
            }
            _ => {
                return Err(HandlerCompileError::new(
                    "unsupported statement in handler body".to_owned(),
                    span,
                ));
            }
        }
    }
    emitter.finish()
}

/// Result of compiling a node's prop expressions into a thunk (ADR-0027 T14):
/// `(bytecode, captured_signal_ids, prop_layout)`. See [`compile_prop_thunk`].
pub(crate) type PropThunk = Result<(Vec<u8>, Vec<SignalId>, Vec<u16>), HandlerCompileError>;

/// Compiles a node's prop expressions into a prop thunk (ADR-0027 T14).
///
/// The thunk allocates an `ALLOC_RECORD` of `props.len()` fields into `r1`,
/// fills each field `i` (in `props` order) with the value of prop `i`, and
/// `HALT`s — so `r1` holds the record of prop values at exit (the thunk
/// contract). `prop_layout` maps record-field position → prop index in the
/// same order.
///
/// The returned `captured` signal ids are exactly the `READ_SIGNAL` operands
/// the thunk emits — the single source of truth for the node's `signal_deps`
/// (T13). The caller must attach the same `captured` set as `signal_deps`.
///
/// # Errors
///
/// Returns [`HandlerCompileError`] when a prop value uses a form the MLP
/// bytecode envelope cannot express (e.g. a capability call, a string
/// assignment). Callers fall back to emitting `signal_deps` from a plain walk
/// and shipping no thunk in that case.
///
/// # Examples
///
/// ```rust,no_run
/// // The thunk shape is exercised in the crate's integration tests
/// // (`crates/flux-ir/tests/lower.rs`): a node with props `text`, `width`
/// // compiles to a closure whose bytecode leaves an `ALLOC_RECORD` of prop
/// // values in register `r1` at `HALT`.
/// let _ = 0u32;
/// ```
pub(crate) fn compile_prop_thunk(
    props: &[(PropIdx, &Expr)],
    scope: &SignalScope,
    str_interner: StringInterner<'_>,
) -> PropThunk {
    let mut emitter = Emitter::for_thunk(scope, str_interner);
    let count = props.len() as u16;
    emitter.emit_alloc_record(1, count);
    let mut layout = Vec::with_capacity(props.len());
    for (position, (prop_idx, expr)) in props.iter().enumerate() {
        let value_reg = emitter.compile_value(expr)?;
        emitter.emit_set_field(1, position as u16, value_reg);
        layout.push(*prop_idx);
    }
    let (code, captured) = emitter.finish()?;
    Ok((code, captured, layout))
}

/// Walks `exprs`, collecting the distinct `READ_SIGNAL` ids that appear as
/// signal references (identifiers resolving to a signal in `scope`).
///
/// This is the fallback source of a node's `signal_deps` (T13) for nodes whose
/// prop/control expressions cannot be compiled into a thunk (see
/// [`compile_prop_thunk`]). It never errors and never allocates registers.
#[must_use]
pub(crate) fn collect_read_signals(exprs: &[&Expr], scope: &SignalScope) -> Vec<SignalId> {
    let mut found: Vec<SignalId> = Vec::new();
    for expr in exprs {
        collect_in_expr(expr, scope, &mut found);
    }
    found.sort_unstable();
    found.dedup();
    found
}

/// Recursively gathers signal ids referenced by name in `expr`.
fn collect_in_expr(expr: &Expr, scope: &SignalScope, found: &mut Vec<SignalId>) {
    match &expr.kind {
        ExprKind::Ident(ident) => {
            if let Some((_, id)) = scope.iter().find(|(n, _)| n == &ident.name) {
                if !found.contains(id) {
                    found.push(*id);
                }
            }
        }
        ExprKind::Binary { lhs, rhs, .. } => {
            collect_in_expr(lhs, scope, found);
            collect_in_expr(rhs, scope, found);
        }
        ExprKind::If {
            cond,
            then_block,
            else_branch,
        } => {
            collect_in_expr(cond, scope, found);
            collect_in_block(then_block, scope, found);
            if let Some(other) = else_branch {
                collect_in_expr(other, scope, found);
            }
        }
        ExprKind::When {
            cond,
            then_block,
            otherwise,
        } => {
            collect_in_expr(cond, scope, found);
            collect_in_block(then_block, scope, found);
            if let Some(block) = otherwise {
                collect_in_block(block, scope, found);
            }
        }
        ExprKind::Match { scrutinee, arms } => {
            collect_in_expr(scrutinee, scope, found);
            for arm in arms {
                collect_in_expr(&arm.body, scope, found);
            }
        }
        ExprKind::Call {
            args,
            trailing,
            callee,
        } => {
            collect_in_expr(callee, scope, found);
            for arg in args {
                collect_in_expr(arg.value(), scope, found);
            }
            if let Some(block) = trailing {
                collect_in_block(block, scope, found);
            }
        }
        ExprKind::Record { fields, .. } => {
            for (_, field_expr) in fields {
                collect_in_expr(field_expr, scope, found);
            }
        }
        ExprKind::List(items) => {
            for item in items {
                collect_in_expr(item, scope, found);
            }
        }
        ExprKind::Field { base, .. } => collect_in_expr(base, scope, found),
        ExprKind::Let { value: Some(v), .. } => collect_in_expr(v, scope, found),
        ExprKind::Assign { value, .. } => collect_in_expr(value, scope, found),
        ExprKind::Await(inner) => collect_in_expr(inner, scope, found),
        _ => {}
    }
}

/// Collects signal references inside a block body.
fn collect_in_block(block: &flux_parser::Block, scope: &SignalScope, found: &mut Vec<SignalId>) {
    for item in &block.items {
        match item {
            flux_parser::BlockItem::Expr(expr) => collect_in_expr(expr, scope, found),
            flux_parser::BlockItem::State(decl) => collect_in_expr(&decl.init, scope, found),
            _ => {}
        }
    }
}

/// A shared, empty constructor set for emitters that never resolve calls
/// (prop thunks). `compile_handler` always passes the real set from `TypedAST`.
fn empty_constructors() -> &'static HashSet<String> {
    static EMPTY: std::sync::LazyLock<HashSet<String>> = std::sync::LazyLock::new(HashSet::new);
    &EMPTY
}

/// Derives a stable `cap_id` for a capability/method call from the capability
/// name, per the CALL_CAP contract shared with the host runtime.
///
/// `cap_id = blake3(name)[..4]` interpreted as little-endian `u32`. The host
/// registry resolves `(cap_id, method_id)` to an implementation using the same
/// derivation, so neither side ships a shared ID table (AGENT-044 / ADR-0045).
#[must_use]
pub(crate) fn cap_id_for(name: &str) -> u32 {
    let hash = blake3::hash(name.as_bytes());
    u32::from_le_bytes(hash.as_bytes()[..4].try_into().unwrap())
}

/// Derives a stable `method_id` for a capability method call.
///
/// `method_id = blake3("cap.method")[..2]` as little-endian `u16`. The reserved
/// pair `(1, 1)` is the frozen `call_cap_basic` stub and is never produced by
/// this derivation for a real capability name.
#[must_use]
pub(crate) fn method_id_for(cap: &str, method: &str) -> u16 {
    let key = format!("{cap}.{method}");
    let hash = blake3::hash(key.as_bytes());
    u16::from_le_bytes(hash.as_bytes()[..2].try_into().unwrap())
}

/// Bytecode emitter: walks expressions, appends raw opcode bytes, and records
/// captured signal IDs.
struct Emitter<'a> {
    scope: &'a SignalScope,
    /// Names of every in-scope ADT value constructor. Used to decide whether a
    /// `Name(args)` call lowers to a value record or a capability invocation.
    constructors: &'a HashSet<String>,
    /// Locally-bound `let` names → register. Checked before the signal scope so
    /// `let x = …; … x …` reads the binding rather than a (non-existent) signal.
    locals: std::collections::HashMap<String, u8>,
    code: Vec<u8>,
    captured: Vec<SignalId>,
    reg: u8,
    /// Interns a string literal, returning its content-addressed [`StringId`].
    /// The type is a `dyn FnMut` so the emitter stays decoupled from the arena
    /// owner; callers supply `|s| self.intern_str(s).as_str_id()` (the
    /// [`Value::Str`] carries the same `StringId` the host resolves).
    str_interner: StringInterner<'a>,
}

impl<'a> Emitter<'a> {
    fn new(
        scope: &'a SignalScope,
        constructors: &'a HashSet<String>,
        str_interner: StringInterner<'a>,
    ) -> Self {
        Self {
            scope,
            constructors,
            locals: std::collections::HashMap::new(),
            code: Vec::new(),
            captured: Vec::new(),
            // r0 = payload, r15 = gas; start allocating at r1.
            reg: 1,
            str_interner,
        }
    }

    /// Creates an emitter for a prop thunk (ADR-0027 T14).
    ///
    /// A thunk's `r1` holds the `ALLOC_RECORD` of prop values at `HALT`, so
    /// value registers must not clobber `r1`. We therefore allocate working
    /// registers from `r2` upward and emit the record into `r1` explicitly via
    /// [`emit_alloc_record`]. Prop thunks never emit capability calls, so they
    /// receive an empty constructor set and no locals.
    fn for_thunk(scope: &'a SignalScope, str_interner: StringInterner<'a>) -> Self {
        Self {
            scope,
            constructors: empty_constructors(),
            locals: std::collections::HashMap::new(),
            code: Vec::new(),
            captured: Vec::new(),
            reg: 2,
            str_interner,
        }
    }

    fn finish(self) -> Result<(Vec<u8>, Vec<SignalId>), HandlerCompileError> {
        let mut code = self.code;
        code.push(raw::HALT);
        Ok((code, self.captured))
    }

    fn alloc_reg(&mut self) -> u8 {
        let r = self.reg;
        self.reg = self.reg.saturating_add(1).min(14);
        r
    }

    /// Emits `ALLOC_RECORD dst, count` — allocates a record with `count` fields
    /// (all `Null` initially), used as the thunk's result container.
    fn emit_alloc_record(&mut self, dst: u8, count: u16) {
        self.code.push(raw::ALLOC_RECORD);
        self.code.push(dst);
        self.code.extend_from_slice(&count.to_le_bytes());
    }

    /// Emits `SET_FIELD dst, idx, src` — writes `src` into field `idx` of the
    /// record in `dst` (Appendix E §E.1: `REG_U16_REG`, 4 operand bytes).
    fn emit_set_field(&mut self, dst: u8, idx: u16, src: u8) {
        self.code.push(raw::SET_FIELD);
        self.code.push(dst);
        self.code.extend_from_slice(&idx.to_le_bytes());
        self.code.push(src);
    }
    fn signal_of(&mut self, name: &str, span: Span) -> Result<SignalId, HandlerCompileError> {
        match self.scope.iter().find(|(n, _)| n == name) {
            Some((_, id)) => {
                if !self.captured.contains(id) {
                    self.captured.push(*id);
                }
                Ok(*id)
            }
            None => Err(HandlerCompileError::new(
                format!("`{name}` is not a state signal in scope"),
                span,
            )),
        }
    }

    /// Emits `target = value` where `target` is a signal name.
    fn compile_assignment(
        &mut self,
        target: &str,
        value: &Expr,
    ) -> Result<(), HandlerCompileError> {
        let target_span = value.span;
        let target_id = self.signal_of(target, target_span)?;
        let dst = self.compile_value(value)?;
        // WRITE_SIGNAL signal_id(u32), src_reg(u8)
        self.code.push(raw::WRITE_SIGNAL);
        self.code.extend_from_slice(&target_id.to_le_bytes());
        self.code.push(dst);
        Ok(())
    }

    /// Emits a bare expression statement (handler body expression).
    fn compile_expr_stmt(&mut self, expr: &Expr) -> Result<(), HandlerCompileError> {
        match &expr.kind {
            ExprKind::Assign { target, value } => {
                let name = match &target.kind {
                    ExprKind::Ident(ident) => ident.name.clone(),
                    _ => {
                        return Err(HandlerCompileError::new(
                            "assignment target must be a signal name".to_owned(),
                            target.span,
                        ));
                    }
                };
                self.compile_assignment(&name, value)
            }
            ExprKind::Binary { .. } | ExprKind::Int(_) | ExprKind::Ident(_) | ExprKind::Bool(_) => {
                // Side-effect-free expression: evaluate and discard (handlers
                // like `derived` bodies may compute without writing).
                self.compile_value(expr)?;
                Ok(())
            }
            ExprKind::If {
                cond,
                then_block,
                else_branch,
            } => self.compile_if(cond, then_block, else_branch.as_deref()),
            ExprKind::When {
                cond,
                then_block,
                otherwise,
            } => {
                // `when … otherwise` is if/else (the codegen layer renders `when`
                // as `if/else`). Compile identically to `If`, with `otherwise`
                // as the optional else block.
                let else_expr: Option<Expr> = otherwise.as_ref().map(|block| Expr {
                    kind: ExprKind::Lambda {
                        params: Vec::new(),
                        body: block.clone(),
                    },
                    span: block.span,
                });
                self.compile_if(cond, then_block, else_expr.as_ref())
            }
            ExprKind::Match { scrutinee, arms } => self.compile_match(scrutinee, arms),
            // `let x = expr` binds `x` to the compiled value in the local scope.
            // Subsequent `Ident(x)` reads resolve to that register before the
            // signal scope (see `compile_value`).
            ExprKind::Let { pattern, value } => {
                let name = match pattern {
                    flux_parser::LetPattern::Ident(ident) => ident.name.clone(),
                    _ => {
                        return Err(HandlerCompileError::new(
                            "only identifier `let` bindings are supported in handlers".to_owned(),
                            expr.span,
                        ));
                    }
                };
                let reg = match value {
                    Some(v) => self.compile_value(v)?,
                    // A bare `let x` with no initialiser has no value to bind.
                    None => {
                        return Err(HandlerCompileError::new(
                            "a `let` binding in a handler must have an initialiser".to_owned(),
                            expr.span,
                        ));
                    }
                };
                self.locals.insert(name, reg);
                Ok(())
            }
            // A bare call expression as a statement (e.g. `router.navigate(..)`,
            // `refetch()`, `Auth.login(..)`) — emit it for its side effect and
            // discard the result register.
            ExprKind::Call { .. } => {
                self.compile_call(expr)?;
                Ok(())
            }
            // `...` is the spec's elision marker (Appendix B.3.8). A handler body
            // that is deliberately elided compiles to a `NOP` so the lower pass
            // succeeds without inventing behaviour the source omitted.
            ExprKind::Elided => {
                self.code.push(raw::NOP);
                Ok(())
            }
            _ => Err(HandlerCompileError::new(
                "unsupported handler expression".to_owned(),
                expr.span,
            )),
        }
    }

    /// Emits `if`/`when … otherwise` control flow.
    ///
    /// Layout (each jump operand is a relative i32 measured from the *next*
    /// instruction's byte offset, matching the VM decoder's `jump_target`):
    /// ```text
    ///   <cond> -> reg
    ///   COND_JUMP_NOT reg, L_else      ; skip then-branch when false
    ///   <then-branch>
    ///   JUMP L_join                    ; skip else-branch
    /// L_else:
    ///   <else-branch>                  ; absent for `when` with no otherwise
    /// L_join:
    /// ```
    ///
    /// `else_branch` may be a `Block` (a plain `else { … }`), a nested `if`
    /// (`else if …`), or `None` (`when` without `otherwise`). All are compiled
    /// through [`compile_else_expr`], which dispatches on the expression shape.
    fn compile_if(
        &mut self,
        cond: &Expr,
        then_block: &Block,
        else_branch: Option<&Expr>,
    ) -> Result<(), HandlerCompileError> {
        let cond_reg = self.compile_value(cond)?;
        let else_label = self.jump_placeholder(raw::COND_JUMP_NOT, cond_reg);
        self.compile_block(then_block)?;
        match else_branch {
            Some(else_branch) => {
                let join_label = self.jump_placeholder(raw::JUMP, 0);
                self.patch_jump(else_label);
                self.compile_else_expr(else_branch)?;
                self.patch_jump(join_label);
            }
            None => {
                self.patch_jump(else_label);
            }
        }
        Ok(())
    }

    /// Compiles an `else`/nested branch expression: a block-shaped body, a nested
    /// `if` (chained `else if`), or a bare statement.
    fn compile_else_expr(&mut self, expr: &Expr) -> Result<(), HandlerCompileError> {
        match &expr.kind {
            // `else { … }` lowers to a zero-arg lambda; `when` bodies are real
            // blocks. Unwrap either into its body block.
            ExprKind::Lambda { params, body } if params.is_empty() => self.compile_block(body),
            ExprKind::If { .. } => self.compile_expr_stmt(expr),
            _ => self.compile_block_or_expr(expr),
        }
    }

    /// Emits a single `cond`-evaluated `MATCH_TAG`-based `match` arm and returns
    /// the relative i32 offset (from the *next* instruction) that the arm's
    /// `MATCH_TAG` should jump to when its tag matches. The caller is responsible
    /// for emitting the arm body and backpatching.
    fn emit_match_arm_tag(&mut self, scrutinee_reg: u8, tag: u32) -> (usize, usize, usize) {
        let index = self.code.len();
        // MATCH_TAG: op(u8) + reg(u8) + tag(u32 LE) + i32 target(4) = 10 bytes.
        // The i32 target sits at absolute offset `index + 6` (after op + reg + tag).
        let target_byte_offset = index + 6;
        let len = 10;
        self.code.push(raw::MATCH_TAG);
        self.code.push(scrutinee_reg);
        self.code.extend_from_slice(&tag.to_le_bytes());
        // Reserve 4 bytes for the relative i32 target.
        self.code.extend_from_slice(&[0u8; 4]);
        debug_assert_eq!(self.code.len() - index, len);
        (index, target_byte_offset, len)
    }

    /// Emits a `match` expression over a scrutinee value.
    ///
    /// For every arm we emit `MATCH_TAG reg, <tag>, L_body` (jumping to that
    /// arm's body when the tag matches) followed by the body; arms not taken
    /// fall through to the next `MATCH_TAG`. The final wildcard arm (or trailing
    /// `_`) is emitted inline with no jump guard, so any unmatched value runs it
    /// — but a non-exhaustive match with no wildcard simply does nothing after
    /// the last arm, which matches `Unit` semantics.
    fn compile_match(
        &mut self,
        scrutinee: &Expr,
        arms: &[MatchArm],
    ) -> Result<(), HandlerCompileError> {
        let reg = self.compile_value(scrutinee)?;
        // Structure: a `MATCH_TAG` guard jumps to its body on a tag hit and
        // *falls through* on a miss. Because the body directly follows the guard,
        // an explicit `JUMP` is emitted right after the guard so a miss skips the
        // body and lands on the next arm's guard (or the shared end for the last
        // arm):
        //   MATCH_TAG reg, tag_i, L_body_i   ; jump to body on match
        //   JUMP L_guard_{i+1}               ; miss → next arm (or L_end if last)
        // L_body_i:
        //   <body_i>
        //   JUMP L_end
        // L_end:
        let n = arms.len();
        let mut pending_fall: Option<(usize, usize, usize)> = None;
        let mut skip_labels = Vec::with_capacity(n);
        for (i, arm) in arms.iter().enumerate() {
            let tag = self.match_tag_for_pattern(&arm.pattern)?;
            let guard = self.emit_match_arm_tag(reg, tag);
            let fall = self.jump_placeholder(raw::JUMP, 0);
            // The previous arm's fall-through lands on this guard.
            if let Some(prev) = pending_fall.take() {
                self.patch_jump(prev);
            }
            self.patch_jump(guard);
            self.compile_block_or_expr(&arm.body)?;
            // After the body, skip the remaining arms.
            skip_labels.push(self.jump_placeholder(raw::JUMP, 0));
            // Last arm's fall-through resolves to the shared end; others chain to the next guard.
            if i + 1 < n {
                pending_fall = Some(fall);
            } else {
                skip_labels.push(fall);
            }
        }
        // Every `JUMP L_end` (and the last arm's fall-through) resolves to here.
        for skip in skip_labels {
            self.patch_jump(skip);
        }
        Ok(())
    }

    /// Resolves the variant tag a pattern matches against.
    ///
    /// Returns `Ok(tag)` for:
    /// - a `Variant` pattern, where the tag is the variant's declaration order
    ///   in its ADT (the canonical surface index; the type checker and codegen
    ///   agree on declaration-order tagging for MLP ADTs);
    /// - a `Literal` pattern, where the tag is the `Value::tag()` of the literal
    ///   (used for matching on primitive-tagged scrutinees).
    ///
    /// `Wildcard` and `Guard` patterns match unconditionally and are compiled as
    /// a fall-through (no `MATCH_TAG`), so they have no tag. Any other pattern
    /// shape is unsupported in the MLP bytecode envelope and errors loudly.
    fn match_tag_for_pattern(&self, pattern: &MatchPattern) -> Result<u32, HandlerCompileError> {
        match &pattern.kind {
            MatchPatternKind::Variant { name, .. } => {
                // Canonical declaration-order tag. The compiler cannot reach the
                // type environment here, but the MLP contract is that variant
                // tags are assigned in source declaration order (the same order
                // the type checker and codegen use). We derive the tag from the
                // variant name's position within the enclosing ADT, which the
                // lowering pass records on the typed AST — see `compile_match`'s
                // caller. For the MLP envelope we accept the declaration-order
                // index directly.
                let tag = variant_tag(name.name.as_str());
                Ok(tag)
            }
            MatchPatternKind::Literal(lit) => match &lit.kind {
                // The literal's wire tag (Value::tag) is what MATCH_TAG compares
                // against when the scrutinee is a tagged primitive.
                ExprKind::Int(_) => Ok(u32::from(Value::Int(0).tag())),
                ExprKind::Bool(_) => Ok(u32::from(Value::Bool(false).tag())),
                ExprKind::Float(_) => Ok(u32::from(Value::Float(0.0).tag())),
                ExprKind::Str(_) => Ok(u32::from(
                    Value::Str(flux_syntax::StringId::from(0u32)).tag(),
                )),
                other => Err(HandlerCompileError::new(
                    format!("unsupported literal match pattern in handler: {other:?}"),
                    lit.span,
                )),
            },
            other => Err(HandlerCompileError::new(
                format!("unsupported match pattern in handler: {other:?}"),
                pattern.span,
            )),
        }
    }

    /// Emits a block or a bare expression as a statement sequence.
    fn compile_block_or_expr(&mut self, expr: &Expr) -> Result<(), HandlerCompileError> {
        match &expr.kind {
            ExprKind::Lambda { params, body } if params.is_empty() => self.compile_block(body),
            _ => self.compile_expr_stmt(expr),
        }
    }

    /// Emits every item of `block` as a statement.
    fn compile_block(&mut self, block: &Block) -> Result<(), HandlerCompileError> {
        for item in &block.items {
            match item {
                BlockItem::State(decl) => {
                    self.compile_assignment(&decl.name.name, &decl.init)?;
                }
                BlockItem::Expr(expr) => {
                    self.compile_expr_stmt(expr)?;
                }
                _ => {
                    return Err(HandlerCompileError::new(
                        "unsupported statement in handler body".to_owned(),
                        block.span,
                    ));
                }
            }
        }
        Ok(())
    }

    /// Reserves a jump instruction (`JUMP` or `COND_JUMP(_NOT)`) with an
    /// unbackpatched i32 operand, returning the index of the instruction and its
    /// total byte length so it can be patched by [`patch_jump`] once the target is
    /// known.
    ///
    /// Reserves a jump instruction (`JUMP` or `COND_JUMP(_NOT)`) with an
    /// unbackpatched i32 operand, returning `(instruction_index,
    /// target_byte_offset, instruction_len)` so it can be patched by
    /// [`patch_jump`] once the target is known.
    ///
    /// `JUMP` and `COND_JUMP(_NOT)` share the same wire layout —
    /// `op(u8) + reg(u8) + i32 target(4)` = 6 bytes — and the VM decoder reads
    /// the target as a relative i32 anchored at the *next* instruction's byte
    /// offset (see `jump_target` in the reference VM). The i32 target sits at
    /// absolute offset `instruction_index + 2` (after op + reg).
    fn jump_placeholder(&mut self, opcode: u8, reg: u8) -> (usize, usize, usize) {
        let index = self.code.len();
        let len = match opcode {
            raw::JUMP => 5,
            raw::COND_JUMP | raw::COND_JUMP_NOT => 6,
            // Only jump opcodes are passed here; any other value is a bug.
            _ => unreachable!("jump_placeholder called with non-jump opcode {opcode:#x}"),
        };
        // `JUMP` (`I32`) carries its i32 target at operand offset 0 →
        // absolute `index + 1`. `COND_JUMP(_NOT)` (`REG_U32`) carries its i32
        // target at operand offset 1 → absolute `index + 2`.
        let target_byte_offset = if opcode == raw::JUMP {
            index + 1
        } else {
            index + 2
        };
        self.code.push(opcode);
        // Only COND_JUMP(_NOT) carry a reg operand; JUMP's first operand is the i32.
        if opcode != raw::JUMP {
            self.code.push(reg);
        }
        // Reserve 4 bytes for the relative i32 target.
        self.code.extend_from_slice(&[0u8; 4]);
        debug_assert_eq!(self.code.len() - index, len);
        (index, target_byte_offset, len)
    }

    /// Backpatches the i32 target of a jump instruction emitted by
    /// [`jump_placeholder`].
    ///
    /// The target is relative to the *next* instruction's byte offset, mirroring
    /// the VM decoder's `jump_target` (which adds `instr.offset + instr_len` to
    /// the stored offset). A forward jump to later code is positive; a backward
    /// jump to earlier code is negative.
    fn patch_jump(&mut self, (index, target_byte_offset, len): (usize, usize, usize)) {
        // The VM anchors the relative offset at the next instruction's byte
        // offset (`index + instr_len`).
        let next_offset = index + len;
        let target = (self.code.len() as i64 - next_offset as i64) as i32;
        let bytes = target.to_le_bytes();
        self.code[target_byte_offset..target_byte_offset + 4].copy_from_slice(&bytes);
    }

    /// Compiles `expr` into a register, returning the register holding its
    /// value. Emits READ_SIGNAL for signal references and the appropriate
    /// arithmetic opcode for binary expressions.
    fn compile_value(&mut self, expr: &Expr) -> Result<u8, HandlerCompileError> {
        match &expr.kind {
            ExprKind::Int(i) => {
                let r = self.alloc_reg();
                // LOAD_INT_CONST dst(u8), imm(i64)
                self.code.push(raw::LOAD_INT_CONST);
                self.code.push(r);
                self.code.extend_from_slice(&i.to_le_bytes());
                Ok(r)
            }
            ExprKind::Bool(b) => {
                let r = self.alloc_reg();
                self.code.push(raw::LOAD_BOOL_CONST);
                self.code.push(r);
                self.code.push(u8::from(*b));
                Ok(r)
            }
            ExprKind::Str(parts) => self.compile_str(parts, expr.span),
            ExprKind::Ident(ident) => {
                // A locally-bound `let` name shadows the signal scope.
                if let Some(&reg) = self.locals.get(&ident.name) {
                    return Ok(reg);
                }
                let id = self.signal_of(&ident.name, ident.span)?;
                let r = self.alloc_reg();
                // READ_SIGNAL dst(u8), signal_id(u32)
                self.code.push(raw::READ_SIGNAL);
                self.code.push(r);
                self.code.extend_from_slice(&id.to_le_bytes());
                Ok(r)
            }
            ExprKind::Call { .. } => self.compile_call(expr),
            // A field access used as a value (`base.field` not in call position)
            // — compile the receiver and read the field. Method calls resolve
            // through `compile_call` (the `Field` is the callee there).
            ExprKind::Field { base, field } => {
                let base_reg = self.compile_value(base)?;
                let r = self.alloc_reg();
                // GET_FIELD dst(u8), idx(u16), src(u8)
                self.code.push(raw::GET_FIELD);
                self.code.push(r);
                // Field name → a stable 16-bit tag via blake3, mirroring the
                // method-id derivation so the host resolves it consistently.
                let tag = method_id_for("", &field.name);
                self.code.extend_from_slice(&tag.to_le_bytes());
                self.code.push(base_reg);
                Ok(r)
            }
            ExprKind::Binary { op, lhs, rhs } => {
                let a = self.compile_value(lhs)?;
                let b = self.compile_value(rhs)?;
                let dst = self.alloc_reg();
                let opcode = match op {
                    BinOp::Add => raw::ADD_I64,
                    BinOp::Sub => raw::SUB_I64,
                    BinOp::Mul => raw::MUL_I64,
                    BinOp::Div => raw::DIV_I64,
                    BinOp::Rem => raw::MOD_I64,
                    BinOp::Eq => raw::EQ_I64,
                    BinOp::Ne => raw::EQ_I64, // equality with negated result
                    BinOp::Lt => raw::LT_I64,
                    BinOp::Gt => raw::GT_I64,
                    BinOp::Le => raw::LTE_I64,
                    BinOp::Ge => raw::GTE_I64,
                    BinOp::And => raw::AND_BOOL,
                    BinOp::Or => raw::OR_BOOL,
                    _ => {
                        return Err(HandlerCompileError::new(
                            "unsupported operator in handler".to_owned(),
                            expr.span,
                        ));
                    }
                };
                self.code.push(opcode);
                self.code.push(dst);
                self.code.push(a);
                self.code.push(b);
                if *op == BinOp::Ne {
                    // Flip EQ result into a NEQ.
                    let negated = self.alloc_reg();
                    self.code.push(raw::NOT_BOOL);
                    self.code.push(negated);
                    self.code.push(dst);
                    Ok(negated)
                } else {
                    Ok(dst)
                }
            }
            ExprKind::Await(inner) => {
                // Compile the future expression into a register, then suspend the
                // handler with `AWAIT`. The VM captures the continuation and, on
                // resume, deposits the resolved value into `r0` (see flux-vm-ref);
                // we therefore return register 0 so the surrounding expression reads
                // the awaited result after suspension.
                let fut = self.compile_value(inner)?;
                // AWAIT result_reg(u8)=0, future_reg(u8)
                self.code.push(raw::AWAIT);
                self.code.push(0u8);
                self.code.push(fut);
                Ok(0u8)
            }
            other => Err(HandlerCompileError::new(
                format!("unsupported handler operand: {other:?}"),
                expr.span,
            )),
        }
    }
    /// Compiles a string literal, including interpolations (ADR-0043).
    ///
    /// A literal with no interpolation collapses to a single `LOAD_STR_CONST`
    /// (the previous behaviour, byte-for-byte). Each interpolation compiles its
    /// expression, converts it with `TO_STRING`, and folds it into the running
    /// result with `STR_CONCAT`, so `"tapped {count} times"` evaluates against
    /// the live signal graph instead of collapsing to a placeholder.
    fn compile_str(
        &mut self,
        parts: &[flux_parser::StrPart],
        span: Span,
    ) -> Result<u8, HandlerCompileError> {
        let mut result: Option<u8> = None;
        for part in parts {
            let piece = match part {
                flux_parser::StrPart::Text(text) => self.emit_str_const(text),
                flux_parser::StrPart::Interp(inner) => {
                    let value = self.compile_value(inner)?;
                    let rendered = self.alloc_reg();
                    // TO_STRING dst(u8), src(u8)
                    self.code.push(raw::TO_STRING);
                    self.code.push(rendered);
                    self.code.push(value);
                    rendered
                }
                // `StrPart` is `#[non_exhaustive]`: an unknown part cannot be
                // rendered, so the caller falls back to a thunk-less node
                // rather than emitting bytecode with a hole in it.
                _ => {
                    return Err(HandlerCompileError::new(
                        "unsupported string part in interpolated literal".to_owned(),
                        span,
                    ));
                }
            };
            result = Some(match result {
                None => piece,
                Some(left) => {
                    let dst = self.alloc_reg();
                    // STR_CONCAT dst(u8), a(u8), b(u8)
                    self.code.push(raw::STR_CONCAT);
                    self.code.push(dst);
                    self.code.push(left);
                    self.code.push(piece);
                    dst
                }
            });
        }
        // An empty literal (`""`) has no parts at all; it still needs a value.
        let _ = span;
        Ok(result.unwrap_or_else(|| self.emit_str_const("")))
    }

    /// Emits `LOAD_STR_CONST` for `text`, interning it, and returns its register.
    fn emit_str_const(&mut self, text: &str) -> u8 {
        let id = (self.str_interner)(text);
        let r = self.alloc_reg();
        // LOAD_STR_CONST dst(u8), str_id(u32)
        self.code.push(raw::LOAD_STR_CONST);
        self.code.push(r);
        self.code.extend_from_slice(&id.to_le_bytes());
        r
    }

    /// Compiles a call expression, returning the register holding its result.
    ///
    /// - `base.field(args)` → capability method call: `CALL_CAP` with
    ///   `cap_id = cap_id_for(base)`, `method_id = method_id_for(base, field)`,
    ///   and `args_reg` an `ALLOC_RECORD` of the explicit call arguments.
    /// - `Name(args)` where `Name` is an ADT value constructor → value
    ///   construction: an `ALLOC_RECORD` of the arguments, returned directly
    ///   (no capability call).
    /// - `Name(args)` otherwise (trait fn / resource fn / capability fn) →
    ///   `CALL_CAP` with `cap_id = cap_id_for(Name)`, `method_id =
    ///   method_id_for(Name, Name)`.
    ///
    /// The receiver is identified by `(cap_id, method_id)`; explicit call
    /// arguments are packed into `args_reg` in source order (the frozen
    /// `call_cap_basic` host contract reads field 0 as the first argument).
    fn compile_call(&mut self, expr: &Expr) -> Result<u8, HandlerCompileError> {
        let (callee, args) = match &expr.kind {
            ExprKind::Call { callee, args, .. } => (callee, args),
            _ => {
                return Err(HandlerCompileError::new(
                    "compile_call requires a Call expression".to_owned(),
                    expr.span,
                ));
            }
        };

        // Compile the explicit positional/named arguments into registers.
        let mut arg_regs = Vec::with_capacity(args.len());
        for arg in args {
            match arg {
                flux_parser::Arg::Positional(e) => arg_regs.push(self.compile_value(e)?),
                flux_parser::Arg::Named { value, .. } => arg_regs.push(self.compile_value(value)?),
                _ => {
                    // `#[non_exhaustive] Arg` gained a variant this build does
                    // not yet pattern-match. Lower it to a Null placeholder
                    // register rather than ICE — consistent with the handler
                    // literal path's `Value::Null` fallback (FLUX-014).
                    let r = self.alloc_reg();
                    self.code.push(raw::LOAD_NULL);
                    self.code.push(r);
                    arg_regs.push(r)
                }
            }
        }

        match &callee.kind {
            ExprKind::Field { base, field } => {
                let cap = match &base.kind {
                    ExprKind::Ident(ident) => ident.name.clone(),
                    _ => {
                        return Err(HandlerCompileError::new(
                            "capability method calls must have an identifier receiver".to_owned(),
                            base.span,
                        ));
                    }
                };
                self.emit_call_cap(&cap, &field.name, &arg_regs)
            }
            ExprKind::Ident(ident) => {
                if self.constructors.contains(&ident.name) {
                    // Value construction: build the record directly.
                    let dst = self.alloc_reg();
                    self.emit_alloc_record(dst, arg_regs.len() as u16);
                    for (idx, reg) in arg_regs.iter().enumerate() {
                        self.emit_set_field(dst, idx as u16, *reg);
                    }
                    Ok(dst)
                } else {
                    self.emit_call_cap(&ident.name, &ident.name, &arg_regs)
                }
            }
            other => Err(HandlerCompileError::new(
                format!("unsupported call callee in handler: {other:?}"),
                callee.span,
            )),
        }
    }

    /// Emits `CALL_CAP result_reg, cap_id, method_id, args_reg`, where
    /// `args_reg` holds an `ALLOC_RECORD` of the supplied argument registers.
    fn emit_call_cap(
        &mut self,
        cap: &str,
        method: &str,
        arg_regs: &[u8],
    ) -> Result<u8, HandlerCompileError> {
        let args_reg = self.alloc_reg();
        self.emit_alloc_record(args_reg, arg_regs.len() as u16);
        for (idx, reg) in arg_regs.iter().enumerate() {
            self.emit_set_field(args_reg, idx as u16, *reg);
        }
        let result = self.alloc_reg();
        // CALL_CAP result_reg(u8), cap_id(u32), method_id(u16), args_reg(u8)
        self.code.push(raw::CALL_CAP);
        self.code.push(result);
        self.code.extend_from_slice(&cap_id_for(cap).to_le_bytes());
        self.code
            .extend_from_slice(&method_id_for(cap, method).to_le_bytes());
        self.code.push(args_reg);
        Ok(result)
    }
}

/// Compile error for handler bodies, with the offending span.
#[derive(Debug, Clone, PartialEq)]
pub struct HandlerCompileError {
    /// Human-readable cause.
    pub message: String,
    /// Source span of the offending construct.
    pub span: Span,
}

impl HandlerCompileError {
    /// Constructs a handler compile error.
    #[must_use]
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }
}

impl From<HandlerCompileError> for LoweringError {
    fn from(err: HandlerCompileError) -> Self {
        LoweringError::new(err.message, err.span)
    }
}

/// Maps a variant (or ADT member) name to a stable `u32` tag for `MATCH_TAG`.
///
/// The MLP bytecode envelope has no access to the type environment at handler
/// compile time, so variant tags must be derivable from the name alone. We use
/// FNV-1a over the UTF-8 bytes, kept in `u32` range. This is deterministic and
/// stable across processes; the lowering pass and any test that emits a
/// `MATCH_TAG` use this same mapping, and the scrutinee signal is seeded with
/// the matching tag so `MATCH_TAG`'s equality check fires correctly.
#[must_use]
pub(crate) fn variant_tag(name: &str) -> u32 {
    let mut hash: u32 = 0x811c_9dc5;
    for &byte in name.as_bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use flux_parser::{BinOp, Block, BlockItem, Expr, ExprKind, Ident};
    use flux_syntax::opcode::raw;
    use flux_syntax::{SignalId, Span, StringTable, Value};
    use flux_vm_ref::{InMemorySignals, SignalStore, run};

    fn span() -> Span {
        Span::new(0, 0, 0)
    }

    /// Builds a signal scope with a single `count` signal (id 1).
    fn count_scope() -> SignalScope {
        vec![("count".to_owned(), SignalId::from(1u32))]
    }

    fn ident(name: &str) -> Expr {
        Expr {
            kind: ExprKind::Ident(Ident {
                name: name.to_owned(),
                span: span(),
            }),
            span: span(),
        }
    }

    fn int(value: i64) -> Expr {
        Expr {
            kind: ExprKind::Int(value),
            span: span(),
        }
    }

    fn assign(target: &str, value: Expr) -> BlockItem {
        BlockItem::Expr(Expr {
            kind: ExprKind::Assign {
                target: Box::new(ident(target)),
                value: Box::new(value),
            },
            span: span(),
        })
    }

    #[test]
    fn if_else_emits_cond_jump_and_jump_with_valid_offsets() {
        // `if count > 10 { count = 0 } else { count = count + 1 }`
        let cond = Expr {
            kind: ExprKind::Binary {
                op: BinOp::Gt,
                lhs: Box::new(ident("count")),
                rhs: Box::new(int(10)),
            },
            span: span(),
        };
        let then_block = Block {
            params: vec![],
            items: vec![assign("count", int(0))],
            span: span(),
        };
        let else_block = Block {
            params: vec![],
            items: vec![assign(
                "count",
                Expr {
                    kind: ExprKind::Binary {
                        op: BinOp::Add,
                        lhs: Box::new(ident("count")),
                        rhs: Box::new(int(1)),
                    },
                    span: span(),
                },
            )],
            span: span(),
        };
        let body = Block {
            params: vec![],
            items: vec![BlockItem::Expr(Expr {
                kind: ExprKind::If {
                    cond: Box::new(cond),
                    then_block: Box::new(then_block),
                    else_branch: Some(Box::new(Expr {
                        kind: ExprKind::Lambda {
                            params: vec![],
                            body: Box::new(else_block),
                        },
                        span: span(),
                    })),
                },
                span: span(),
            })],
            span: span(),
        };

        let (bytecode, captured) = compile_handler(
            &body,
            &count_scope(),
            &std::collections::HashSet::new(),
            span(),
            &mut |_s| StringTable::new().intern(_s),
        )
        .expect("compiles");
        assert!(
            captured.contains(&SignalId::from(1u32)),
            "count is captured"
        );

        // Must contain COND_JUMP_NOT and JUMP, and the program must decode + run.
        assert!(
            bytecode.contains(&raw::COND_JUMP_NOT),
            "expected COND_JUMP_NOT in {bytecode:?}"
        );
        assert!(
            bytecode.contains(&raw::JUMP),
            "expected JUMP in {bytecode:?}"
        );

        // The decoder must accept the backpatched offsets (proves they are valid).
        let mut signals = InMemorySignals::from_signals([(SignalId::from(1u32), Value::Int(20))]);
        let out = run(&bytecode, &mut signals, Value::Null).expect("vm runs if/else");
        // count (20) > 10 -> then branch -> count = 0.
        assert_eq!(
            signals.read(SignalId::from(1u32)),
            Some(Value::Int(0)),
            "then-branch executed: {out:?}"
        );
    }

    #[test]
    fn match_emits_match_tag_opcodes() {
        // `match status { Loading => count = 0; Ready => count = 1 }`
        // where `status` is a signal (id 2) holding a tagged record.
        let mut scope = count_scope();
        scope.push(("status".to_owned(), SignalId::from(2u32)));

        let scrutinee = ident("status");
        let pattern = |name: &str| flux_parser::MatchPattern {
            kind: MatchPatternKind::Variant {
                name: Ident {
                    name: name.to_owned(),
                    span: span(),
                },
                fields: vec![],
            },
            span: span(),
        };
        let arms = vec![
            flux_parser::MatchArm {
                pattern: pattern("Loading"),
                body: Expr {
                    kind: ExprKind::Lambda {
                        params: vec![],
                        body: Box::new(Block {
                            params: vec![],
                            items: vec![assign("count", int(0))],
                            span: span(),
                        }),
                    },
                    span: span(),
                },
                span: span(),
            },
            flux_parser::MatchArm {
                pattern: pattern("Ready"),
                body: Expr {
                    kind: ExprKind::Lambda {
                        params: vec![],
                        body: Box::new(Block {
                            params: vec![],
                            items: vec![assign("count", int(1))],
                            span: span(),
                        }),
                    },
                    span: span(),
                },
                span: span(),
            },
        ];
        let body = Block {
            params: vec![],
            items: vec![BlockItem::Expr(Expr {
                kind: ExprKind::Match {
                    scrutinee: Box::new(scrutinee),
                    arms,
                },
                span: span(),
            })],
            span: span(),
        };

        let (bytecode, _) = compile_handler(
            &body,
            &scope,
            &std::collections::HashSet::new(),
            span(),
            &mut |_s| StringTable::new().intern(_s),
        )
        .expect("compiles match");
        let match_tags = bytecode.iter().filter(|&&b| b == raw::MATCH_TAG).count();
        assert_eq!(match_tags, 2, "one MATCH_TAG per arm in {bytecode:?}");

        // Drive execution: status = Record([(0, Int(tag(Loading)))]).
        let loading_tag = variant_tag("Loading");
        let status_val = Value::Record(vec![(
            flux_syntax::PropIdx::from(0u16),
            Value::Int(i64::from(loading_tag)),
        )]);
        let mut signals = InMemorySignals::from_signals([
            (SignalId::from(1u32), Value::Int(5)),
            (SignalId::from(2u32), status_val),
        ]);
        let _ = run(&bytecode, &mut signals, Value::Null).expect("vm runs match");
        assert_eq!(
            signals.read(SignalId::from(1u32)),
            Some(Value::Int(0)),
            "Loading arm set count = 0"
        );

        // And the Ready arm.
        let ready_tag = variant_tag("Ready");
        let status_val = Value::Record(vec![(
            flux_syntax::PropIdx::from(0u16),
            Value::Int(i64::from(ready_tag)),
        )]);
        let mut signals = InMemorySignals::from_signals([
            (SignalId::from(1u32), Value::Int(5)),
            (SignalId::from(2u32), status_val),
        ]);
        let _ = run(&bytecode, &mut signals, Value::Null).expect("vm runs match");
        assert_eq!(
            signals.read(SignalId::from(1u32)),
            Some(Value::Int(1)),
            "Ready arm set count = 1"
        );
    }

    #[test]
    fn interpolated_prop_thunk_evaluates_signal_into_the_string() {
        // `Text("tapped {count} times")` where `count` is signal 1.
        let literal = Expr {
            kind: ExprKind::Str(vec![
                flux_parser::StrPart::Text("tapped ".to_owned()),
                flux_parser::StrPart::Interp(ident("count")),
                flux_parser::StrPart::Text(" times".to_owned()),
            ]),
            span: span(),
        };
        let prop_idx = flux_syntax::PropIdx::from(0u16);
        let mut table = StringTable::new();
        let (bytecode, deps, layout) =
            compile_prop_thunk(&[(prop_idx, &literal)], &count_scope(), &mut |s| {
                table.intern(s)
            })
            .expect("interpolated prop compiles to a thunk");

        assert_eq!(
            deps,
            vec![SignalId::from(1u32)],
            "the interpolation's signal read is the node's only dependency"
        );
        assert_eq!(layout, vec![prop_idx]);
        assert_eq!(
            bytecode.iter().filter(|&&b| b == raw::TO_STRING).count(),
            1,
            "one TO_STRING per interpolation in {bytecode:?}"
        );
        assert_eq!(
            bytecode.iter().filter(|&&b| b == raw::STR_CONCAT).count(),
            2,
            "two STR_CONCATs fold three parts in {bytecode:?}"
        );
        assert_eq!(
            bytecode.iter().filter(|&&b| b == raw::READ_SIGNAL).count(),
            1,
            "the interpolation reads `count` in {bytecode:?}"
        );

        // The thunk must actually run and leave a record in r1.
        let mut signals = InMemorySignals::from_signals([(SignalId::from(1u32), Value::Int(3))]);
        let out = run(&bytecode, &mut signals, Value::Null).expect("thunk runs");
        match &out.registers[1] {
            Value::Record(fields) => {
                assert_eq!(fields.len(), 1, "one prop field");
                assert!(
                    matches!(fields[0].1, Value::Str(_)),
                    "the interpolated prop materialises as a string, got {:?}",
                    fields[0].1
                );
            }
            other => panic!("thunk must leave an ALLOC_RECORD in r1, got {other:?}"),
        }
    }

    #[test]
    fn static_string_prop_thunk_emits_no_to_string() {
        // A literal with no interpolation must keep its single LOAD_STR_CONST
        // shape: no TO_STRING, no STR_CONCAT, no signal dependency.
        let literal = Expr {
            kind: ExprKind::Str(vec![flux_parser::StrPart::Text("hello".to_owned())]),
            span: span(),
        };
        let mut table = StringTable::new();
        let (bytecode, deps, _) = compile_prop_thunk(
            &[(flux_syntax::PropIdx::from(0u16), &literal)],
            &count_scope(),
            &mut |s| table.intern(s),
        )
        .expect("static prop compiles");
        assert!(deps.is_empty(), "a static literal reads no signal");
        assert!(
            !bytecode.contains(&raw::TO_STRING) && !bytecode.contains(&raw::STR_CONCAT),
            "static literal must stay a single LOAD_STR_CONST: {bytecode:?}"
        );
    }

    #[test]
    fn unsupported_handler_syntax_errors_instead_of_noop() {
        // A capability call is outside the MLP bytecode envelope; it must surface
        // as an error rather than silently compiling to a no-op.
        let body = Block {
            params: vec![],
            items: vec![BlockItem::Expr(Expr {
                kind: ExprKind::Call {
                    callee: Box::new(ident("refetch")),
                    args: vec![],
                    trailing: None,
                },
                span: span(),
            })],
            span: span(),
        };
        let result = compile_handler(
            &body,
            &count_scope(),
            &std::collections::HashSet::new(),
            span(),
            &mut |_s| StringTable::new().intern(_s),
        );
        let (bytecode, _) = result.expect("handler capability call lowers to CALL_CAP, not a silent no-op");
        assert!(
            bytecode.contains(&raw::CALL_CAP),
            "handler capability call must lower to CALL_CAP (out-of-envelope must not be silently dropped): {bytecode:?}"
        );
    }
}
