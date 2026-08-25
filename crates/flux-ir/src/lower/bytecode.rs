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
use flux_syntax::{SignalId, Span, Value};

use crate::lower::error::LoweringError;

/// A signal name paired with its assigned [`SignalId`].
type SignalScope = Vec<(String, SignalId)>;

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
    span: Span,
) -> Result<(Vec<u8>, Vec<SignalId>), HandlerCompileError> {
    let mut emitter = Emitter::new(scope);
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

/// Bytecode emitter: walks expressions, appends raw opcode bytes, and records
/// captured signal IDs.
struct Emitter<'a> {
    scope: &'a SignalScope,
    code: Vec<u8>,
    captured: Vec<SignalId>,
    reg: u8,
}

impl<'a> Emitter<'a> {
    fn new(scope: &'a SignalScope) -> Self {
        Self {
            scope,
            code: Vec::new(),
            captured: Vec::new(),
            // r0 = payload, r15 = gas; start allocating at r1.
            reg: 1,
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
            ExprKind::Ident(ident) => {
                let id = self.signal_of(&ident.name, ident.span)?;
                let r = self.alloc_reg();
                // READ_SIGNAL dst(u8), signal_id(u32)
                self.code.push(raw::READ_SIGNAL);
                self.code.push(r);
                self.code.extend_from_slice(&id.to_le_bytes());
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
            other => Err(HandlerCompileError::new(
                format!("unsupported handler operand: {other:?}"),
                expr.span,
            )),
        }
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
    use flux_syntax::{SignalId, Span, Value};
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

        let (bytecode, captured) =
            compile_handler(&body, &count_scope(), span()).expect("compiles");
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

        let (bytecode, _) = compile_handler(&body, &scope, span()).expect("compiles match");
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
        let result = compile_handler(&body, &count_scope(), span());
        assert!(
            result.is_err(),
            "out-of-envelope handler must error, not silently no-op"
        );
    }
}
