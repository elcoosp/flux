package dev.flux.host.wire

/**
 * Builds the `Hello` handshake frame the dev server requires before it replies
 * with an `Init` (Appendix D §D.12.1). The byte layout matches
 * `flux_ir_serde::HelloFrame::to_bytes` exactly:
 *
 * ```
 * MAGIC(u32 LE = 0x465C5558) | version(u8 = 1) | kind(u8 = 0x01)
 *   | platform(u16 len + utf8) | device(u16 len + utf8)
 *   | cap_count(u16) [cap triples…]
 * ```
 *
 * Sending this on socket open is what lets the server answer with the full tree;
 * without it the connection hangs at "connecting" forever (the server only
 * fans out `Init` after a valid `Hello`).
 *
 * @property platform host platform string, e.g. `"android"`.
 * @property device device model string, e.g. `"Pixel 5"`.
 * @return the wire bytes of the `Hello` frame.
 */
public fun helloFrameBytes(
    platform: String,
    device: String,
): ByteArray {
    val out = ArrayList<Byte>()
    // MAGIC "FLUX" little-endian (u32 = 0x465C5558): 0x58 0x55 0x5C 0x46.
    out.add(0x58.toByte())
    out.add(0x55.toByte())
    out.add(0x5C.toByte())
    out.add(0x46.toByte())
    out.add(1) // protocol version
    out.add(0x01) // FrameKind::Hello
    writeStr(out, platform)
    writeStr(out, device)
    out.add(0) // cap_count low
    out.add(0) // cap_count high
    return out.toByteArray()
}

private fun writeStr(
    out: ArrayList<Byte>,
    s: String,
) {
    val bytes = s.toByteArray(Charsets.UTF_8)
    out.add((bytes.size and 0xFF).toByte())
    out.add(((bytes.size ushr 8) and 0xFF).toByte())
    out.addAll(bytes.toList())
}
