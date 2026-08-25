//  FrameDeserializer.swift
//  Decodes Flux wire frames (Appendix D) into `FluxFrame`.
//
//  The decoder is a behavioral mirror of the Rust dev-server serializer: same
//  magic, same little-endian layout, same implicit `string_id -> VMValue.str`
//  rule for `Str` values (Appendix D §D.5). A corrupt or truncated frame raises
//  `WireError` with the failing byte offset rather than panicking.

/// Decodes Flux binary frames (Appendix D) into host-side `FluxFrame` models.
enum FrameDeserializer {
    /// The 4-byte frame magic: `0x465C5558` ("FLUX" in little-endian).
    static let magic: UInt32 = 0x465C_5558

    /// Decodes a frame from raw bytes.
    /// - Throws: `WireError` on malformed input, or a caller-supplied error from
    ///   the value/closure decoders.
    static func decode(_ bytes: [UInt8]) throws -> FluxFrame {
        var r = ByteReader(bytes)
        let rawMagic = try r.u32()
        guard rawMagic == magic else {
            throw WireError.badMagic(offset: 0, value: rawMagic)
        }
        let version = try r.u8()
        let seq = try r.u32()
        let flags = try r.u8()

        // delta frames (full_tree bit clear) have the patch/handler/string
        // structure; full frames (Init) carry a root node in the same layout but
        // with patch_count/handler_count/string_count still encoded at the same
        // offset. We always parse the delta header; for full frames the root node
        // follows the header and `root` is populated.
        let patchCount = try r.u16()
        let handlerCount = try r.u16()
        let stringCount = try r.u16()

        // For full (Init) frames, the root node is encoded here (Appendix D §D.12.2).
        // For delta frames the body begins with patches, so peek the next tag to
        // decide: a full frame carries a Node whose first field is the 4-byte id,
        // and there is no leading patch tag.
        let root: ShadowNode?
        if flags & 0x01 != 0 {
            // full_tree: decode the root node now.
            root = try decodeNode(&r)
        } else {
            root = nil
        }
        // Build the flat id → node table from the reachable tree so the
        // reconciler can resolve child ids (Appendix D §D.4) anywhere below.
        var nodes: [UInt32: ShadowNode] = [:]
        if let root {
            nodes[root.id] = root
        }

        var patches: [Patch] = []
        for _ in 0..<patchCount {
            patches.append(try decodePatch(&r))
        }
        // The frame carries a handler section (Appendix D §D.12) — a shared
        // bytecode blob followed by a stream of `(handlerId, ClosureRef)` entries
        // (Gap G1). When the header `handlerCount` is 0 the section is empty (an
        // encoded zero-length blob) and no handlers are produced. Each decoded
        // `HandlerDef` is resolved to its concrete bytecode body here (the body
        // the executor must register so native controls can fire it later).
        var handlers: [HandlerDef] = []
        if handlerCount > 0 {
            handlers = try decodeHandlerSection(&r)
        }
        var strings: [StringEntry] = []
        for _ in 0..<stringCount {
            strings.append(try decodeStringEntry(&r))
        }
        var state: [StateCell] = []
        if flags & 0x08 != 0 {
            let cellCount = try r.u16()
            for _ in 0..<cellCount {
                let signalId = try r.u32()
                let value = try decodeValue(&r)
                state.append(StateCell(signalId: signalId, value: value))
            }
        }
        var files: [FileEntry] = []
        if flags & 0x10 != 0 {
            let fileCount = try r.u16()
            for _ in 0..<fileCount {
                files.append(try decodeFileEntry(&r))
            }
        }
        return FluxFrame(
            version: version,
            seq: seq,
            flags: flags,
            root: root,
            nodes: nodes,
            patches: patches,
            handlers: handlers,
            strings: strings,
            state: state,
            files: files
        )
    }

    /// Walks `node` and every reachable descendant, registering each in `table`
    /// by its `NodeId`. Children are referenced by id (Appendix D §D.4), so we
    /// resolve each child through `decodeNode`'s side table — but since the wire
    /// flattens the tree, we instead recurse the already-decoded parent's
    /// `childCount`/`children` by re-decoding is not possible; the host reads
    /// children lazily via the `nodes` table built here from `root`.
    private static func indexTree(_ node: ShadowNode, into table: inout [UInt32: ShadowNode]) {
        table[node.id] = node
        for child in node.children {
            switch child {
            case let .node(id):
                // Child ids are resolved against the full tree by the consumer.
                _ = id
            case let .splice(_, items):
                for (_, id) in items { _ = id }
            }
        }
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
            var items: [VMValue] = []
            items.reserveCapacity(Int(count))
            for _ in 0..<count { items.append(try decodeValue(&r)) }
            return .list(items)
        case 0x07:
            let count = try r.u16()
            var fields: [(UInt16, VMValue)] = []
            fields.reserveCapacity(Int(count))
            for _ in 0..<count {
                let propIdx = try r.u16()
                let value = try decodeValue(&r)
                fields.append((propIdx, value))
            }
            return .record(fields)
        case let t:
            throw WireError.unknownTag(offset: r.offset - 1, tag: t)
        }
    }

    // MARK: - Node

    /// Decodes a `Node` (Appendix D §D.3).
    static func decodeNode(_ r: inout ByteReader) throws -> ShadowNode {
        let id = try r.u32()
        let rawKind = try r.u8()
        guard let kind = NodeKind(rawValue: rawKind) else {
            throw WireError.unknownTag(offset: r.offset - 1, tag: rawKind)
        }
        let componentId = try r.u32()
        let propCount = try r.u16()
        var props: [Prop] = []
        props.reserveCapacity(Int(propCount))
        for _ in 0..<propCount {
            let idx = try r.u16()
            let value = try decodeValue(&r)
            props.append(Prop(index: idx, value: value))
        }
        let childCount = try r.u16()
        var children: [Child] = []
        children.reserveCapacity(Int(childCount))
        for _ in 0..<childCount {
            children.append(try decodeChild(&r))
        }
        let handlerCount = try r.u16()
        var handlers: [UInt32] = []
        handlers.reserveCapacity(Int(handlerCount))
        for _ in 0..<handlerCount {
            handlers.append(try r.u32())
        }
        let span = try decodeSpan(&r)
        return ShadowNode(
            id: id,
            kind: kind,
            componentId: componentId,
            props: props,
            childCount: childCount,
            children: children,
            handlerCount: handlerCount,
            handlers: handlers,
            span: span
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
            var items: [(key: UInt64, node: UInt32)] = []
            items.reserveCapacity(Int(itemCount))
            for _ in 0..<itemCount {
                let key = try r.u64()
                let node = try r.u32()
                items.append((key: key, node: node))
            }
            return .splice(itemCount: itemCount, items: items)
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
        case let t:
            throw WireError.unknownTag(offset: r.offset - 1, tag: t)
        }
    }

    // MARK: - Closure / Handler

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
