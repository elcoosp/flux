//! Expression, block and pattern nodes of the surface syntax tree.

use flux_syntax::Span;

use crate::ast::{LetPattern, MatchArm, Pattern};

use crate::ast::{Arg, Ident, Param, Type};

/// A brace-delimited block: optional closure parameters plus a body.
#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    /// Closure parameters written as `{ item, index => … }`.
    pub params: Vec<Pattern>,
    /// Body items in source order.
    pub items: Vec<BlockItem>,
    /// Span of the block including both braces.
    pub span: Span,
}

/// One entry in a block body.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum BlockItem {
    /// `state count: Int = 0`.
    State(StateDecl),
    /// `derived double = count * 2` — a computed signal that re-derives from
    /// other signals whenever they change (FLUX-072 #12).
    Derived(DerivedDecl),
    /// `width: size` — a prop entry in a trailing prop block.
    Prop {
        /// Prop name.
        name: Ident,
        /// Prop value.
        value: Expr,
    },
    /// A bare expression.
    Expr(Expr),
}

/// `state count: Int = 0`.
#[derive(Clone, Debug, PartialEq)]
pub struct StateDecl {
    /// State cell name.
    pub name: Ident,
    /// Declared type, absent when it is inferred from the initialiser.
    pub ty: Option<Type>,
    /// Initial value.
    pub init: Expr,
    /// Span of the declaration.
    pub span: Span,
}

/// `derived double = count * 2`.
///
/// A computed signal: it re-evaluates `init` whenever any signal it reads
/// changes, so it never desyncs from its sources (FLUX-072 #12).
#[derive(Clone, Debug, PartialEq)]
pub struct DerivedDecl {
    /// Computed-signal name.
    pub name: Ident,
    /// Declared type, absent when inferred from the body.
    pub ty: Option<Type>,
    /// The expression that derives the value.
    pub init: Expr,
    /// Span of the declaration.
    pub span: Span,
}

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

/// A binary operator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum BinOp {
    /// `+`.
    Add,
    /// `-`.
    Sub,
    /// `*`.
    Mul,
    /// `/`.
    Div,
    /// `%`.
    Rem,
    /// `==`.
    Eq,
    /// `!=`.
    Ne,
    /// `<`.
    Lt,
    /// `>`.
    Gt,
    /// `<=`.
    Le,
    /// `>=`.
    Ge,
    /// `&&`.
    And,
    /// `||`.
    Or,
}

impl BinOp {
    /// Parses an operator from its source spelling.
    ///
    /// Returns `None` when `text` is not one of the operators in Appendix B.
    ///
    /// # Examples
    ///
    /// ```
    /// use flux_parser::BinOp;
    ///
    /// assert_eq!(BinOp::from_source("<="), Some(BinOp::Le));
    /// assert_eq!(BinOp::from_source("<>"), None);
    /// ```
    #[must_use]
    pub fn from_source(text: &str) -> Option<Self> {
        match text {
            "+" => Some(Self::Add),
            "-" => Some(Self::Sub),
            "*" => Some(Self::Mul),
            "/" => Some(Self::Div),
            "%" => Some(Self::Rem),
            "==" => Some(Self::Eq),
            "!=" => Some(Self::Ne),
            "<" => Some(Self::Lt),
            ">" => Some(Self::Gt),
            "<=" => Some(Self::Le),
            ">=" => Some(Self::Ge),
            "&&" => Some(Self::And),
            "||" => Some(Self::Or),
            _ => None,
        }
    }
}
