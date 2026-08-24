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
}

/// A forward-only, little-endian reader over an immutable byte buffer.
struct ByteReader {
    private let data: [UInt8]
    private(set) var offset: Int

    /// Creates a reader over `bytes`, starting at offset 0.
    init(_ bytes: [UInt8]) {
        self.data = bytes
        self.offset = 0
    }

    /// `true` once the cursor has consumed the whole buffer.
    var isAtEnd: Bool { offset >= data.count }

    /// Number of bytes still available to read.
    var remaining: Int { max(0, data.count - offset) }

    private mutating func take(_ count: Int) throws -> [UInt8] {
        let end = offset + count
        guard end <= data.count else {
            throw WireError.unexpectedEnd(offset: offset, needed: count, available: remaining)
        }
        let slice = Array(data[offset..<end])
        offset = end
        return slice
    }

    /// Reads a single unsigned byte.
    mutating func u8() throws -> UInt8 {
        try take(1)[0]
    }

    /// Reads a little-endian `UInt16` from exactly two bytes.
    mutating func u16() throws -> UInt16 {
        let b = try take(2)
        return UInt16(b[0]) | (UInt16(b[1]) << 8)
    }

    /// Reads a little-endian `UInt32` from exactly four bytes.
    mutating func u32() throws -> UInt32 {
        let b = try take(4)
        return UInt32(b[0]) | (UInt32(b[1]) << 8) | (UInt32(b[2]) << 16) | (UInt32(b[3]) << 24)
    }

    /// Reads a little-endian `UInt64` from exactly eight bytes.
    mutating func u64() throws -> UInt64 {
        let b = try take(8)
        var value: UInt64 = 0
        for i in 0..<8 { value |= UInt64(b[i]) << (8 * i) }
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
}
