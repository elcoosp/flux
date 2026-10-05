//! Allocation-free `Writer`/`Reader` cursor pair for the wire codec.

use super::WireError;

/// A grow-only little-endian byte sink.
pub(crate) struct Writer {
    buf: Vec<u8>,
    /// Wire protocol version this writer targets (ADR-0059). v3 writes
    /// user-authored collection counts as `u32`; v2 writes them as `u16`.
    /// Defaults to [`crate::frame::PROTOCOL_VERSION`] so any encoder that
    /// does not call [`Self::set_version`] emits the newest layout.
    version: u8,
}

impl Writer {
    pub(crate) fn new() -> Self {
        Self {
            buf: Vec::new(),
            version: crate::frame::PROTOCOL_VERSION,
        }
    }

    /// Builds a `Writer` around a caller-owned buffer, clearing it first.
    ///
    /// The dev server encodes a frame on every hot-reload edit; reusing one
    /// scratch `Vec<u8>` across frames (instead of `new()`'s fresh allocation
    /// each call) keeps that hot path allocation-free after warm-up. The buffer
    /// is `clear()`ed (capacity preserved), not dropped.
    pub(crate) fn from_vec(mut buf: Vec<u8>) -> Self {
        buf.clear();
        Self {
            buf,
            version: crate::frame::PROTOCOL_VERSION,
        }
    }

    pub(crate) fn u8(&mut self, value: u8) {
        self.buf.push(value);
    }

    pub(crate) fn u16(&mut self, value: u16) {
        self.buf.extend_from_slice(&value.to_le_bytes());
    }

    /// Checked length-prefix write. A silent `as u16` truncation desyncs
    /// every host decoder (audit H14): panic the encode instead.
    ///
    /// Kept for internal encoders whose inputs are bounded by construction
    /// (spans, closure captures, signal-meta layout — all small-per-node).
    /// Migrate any encoder that can see user-authored sizes (a 70k-item
    /// `List` prop, a >65k-entry state seed) to [`Self::u16_len_checked`] as
    /// that migration lands.
    pub(crate) fn u16_len(&mut self, n: usize, what: &'static str) {
        match self.u16_len_checked(n, what) {
            Ok(()) => {}
            Err(_) => {
                panic!("wire encode: {what} length {n} exceeds u16 prefix width (audit H14)");
            }
        }
    }

    /// Fallible length-prefix write. Returns [`WireError::LengthExceedsU16`]
    /// when `n` does not fit in the wire's `u16` prefix, so the encode path
    /// can surface the condition as an `Error` frame instead of panicking
    /// the pipeline thread on a legitimate-but-large program (audit H14).
    ///
    /// # Errors
    ///
    /// Returns [`WireError::LengthExceedsU16`] when `n > u16::MAX`.
    pub(crate) fn u16_len_checked(
        &mut self,
        n: usize,
        what: &'static str,
    ) -> Result<(), WireError> {
        match u16::try_from(n) {
            Ok(len) => {
                self.u16(len);
                Ok(())
            }
            Err(_) => Err(WireError::LengthExceedsU16 { what, n }),
        }
    }

    /// Overrides the protocol version this writer emits (ADR-0059). Called by
    /// frame encoders that carry a source version, so re-encoding a v2-decoded
    /// frame reproduces v2 bytes.
    pub(crate) fn set_version(&mut self, version: u8) {
        self.version = version;
    }

    /// This writer's protocol version.
    pub(crate) fn version(&self) -> u8 {
        self.version
    }

    /// Writes a collection/string length prefix using the width appropriate
    /// to this writer's protocol version:
    /// * v3: `u32` (user-authored collections can exceed 65 k, ADR-0059)
    /// * v2: `u16` (the older narrow form)
    ///
    /// Unifies every user-authored-length site behind one call so the encoder
    /// sites do not branch on version — the dispatch lives here.
    pub(crate) fn count_prefix(
        &mut self,
        n: usize,
        what: &'static str,
    ) -> Result<(), WireError> {
        if self.version >= 3 {
            self.u32_len_checked(n, what)
        } else {
            self.u16_len_checked(n, what)
        }
    }

    /// Checked `u32` length-prefix write (v3 collections). Returns
    /// [`WireError::LengthExceedsU32`] on overflow — unreachable on 64-bit
    /// platforms but keeps the encode path total.
    ///
    /// # Errors
    ///
    /// [`WireError::LengthExceedsU32`] if `n > u32::MAX`.
    pub(crate) fn u32_len_checked(
        &mut self,
        n: usize,
        what: &'static str,
    ) -> Result<(), WireError> {
        match u32::try_from(n) {
            Ok(len) => {
                self.u32(len);
                Ok(())
            }
            Err(_) => Err(WireError::LengthExceedsU32 { what, n }),
        }
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
    /// Wire protocol version of the frame this reader is decoding. Used by
    /// version-conditional length-prefix reads (ADR-0059): v3 uses `u32`
    /// where v2 used `u16`. Set once at construction from the frame header's
    /// version byte; every nested `decode_*` shares the same reader, so the
    /// version threads through the entire decode tree without a per-call arg.
    version: u8,
}

impl<'a> Reader<'a> {
    /// Reader for a frame at the current protocol version. Prefer
    /// [`Self::with_version`] when the caller has already read the frame
    /// header's version byte.
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            pos: 0,
            version: crate::frame::PROTOCOL_VERSION,
        }
    }

    /// Reader for a frame at the given protocol version (from
    /// `read_frame_type`). Every length-prefix read in this cursor consults
    /// `version` to pick u16-vs-u32 semantics.
    pub(crate) fn with_version(bytes: &'a [u8], version: u8) -> Self {
        Self {
            bytes,
            pos: 0,
            version,
        }
    }

    /// The protocol version this reader is decoding for. Used by
    /// version-conditional helpers (`u16_or_u32` etc.) elsewhere in the wire
    /// codec.
    pub(crate) fn protocol_version(&self) -> u8 {
        self.version
    }

    /// Reads a collection length prefix. In v3 the prefix is `u32`; in v2 it
    /// is `u16` (widened for internal use). Either way the caller gets a
    /// `usize` and never has to think about the version.
    pub(crate) fn count(&mut self, context: &'static str) -> Result<u32, WireError> {
        if self.version >= 3 {
            self.u32(context)
        } else {
            Ok(u32::from(self.u16(context)?))
        }
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
