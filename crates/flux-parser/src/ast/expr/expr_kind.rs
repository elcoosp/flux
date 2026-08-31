use crate::ast::*;
/// An expression together with its span.
#[derive(Clone, Debug, PartialEq)]
pub struct Expr {
    /// What kind of expression this is.
    pub kind: ExprKind,
    /// Span of the expression.
    pub span: Span,
}

/// The expression forms of Appendix B.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum ExprKind {
    /// An integer literal.
    Int(i64),
    /// A float literal.
    Float(f64),
    /// A boolean literal.
    Bool(bool),
    /// A string literal, split into literal text and interpolations.
    Str(Vec<StrPart>),
    /// A list literal.
    List(Vec<Expr>),
    /// The `Null` value literal (FLUX-053 / ADR-0051). The sole inhabitant of
    /// every `Option[T]`; used by optional-chaining desugar and as a
    /// user-writable literal for an absent value.
    Null,
    /// A variable reference.
    Ident(Ident),
    /// The elided body marker `...` used by the Appendix B.3.8 example.
    Elided,
    /// `Font { family: "", size: 17.0 }`.
    Record {
        /// Record type name.
        name: Ident,
        /// Fields in source order.
        fields: Vec<(Ident, Expr)>,
    },
    /// A binary operation such as `a + b` or `a == b`.
    Binary {
        /// Operator as written.
        op: BinOp,
        /// Left operand.
        lhs: Box<Expr>,
        /// Right operand.
        rhs: Box<Expr>,
    },
    /// Field access, e.g. `user.name`.
    Field {
        /// Receiver expression.
        base: Box<Expr>,
        /// Field name.
        field: Ident,
    },
    /// Null-safe field access, e.g. `user?.name` (FLUX-053 / ADR-0051).
    /// Yields `Option[T]` when the base is `Option[...]`; short-circuits to
    /// `Null` when the base is `Null`.
    OptField {
        /// Receiver expression, whose type must be `Option[...]`.
        base: Box<Expr>,
        /// Field name.
        field: Ident,
    },
    /// A call, optionally with a trailing block (`Column(gap: 8) { … }`).
    Call {
        /// Callee expression.
        callee: Box<Expr>,
        /// Positional and named arguments.
        args: Vec<Arg>,
        /// Trailing block, when present.
        trailing: Option<Box<Block>>,
    },
    /// `let x = expr`.
    Let {
        /// Bound pattern.
        pattern: LetPattern,
        /// Initialiser, absent for a bare `let x`.
        value: Option<Box<Expr>>,
    },
    /// `target = expr`.
    Assign {
        /// Assignment target.
        target: Box<Expr>,
        /// Assigned value.
        value: Box<Expr>,
    },
    /// `if cond { … } else { … }`.
    If {
        /// Condition.
        cond: Box<Expr>,
        /// Then branch.
        then_block: Box<Block>,
        /// Else branch: another `if` expression or a block.
        else_branch: Option<Box<Expr>>,
    },
    /// `when cond { … } otherwise { … }`.
    When {
        /// Condition.
        cond: Box<Expr>,
        /// Body evaluated when the condition holds.
        then_block: Box<Block>,
        /// Fallback body.
        otherwise: Option<Box<Block>>,
    },
    /// `match scrutinee { pattern => expr … }`.
    Match {
        /// Value being matched.
        scrutinee: Box<Expr>,
        /// Arms in source order; always at least one.
        arms: Vec<MatchArm>,
    },
    /// `ForEach(items, key: fn(i) { i }) { item => … }`.
    ForEach {
        /// Collection expression.
        items: Box<Expr>,
        /// Key function.
        key: Box<Expr>,
        /// Loop body.
        body: Box<Block>,
    },
    /// `provide Ctx with value`.
    Provide {
        /// Context name.
        context: Ident,
        /// Provided value.
        value: Box<Expr>,
    },
    /// `useContext(RouterContext)`.
    UseContext(Ident),
    /// `fn (a, b) { … }`.
    Lambda {
        /// Parameters, empty when the list is omitted.
        params: Vec<Param>,
        /// Lambda body.
        body: Box<Block>,
    },
    /// A block-shaped lifecycle expression such as `onMount { … }`.
    Lifecycle {
        /// Which lifecycle form this is.
        kind: LifecycleKind,
        /// Body block.
        body: Box<Block>,
    },
    /// `resource(fn { … })`.
    Resource(Box<Expr>),
    /// `await <expr>` — suspend the handler, surfacing the awaited value as a
    /// reactive `Pending` until the future resolves (MLP v2 first-class async,
    /// ADR-0044). Lowers to the `AWAIT` bytecode opcode.
    Await(Box<Expr>),
    /// `createRef[TextField]()`.
    CreateRef {
        /// Generic arguments, empty when omitted.
        args: Vec<Type>,
    },
}

/// One piece of a string literal.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum StrPart {
    /// Literal text, escapes left exactly as written in the source.
    Text(String),
    /// An interpolated expression written inside `{ }`.
    Interp(Expr),
}

/// The block-bodied lifecycle forms of Appendix B.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum LifecycleKind {
    /// `onMount`.
    OnMount,
    /// `onCleanup`.
    OnCleanup,
    /// `effect`.
    Effect,
    /// `derived`.
    Derived,
    /// `batch`.
    Batch,
    /// `untrack`.
    Untrack,
}
