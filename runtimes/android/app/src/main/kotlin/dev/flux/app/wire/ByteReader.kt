package dev.flux.app.wire

/**
 * A little-endian bit reader over a [ByteArray], the shared primitive for the
 * [FrameDeserializer]. All multi-byte integers in the wire protocol are
 * little-endian (Appendix D §D.1). Bounds-checked: every read throws
 * [WireError] past the buffer end so the caller can surface a red error overlay
 * rather than panic.
 */
public class ByteReader(
    private val data: ByteArray,
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
