//! Signal-graph delta codecs (Appendix D §D.10 / §D.11).

use std::string::String as StdString;

use flux_syntax::{FileId, SignalId, Value};

use super::WireError;
use super::cursor::Reader;
use super::value::{decode_value, encode_value};

/// A delta over the live signal graph (Appendix D §D.10).
#[derive(Clone, Debug, Default)]
#[allow(dead_code)]
pub(crate) struct StateDelta {
    /// `(signal_id, value)` pairs, in any order.
    pub cells: Vec<(SignalId, Value)>,
}

impl StateDelta {
    #[allow(dead_code)]
    pub(crate) fn encode(w: &mut super::cursor::Writer, delta: &StateDelta) {
        w.u16(delta.cells.len() as u16);
        for (signal, value) in &delta.cells {
            w.u32(*signal);
            encode_value(w, value);
        }
    }

    #[allow(dead_code)]
    pub(crate) fn decode(r: &mut Reader<'_>) -> Result<StateDelta, WireError> {
        let count = r.u16("state.count")?;
        r.ensure_capacity(count as usize, "state.cells")?;
        let mut cells = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let signal = r.u32("state.signal")?;
            let value = decode_value(r)?;
            cells.push((signal, value));
        }
        Ok(StateDelta { cells })
    }
}

/// New or changed source-file path mappings (Appendix D §D.11).
#[derive(Clone, Debug, Default)]
#[allow(dead_code)]
pub(crate) struct SourceMapDelta {
    /// `(file_id, path)` pairs.
    pub files: Vec<(FileId, StdString)>,
}

impl SourceMapDelta {
    #[allow(dead_code)]
    pub(crate) fn encode(w: &mut super::cursor::Writer, delta: &SourceMapDelta) {
        w.u16(delta.files.len() as u16);
        for (file_id, path) in &delta.files {
            w.u32(*file_id);
            w.u16(path.len() as u16);
            w.bytes(path.as_bytes());
        }
    }

    #[allow(dead_code)]
    pub(crate) fn decode(r: &mut Reader<'_>) -> Result<SourceMapDelta, WireError> {
        let count = r.u16("srcmap.count")?;
        r.ensure_capacity(count as usize, "srcmap.files")?;
        let mut files = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let file_id = r.u32("srcmap.file")?;
            let len = r.u16("srcmap.path_len")? as usize;
            let raw = r.bytes(len, "srcmap.path")?;
            let path = std::str::from_utf8(raw)
                .map_err(|_| WireError::InvalidUtf8 {
                    context: "srcmap.path",
                    at: r.pos() - len,
                })?
                .to_owned();
            files.push((file_id, path));
        }
        Ok(SourceMapDelta { files })
    }
}
