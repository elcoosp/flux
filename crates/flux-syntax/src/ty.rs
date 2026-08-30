//! Structural type representation shared by the checker, IR and codegen
//! (Appendix C §C.1).

use crate::ids::StringId;

/// A structural type.
///
/// The type checker produces these; both codegen backends consume them. Type
/// variables and constrained variables only appear before generalisation and
/// monomorphisation are complete.
///
/// # Examples
///
/// ```
/// use flux_syntax::TypeKind;
///
/// let list = TypeKind::List(Box::new(TypeKind::Int));
/// assert_eq!(list.element_type(), Some(&TypeKind::Int));
/// assert!(list.is_concrete());
/// assert!(!TypeKind::Var(0).is_concrete());
/// ```
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum TypeKind {
    /// `Int`.
    Int,
    /// `Float`.
    Float,
    /// `Bool`.
    Bool,
    /// `String`.
    String,
    /// `Unit`.
    Unit,
    /// `List[T]`.
    List(Box<TypeKind>),
    /// `Map[K, V]`.
    Map(Box<TypeKind>, Box<TypeKind>),
    /// `Option[T]`.
    Option(Box<TypeKind>),
    /// `Fn(A, B) -> R`.
    Fn(Vec<TypeKind>, Box<TypeKind>),
    /// Anonymous record with named fields.
    Record(Vec<(StringId, TypeKind)>),
    /// One variant of an algebraic data type.
    Variant(StringId, Vec<TypeKind>),
    /// Unresolved inference variable.
    Var(u32),
    /// Inference variable bounded by one or more trait names.
    Constrained(u32, Vec<StringId>),
}

impl TypeKind {
    /// Returns the element type of a `List` or `Option`, or `None` otherwise.
    #[must_use]
    pub fn element_type(&self) -> Option<&Self> {
        match self {
            Self::List(element) | Self::Option(element) => Some(element),
            _ => None,
        }
    }

    /// Returns `true` when the type is a primitive scalar or `Unit`.
    #[must_use]
    pub const fn is_primitive(&self) -> bool {
        matches!(
            self,
            Self::Int | Self::Float | Self::Bool | Self::String | Self::Unit
        )
    }

    /// Returns `true` when the type contains no unresolved inference variable.
    ///
    /// Monomorphisation may only proceed for concrete types.
    #[must_use]
    pub fn is_concrete(&self) -> bool {
        match self {
            Self::Var(_) | Self::Constrained(_, _) => false,
            Self::Int | Self::Float | Self::Bool | Self::String | Self::Unit => true,
            Self::List(inner) | Self::Option(inner) => inner.is_concrete(),
            Self::Map(key, value) => key.is_concrete() && value.is_concrete(),
            Self::Fn(params, ret) => params.iter().all(Self::is_concrete) && ret.is_concrete(),
            Self::Record(fields) => fields.iter().all(|(_, ty)| ty.is_concrete()),
            Self::Variant(_, payload) => payload.iter().all(Self::is_concrete),
        }
    }
}

impl std::fmt::Display for TypeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Int => write!(f, "Int"),
            Self::Float => write!(f, "Float"),
            Self::Bool => write!(f, "Bool"),
            Self::String => write!(f, "String"),
            Self::Unit => write!(f, "Unit"),
            Self::List(inner) => write!(f, "List[{inner}]"),
            Self::Map(key, value) => write!(f, "Map[{key}, {value}]"),
            Self::Option(inner) => write!(f, "Option[{inner}]"),
            Self::Fn(params, ret) => {
                let params = params
                    .iter()
                    .map(|p| p.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "Fn({params}) -> {ret}")
            }
            Self::Record(fields) => {
                let fields = fields
                    .iter()
                    .map(|(id, ty)| format!("{}: {}", *id, ty))
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "{{ {fields} }}")
            }
            Self::Variant(id, payload) => {
                let payload = payload
                    .iter()
                    .map(|p| p.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "{}({payload})", *id)
            }
            Self::Var(id) => write!(f, "?{id}"),
            Self::Constrained(id, traits) => {
                let traits = traits
                    .iter()
                    .map(|t| (*t).to_string())
                    .collect::<Vec<_>>()
                    .join(" + ");
                write!(f, "?{id}: {traits}")
            }
        }
    }
}
