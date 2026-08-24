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

use flux_parser::{BinOp, Block, BlockItem, Expr, ExprKind};
use flux_syntax::opcode::raw;
use flux_syntax::{SignalId, Span};

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
            _ => Err(HandlerCompileError::new(
                "unsupported handler expression".to_owned(),
                expr.span,
            )),
        }
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
