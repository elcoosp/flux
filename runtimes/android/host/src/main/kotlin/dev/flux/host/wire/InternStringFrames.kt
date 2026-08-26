package dev.flux.host.wire

/**
 * The two wire frames of the dynamic string-interning RPC (brittleness 4d,
 * Appendix D §D.12.6 / §D.12.7, mirroring `flux-ir-serde`'s
 * `InternStringFrame` / `StringInternedFrame`).
 *
 * When the host needs a canonical id for a string the wire string table did not
 * carry (e.g. a `TO_STRING`/`STR_CONCAT` render produced at runtime, or a
 * native event whose text was never shipped), it **must not** synthesize a
 * hash-based id locally. It suspends and sends [internStringFrameBytes] to the
 * dev server; the server interns the text into its authoritative `StringTable`
 * and replies with [stringInternedId], which is always `<
 * [STRING_ID_CANONICAL_CEILING]` and therefore safe to put on the wire. This
 * retires the host-side hash-based synthetic fallback (brittleness 4d).
 *
 * The `InternString` request carries a `u16` length-prefixed UTF-8 payload. The
 * `StringInterned` response carries a single `u32` canonical id. Both share the
 * `magic(4) version(1) frame_type(1)` header the other frames use.
 *
 * The wire constants below: `FRAME_INTERN_STRING` (0x07) is the `InternString`
 * request `frame_type`, `FRAME_STRING_INTERNED` (0x08) the `StringInterned`
 * response `frame_type`, and `STRING_ID_CANONICAL_CEILING` (0x8000_0000) the
 * bit ceiling above which an id is **not** canonical (mirrors
 * `flux-ir-serde::STRING_ID_CANONICAL_CEILING`).
 */
internal const val FRAME_INTERN_STRING: UByte = 0x07u

internal const val FRAME_STRING_INTERNED: UByte = 0x08u

internal const val STRING_ID_CANONICAL_CEILING: UInt = 0x8000_0000u

/**
 * Encodes an `InternString` request frame from [text].
 *
 * @param text the UTF-8 string to intern on the dev server.
 * @return the wire bytes of the request frame.
 */
internal fun internStringFrameBytes(text: String): ByteArray {
    val out = ArrayList<Byte>()
    // Shared header: MAGIC (u32 LE) | version (u8) | frame_type (u8).
    out.add(0x58.toByte())
    out.add(0x55.toByte())
    out.add(0x5C.toByte())
    out.add(0x46.toByte())
    out.add(1) // protocol version
    out.add(FRAME_INTERN_STRING.toInt().toByte()) // 0x07
    // len (u16 LE) | bytes.
    val bytes = text.toByteArray(Charsets.UTF_8)
    out.add((bytes.size and 0xFF).toByte())
    out.add(((bytes.size ushr 8) and 0xFF).toByte())
    out.addAll(bytes.toList())
    return out.toByteArray()
}

/**
 * Decodes the canonical id from a `StringInterned` response frame.
 *
 * Returns `null` when [bytes] is not a well-formed `StringInterned` frame (wrong
 * magic/kind or truncated), so an unexpected reply is treated as a failed intern
 * rather than a crash.
 *
 * @param bytes the raw response bytes received from the dev server.
 * @return the canonical `StringId`, or `null` on a malformed frame.
 */
internal fun stringInternedId(bytes: ByteArray): UInt? {
    if (bytes.size < 10) return null
    val magic =
        (bytes[0].toLong() and 0xFF) or
            ((bytes[1].toLong() and 0xFF) shl 8) or
            ((bytes[2].toLong() and 0xFF) shl 16) or
            ((bytes[3].toLong() and 0xFF) shl 24)
    if (magic != 0x465C5558L) return null
    val kind = bytes[5].toLong() and 0xFF
    if (kind != FRAME_STRING_INTERNED.toLong()) return null
    return (
        (bytes[6].toLong() and 0xFF) or
            ((bytes[7].toLong() and 0xFF) shl 8) or
            ((bytes[8].toLong() and 0xFF) shl 16) or
            ((bytes[9].toLong() and 0xFF) shl 24)
    ).toUInt()
}
