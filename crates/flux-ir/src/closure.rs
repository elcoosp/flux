//! Handler closure bytecode container (Appendix C §C.1, ADR-0014).
//!
//! A `ClosureIR` is the bytecode form of a handler body, shipped as data to the
//! host VM. Handlers do not capture values — they capture [`SignalId`]s (ADR-0014)
//! — so a closure is just its bytecode plus the list of signals it reads or
//! writes. The host `FluxBytecodeVM` evaluates it against the live signal graph.

use flux_syntax::{HandlerId, SignalId, Span, TypeId};

/// The bytecode representation of a single handler body.
///
/// Serialised verbatim over the wire (Appendix D) and stored in the arena's
/// closure table. `span` is retained so VM faults can be reported against the
/// original `.flux` source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClosureIR {
    /// Index in the host app's closure table.
    pub id: HandlerId,
    /// Register-based bytecode (Appendix E) for this handler.
    pub bytecode: Vec<u8>,
    /// Signals this handler reads or writes, captured by reference (ADR-0014).
    pub captured_signals: Vec<SignalId>,
    /// Source span the handler was lowered from.
    pub span: Span,
    /// Parameter types, in declaration order.
    pub param_types: Vec<TypeId>,
    /// Return type of the handler.
    pub return_type: TypeId,
}

impl ClosureIR {
    /// Creates a closure with explicit metadata.
    ///
    /// # Examples
    ///
    /// ```
    /// use flux_ir::ClosureIR;
    /// use flux_syntax::{HandlerId, Span};
    ///
    /// let closure = ClosureIR::new(HandlerId::from(1u32), vec![0x00], vec![], Span::new(0, 0, 4));
    /// assert_eq!(closure.id, HandlerId::from(1u32));
    /// assert!(closure.captured_signals.is_empty());
    /// ```
    #[must_use]
    pub fn new(
        id: HandlerId,
        bytecode: Vec<u8>,
        captured_signals: Vec<SignalId>,
        span: Span,
    ) -> Self {
        Self {
            id,
            bytecode,
            captured_signals,
            span,
            param_types: Vec::new(),
            return_type: TypeId::from(0u32),
        }
    }

    /// Returns the bytecode as a slice of `Value`-agnostic bytes.
    ///
    /// The VM owns decoding; this crate only carries the bytes.
    #[must_use]
    pub fn bytecode(&self) -> &[u8] {
        &self.bytecode
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_records_metadata() {
        let span = Span::new(2, 10, 20);
        let closure = ClosureIR::new(HandlerId::from(5u32), vec![0xB0, 0, 7], vec![3, 4], span);
        assert_eq!(closure.id, HandlerId::from(5u32));
        assert_eq!(closure.bytecode, vec![0xB0, 0, 7]);
        assert_eq!(
            closure.captured_signals,
            vec![SignalId::from(3u32), SignalId::from(4u32)]
        );
        assert_eq!(closure.span, span);
    }

    #[test]
    fn closures_compare_by_content() {
        let a = ClosureIR::new(
            HandlerId::from(1u32),
            vec![0x00],
            vec![],
            Span::new(0, 0, 1),
        );
        let b = ClosureIR::new(
            HandlerId::from(1u32),
            vec![0x00],
            vec![],
            Span::new(0, 0, 1),
        );
        assert_eq!(a, b);
    }
}
