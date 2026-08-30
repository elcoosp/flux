package dev.flux.host.wire

import dev.flux.host.FluxError
import dev.flux.host.FluxErrorExcerpt as HostExcerpt
import dev.flux.host.FluxErrorKind
import dev.flux.host.SourceSpan

/**
 * Binary frame parser for the Flux wire protocol (Appendix D).
 *
 * Decodes the shared `FLUX` header (§D.1), the node tree (§D.3), values (§D.5),
 * patches (§D.2), string entries (§D.9), and state delta (§D.10). The result is
 * an immutable [FluxFrame] that the runtime applies to the [dev.flux.host.shadow.ShadowTree].
 *
 * Decoding is total: a malformed buffer raises [WireError] rather than
 * producing a half-built tree, so the host can show a red error overlay instead
 * of crashing (Appendix E §E.6).
 *
 * The 6-byte header is magic(4) | version(1) | kind(1); the remaining layout
 * depends on kind (Appendix D §D.1):
 * - FRAME_INIT (0x02): seq, root, a u32 count of extraNodes, the signal
 *   state_seed, the source_map, the string_count (u32) + string table, then the
 *   handler (closure) section.
 * - FRAME_DELTA (0x04): seq, flags, patch_count, handler_count, string_count
 *   (all u16), then the patch stream, the string delta, the handler section.
 */
public object FrameDeserializer {
    /** Little-endian magic FLUX (Appendix D §D.1). */
    public const val MAGIC: UInt = 0x465C5558u

    /** The wire protocol version this host implements. A frame whose version
     * byte differs is rejected fail-closed (FLUX-050 / ADR-0056): an old host
     * must never silently mis-decode a newer server's frames, and a new host
     * must never accept an older server's incompatible layout. The mismatch
     * surfaces as a red banner, never a crash. */
    public const val PROTOCOL_VERSION: UByte = 0x02u

    /** Protocol versions this host can decode. The wire (Rust `flux-ir-serde`)
     * always writes the ADR-0057 excerpt flag, and the current encoder emits
     * protocol version 2, but the `FrameBuilder` test helper and committed
     * fixtures exercise both v1 and v2, so both are accepted; any other version
     * is rejected fail-closed (FLUX-050 / ADR-0056). */
    private val SUPPORTED_VERSIONS: Set<UByte> = setOf(0x01u, 0x02u)

    /** FluxFrame kind constants mirroring crates/flux-ir-serde/src/frame.rs. */
    private const val FRAME_INIT: UByte = 0x02u
    private const val FRAME_DELTA: UByte = 0x04u
    private const val FRAME_ERROR: UByte = 0x03u
    private const val FRAME_HEARTBEAT: UByte = 0x05u
    private const val FRAME_INTERN_STRING: UByte = 0x07u
    private const val FRAME_STRING_INTERNED: UByte = 0x08u

    /** Delta flag bit gating a trailing signal_meta section. */
    private const val FLAG_NODE_HAS_SIGNAL_DEPS: UByte = 0x40u

    /** Decodes [bytes] into a [FluxFrame], or raises [WireError] on a malformed frame. */
    public fun deserialize(bytes: ByteArray): FluxFrame {
        val r = ByteReader(bytes)
        val magic = r.u32().toUInt()
        if (magic != MAGIC) {
            throw WireError("bad magic 0x%08X (expected 0x%08X)".format(magic.toLong(), MAGIC.toLong()))
        }
        val version = r.u8().toUByte()
        if (version !in SUPPORTED_VERSIONS) {
            // Fail-closed handshake (FLUX-050 / ADR-0056): refuse to decode a
            // frame whose protocol version the host does not implement. An old
            // host + new server (or vice-versa) must surface an actionable red
            // banner, not a silent mis-decode / crash.
            throw WireError(
                "protocol version 0x%02X not supported by host (expected 0x%02X); " +
                    "update the host or the dev server".format(version.toInt(), PROTOCOL_VERSION.toInt()),
            )
        }
        val kind = r.u8().toUByte()
        return when (kind) {
            FRAME_INIT -> decodeInit(r, version)
            FRAME_DELTA -> decodeDelta(r, version)
            FRAME_ERROR -> decodeError(r, version)
            FRAME_HEARTBEAT, FRAME_INTERN_STRING, FRAME_STRING_INTERNED ->
                controlFrame(version, kind)
            else -> throw WireError("unknown frame kind 0x%02X".format(kind.toInt()))
        }
    }

    /** Decodes an Init (full-tree) frame (Appendix D §D.12.2). */
    private fun decodeInit(
        r: ByteReader,
        version: UByte,
    ): FluxFrame {
        val seq = r.u32().toUInt()
        val root = decodeNode(r)
        // Appendix D §D.12.2: the full tree is root followed by a u32 count of
        // descendant nodes, flat.
        val extraCount = r.u32().toUInt()
        val extraNodes = ArrayList<WireNode>(extraCount.toInt())
        repeat(extraCount.toInt()) { extraNodes.add(decodeNode(r)) }
        // signal state_seed: u16 count of (u32 signalId, value).
        val seedCount = r.u16()
        val stateDelta = ArrayList<Pair<UInt, WireValue>>(seedCount)
        repeat(seedCount) {
            val id = r.u32().toUInt()
            stateDelta.add(id to decodeValue(r))
        }
        // source_map: u16 count of (u32 fileId, u16 len + utf8 path).
        val smCount = r.u16()
        repeat(smCount) {
            r.u32() // fileId
            val len = r.u16()
            r.utf8(len) // path (consumed; not modeled on the Android FluxFrame)
        }
        // string_count is a u32 (Appendix D §D.12.2). These are LITERAL strings
        // only (text props, etc.); component-name interning lives in its own
        // section below so the two id spaces never collide on the wire.
        val strCount = r.u32().toUInt()
        val strings = ArrayList<StringEntry>(strCount.toInt())
        repeat(strCount.toInt()) { strings.add(decodeStringEntry(r)) }
        // Appendix D §D.9: component-name interning, a SEPARATE `u16` count then
        // `(u32 ComponentId, utf8 name)` pairs. These bind each node's
        // `componentId` to its adapter name ("Text", "Column", ...). They are
        // NOT string literals and MUST NOT be fed to the string resolver; the
        // registry consumes them via `componentNames`.
        val componentCount = r.u16()
        val componentNames = ArrayList<StringEntry>(componentCount)
        repeat(componentCount) {
            val cid = r.u32().toUInt()
            val nameLen = r.u16()
            val name = r.utf8(nameLen)
            componentNames.add(StringEntry(cid, name))
        }
        // Handler (closure) section (Appendix D §D.12, Gap G1): always present
        // as a self-describing blob + HandlerDef stream.
        val (blob, handlers) = decodeHandlerSection(r, version)
        // ADR-0027 (FA-IRWIRE): optional signal_meta section, gated by a 1-byte
        // presence marker (a `0` marker means no dynamic nodes this frame).
        var signalMeta = emptyMap<UInt, NodeSignalMeta>()
        if (r.has(1)) {
            val marker = r.u8()
            if (marker != 0) signalMeta = decodeSignalMetaSection(r, version)
        }
        return FluxFrame(
            version = version,
            seq = seq,
            fullTree = true,
            patches = emptyList(),
            root = root,
            strings = strings,
            componentNames = componentNames,
            stateDelta = stateDelta,
            handlers = handlers,
            bytecodeBlob = blob,
            extraNodes = extraNodes,
            signalMeta = signalMeta,
        )
    }

    /** Decodes a Delta (patch) frame (Appendix D §D.1 + §D.2). */
    private fun decodeDelta(
        r: ByteReader,
        version: UByte,
    ): FluxFrame {
        val seq = r.u32().toUInt()
        val flags = r.u8()
        val patchCount = r.u16()
        val handlerCount = r.u16()
        val strCount = r.u16()
        val patches = ArrayList<Patch>(patchCount)
        repeat(patchCount) { patches.add(decodePatch(r)) }
        val strings = ArrayList<StringEntry>(strCount)
        repeat(strCount) { strings.add(decodeStringEntry(r)) }
        val (blob, handlers) = decodeHandlerSection(r, version)
        // ADR-0027 (FA-IRWIRE): `signal_meta` trails a Delta directly (no marker
        // byte, unlike Init) only when its `flags` carry FLAG_NODE_HAS_SIGNAL_DEPS.
        var signalMeta = emptyMap<UInt, NodeSignalMeta>()
        if ((flags and FLAG_NODE_HAS_SIGNAL_DEPS.toInt()) != 0) {
            signalMeta = decodeSignalMetaSection(r, version)
        }
        return FluxFrame(
            version = version,
            seq = seq,
            fullTree = false,
            patches = patches,
            root = null,
            strings = strings,
            stateDelta = emptyList(),
            handlers = handlers,
            bytecodeBlob = blob,
            extraNodes = emptyList(),
            signalMeta = signalMeta,
        )
    }

    /**
     * Decodes an `Error` frame (Appendix D §D.12.3) into a [FluxFrame] carrying a
     * [FluxFrame.serverError], so the executor surfaces it as a banner while the
     * last good tree stays on screen (Appendix E §E.6). The server ships a
     * what/why/how message plus an ADR-0057 excerpt (path:line:col + snippet) when
     * source is available.
     */
    private fun decodeError(r: ByteReader, version: UByte): FluxFrame {
        val seq = r.u32().toUInt()
        val msgLen = r.u16()
        val message = r.utf8(msgLen)
        val hasSpan = r.u8()
        val span: FluxSpan? =
            if (hasSpan != 0) {
                FluxSpan(r.u32().toUInt(), r.u32().toUInt(), r.u32().toUInt())
            } else {
                null
            }
        // ADR-0057: trailing server-computed excerpt (gated by `has`). The wire
        // always writes this flag (Rust `encode_closure_ref`), so read it
        // unconditionally, matching the source-of-truth encoder.
        val wireExcerpt =
            if (r.u8() != 0) {
                FluxErrorExcerpt(
                    fileId = r.u32().toUInt(),
                    byteStart = r.u32().toUInt(),
                    byteEnd = r.u32().toUInt(),
                    line = r.u16().toUShort(),
                    col = r.u16().toUShort(),
                    snippet = r.utf8(r.u16()),
                )
            } else {
                null
            }
        val fluxError =
            FluxError(
                kind = FluxErrorKind.COMPILE,
                message = message,
                span = span?.let { SourceSpan(it.fileId, it.start, it.end) },
                excerpt =
                    wireExcerpt?.let { ex ->
                        HostExcerpt(
                            path = "file ${ex.fileId}",
                            line = ex.line.toUInt(),
                            col = ex.col.toUInt(),
                            snippet = ex.snippet,
                        )
                    },
            )
        return FluxFrame(
            version = version,
            seq = seq,
            fullTree = false,
            patches = emptyList(),
            root = null,
            strings = emptyList(),
            stateDelta = emptyList(),
            handlers = emptyList(),
            bytecodeBlob = null,
            extraNodes = emptyList(),
            signalMeta = emptyMap(),
            serverError = fluxError,
            isControl = false,
        )
    }

    /** Builds a no-op [FluxFrame] for housekeeping frames that carry no tree data. */
    private fun controlFrame(version: UByte, kind: UByte): FluxFrame {
        return FluxFrame(
            version = version,
            seq = 0u,
            fullTree = false,
            patches = emptyList(),
            root = null,
            strings = emptyList(),
            stateDelta = emptyList(),
            handlers = emptyList(),
            bytecodeBlob = null,
            extraNodes = emptyList(),
            signalMeta = emptyMap(),
            serverError = null,
            isControl = true,
        )
    }

    /** Decodes the handler (closure) section (Appendix D §D.12, Gap G1). */
    private fun decodeHandlerSection(r: ByteReader, version: UByte): Pair<BytecodeBlob, List<HandlerDef>> {
        val blob = decodeBytecodeBlob(r)
        val defs = ArrayList<HandlerDef>(0)
        val count = r.u16()
        repeat(count) { defs.add(decodeHandlerDef(r, version)) }
        return blob to defs
    }

    private fun decodePatch(r: ByteReader): Patch {
        val tag = r.u8().toUByte()
        return when (tag.toInt()) {
            0x01 ->
                Patch( // Replace: u32 id, Node
                    tag,
                    id = r.u32().toUInt(),
                    parentId = 0u,
                    index = 0u,
                    node = decodeNode(r),
                    diff = null,
                    keyCount = 0u,
                    keys = emptyList(),
                    closure = null,
                )
            0x02 -> { // Update: u32 id, PropDiff
                val id = r.u32().toUInt()
                Patch(
                    tag,
                    id = id,
                    parentId = 0u,
                    index = 0u,
                    node = null,
                    diff = decodePropDiff(r),
                    keyCount = 0u,
                    keys = emptyList(),
                    closure = null,
                )
            }
            0x03 -> { // Insert: u32 parent_id, u16 index, Node
                val parent = r.u32().toUInt()
                val index = r.u16().toUShort()
                Patch(
                    tag,
                    id = 0u,
                    parentId = parent,
                    index = index,
                    node = decodeNode(r),
                    diff = null,
                    keyCount = 0u,
                    keys = emptyList(),
                    closure = null,
                )
            }
            0x04 ->
                Patch( // Remove: u32 id
                    tag,
                    id = r.u32().toUInt(),
                    parentId = 0u,
                    index = 0u,
                    node = null,
                    diff = null,
                    keyCount = 0u,
                    keys = emptyList(),
                    closure = null,
                )
            0x05 -> { // Reorder: u32 parent_id, u16 key_count, [u32; key_count]
                val parent = r.u32().toUInt()
                val keyCount = r.u16().toUShort()
                val keys = ArrayList<UInt>(keyCount.toInt())
                repeat(keyCount.toInt()) { keys.add(r.u32().toUInt()) }
                Patch(
                    tag,
                    id = 0u,
                    parentId = parent,
                    index = 0u,
                    node = null,
                    diff = null,
                    keyCount = keyCount,
                    keys = keys,
                    closure = null,
                )
            }
            0x06 -> { // Handler: u32 id, ClosureRef
                val id = r.u32().toUInt()
                Patch(
                    tag,
                    id = id,
                    parentId = 0u,
                    index = 0u,
                    node = null,
                    diff = null,
                    keyCount = 0u,
                    keys = emptyList(),
                    closure = decodeClosureRef(r),
                )
            }
            0x07 -> { // Reattach: u32 old_id, u32 new_id, Node
                val old = r.u32().toUInt()
                val new = r.u32().toUInt()
                Patch(
                    tag,
                    id = new,
                    parentId = 0u,
                    index = 0u,
                    node = decodeNode(r),
                    diff = null,
                    keyCount = 0u,
                    keys = emptyList(),
                    closure = null,
                    oldId = old,
                    newId = new,
                )
            }
            else -> throw WireError("unknown patch tag ${tag.toInt()} at offset ${r.position}")
        }
    }

    private fun decodePropDiff(r: ByteReader): PropDiff {
        val changeCount = r.u16()
        val changes = ArrayList<Pair<UShort, WireValue>>(changeCount)
        repeat(changeCount) {
            val idx = r.u16().toUShort()
            changes.add(idx to decodeValue(r))
        }
        val removalCount = r.u16()
        val removals = ArrayList<UShort>(removalCount)
        repeat(removalCount) { removals.add(r.u16().toUShort()) }
        return PropDiff(changes, removals)
    }

    private fun decodeClosureRef(r: ByteReader): ClosureRef {
        val hash = r.bytes(8)
        val offset = r.u32().toUInt()
        val len = r.u16().toUShort()
        val signalCount = r.u16()
        val signals = ArrayList<UInt>(signalCount)
        repeat(signalCount) { signals.add(r.u32().toUInt()) }
        // Appendix D §D.7: trailing source span (file_id, start, end).
        val span = FluxSpan(r.u32().toUInt(), r.u32().toUInt(), r.u32().toUInt())
        // ADR-0057: trailing server-computed excerpt (gated by `has`). The wire
        // always writes this flag (Rust `encode_closure_ref`), so it is read
        // unconditionally, matching the source-of-truth encoder.
        val excerpt =
            if (r.u8() != 0) {
                FluxErrorExcerpt(
                    fileId = r.u32().toUInt(),
                    byteStart = r.u32().toUInt(),
                    byteEnd = r.u32().toUInt(),
                    line = r.u16().toUShort(),
                    col = r.u16().toUShort(),
                    snippet = r.utf8(r.u16()),
                )
            } else {
                null
            }
        return ClosureRef(hash, offset, len, signals, span, excerpt)
    }

    /** Decodes the ADR-0027 (FA-IRWIRE) signal_meta section (Appendix D §T13). */
    private fun decodeSignalMetaSection(r: ByteReader, version: UByte): Map<UInt, NodeSignalMeta> {
        val count = r.u16()
        val out = LinkedHashMap<UInt, NodeSignalMeta>(count)
        repeat(count) {
            val nodeId = r.u32().toUInt()
            val depCount = r.u16()
            val deps = ArrayList<UInt>(depCount)
            repeat(depCount) { deps.add(r.u32().toUInt()) }
            val thunkPresent = r.u8()
            val thunk: ClosureRef? = if (thunkPresent != 0) decodeClosureRef(r) else null
            val layoutCount = r.u16()
            val layout = ArrayList<UShort>(layoutCount)
            repeat(layoutCount) { layout.add(r.u16().toUShort()) }
            // FLUX-072 / ADR-0050: trailing per-ForEach `item_slot` (u8 present + u32 id).
            val itemSlotPresent = r.u8()
            val itemSlot: UInt? = if (itemSlotPresent != 0) r.u32().toUInt() else null
            out[nodeId] = NodeSignalMeta(deps, thunk, layout, itemSlot)
        }
        return out
    }

    /** Decodes the shared handler-bytecode blob (Appendix D §D.12) as a zero-copy window over the frame buffer. */
    private fun decodeBytecodeBlob(r: ByteReader): BytecodeBlob {
        val len = r.u32().toInt()
        val offset = r.position
        r.bytes(len) // advance past the blob (no copy; the window references `r.data`)
        return BytecodeBlob(r.data, offset, len)
    }

    /** Decodes one `HandlerDef` (Appendix D §D.8): a `HandlerId` plus its `ClosureRef`. */
    private fun decodeHandlerDef(r: ByteReader, version: UByte): HandlerDef {
        val id = r.u32().toUInt()
        val closure = decodeClosureRef(r)
        return HandlerDef(id, closure)
    }

    private fun decodeStringEntry(r: ByteReader): StringEntry {
        val id = r.u32().toUInt()
        val len = r.u16()
        val text = r.utf8(len)
        return StringEntry(id, text)
    }

    private fun decodeNode(r: ByteReader): WireNode {
        val id = r.u32().toUInt()
        val kindByte = r.u8()
        // Bit 0x20 of the kind byte carries the `@pure` flag (Appendix D §D.4);
        // the low bits are the wire NodeKind tag.
        val isPure = (kindByte and 0x20) != 0
        // The wire kind byte is the NodeKind enum (0 = component, 1 = primitive);
        // we keep it only as a fallback tag. Real resolution is by `componentId`
        // against the synced string table (see AdapterRegistry / ShadowTree),
        // which maps the id to the component name ("Text", "Column", ...).
        val kind = (kindByte and 0x1F).toString()
        val componentId = r.u32().toUInt()
        val propCount = r.u16()
        val props = ArrayList<Pair<UShort, WireValue>>(propCount)
        repeat(propCount) {
            val idx = r.u16().toUShort()
            props.add(idx to decodeValue(r))
        }
        val childCount = r.u16()
        val children = ArrayList<WireChild>(childCount)
        repeat(childCount) { children.add(decodeChild(r)) }
        val handlerCount = r.u16()
        val handlerIds = ArrayList<UInt>(handlerCount)
        repeat(handlerCount) { handlerIds.add(r.u32().toUInt()) }
        val spanFile = r.u32().toUInt()
        val spanStart = r.u32().toUInt()
        val spanEnd = r.u32().toUInt()
        return WireNode(id, kind, componentId, props, children, handlerIds, isPure, spanFile, spanStart, spanEnd)
    }

    private fun decodeChild(r: ByteReader): WireChild =
        when (val tag = r.u8()) {
            0x01 -> WireChild.Node(r.u32().toUInt())
            0x02 -> {
                val itemCount = r.u16()
                val items = ArrayList<Pair<ULong, UInt>>(itemCount)
                repeat(itemCount) {
                    val key = r.u32().toULong() // u64 key (low 32 read)
                    r.u32() // high 32 of key (disjoint in MLP host: key is u32)
                    items.add(key to r.u32().toUInt())
                }
                WireChild.Splice(items)
            }
            else -> throw WireError("unknown child tag $tag at offset ${r.position}")
        }

    private fun decodeValue(r: ByteReader): WireValue =
        when (val tag = r.u8()) {
            0x00 -> WireValue.Null
            0x01 -> WireValue.IntVal(r.i64())
            0x02 -> WireValue.FloatVal(r.f64())
            0x03 -> WireValue.BoolVal(r.u8() != 0)
            0x04 -> WireValue.StrVal(r.u32().toUInt())
            0x05 -> WireValue.HandlerRefVal(r.u32().toUInt())
            0x06 -> {
                val count = r.u16()
                val items = ArrayList<WireValue>(count)
                repeat(count) { items.add(decodeValue(r)) }
                WireValue.ListVal(items)
            }
            0x07 -> {
                val count = r.u16()
                val fields = ArrayList<WireValue.RecordVal.Field>(count)
                repeat(count) {
                    val idx = r.u16().toUShort()
                    fields.add(WireValue.RecordVal.Field(idx, decodeValue(r)))
                }
                WireValue.RecordVal(fields)
            }
            else -> throw WireError("unknown value tag $tag at offset ${r.position}")
        }
}
