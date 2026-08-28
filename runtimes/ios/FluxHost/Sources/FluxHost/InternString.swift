//  InternString.swift
//  Host-side `InternString` / `StringInterned` wire frames (Appendix D §D.12.6 /
//  §D.12.7, brittleness 4c) plus the async interning abstraction the VM uses to
//  retire the local high-range `synthetic_str_id` fallback.
//
//  Previously, when the VM needed to intern a freshly-derived string (a
//  `STR_CONCAT` result or a `TO_STRING` rendering) it generated a synthetic id
//  in the high half (`>= 0x8000_0000`) of the id space — a `StringTable` that
//  never consulted the dev server. Those ids were non-canonical: the same text
//  interned on the server (which owns the authoritative `StringTable`, Appendix
//  C) would get a different, low id, so a host-derived string could never match
//  a server-interned one. `InternString`/`StringInterned` removes that
//  divergence: the host asks the server for the canonical id and caches it
//  locally, so every id that flows across the wire is `< STRING_ID_CANONICAL_CEILING`.

import Foundation

/// The `MAGIC` header shared by every Flux wire frame (Appendix D §D.1).
/// `0x465C5558` little-endian → bytes `58 55 5C 46` ("FLUX").
let fluxWireMagic: UInt32 = 0x465C_5558

/// Frame-kind byte for the `InternString` request (Host → Server, brittleness 4c).
let frameKindInternString: UInt8 = 0x07

/// Frame-kind byte for the `StringInterned` response (Server → Host, 4c).
let frameKindStringInterned: UInt8 = 0x08

/// Bit ceiling for canonical string ids.
///
/// Ids below this bit are assigned by the server's string table (Appendix D
/// §D.9) and are stable across edits. The host must only ever place ids `<
/// this` into a `FluxValue.str` it publishes, since anything at or above silently
/// bypasses interning and reintroduces the brittleness 4c was raised to remove.
let stringIdCanonicalCeiling: UInt32 = 0x8000_0000

/// Builds the wire bytes of an `InternString` request frame (Appendix D §D.12.6).
///
/// Layout (after the shared `magic(4) | version(1) | kind(1)` header):
/// `len(u16 LE) | bytes(len UTF-8)`. The host sends raw UTF-8 for the string it
/// needs a canonical id for and awaits a `StringInternedFrame` carrying the
/// server-assigned id.
/// - Parameter text: the string to intern. Must be valid UTF-8 (the VM only ever
///   interns concrete Swift `String`s, which are valid UTF-8 by construction).
/// - Returns: the frame bytes to send over the transport.
func internStringFrameBytes(_ text: String) -> Data {
    let payload = Data(text.utf8)
    var data = Data()
    data.append(0x58); data.append(0x55); data.append(0x5C); data.append(0x46) // MAGIC
    data.append(1) // protocol version
    data.append(frameKindInternString)
    let len = UInt16(payload.count)
    data.append(UInt8(len & 0xFF))
    data.append(UInt8((len >> 8) & 0xFF))
    data.append(payload)
    return data
}

/// Decodes a `StringInterned` response frame (Appendix D §D.12.7).
///
/// Layout (after the shared header): `id(u32 LE)`. Returns `nil` on a
/// malformed/short buffer or a non-matching frame kind, so a corrupt response is
/// treated as "no canonical id" rather than crashing the decode.
/// - Parameter bytes: the raw frame bytes received from the server.
/// - Returns: the server-assigned canonical string id, or `nil`.
func decodeStringInternedFrame(_ bytes: [UInt8]) -> UInt32? {
    guard bytes.count >= 10 else { return nil } // magic(4)+ver(1)+kind(1)+id(4)
    guard bytes[0] == 0x58, bytes[1] == 0x55, bytes[2] == 0x5C, bytes[3] == 0x46 else { return nil }
    guard bytes[4] == 1 else { return nil }
    guard bytes[5] == frameKindStringInterned else { return nil }
    let id = UInt32(bytes[6]) | (UInt32(bytes[7]) << 8) | (UInt32(bytes[8]) << 16) | (UInt32(bytes[9]) << 24)
    return id
}

/// An asynchronous string interner the VM consults whenever it must publish a
/// freshly-derived string (a `STR_CONCAT` result or a `TO_STRING` rendering).
///
/// This replaces the synchronous, high-range `synthetic_str_id` fallback the VM
/// used to generate locally. The production implementation (`InternStringClient`)
/// sends an `InternString` frame over the host transport and awaits the server's
/// `StringInterned` reply, caching the canonical id so repeated strings reuse the
/// same low id the server assigned. A `NoOpStringInterner` provides the offline
/// (conformance-vector / unit-test) behaviour where interning is not exercised.
@MainActor
public protocol AnyStringInterner: AnyObject {
    /// Resolves `text` to its canonical `StringId`, interning it on first sight.
    ///
    /// Implementations must return an id `< stringIdCanonicalCeiling`; a synthetic
    /// high-range id is never acceptable. The call is `async` because the canonical
    /// id is produced by the dev server, so the VM pauses its evaluation until the
    /// reply arrives (the dispatch runs off the UI thread; see `FluxExecutor`).
    func intern(_ text: String) async -> UInt32
}

/// Offline interner used when no live transport is attached (the ISA conformance
/// vectors and the bulk of the unit suite). Interning is a no-op that yields id 0
/// (Appendix E §E.1: offline evaluation never publishes derived strings, so this
/// mirrors the prior `EmptyStringTable` behaviour and keeps the conformance
/// vectors deterministic).
@MainActor
public final class NoOpStringInterner: AnyStringInterner {
    public init() {}
    public func intern(_ text: String) async -> UInt32 { 0 }
}
