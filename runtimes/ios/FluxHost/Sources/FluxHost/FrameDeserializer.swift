//  FrameDeserializer.swift
//  Decodes Flux wire frames (Appendix D) into `FluxFrame`.
//
//  The decoder is a behavioral mirror of the Rust dev-server serializer: same
//  magic, same little-endian layout, same implicit `string_id -> VMValue.str`
//  rule for `Str` values (Appendix D §D.5). A corrupt or truncated frame raises
//  `WireError` with the failing byte offset rather than panicking.

/// Frame kind and flag constants mirroring the Rust wire protocol
/// (`crates/flux-ir-serde/src/frame.rs`, Appendix D §D.1).
private enum FrameKind {
    /// `Init` (full-tree) frame type byte.
    static let initByte: UInt8 = 0x02
    /// `Delta` (patch) frame type byte.
    static let deltaByte: UInt8 = 0x04
    /// `Delta` flag bit indicating a trailing `signal_meta` section.
    static let flagNodeHasSignalDeps: UInt8 = 0x40
    /// `Error` frame type byte (Appendix D §D.12.3).
    static let errorByte: UInt8 = 0x03
    /// `Heartbeat` frame type byte (Appendix D §D.12.5).
    static let heartbeatByte: UInt8 = 0x05
    /// `InternString` request frame type byte (Appendix D §D.12.6, Host → Server).
    static let internStringByte: UInt8 = 0x07
    /// `StringInterned` response frame type byte (Appendix D §D.12.7, Server → Host).
    static let stringInternedByte: UInt8 = 0x08
}

/// Decodes Flux binary frames (Appendix D) into host-side `FluxFrame` models.
enum FrameDeserializer {
    /// The 4-byte frame magic: `0x465C5558` (FLUX in little-endian).
    static let magic: UInt32 = 0x465C_5558

    /// Decodes a frame from raw bytes.
    ///
    /// The 6-byte header is `magic(4) | version(1) | kind(1)`; the remaining
    /// layout depends on `kind` (Appendix D §D.1):
    /// - `FRAME_INIT` (0x02): `seq`, `root`, a `u32` count of `extraNodes`
    ///   (the remaining descendants, flat), the signal `state_seed`, the
    ///   `source_map`, the `string_count` (u32) + string table, the handler
    ///   (closure) section, and an optional `signal_meta` marker.
    /// - `FRAME_DELTA` (0x04): `seq`, `flags`, `patch_count`, `handler_count`,
    ///   `string_count` (all u16), then the patch stream, the string delta, the
    ///   handler section, and an optional `signal_meta` (gated by
    ///   `FLAG_NODE_HAS_SIGNAL_DEPS` in `flags`).
    ///
    /// Every node in an Init frame — root and extras — is registered in `nodes`
    /// so the reconciler resolves child ids without a second round-trip.
    /// - Throws: `WireError` on malformed input, or a caller-supplied error from
    ///   the value/closure decoders.
    static func decode(_ bytes: [UInt8]) throws -> FluxFrame {
        var r = ByteReader(bytes)
        let rawMagic = try r.u32()
        guard rawMagic == magic else {
            throw WireError.badMagic(offset: 0, value: rawMagic)
        }
        let version = try r.u8()
        let kind = try r.u8()
        switch kind {
        case FrameKind.initByte:
            return try decodeInit(&r, version: version)
        case FrameKind.deltaByte:
            return try decodeDelta(&r, version: version)
        case FrameKind.errorByte:
            return try decodeError(&r, version: version)
        case FrameKind.heartbeatByte, FrameKind.internStringByte, FrameKind.stringInternedByte:
            // Housekeeping frames: no tree mutation, no reconciler action.
            return controlFrame(version: version, kind: kind)
        default:
            throw WireError.unknownTag(offset: 5, tag: kind)
        }
    }

    /// Decodes an `Init` frame (Appendix D §D.12.2).
    private static func decodeInit(_ r: inout ByteReader, version: UInt8) throws -> FluxFrame {
        // Payload begins after the 6-byte header (magic + version + kind).
        let seq = try r.u32()
        let root = try decodeNode(&r)
        // Appendix D §D.12.2: `root` is followed by a `u32` count then every
        // descendant node, flat. Register each in the node table.
        let extraCount = try r.u32()
        var nodes: [UInt32: ShadowNode] = [root.id: root]
        for _ in 0..<extraCount {
            let node = try decodeNode(&r)
            nodes[node.id] = node
        }
        // signal `state_seed`: u16 count of (u32 signalId, value).
        let seedCount = try r.u16()
        var state: [StateCell] = []
        for _ in 0..<seedCount {
            let signalId = try r.u32()
            let value = try decodeValue(&r)
            state.append(StateCell(signalId: signalId, value: value))
        }
        // `source_map`: u16 count of (u32 fileId, u16 len + utf8 path).
        let smCount = try r.u16()
        var files: [FileEntry] = []
        for _ in 0..<smCount {
            let fileId = try r.u32()
            let len = try r.u16()
            let path = try r.utf8(Int(len))
            files.append(FileEntry(fileId: fileId, path: path))
        }
        // `string_count` is a u32 (Appendix D §D.12.2). These are prop string
        // literals only — NOT component names (those follow in `component_names`).
        let strCount = try r.u32()
        var strings: [StringEntry] = []
        for _ in 0..<strCount {
            strings.append(try decodeStringEntry(&r))
        }
        // `component_names`: u16 count, then per entry `(u32 cid, u16 name_len, utf8 name)`
        // (Appendix D §D.12.2, FLUX-019 split). These bind each component id to its
        // adapter name so the reconciler can resolve primitives (Text/Column/Button).
        // Kept separate from `strings` to avoid id collisions in the string resolver.
        let compCount = try r.u16()
        var componentNames: [StringEntry] = []
        for _ in 0..<compCount {
            let cid = try r.u32()
            let nameLen = Int(try r.u16())
            let name = try r.utf8(nameLen)
            componentNames.append(StringEntry(stringId: cid, value: name))
        }
        // Handler (closure) section (Appendix D §D.12, Gap G1).
        let handlers = try decodeHandlerSection(&r)
        // ADR-0027 (FA-IRWIRE): optional `signal_meta` section, gated by a
        // 1-byte presence marker so back-compatible decoders skip it. The counter
        // ADR-0027 (FA-IRWIRE): `signal_meta` trails every `Init` frame after
        // `files`. The section is gated by a `1` marker byte; a `0` marker means
        // it is absent (no dynamic nodes), which keeps `signalMeta` empty.
        var signalMeta: [UInt32: NodeSignalMeta] = [:]
        if r.remaining > 0 {
            let marker = try r.u8()
            if marker == 1 {
                signalMeta = try decodeSignalMetaSection(&r)
            }
        }
        return FluxFrame(
            version: version,
            seq: seq,
            flags: FrameKind.initByte,
            root: root,
            nodes: nodes,
            patches: [],
            handlers: handlers,
            strings: strings,
            state: state,
            files: files,
            componentNames: componentNames,
            signalMeta: signalMeta
        )
    }

    /// Decodes a `Delta` (patch) frame (Appendix D §D.1 + §D.2).
    private static func decodeDelta(_ r: inout ByteReader, version: UInt8) throws -> FluxFrame {
        // Payload after the 6-byte header: seq, flags, then three u16 counts.
        let seq = try r.u32()
        let flags = try r.u8()
        let patchCount = try r.u16()
        let handlerCount = try r.u16()
        let strCount = try r.u16()
        var patches: [Patch] = []
        patches.reserveCapacity(Int(patchCount))
        for _ in 0..<patchCount {
            patches.append(try decodePatch(&r))
        }
        var strings: [StringEntry] = []
        strings.reserveCapacity(Int(strCount))
        for _ in 0..<strCount {
            strings.append(try decodeStringEntry(&r))
        }
        // Handler (closure) section (Appendix D §D.12, Gap G1) — present when
        // `handlerCount > 0`; `decodeHandlerSection` tolerates a zero blob.
        let handlers = try decodeHandlerSection(&r)
        // ADR-0027 (FA-IRWIRE): `signal_meta` trails a Delta directly (no marker
        // byte, unlike Init) only when its `flags` carry
        // `FLAG_NODE_HAS_SIGNAL_DEPS`; the encoder emits the section immediately
        // after the handler blob in that case (Appendix D §T13).
        var signalMeta: [UInt32: NodeSignalMeta] = [:]
        if (flags & FrameKind.flagNodeHasSignalDeps) != 0 {
            signalMeta = try decodeSignalMetaSection(&r)
        }
        return FluxFrame(
            version: version,
            seq: seq,
            flags: flags,
            root: nil,
            nodes: [:],
            patches: patches,
            handlers: handlers,
            strings: strings,
            state: [],
            files: [],
            componentNames: [],
            signalMeta: signalMeta
        )
    }

    /// Decodes an `Error` frame (Appendix D §D.12.3).
    ///
    /// The Rust encoder lays out `seq(u32) | message(u16-len UTF-8) |
    /// has_span(u8) | span?`; a `span` is present only when `has_span != 0`.
    /// There is deliberately no diagnostics array on the wire — the handoff's
    /// assumed `diagnostics: [String]` field does not exist in the encoder.
    private static func decodeError(_ r: inout ByteReader, version: UInt8) throws -> FluxFrame {
        let seq = try r.u32()
        let msgLen = Int(try r.u16())
        let message = try r.utf8(msgLen)
        let hasSpan = try r.u8()
        let span: FluxSpan? = (hasSpan != 0) ? try decodeSpan(&r) : nil
        return FluxFrame(
            version: version,
            seq: seq,
            flags: FrameKind.errorByte,
            root: nil,
            nodes: [:],
            patches: [],
            handlers: [],
            strings: [],
            state: [],
            files: [],
            componentNames: [],
            signalMeta: [:],
            error: ServerError(message: message, span: span),
            isControl: false
        )
    }

    /// Builds a no-op `FluxFrame` for housekeeping frames that carry no tree
    /// data (`Heartbeat` 0x05, `InternString` 0x07, `StringInterned` 0x08).
    /// The executor short-circuits these before touching the live tree.
    private static func controlFrame(version: UInt8, kind: UInt8) -> FluxFrame {
        FluxFrame(
            version: version,
            seq: 0,
            flags: kind,
            root: nil,
            nodes: [:],
            patches: [],
            handlers: [],
            strings: [],
            state: [],
            files: [],
            componentNames: [],
            signalMeta: [:],
            error: nil,
            isControl: true
        )
    }

    // MARK: - Value

    /// Decodes a `Value` (Appendix D §D.5).
    static func decodeValue(_ r: inout ByteReader) throws -> VMValue {
        let tag = try r.u8()
        switch tag {
        case 0x00: return .null
        case 0x01: return .int(try r.i64())
        case 0x02: return .float(try r.f64())
        case 0x03: return .bool(try r.u8() != 0)
        case 0x04: return .str(try r.u32())
        case 0x05: return .handlerRef(try r.u32())
        case 0x06:
            let count = try r.u16()
            var items = ContiguousArray<VMValue>()
            items.reserveCapacity(Int(count))
            for _ in 0..<count { items.append(try decodeValue(&r)) }
            return .list(Array(items))
        case 0x07:
            let count = try r.u16()
            var fields = ContiguousArray<(UInt16, VMValue)>()
            fields.reserveCapacity(Int(count))
            for _ in 0..<count {
                let propIdx = try r.u16()
                let value = try decodeValue(&r)
                fields.append((propIdx, value))
            }
            return .record(Array(fields))
        case let t:
            throw WireError.unknownTag(offset: r.offset - 1, tag: t)
        }
    }

    // MARK: - Node

    /// Decodes a `Node` (Appendix D §D.3).
    static func decodeNode(_ r: inout ByteReader) throws -> ShadowNode {
        let id = try r.u32()
        let rawKind = try r.u8()
        // The kind byte packs the `NodeKind` in the low 5 bits (0x1F) and the
        // `@pure` flag in bit 0x20 (Appendix D §D.3). Mask them apart so a pure
        // node resolves to a valid kind instead of throwing `unknownTag` — mirrors
        // the Android port.
        let kindByte = rawKind & 0x1F
        let isPure = (rawKind & 0x20) != 0
        guard let kind = NodeKind(rawValue: kindByte) else {
            throw WireError.unknownTag(offset: r.offset - 1, tag: rawKind)
        }
        let componentId = try r.u32()
        let propCount = try r.u16()
        var props = ContiguousArray<Prop>()
        props.reserveCapacity(Int(propCount))
        for _ in 0..<propCount {
            let idx = try r.u16()
            let value = try decodeValue(&r)
            props.append(Prop(index: idx, value: value))
        }
        let childCount = try r.u16()
        var children = ContiguousArray<Child>()
        children.reserveCapacity(Int(childCount))
        for _ in 0..<childCount {
            children.append(try decodeChild(&r))
        }
        let handlerCount = try r.u16()
        var handlers = ContiguousArray<UInt32>()
        handlers.reserveCapacity(Int(handlerCount))
        for _ in 0..<handlerCount {
            handlers.append(try r.u32())
        }
        let span = try decodeSpan(&r)
        return ShadowNode(
            id: id,
            kind: kind,
            componentId: componentId,
            props: Array(props),
            childCount: childCount,
            children: Array(children),
            handlerCount: handlerCount,
            handlers: Array(handlers),
            span: span,
            isPure: isPure
        )
    }

    /// Decodes a `Child` (Appendix D §D.4).
    static func decodeChild(_ r: inout ByteReader) throws -> Child {
        let tag = try r.u8()
        switch tag {
        case 0x01:
            return .node(try r.u32())
        case 0x02:
            let itemCount = try r.u16()
            var items = ContiguousArray<(key: UInt64, node: UInt32)>()
            items.reserveCapacity(Int(itemCount))
            for _ in 0..<itemCount {
                let key = try r.u64()
                let node = try r.u32()
                items.append((key: key, node: node))
            }
            return .splice(itemCount: itemCount, items: Array(items))
        case let t:
            throw WireError.unknownTag(offset: r.offset - 1, tag: t)
        }
    }

    // MARK: - Patch

    /// Decodes a single `Patch` (Appendix D §D.2).
    static func decodePatch(_ r: inout ByteReader) throws -> Patch {
        let tag = try r.u8()
        switch tag {
        case 0x01:
            let id = try r.u32()
            let node = try decodeNode(&r)
            return .replace(id: id, node: node)
        case 0x02:
            let id = try r.u32()
            let changeCount = try r.u16()
            var changes: [Prop] = []
            changes.reserveCapacity(Int(changeCount))
            for _ in 0..<changeCount {
                let idx = try r.u16()
                let value = try decodeValue(&r)
                changes.append(Prop(index: idx, value: value))
            }
            let removalCount = try r.u16()
            var removals: [UInt16] = []
            removals.reserveCapacity(Int(removalCount))
            for _ in 0..<removalCount {
                removals.append(try r.u16())
            }
            return .update(id: id, changes: changes, removals: removals)
        case 0x03:
            let parentId = try r.u32()
            let index = try r.u16()
            let node = try decodeNode(&r)
            return .insert(parentId: parentId, index: index, node: node)
        case 0x04:
            return .remove(id: try r.u32())
        case 0x05:
            let parentId = try r.u32()
            let keyCount = try r.u16()
            var keys: [UInt32] = []
            keys.reserveCapacity(Int(keyCount))
            for _ in 0..<keyCount { keys.append(try r.u32()) }
            return .reorder(parentId: parentId, keys: keys)
        case 0x06:
            let id = try r.u32()
            let closure = try decodeClosureRef(&r)
            return .handler(id: id, closure: closure)
        case 0x07:
            // `Reattach` (Appendix D §D.2, roadmap Phase 3): `old_id`, `new_id`,
            // then the new node shape to apply to the preserved instance.
            let oldId = try r.u32()
            let newId = try r.u32()
            let node = try decodeNode(&r)
            return .reattach(old: oldId, new: newId, node: node)
        case let t:
            throw WireError.unknownTag(offset: r.offset - 1, tag: t)
        }
    }

    // MARK: - Closure / Handler

    /// Decodes the ADR-0027 (FA-IRWIRE) `signal_meta` section: a `u16` node
    /// count followed by, per node, `NodeSignalMeta` (Appendix D §T13).
    ///
    /// Layout (matching `flux-ir-serde::wire::encode_signal_meta_section`):
    /// ```
    /// node_count: u16
    /// for each node:
    ///   node_id:   u32
    ///   dep_count: u16
    ///   deps:      [u32; dep_count]
    ///   has_thunk: u8            // 1 ⇒ closure follows, 0 ⇒ none
    ///   thunk:     ClosureRef?   // present iff has_thunk == 1
    ///   layout_count: u16
    ///   layout:    [u16; layout_count]
    /// ```
    static func decodeSignalMetaSection(_ r: inout ByteReader) throws -> [UInt32: NodeSignalMeta] {
        let count = try r.u16()
        var out: [UInt32: NodeSignalMeta] = [:]
        out.reserveCapacity(Int(count))
        for _ in 0..<count {
            let nodeId = try r.u32()
            let depCount = try r.u16()
            var deps: [UInt32] = []
            deps.reserveCapacity(Int(depCount))
            for _ in 0..<depCount { deps.append(try r.u32()) }
            let thunkPresent = try r.u8()
            let thunk: ClosureRef? = (thunkPresent == 1) ? try decodeClosureRef(&r) : nil
            let layoutCount = try r.u16()
            var layout: [UInt16] = []
            layout.reserveCapacity(Int(layoutCount))
            for _ in 0..<layoutCount { layout.append(try r.u16()) }
            out[nodeId] = NodeSignalMeta(deps: deps, thunk: thunk, layout: layout)
        }
        return out
    }

    /// Decodes a `ClosureRef` (Appendix D §D.7).
    static func decodeClosureRef(_ r: inout ByteReader) throws -> ClosureRef {
        let hash = try r.bytes(8)
        let bytecodeOffset = try r.u32()
        let bytecodeLen = try r.u16()
        let signalCount = try r.u16()
        var signals: [UInt32] = []
        signals.reserveCapacity(Int(signalCount))
        for _ in 0..<signalCount { signals.append(try r.u32()) }
        let span = try decodeSpan(&r)
        return ClosureRef(
            hash: hash,
            bytecodeOffset: bytecodeOffset,
            bytecodeLen: bytecodeLen,
            signalCount: signalCount,
            signals: signals,
            span: span
        )
    }

    /// Decodes a `HandlerDef` (Appendix D §D.8).
    static func decodeHandlerDef(_ r: inout ByteReader) throws -> HandlerDef {
        let handlerId = try r.u32()
        let closure = try decodeClosureRef(&r)
        return HandlerDef(handlerId: handlerId, closure: closure, bytecode: nil)
    }

    /// Resolves handler definitions from the frame's shared handler section
    /// (Appendix D §D.12), producing `HandlerDef`s that carry their concrete
    /// bytecode. The section is a `u32` blob length followed by the raw
    /// bytecode, then a `u16` count of `(handlerId, ClosureRef)` entries whose
    /// `bytecode_offset`/`bytecode_len` index the blob. The caller only invokes
    /// this when the header declared `handlerCount > 0`.
    private static func decodeHandlerSection(
        _ r: inout ByteReader
    ) throws -> [HandlerDef] {
        let blobLen = try r.u32()
        let blob = try r.bytes(Int(blobLen))
        guard !blob.isEmpty else {
            // A zero-length blob with `handlerCount > 0` is contradictory; emit
            // no runnable handlers rather than fabricating bodies.
            return []
        }
        let count = try r.u16()
        var resolved: [HandlerDef] = []
        resolved.reserveCapacity(Int(count))
        for _ in 0..<count {
            let handlerId = try r.u32()
            let closure = try decodeClosureRef(&r)
            let start = Int(closure.bytecodeOffset)
            let end = start + Int(closure.bytecodeLen)
            guard let slice = blob[safe: start..<end] else {
                throw WireError.unexpectedEnd(offset: start, needed: end - start, available: blob.count)
            }
            resolved.append(HandlerDef(handlerId: handlerId, closure: closure, bytecode: Array(slice)))
        }
        return resolved
    }

    // MARK: - String / File / Span

    static func decodeStringEntry(_ r: inout ByteReader) throws -> StringEntry {
        let stringId = try r.u32()
        let len = try r.u16()
        let value = try r.utf8(Int(len))
        return StringEntry(stringId: stringId, value: value)
    }

    static func decodeFileEntry(_ r: inout ByteReader) throws -> FileEntry {
        let fileId = try r.u32()
        let len = try r.u16()
        let path = try r.utf8(Int(len))
        return FileEntry(fileId: fileId, path: path)
    }

    static func decodeSpan(_ r: inout ByteReader) throws -> FluxSpan {
        let fileId = try r.u32()
        let start = try r.u32()
        let end = try r.u32()
        return FluxSpan(fileId: fileId, start: start, end: end)
    }
}
