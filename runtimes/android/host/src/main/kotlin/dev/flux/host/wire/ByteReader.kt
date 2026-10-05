package dev.flux.host.wire

/**
 * A little-endian bit reader over a [ByteArray], the shared primitive for the
 * [FrameDeserializer]. All multi-byte integers in the wire protocol are
 * little-endian (Appendix D §D.1). Bounds-checked: every read throws
 * [WireError] past the buffer end so the caller can surface a red error overlay
 * rather than panic.
 */
public class ByteReader(
    internal val data: ByteArray,
    private var pos: Int = 0,
) {
    /** Current read position. */
    public val position: Int get() = pos

    /** Remaining unread bytes. */
    public val remaining: Int get() = data.size - pos

    /** True when at least [n] bytes remain. */
    public fun has(n: Int): Boolean = remaining >= n

    /** Reads a single unsigned byte. */
    public fun u8(): Int {
        require(1)
        return data[pos++].toInt() and 0xFF
    }

    /** Reads a `u16` little-endian. */
    public fun u16(): Int {
        val lo = u8()
        val hi = u8()
        return lo or (hi shl 8)
    }

    /** Reads a `u32` little-endian. */
    public fun u32(): Long {
        val b0 = u8().toLong()
        val b1 = u8().toLong()
        val b2 = u8().toLong()
        val b3 = u8().toLong()
        return b0 or (b1 shl 8) or (b2 shl 16) or (b3 shl 24)
    }

    /** Reads an `i32` little-endian (sign-extended from u32). */
    public fun i32(): Int = u32().toInt()

    /** Reads an `i64` little-endian. */
    public fun i64(): Long {
        var v = 0L
        repeat(8) { v = v or (u8().toLong() shl (8 * it)) }
        return v
    }

    /** Reads an `f64` little-endian. */
    public fun f64(): Double = Double.fromBits(i64())

    /** Reads exactly [n] bytes. */
    public fun bytes(n: Int): ByteArray {
        require(n)
        val out = data.copyOfRange(pos, pos + n)
        pos += n
        return out
    }

    /** Reads a UTF-8 string of [len] bytes. */
    public fun utf8(len: Int): String = String(bytes(len), Charsets.UTF_8)

    private fun require(n: Int) {
        if (remaining < n) throw WireError("unexpected end of frame: need $n bytes at offset $pos, have $remaining")
    }

    /** Wire protocol version of the frame currently being decoded. Set by
     * [FrameDeserializer.deserialize] immediately after the version byte is
     * validated, before any length-prefixed section is read. [count] dispatches
     * on this value: v2 reads a widened `u16`, v3 reads a raw `u32` (ADR-0059).
     * The default sentinel `0u` makes a missing [setVersion] surface as a
     * `WireError` from [count] instead of silently picking the wrong layout. */
    private var version: UByte = 0u

    /** Records the wire protocol version for subsequent [count] reads. Called
     * by [FrameDeserializer.deserialize] after the version byte is validated. */
    internal fun setVersion(v: UByte) {
        version = v
    }

    /** Reads a length-prefixed count using the layout selected by [version].
     * On v2 the count is a little-endian `u16`; on v3 it is a `u32` (ADR-0059).
     * [context] names the field for diagnostics. A v3 count above
     * `Int.MAX_VALUE` is rejected fail-closed rather than truncated to a
     * negative value. */
    internal fun count(context: String): Int =
        when (version.toInt()) {
            2 -> u16()
            3 -> {
                val raw = u32()
                if (raw > Int.MAX_VALUE.toLong()) {
                    throw WireError("count '$context' exceeds Int.MAX_VALUE: $raw")
                }
                raw.toInt()
            }
            else -> throw WireError("count('$context') called before setVersion (version=$version)")
        }
}

/**
 * Raised when a wire frame cannot be decoded.
 *
 * Carries the position where decoding failed so the host can show a source span
 * if available, or a concise red banner otherwise (Appendix E §E.6 error frame).
 */
public class WireError(
    message: String,
) : Exception(message)
