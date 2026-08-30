//! Handler-definition stream and shared bytecode blob codec (Appendix D §D.8 +
//! §D.12 handler section).

use flux_syntax::{ClosureRef, HandlerId};

use super::closure_ref::{decode_closure_ref, encode_closure_ref};
use super::cursor::{Reader, Writer};

/// Encodes one `HandlerDef` entry (Appendix D §D.8): the `HandlerId` followed
/// by its `ClosureRef` body.
pub(crate) fn encode_handler_def(w: &mut Writer, id: HandlerId, closure: &ClosureRef) {
    w.u32(id);
    encode_closure_ref(w, closure);
}

/// Decodes one `HandlerDef` entry (Appendix D §D.8).
pub(crate) fn decode_handler_def(
    r: &mut Reader<'_>,
) -> Result<(HandlerId, ClosureRef), crate::wire::WireError> {
    let id = r.u32("handler.id")?;
    let closure = decode_closure_ref(r)?;
    Ok((id, closure))
}

/// Encodes the shared bytecode blob (Appendix D §D.12 handler section): a
/// `u32` byte length followed by the raw little-endian bytecode.
pub(crate) fn encode_bytecode_blob(w: &mut Writer, blob: &[u8]) {
    w.u32(blob.len() as u32);
    w.bytes(blob);
}

/// Decodes the shared bytecode blob, returning the raw bytes (Appendix D §D.12).
pub(crate) fn decode_bytecode_blob(r: &mut Reader<'_>) -> Result<Vec<u8>, crate::wire::WireError> {
    let len = r.u32("bytecode_blob.len")? as usize;
    r.bytes(len, "bytecode_blob").map(|slice| slice.to_vec())
}
