//! Type declarations and type expressions (Appendix B.2 "Type Expressions").

use flux_syntax::Span;

use crate::ast::{FnName, Ident, Param};

/// `type Shape = | Circle(Float) | Square(Float)`.
#[derive(Clone, Debug, PartialEq)]
pub struct TypeDecl {
    /// Type name.
    pub name: Ident,
    /// Generic parameters, empty when not generic.
    pub generics: Vec<TypeParam>,
    /// Variants in declaration order; always at least one.
    pub variants: Vec<Variant>,
    /// Span of the whole declaration.
    pub span: Span,
}

/// One variant of an algebraic data type.
#[derive(Clone, Debug, PartialEq)]
pub struct Variant {
    /// Variant name.
    pub name: Ident,
    /// Positional payload types, empty for a unit variant.
    pub fields: Vec<Type>,
    /// Span of the variant.
    pub span: Span,
}

/// `trait Numeric[T] { fn zero() -> T }`.
#[derive(Clone, Debug, PartialEq)]
pub struct TraitDecl {
    /// Trait name.
    pub name: Ident,
    /// Generic parameters, empty when not generic.
    pub generics: Vec<TypeParam>,
    /// Method signatures in declaration order.
    pub methods: Vec<MethodSig>,
    /// Span of the whole declaration.
    pub span: Span,
}

/// `capability Camera { fn capture() -> Data }`.
#[derive(Clone, Debug, PartialEq)]
pub struct CapabilityDecl {
    /// Capability name.
    pub name: Ident,
    /// Method signatures in declaration order.
    pub methods: Vec<MethodSig>,
    /// Span of the whole declaration.
    pub span: Span,
}

/// A method signature inside a `trait` or `capability` block.
#[derive(Clone, Debug, PartialEq)]
pub struct MethodSig {
    /// Method name, possibly a symbolic operator.
    pub name: FnName,
    /// Generic parameters, empty when not generic.
    pub generics: Vec<TypeParam>,
    /// Parameters in declaration order.
    pub params: Vec<Param>,
    /// Declared return type, absent when the method returns `Unit`.
    pub ret: Option<Type>,
    /// Span of the signature.
    pub span: Span,
}

/// A generic parameter with an optional single trait bound.
#[derive(Clone, Debug, PartialEq)]
pub struct TypeParam {
    /// Parameter name.
    pub name: Ident,
    /// Trait bound, e.g. the `Numeric` in `[T: Numeric]`.
    pub bound: Option<Ident>,
    /// Span of the parameter.
    pub span: Span,
}

/// A type expression.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum TypeKindAst {
    /// A built-in scalar: `Int`, `Float`, `Bool`, `String` or `Unit`.
    Primitive(String),
    /// A named type with optional generic arguments, e.g. `Option[Int]`.
    Named {
        /// Type constructor name.
        name: Ident,
        /// Generic arguments, empty when applied to none.
        args: Vec<Type>,
    },
    /// A structural record type, e.g. `{ x: Int, y: Int }`.
    Record(Vec<(Ident, Type)>),
    /// A function type, e.g. `Fn(Int) -> Bool`.
    Fn {
        /// Parameter types.
        params: Vec<Type>,
        /// Return type.
        ret: Box<Type>,
    },
}

/// A type expression together with its span.
#[derive(Clone, Debug, PartialEq)]
pub struct Type {
    /// What kind of type this is.
    pub kind: TypeKindAst,
    /// Span of the type expression.
    pub span: Span,
}
