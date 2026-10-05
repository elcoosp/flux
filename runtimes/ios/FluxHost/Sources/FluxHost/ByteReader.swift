//  ByteReader.swift
//  Little-endian cursor decoder for the Flux wire protocol (Appendix D).
//
//  Every multi-byte integer on the wire is little-endian. `ByteReader` advances
//  an offset through an immutable `[UInt8]` buffer and fails with a `WireError`
//  carrying the byte offset when a read would overrun the buffer, so frame
//  corruption is reported precisely rather than as an out-of-bounds panic.

/// Errors raised while decoding a wire frame (Appendix D).
enum WireError: Error, Equatable, Sendable {
    /// The buffer ended before `count` more bytes could be read.
    case unexpectedEnd(offset: Int, needed: Int, available: Int)
    /// A tag byte did not match any known variant of its union.
    case unknownTag(offset: Int, tag: UInt8)
    /// A frame did not begin with the `FLUX` magic.
    case badMagic(offset: Int, value: UInt32)
    /// The protocol version byte is outside the host's supported range
    /// `[protocolVersionMin, protocolVersion]` (FLUX-050 / ADR-0056). The
    /// handshake fails closed: an old host must not mis-decode a newer
    /// server's frames, nor a new host an older server's.
    case unsupportedVersion(offset: Int, actual: UInt8, expected: UInt8)
    /// The Delta frame's D.1 `handler_count` header does not match the number of
    /// `HandlerDef`s decoded from the D.12 handler section (T-316.6). A mismatch
    /// means the server and host disagree on the frame layout.
    case handlerCountMismatch(expected: Int, actual: Int)
}

/// A forward-only, little-endian reader over an immutable byte buffer.
struct ByteReader {
    private let data: [UInt8]
    private(set) var offset: Int

    /// The protocol version of the frame currently being decoded. Set by
    /// `FrameDeserializer.decode` immediately after the version byte is read
    /// and validated, before any length-prefixed section is decoded. `count()`
    /// dispatches on this value: v2 reads a widened `u16`, v3 reads a `u32`
    /// (ADR-0059). The default `0` is a sentinel so that a `count()` call
    /// before `setVersion` fails loudly instead of silently picking the wrong
    /// layout.
    private(set) var version: UInt8 = 0

    /// Creates a reader over `bytes`, starting at offset 0. The caller must
    /// invoke `setVersion` before any `count()` call.
    init(_ bytes: [UInt8]) {
        self.data = bytes
        self.offset = 0
    }

    /// Records the wire protocol version for subsequent `count()` reads.
    /// Called by `FrameDeserializer.decode` after the version byte is read.
    mutating func setVersion(_ v: UInt8) {
        self.version = v
    }

    /// `true` once the cursor has consumed the whole buffer.
    var isAtEnd: Bool { offset >= data.count }

    /// Number of bytes still available to read.
    var remaining: Int { max(0, data.count - offset) }

    /// Reads `count` raw bytes (allocates a new array — used only by `bytes`/`utf8`).
    private mutating func take(_ count: Int) throws -> [UInt8] {
        let end = offset + count
        guard end <= data.count else {
            throw WireError.unexpectedEnd(offset: offset, needed: count, available: remaining)
        }
        let slice = Array(data[offset..<end])
        offset = end
        return slice
    }

    /// Reads one byte without allocating a heap array — the hot path for every
    /// integer read. A frame decode formerly allocated a fresh `[UInt8]` per
    /// integer via `take`; reading through the buffer subscript instead removes
    /// thousands of tiny allocations on a large frame (review: ByteReader.take).
    private mutating func readByte() throws -> UInt8 {
        guard offset < data.count else {
            throw WireError.unexpectedEnd(offset: offset, needed: 1, available: remaining)
        }
        let b = data[offset]
        offset &+= 1
        return b
    }

    /// Reads a single unsigned byte.
    mutating func u8() throws -> UInt8 {
        try readByte()
    }

    /// Reads a little-endian `UInt16` from exactly two bytes.
    mutating func u16() throws -> UInt16 {
        let lo = UInt16(try readByte())
        let hi = UInt16(try readByte())
        return lo | (hi << 8)
    }

    /// Reads a little-endian `UInt32` from exactly four bytes.
    mutating func u32() throws -> UInt32 {
        let b0 = UInt32(try readByte())
        let b1 = UInt32(try readByte())
        let b2 = UInt32(try readByte())
        let b3 = UInt32(try readByte())
        return b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
    }

    /// Reads a little-endian `UInt64` from exactly eight bytes.
    mutating func u64() throws -> UInt64 {
        var value: UInt64 = 0
        for i in 0..<8 { value |= UInt64(try readByte()) << (8 * i) }
        return value
    }

    /// Reads a little-endian `Int64` from exactly eight bytes.
    mutating func i64() throws -> Int64 {
        Int64(bitPattern: try u64())
    }

    /// Reads a little-endian `Double` (IEEE-754) from exactly eight bytes.
    mutating func f64() throws -> Double {
        Double(bitPattern: try u64())
    }

    /// Reads exactly `count` raw bytes.
    mutating func bytes(_ count: Int) throws -> [UInt8] {
        try take(count)
    }

    /// Reads `count` UTF-8 bytes as a `String`, replacing invalid sequences with
    /// the Unicode replacement character rather than failing (Appendix D strings
    /// are always well-formed UTF-8, but a corrupt frame should not crash).
    mutating func utf8(_ count: Int) throws -> String {
        let raw = try bytes(count)
        return String(decoding: raw, as: UTF8.self)
    }

    /// Reads a length-prefixed count using the layout selected by the current
    /// protocol version. On v2 the count is a little-endian `UInt16` widened
    /// to `UInt32`; on v3 it is a raw `UInt32` (ADR-0059). `context` names the
    /// field for diagnostics.
    mutating func count(_ context: String) throws -> UInt32 {
        switch version {
        case 2:
            return UInt32(try u16())
        case 3:
            return try u32()
        default:
            // Unreachable in normal operation: `FrameDeserializer.decode`
            // validates the version byte and calls `setVersion` before any
            // count read. This guards against a caller forgetting the set.
            throw WireError.unsupportedVersion(offset: offset, actual: version, expected: 3)
        }
    }
}

/// Bounds-checked slice access used when slicing a handler bytecode blob, where
/// an out-of-range slice indicates a malformed frame rather than a programming
/// error.
extension Array {
    /// Returns the sub-sequence `bounds` if it lies fully within the array,
    /// otherwise `nil` (instead of trapping like `ArraySlice`).
    subscript(safe bounds: Range<Int>) -> ArraySlice<Element>? {
        guard bounds.lowerBound >= 0, bounds.upperBound <= count,
              bounds.lowerBound <= bounds.upperBound else {
            return nil
        }
        return self[bounds]
    }
}
