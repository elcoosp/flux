//! The typed surface syntax tree produced by [`crate::parse`].
//!
//! Every node carries a [`Span`] so downstream passes (type checking,
//! lowering, diagnostics) can point back at the exact source bytes. The shapes
//! mirror Appendix B of `/docs/spec/mlp-appendices.md`.

mod expr;
mod pattern;
mod types;

use flux_syntax::Span;

pub use types::{
    CapabilityDecl, MethodSig, TraitDecl, Type, TypeDecl, TypeKindAst, TypeParam, Variant,
};

pub use pattern::{LetPattern, MatchArm, MatchPattern, MatchPatternKind, Pattern};

pub use expr::{BinOp, Block, BlockItem, Expr, ExprKind, LifecycleKind, StateDecl, StrPart};

/// A call argument: positional or named.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Arg {
    /// A positional argument.
    Positional(Expr),
    /// `text: "Increment"` — a named argument.
    Named {
        /// Argument name.
        name: Ident,
        /// Argument value.
        value: Expr,
    },
}

impl Arg {
    /// Returns the argument's value expression.
    #[must_use]
    pub fn value(&self) -> &Expr {
        match self {
            Self::Positional(expr) | Self::Named { value: expr, .. } => expr,
        }
    }
}

/// A parsed source file: the ordered declarations it contains.
#[derive(Clone, Debug, PartialEq)]
pub struct Ast {
    /// Top-level declarations in source order.
    pub decls: Vec<Decl>,
    /// Span covering the whole file.
    pub span: Span,
}

/// A top-level declaration.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Decl {
    /// `import Name from "path"`.
    Import(ImportDecl),
    /// `use a::b::*`.
    Use(UseDecl),
    /// `component Name[T](props) { … }`.
    Component(ComponentDecl),
    /// `fn name(args) -> Ty { … }`.
    Fn(FnDecl),
    /// `type Name = | A | B(Int)`.
    Type(TypeDecl),
    /// `trait Name[T] { … }`.
    Trait(TraitDecl),
    /// `capability Name { … }`.
    Capability(CapabilityDecl),
    /// `Color.red = RGB(1.0, 0.0, 0.0)`.
    Const(ConstBinding),
}

impl Decl {
    /// Returns the span of the declaration.
    #[must_use]
    pub fn span(&self) -> Span {
        match self {
            Self::Import(decl) => decl.span,
            Self::Use(decl) => decl.span,
            Self::Component(decl) => decl.span,
            Self::Fn(decl) => decl.span,
            Self::Type(decl) => decl.span,
            Self::Trait(decl) => decl.span,
            Self::Capability(decl) => decl.span,
            Self::Const(decl) => decl.span,
        }
    }
}

/// An identifier together with the span it was written at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ident {
    /// The identifier text exactly as written.
    pub name: String,
    /// Span of the identifier.
    pub span: Span,
}

/// `import Name from "module/path"`.
#[derive(Clone, Debug, PartialEq)]
pub struct ImportDecl {
    /// Local name bound by the import.
    pub name: Ident,
    /// Module path string literal, with escapes left as written.
    pub source: String,
    /// Span of the whole declaration.
    pub span: Span,
}

/// `use a::b` or `use a::b::*`.
#[derive(Clone, Debug, PartialEq)]
pub struct UseDecl {
    /// Path segments, outermost first.
    pub segments: Vec<Ident>,
    /// Whether the path ended in a `::*` glob.
    pub glob: bool,
    /// Span of the whole declaration.
    pub span: Span,
}

/// `Color.red = RGB(1.0, 0.0, 0.0)` — a module-level associated constant.
#[derive(Clone, Debug, PartialEq)]
pub struct ConstBinding {
    /// Dotted path on the left of `=`, at least two segments long.
    pub path: Vec<Ident>,
    /// Bound value.
    pub value: Expr,
    /// Span of the whole binding.
    pub span: Span,
}

/// `@pure component Avatar(url: String) { … }`.
#[derive(Clone, Debug, PartialEq)]
pub struct ComponentDecl {
    /// Annotations written before the `component` keyword.
    pub annotations: Vec<Annotation>,
    /// Component name.
    pub name: Ident,
    /// Generic parameters, empty when the component is not generic.
    pub generics: Vec<TypeParam>,
    /// Declared props, empty when there is no prop list.
    pub props: Vec<PropDecl>,
    /// Component body.
    pub body: Block,
    /// Span of the whole declaration, including annotations.
    pub span: Span,
}

/// `@pure` or `@memo(depth: 2)`.
#[derive(Clone, Debug, PartialEq)]
pub struct Annotation {
    /// Annotation name without the `@`.
    pub name: Ident,
    /// Arguments, empty when the annotation has no argument list.
    pub args: Vec<Arg>,
    /// Span of the annotation.
    pub span: Span,
}

/// A single declared prop, e.g. `size: Float = 12.0`.
#[derive(Clone, Debug, PartialEq)]
pub struct PropDecl {
    /// Prop name.
    pub name: Ident,
    /// Declared type.
    pub ty: Type,
    /// Default value, present when the prop is optional.
    pub default: Option<Expr>,
    /// Span of the prop declaration.
    pub span: Span,
}

/// `fn name[T](a: Int) -> Int { … }`.
#[derive(Clone, Debug, PartialEq)]
pub struct FnDecl {
    /// Function name, possibly a symbolic operator.
    pub name: FnName,
    /// Generic parameters, empty when not generic.
    pub generics: Vec<TypeParam>,
    /// Parameters in declaration order.
    pub params: Vec<Param>,
    /// Declared return type, absent when the function returns `Unit`.
    pub ret: Option<Type>,
    /// Function body.
    pub body: Block,
    /// Span of the whole declaration.
    pub span: Span,
}

/// A function or trait-method name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FnName {
    /// The name as written: an identifier or an operator such as `+`.
    pub text: String,
    /// `true` when the name is a symbolic operator.
    pub is_operator: bool,
    /// Span of the name.
    pub span: Span,
}

/// A function or lambda parameter.
#[derive(Clone, Debug, PartialEq)]
pub struct Param {
    /// Parameter name.
    pub name: Ident,
    /// Declared type; absent for inferred lambda parameters.
    pub ty: Option<Type>,
    /// Default value, when the parameter is optional.
    pub default: Option<Expr>,
    /// Span of the parameter.
    pub span: Span,
}
