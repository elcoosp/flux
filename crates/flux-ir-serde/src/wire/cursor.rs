//! Allocation-free `Writer`/`Reader` cursor pair for the wire codec.

use super::WireError;

/// A grow-only little-endian byte sink.
pub(crate) struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    pub(crate) fn new() -> Self {
        Self { buf: Vec::new() }
    }

    /// Builds a `Writer` around a caller-owned buffer, clearing it first.
    ///
    /// The dev server encodes a frame on every hot-reload edit; reusing one
    /// scratch `Vec<u8>` across frames (instead of `new()`'s fresh allocation
    /// each call) keeps that hot path allocation-free after warm-up. The buffer
    /// is `clear()`ed (capacity preserved), not dropped.
    pub(crate) fn from_vec(mut buf: Vec<u8>) -> Self {
        buf.clear();
        Self { buf }
    }

    pub(crate) fn u8(&mut self, value: u8) {
        self.buf.push(value);
    }

    pub(crate) fn u16(&mut self, value: u16) {
        self.buf.extend_from_slice(&value.to_le_bytes());
    }

    pub(crate) fn u32(&mut self, value: u32) {
        self.buf.extend_from_slice(&value.to_le_bytes());
    }

    pub(crate) fn u64(&mut self, value: u64) {
        self.buf.extend_from_slice(&value.to_le_bytes());
    }

    pub(crate) fn bytes(&mut self, value: &[u8]) {
        self.buf.extend_from_slice(value);
    }

    /// Number of bytes written so far; used by callers that reserve a length
    /// slot and back-patch it after the body is encoded.
    pub(crate) fn buf_len(&self) -> usize {
        self.buf.len()
    }

    /// Overwrites the `u32` little-endian value at `offset` (must already be
    /// allocated in the buffer). Used to back-patch a length prefix once the
    /// body size is known.
    pub(crate) fn patch_u32_at(&mut self, offset: usize, value: u32) {
        let bytes = value.to_le_bytes();
        self.buf[offset..offset + 4].copy_from_slice(&bytes);
    }

    pub(crate) fn into_vec(self) -> Vec<u8> {
        self.buf
    }
}

/// A cursor over a little-endian byte source.
pub(crate) struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    /// Current read offset, used by callers to detect end-of-buffer.
    pub(crate) fn pos(&self) -> usize {
        self.pos
    }

    /// Bytes still available to read from the current position.
    #[must_use]
    pub(crate) fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.pos)
    }

    /// Rejects a declared element `count` that is impossible to satisfy with the
    /// bytes still available (LANE-D, OOM hardening).
    ///
    /// Every decoded element occupies at least one byte on the wire, so a count
    /// larger than `remaining()` can never be fulfilled — it is corruption, not a
    /// real collection. Without this guard an attacker-controlled `u32` count in
    /// an `Init` frame would drive `Vec::with_capacity(count)` to attempt a
    /// multi-gigabyte allocation and abort the process (libFuzzer flags this as
    /// `out-of-memory`). We fail with [`WireError::Truncated`] instead.
    pub(crate) fn ensure_capacity(
        &self,
        count: usize,
        context: &'static str,
    ) -> Result<(), WireError> {
        if count > self.remaining() {
            return Err(WireError::Truncated {
                at: self.pos,
                needed: count,
                context,
                available: self.remaining(),
            });
        }
        Ok(())
    }

    pub(crate) fn take(
        &mut self,
        needed: usize,
        context: &'static str,
    ) -> Result<&'a [u8], WireError> {
        let available = self.bytes.len() - self.pos;
        if available < needed {
            return Err(WireError::Truncated {
                at: self.pos,
                needed,
                context,
                available,
            });
        }
        let slice = &self.bytes[self.pos..self.pos + needed];
        self.pos += needed;
        Ok(slice)
    }

    pub(crate) fn u8(&mut self, context: &'static str) -> Result<u8, WireError> {
        Ok(self.take(1, context)?[0])
    }

    pub(crate) fn u16(&mut self, context: &'static str) -> Result<u16, WireError> {
        let mut buf = [0_u8; 2];
        buf.copy_from_slice(self.take(2, context)?);
        Ok(u16::from_le_bytes(buf))
    }

    pub(crate) fn u32(&mut self, context: &'static str) -> Result<u32, WireError> {
        let mut buf = [0_u8; 4];
        buf.copy_from_slice(self.take(4, context)?);
        Ok(u32::from_le_bytes(buf))
    }

    pub(crate) fn u64(&mut self, context: &'static str) -> Result<u64, WireError> {
        let mut buf = [0_u8; 8];
        buf.copy_from_slice(self.take(8, context)?);
        Ok(u64::from_le_bytes(buf))
    }

    pub(crate) fn i64(&mut self, context: &'static str) -> Result<i64, WireError> {
        Ok(self.u64(context)? as i64)
    }

    pub(crate) fn bytes(
        &mut self,
        len: usize,
        context: &'static str,
    ) -> Result<&'a [u8], WireError> {
        self.take(len, context)
    }
}
