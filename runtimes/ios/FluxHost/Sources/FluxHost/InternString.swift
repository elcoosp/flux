//  InternString.swift
//  Host-side `InternString` / `StringInterned` wire-frame constants and the
//  canonical-id ceiling guard (Appendix D §D.12.6 / §D.12.7, brittleness 4c).
//
//  Under the current local-interning model (§AGENTS.md §3.8 + ADR-0027 T14), the
//  host interns freshly-derived strings (STR_CONCAT results, TO_STRING renders,
//  native event payloads) synchronously into the shared
//  `MaterializationStringTable` — no `InternString` RPC round-trip to the dev
//  server. The wire frame types and constants below are retained so the host can
//  recognize and drop any `StringInterned` reply a legacy/connected server might
//  still emit (it is a no-op under local interning, handled in
//  `FluxExecutor.handleFrame`). The canonical-id ceiling guard
//  (`assertCanonicalStringId`) remains active: host-local derived ids are `>=
//  0xC000_0000` and must never be placed into a `FluxValue.str` the host
//  publishes on the Init/Delta path — only server-seeded ids cross the wire.

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
/// this` into a `FluxValue.str` it publishes on the Init/Delta path, since
/// anything at or above is a host-local derived id (see `MaterializationStringTable`)
/// that must never cross the wire (AGENTS.md §3.8 — canonicality is absolute).
let stringIdCanonicalCeiling: UInt32 = 0x8000_0000

/// Asserts `id` is a canonical wire id (`< stringIdCanonicalCeiling`).
///
/// Ids at/above the ceiling are host-side synthetic fallbacks that must never
/// be emitted on the Init/Delta path. A server `InternString` reply is
/// server-assigned and must always be canonical; if it is not, the emit path
/// has a bug and we fail loud (FLUX-084) rather than silently placing a
/// non-canonical id where the VM/adapter expects a canonical one.
///
/// - Parameter id: the id to validate.
/// - Throws: `StringIdCeilingError` when `id >= stringIdCanonicalCeiling`.
func assertCanonicalStringId(_ id: UInt32) throws {
    guard id < stringIdCanonicalCeiling else {
        throw StringIdCeilingError(id: id)
    }
}

/// Error raised by `assertCanonicalStringId` when a wire path would emit a
/// `>= stringIdCanonicalCeiling` id (FLUX-084).
struct StringIdCeilingError: LocalizedError {
    /// The offending id.
    let id: UInt32
    var errorDescription: String? {
        String(
            format: "canonical string id 0x%08X must be below ceiling 0x%08X; a >=ceiling id is a synthetic fallback that must never be emitted",
            id,
            stringIdCanonicalCeiling
        )
    }
}

/// Builds the wire bytes of an `InternString` request frame (Appendix D §D.12.6).
///
/// Layout (after the shared `magic(4) | version(1) | kind(1)` header):
/// `len(u16 LE) | bytes(len UTF-8)`. The host sends raw UTF-8 for the string it
/// needs a canonical id for and awaits a `StringInternedFrame` carrying the
/// server-assigned id.
///
/// Under the current local-interning model this is **not called** by the host
/// (strings are interned into the shared table directly); the helper is retained
/// for wire-compatibility and test coverage of the frame encoding.
///
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
///
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
