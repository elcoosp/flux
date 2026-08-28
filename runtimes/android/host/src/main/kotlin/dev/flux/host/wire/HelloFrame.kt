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
    // cap_count (u16 LE) + capability triples (name, version u32, features).
    val caps = advertisedCapabilities
    out.add((caps.size and 0xFF).toByte())
    out.add(((caps.size ushr 8) and 0xFF).toByte())
    for ((name, version, features) in caps) {
        writeStr(out, name)
        out.add((version.toInt() and 0xFF).toByte())
        out.add(((version.toInt() ushr 8) and 0xFF).toByte())
        out.add(((version.toInt() ushr 16) and 0xFF).toByte())
        out.add(((version.toInt() ushr 24) and 0xFF).toByte())
        out.add((features.size and 0xFF).toByte())
        out.add(((features.size ushr 8) and 0xFF).toByte())
        for (feature in features) writeStr(out, feature)
    }
    return out.toByteArray()
}

// ===== GENERATED-BEGIN (derived from flux-devserver capability_idl; do not edit) =====
private val idlCapabilities: List<Triple<String, UInt, List<Pair<String, UInt>>>> = listOf(
    Triple("Camera", 1u, listOf(
        "take" to 1u,
        "startPreview" to 2u,
        "stopPreview" to 3u,
    )),
    Triple("Storage", 2u, listOf(
        "set" to 1u,
        "get" to 2u,
        "delete" to 3u,
    )),
    Triple("Router", 3u, listOf(
        "navigate" to 1u,
    )),
    Triple("Clipboard", 4u, listOf(
        "set" to 1u,
        "get" to 2u,
    )),
    Triple("Geolocation", 5u, listOf(
        "get" to 1u,
    )),
)
// ===== GENERATED-END =====

/**
 * The capabilities this host build advertises (Appendix D §D.12.1, §24.4),
 * as `(name, version, features)` triples. The dev server validates the set
 * against the compiled `.flux` requirements; a mismatch is a clear `Error`
 * frame rather than a silent runtime fault. The ids/names here are generated
 * from the framework's capability IDL and match the native `CapabilityRegistry`
 * table and `stdlib/capabilities.flux`.
 */
public val advertisedCapabilities: List<Triple<String, UInt, List<String>>> =
    idlCapabilities.map { (name, version, methods) -> Triple(name, version, methods.map { it.first }) }

private fun writeStr(
    out: ArrayList<Byte>,
    s: String,
) {
    val bytes = s.toByteArray(Charsets.UTF_8)
    out.add((bytes.size and 0xFF).toByte())
    out.add(((bytes.size ushr 8) and 0xFF).toByte())
    out.addAll(bytes.toList())
}
