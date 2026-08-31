use flux_syntax::Span;
use flux_vm_ref::VmErrorKind;
/// A runtime VM fault, classified by its stable [`VmErrorKind`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuntimeError {
    /// The category of fault (must match the ISA vector contract).
    pub kind: VmErrorKind,
    /// Byte offset of the offending instruction, when available.
    pub offset: u32,
    /// Source span, when the handler was lowered from `.flux`.
    pub span: Option<Span>,
}
