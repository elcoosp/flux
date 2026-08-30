//! `Patch` wire codec (Appendix D §D.2).

use flux_syntax::Patch;

use super::WireError;
use super::closure_ref::decode_closure_ref;
use super::cursor::Reader;
use super::node::{decode_node, encode_node};
use super::prop_diff::{decode_prop_diff, encode_prop_diff};

pub(crate) fn encode_patch(w: &mut super::cursor::Writer, patch: &Patch) {
    w.u8(patch.tag());
    match patch {
        Patch::Replace { id, node } => {
            w.u32(*id);
            encode_node(w, node);
        }
        Patch::Update { id, props_diff } => {
            w.u32(*id);
            encode_prop_diff(w, props_diff);
        }
        Patch::Insert {
            parent,
            index,
            node,
        } => {
            w.u32(*parent);
            w.u16(*index);
            encode_node(w, node);
        }
        Patch::Remove { id } => {
            w.u32(*id);
        }
        Patch::Reorder { parent, keys } => {
            w.u32(*parent);
            w.u16(keys.len() as u16);
            for key in keys {
                w.u32(*key);
            }
        }
        Patch::Handler { id, closure } => {
            w.u32(*id);
            super::closure_ref::encode_closure_ref(w, closure);
        }
        Patch::Reattach {
            old_id,
            new_id,
            node,
        } => {
            w.u32(*old_id);
            w.u32(*new_id);
            encode_node(w, node);
        }
        // `Patch` is `#[non_exhaustive]`. An unknown variant cannot be encoded
        // without a wire tag, so it is skipped; the differ/pre-flight stage
        // guarantees only known variants reach the serializer.
        _ => {}
    }
}

pub(crate) fn decode_patch(r: &mut Reader<'_>) -> Result<Patch, WireError> {
    let tag = r.u8("patch.tag")?;
    match tag {
        0x01 => {
            let id = r.u32("patch.replace.id")?;
            let node = decode_node(r)?;
            Ok(Patch::Replace { id, node })
        }
        0x02 => {
            let id = r.u32("patch.update.id")?;
            let props_diff = decode_prop_diff(r)?;
            Ok(Patch::Update { id, props_diff })
        }
        0x03 => {
            let parent = r.u32("patch.insert.parent")?;
            let index = r.u16("patch.insert.index")?;
            let node = decode_node(r)?;
            Ok(Patch::Insert {
                parent,
                index,
                node,
            })
        }
        0x04 => {
            let id = r.u32("patch.remove.id")?;
            Ok(Patch::Remove { id })
        }
        0x05 => {
            let parent = r.u32("patch.reorder.parent")?;
            let key_count = r.u16("patch.reorder.keys")?;
            r.ensure_capacity(key_count as usize, "patch.reorder")?;
            let mut keys = Vec::with_capacity(key_count as usize);
            for _ in 0..key_count {
                keys.push(r.u32("patch.reorder.key")?);
            }
            Ok(Patch::Reorder { parent, keys })
        }
        0x06 => {
            let id = r.u32("patch.handler.id")?;
            let closure = decode_closure_ref(r)?;
            Ok(Patch::Handler { id, closure })
        }
        0x07 => {
            let old_id = r.u32("patch.reattach.old_id")?;
            let new_id = r.u32("patch.reattach.new_id")?;
            let node = decode_node(r)?;
            Ok(Patch::Reattach {
                old_id,
                new_id,
                node,
            })
        }
        other => Err(WireError::InvalidTag {
            tag: other,
            context: "patch",
            at: r.pos() - 1,
        }),
    }
}
