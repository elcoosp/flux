//! `Child` wire codec (Appendix D §D.4).

use flux_syntax::Child;

use super::core::WireError;
use super::cursor::{Reader, Writer};

pub(crate) fn encode_child(w: &mut Writer, child: &Child) {
    match child {
        Child::Node(id) => {
            w.u8(0x01);
            w.u32(*id);
        }
        Child::Splice { items } => {
            w.u8(0x02);
            w.u16(items.len() as u16);
            for (key, id) in items {
                w.u64(*key);
                w.u32(*id);
            }
        }
        // `Child` is `#[non_exhaustive]` for future slot kinds; we cannot
        // encode an unknown kind, so we emit nothing rather than panic. The
        // dev server rejects trees containing unknown children before they
        // reach serialization (AGENTS.md: no `unreachable!` in prod).
        _ => {}
    }
}

pub(crate) fn decode_child(r: &mut Reader<'_>) -> Result<Child, WireError> {
    let tag = r.u8("child.tag")?;
    match tag {
        0x01 => Ok(Child::Node(r.u32("child.node")?)),
        0x02 => {
            let count = r.u16("child.splice.count")?;
            r.ensure_capacity(count as usize, "child.splice")?;
            let mut items = Vec::with_capacity(count as usize);
            for _ in 0..count {
                let key = r.u64("child.splice.key")?;
                let id = r.u32("child.splice.node")?;
                items.push((key, id));
            }
            Ok(Child::Splice { items })
        }
        other => Err(WireError::InvalidTag {
            tag: other,
            context: "child",
            at: r.pos() - 1,
        }),
    }
}
