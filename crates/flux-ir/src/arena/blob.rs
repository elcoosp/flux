//! Cold-data blob (de)serialisation for `IRArena` (Appendix C §C.1).
//!
//! Props/children/handlers are packed into length-prefixed little-endian
//! blobs so the arena's hot `Vec`s stay fixed-width; this module owns the
//! `Cursor` reader and the per-type pack/unpack helpers plus the
//! `hash_children` structural fold.
use flux_syntax::{Child, HandlerId, Props, Value};
// ── blob (de)serialisation ────────────────────────────────────────────────

pub(crate) struct Cursor<'b> {
    bytes: &'b [u8],
    pos: usize,
}

impl<'b> Cursor<'b> {
    fn new(bytes: &'b [u8]) -> Self {
        Self { bytes, pos: 0 }
    }
    fn take(&mut self, n: usize) -> &'b [u8] {
        let slice = &self.bytes[self.pos..self.pos + n];
        self.pos += n;
        slice
    }
    fn u8(&mut self) -> u8 {
        self.bytes[self.pos]
    }
    fn u16(&mut self) -> u16 {
        let mut buf = [0u8; 2];
        buf.copy_from_slice(self.take(2));
        u16::from_le_bytes(buf)
    }
    fn u32(&mut self) -> u32 {
        let mut buf = [0u8; 4];
        buf.copy_from_slice(self.take(4));
        u32::from_le_bytes(buf)
    }
    fn u64(&mut self) -> u64 {
        let mut buf = [0u8; 8];
        buf.copy_from_slice(self.take(8));
        u64::from_le_bytes(buf)
    }
    fn advance(&mut self, n: usize) {
        self.pos += n;
    }
}

pub(crate) fn pack_props(blob: &mut Vec<u8>, props: &Props) {
    blob.extend_from_slice(&(props.fields().len() as u16).to_le_bytes());
    for (idx, value) in props.fields() {
        blob.extend_from_slice(&idx.to_le_bytes());
        pack_value(blob, value);
    }
}

pub(crate) fn unpack_props(bytes: &[u8]) -> Props {
    let mut cur = Cursor::new(bytes);
    let count = cur.u16();
    let mut fields = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let idx = cur.u16();
        let value = unpack_value(&mut cur);
        fields.push((idx, value));
    }
    Props::from_fields(fields)
}

pub(crate) fn pack_value(blob: &mut Vec<u8>, value: &Value) {
    blob.push(value.tag());
    match value {
        Value::Null => {}
        Value::Int(i) => blob.extend_from_slice(&i.to_le_bytes()),
        Value::Float(f) => blob.extend_from_slice(&f.to_le_bytes()),
        Value::Bool(b) => blob.push(u8::from(*b)),
        Value::Str(id) | Value::HandlerRef(id) => blob.extend_from_slice(&id.to_le_bytes()),
        Value::List(items) => {
            blob.extend_from_slice(&(items.len() as u16).to_le_bytes());
            for item in items {
                pack_value(blob, item);
            }
        }
        Value::Record(fields) => {
            blob.extend_from_slice(&(fields.len() as u16).to_le_bytes());
            for (idx, val) in fields {
                blob.extend_from_slice(&idx.to_le_bytes());
                pack_value(blob, val);
            }
        }
        _ => {}
    }
}

pub(crate) fn unpack_value(cur: &mut Cursor<'_>) -> Value {
    const TAG_NULL: u8 = 0x00;
    const TAG_INT: u8 = 0x01;
    const TAG_FLOAT: u8 = 0x02;
    const TAG_BOOL: u8 = 0x03;
    const TAG_STR: u8 = 0x04;
    const TAG_HANDLER: u8 = 0x05;
    const TAG_LIST: u8 = 0x06;
    const TAG_RECORD: u8 = 0x07;
    let tag = cur.u8();
    cur.advance(1);
    match tag {
        TAG_NULL => Value::Null,
        TAG_INT => Value::Int(cur.u64() as i64),
        TAG_FLOAT => Value::Float(f64::from_bits(cur.u64())),
        TAG_BOOL => {
            let b = cur.u8();
            cur.advance(1);
            Value::Bool(b != 0)
        }
        TAG_STR => Value::Str(cur.u32()),
        TAG_HANDLER => Value::HandlerRef(cur.u32()),
        TAG_LIST => {
            let count = cur.u16();
            let mut items = Vec::with_capacity(count as usize);
            for _ in 0..count {
                items.push(unpack_value(cur));
            }
            Value::List(items)
        }
        TAG_RECORD => {
            let count = cur.u16();
            let mut fields = Vec::with_capacity(count as usize);
            for _ in 0..count {
                let idx = cur.u16();
                let val = unpack_value(cur);
                fields.push((idx, val));
            }
            Value::Record(fields)
        }
        _ => Value::Null,
    }
}

pub(crate) fn pack_children(blob: &mut Vec<u8>, children: &[Child]) {
    blob.extend_from_slice(&(children.len() as u16).to_le_bytes());
    for child in children {
        match child {
            Child::Node(id) => {
                blob.push(0);
                blob.extend_from_slice(&id.to_le_bytes());
            }
            Child::Splice { items } => {
                blob.push(1);
                blob.extend_from_slice(&(items.len() as u16).to_le_bytes());
                for (key, id) in items {
                    blob.extend_from_slice(&key.to_le_bytes());
                    blob.extend_from_slice(&id.to_le_bytes());
                }
            }
            _ => {}
        }
    }
}

pub(crate) fn unpack_children(bytes: &[u8]) -> Vec<Child> {
    let mut cur = Cursor::new(bytes);
    let count = cur.u16();
    let mut out = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let tag = cur.u8();
        cur.advance(1);
        match tag {
            0 => out.push(Child::Node(cur.u32())),
            1 => {
                let n = cur.u16();
                let mut items = Vec::with_capacity(n as usize);
                for _ in 0..n {
                    let key = cur.u64();
                    let id = cur.u32();
                    items.push((key, id));
                }
                out.push(Child::Splice { items });
            }
            _ => {}
        }
    }
    out
}

pub(crate) fn pack_handlers(blob: &mut Vec<u8>, handlers: &[HandlerId]) {
    blob.extend_from_slice(&(handlers.len() as u16).to_le_bytes());
    for id in handlers {
        blob.extend_from_slice(&id.to_le_bytes());
    }
}

pub(crate) fn unpack_handlers(bytes: &[u8]) -> Vec<HandlerId> {
    let mut cur = Cursor::new(bytes);
    let count = cur.u16();
    let mut out = Vec::with_capacity(count as usize);
    for _ in 0..count {
        out.push(cur.u32());
    }
    out
}

/// Folds the ordered sequence of child slots into a content hash capturing the
/// node's structural layout (independent of props/handlers).
///
/// Each slot contributes `Child::Node(id)` or `Child::Splice { items }`
/// (the ordered `(key, child_id)` pairs). Reordering children, adding,
/// removing, or changing a key all change the digest, while a purely
/// prop-level edit leaves it unchanged. The fold is order-sensitive so that
/// `A,B` and `B,A` hash differently (driving the `Reorder` path).
pub(crate) fn hash_children(children: &[Child]) -> u64 {
    let mut accumulator: u64 = 0xcbf2_9ce4_8422_2325;
    for (slot, child) in children.iter().enumerate() {
        let mut hasher = blake3::Hasher::new();
        hasher.update(&(slot as u64).to_le_bytes());
        match child {
            Child::Node(id) => {
                hasher.update(&[0]);
                hasher.update(&id.to_le_bytes());
            }
            Child::Splice { items } => {
                hasher.update(&[1]);
                hasher.update(&(items.len() as u64).to_le_bytes());
                for (key, id) in items {
                    hasher.update(&key.to_le_bytes());
                    hasher.update(&id.to_le_bytes());
                }
            }
            // `Child` is `#[non_exhaustive]`; unknown future variants hash a
            // distinct sentinel so they remain distinguishable.
            &_ => {
                hasher.update(&[0xff]);
            }
        }
        let mut digest = [0_u8; 8];
        digest.copy_from_slice(&hasher.finalize().as_bytes()[..8]);
        accumulator ^= u64::from_le_bytes(digest);
    }
    accumulator
}
