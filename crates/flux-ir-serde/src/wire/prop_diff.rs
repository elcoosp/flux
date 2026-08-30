//! `PropDiff` wire codec (Appendix D §D.6).

use flux_syntax::PropDiff;

use super::WireError;
use super::cursor::Reader;
use super::value::{decode_value, encode_value};

pub(crate) fn encode_prop_diff(w: &mut super::cursor::Writer, diff: &PropDiff) {
    w.u16(diff.changes.len() as u16);
    for (index, value) in &diff.changes {
        w.u16(*index);
        encode_value(w, value);
    }
    w.u16(diff.removals.len() as u16);
    for index in &diff.removals {
        w.u16(*index);
    }
}

pub(crate) fn decode_prop_diff(r: &mut Reader<'_>) -> Result<PropDiff, WireError> {
    let change_count = r.u16("propdiff.change_count")?;
    r.ensure_capacity(change_count as usize, "propdiff.changes")?;
    let mut changes = Vec::with_capacity(change_count as usize);
    for _ in 0..change_count {
        let index = r.u16("propdiff.index")?;
        let value = decode_value(r)?;
        changes.push((index, value));
    }
    let removal_count = r.u16("propdiff.removal_count")?;
    r.ensure_capacity(removal_count as usize, "propdiff.removals")?;
    let mut removals = Vec::with_capacity(removal_count as usize);
    for _ in 0..removal_count {
        removals.push(r.u16("propdiff.removal")?);
    }
    Ok(PropDiff { changes, removals })
}
