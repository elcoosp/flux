//! Runtime values shared by the IR, the VM and the wire protocol
//! (Appendix C §C.1, Appendix D §D.5).

use crate::ids::{HandlerId, PropIdx, StringId};

/// A value that can appear in props, in signal cells, or on the VM stack.
///
/// Strings are represented by their [`StringId`]; resolve them through the
/// owning [`crate::StringTable`].
///
/// # Examples
///
/// ```
/// use flux_syntax::Value;
///
/// assert_eq!(Value::Int(1).tag(), 0x01);
/// assert_eq!(Value::Null.tag(), 0x00);
/// ```
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Value {
    /// 64-bit signed integer.
    Int(i64),
    /// 64-bit IEEE-754 float.
    Float(f64),
    /// Boolean.
    Bool(bool),
    /// Interned string.
    Str(StringId),
    /// Homogeneous list.
    List(Vec<Value>),
    /// Record keyed by prop index.
    Record(Vec<(PropIdx, Value)>),
    /// Reference to a hot-swappable handler closure.
    HandlerRef(HandlerId),
    /// Absence of a value; also the representation of `Unit`.
    Null,
}

impl Value {
    /// Wire type tag as specified by Appendix D §D.5.
    ///
    /// The tags are load-bearing: the Swift and Kotlin deserializers switch on
    /// exactly these bytes.
    #[must_use]
    pub const fn tag(&self) -> u8 {
        match self {
            Self::Null => 0x00,
            Self::Int(_) => 0x01,
            Self::Float(_) => 0x02,
            Self::Bool(_) => 0x03,
            Self::Str(_) => 0x04,
            Self::HandlerRef(_) => 0x05,
            Self::List(_) => 0x06,
            Self::Record(_) => 0x07,
        }
    }

    /// Returns the integer payload, or `None` for any other variant.
    #[must_use]
    pub const fn as_int(&self) -> Option<i64> {
        match self {
            Self::Int(value) => Some(*value),
            _ => None,
        }
    }

    /// Returns the float payload, or `None` for any other variant.
    #[must_use]
    pub const fn as_float(&self) -> Option<f64> {
        match self {
            Self::Float(value) => Some(*value),
            _ => None,
        }
    }

    /// Returns the boolean payload, or `None` for any other variant.
    #[must_use]
    pub const fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }

    /// Returns the interned string ID, or `None` for any other variant.
    #[must_use]
    pub const fn as_str_id(&self) -> Option<StringId> {
        match self {
            Self::Str(id) => Some(*id),
            _ => None,
        }
    }

    /// Returns the referenced handler, or `None` for any other variant.
    #[must_use]
    pub const fn as_handler(&self) -> Option<HandlerId> {
        match self {
            Self::HandlerRef(id) => Some(*id),
            _ => None,
        }
    }

    /// Feeds a canonical byte encoding of this value into `hasher`.
    ///
    /// Content addressing must be reproducible across processes, so this uses
    /// an explicit little-endian encoding rather than [`std::hash::Hash`],
    /// whose output is not specified. Floats are hashed by their bit pattern
    /// after canonicalising `NaN`, so two `NaN` props hash equal.
    pub fn hash_into(&self, hasher: &mut blake3::Hasher) {
        hasher.update(&[self.tag()]);
        match self {
            Self::Null => {}
            Self::Int(value) => {
                hasher.update(&value.to_le_bytes());
            }
            Self::Float(value) => {
                let canonical = if value.is_nan() { f64::NAN } else { *value };
                hasher.update(&canonical.to_bits().to_le_bytes());
            }
            Self::Bool(value) => {
                hasher.update(&[u8::from(*value)]);
            }
            Self::Str(id) | Self::HandlerRef(id) => {
                hasher.update(&id.to_le_bytes());
            }
            Self::List(items) => {
                hasher.update(&(items.len() as u32).to_le_bytes());
                for item in items {
                    item.hash_into(hasher);
                }
            }
            Self::Record(fields) => {
                hasher.update(&(fields.len() as u32).to_le_bytes());
                for (index, value) in fields {
                    hasher.update(&index.to_le_bytes());
                    value.hash_into(hasher);
                }
            }
        }
    }
}
