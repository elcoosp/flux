//  HttpCapabilities+Entries.swift
//  FLUX-047 (the remaining host bodies): `Http` (cap 14) and `Persist` (cap 15).
//
//  Synchronous `Persist` (15) reuses the injected `StorageBackend` (same OS
//  `.storage` gate as `Storage`) plus an enumeration for `query`. Asynchronous
//  `Http` (14) allocates a `Pending` cell and stashes the request in
//  `store`; the executor's `AsyncResolver` (which owns the live `StringTable`)
//  performs the call and settles the cell. See `HttpCapabilities.swift`.

extension CapabilityRegistry {
    /// The FLUX-047 capability set as `(capId, methodId, impl)` triples.
    /// - Parameters:
    ///   - store: the shared pending-`Http`-request store.
    ///   - transport: the network transport (`URLSessionHttpTransport` in prod).
    ///   - backend: the `Persist` backend (`StorageBackend`); defaults in-memory.
    static func httpPersistEntries(
        store: HttpRequestStore,
        transport: HttpTransport,
        backend: any StorageBackend = InMemoryStorageBackend()
    ) -> [(UInt32, UInt16, CapabilityImpl)] {
        var entries: [(UInt32, UInt16, CapabilityImpl)] = []

        // MARK: Persist (cap 15) — synchronous structured, queryable persistence.
        // Persist.put(key, value) (15,1): store the value and return its cell id.
        entries.append((15, 1, { (_: UInt32, _: UInt16, arg: FluxValue, signals: inout SignalStore) in
            guard case let .record(fields) = arg, fields.count >= 2 else { throw VmError.typeMismatch(offset: 0) }
            guard case let .str(keyId) = fields[0].value else { throw VmError.typeMismatch(offset: 0) }
            backend.put(keyId, fields[1].value)
            let id = signals.allocateCell()
            signals.write(id, fields[1].value)
            return id
        }))
        // Persist.get(key) (15,2): read the stored value (default null) via its cell.
        entries.append((15, 2, { (_: UInt32, _: UInt16, arg: FluxValue, signals: inout SignalStore) in
            guard case let .record(fields) = arg, !fields.isEmpty else { throw VmError.typeMismatch(offset: 0) }
            guard case let .str(keyId) = fields[0].value else { throw VmError.typeMismatch(offset: 0) }
            let id = signals.allocateCell()
            signals.write(id, backend.get(keyId) ?? .null)
            return id
        }))
        // Persist.query(where) (15,3): return every stored record as a FluxValue list.
        entries.append((15, 3, { (_: UInt32, _: UInt16, _: FluxValue, signals: inout SignalStore) in
            let items = backend.entries().map { (keyId, value) -> FluxValue in
                .record([(UInt16(0), .str(keyId)), (UInt16(1), value)])
            }
            let id = signals.allocateCell()
            signals.write(id, .list(items))
            return id
        }))
        // Persist.delete(key) (15,4): clear the stored value.
        entries.append((15, 4, { (_: UInt32, _: UInt16, arg: FluxValue, signals: inout SignalStore) in
            guard case let .record(fields) = arg, !fields.isEmpty else { throw VmError.typeMismatch(offset: 0) }
            guard case let .str(keyId) = fields[0].value else { throw VmError.typeMismatch(offset: 0) }
            backend.put(keyId, nil)
            let id = signals.allocateCell()
            signals.write(id, .bool(true))
            return id
        }))

        // MARK: Http (cap 14) — asynchronous network requests (ADR-0045).
        // Http.fetch(url, options) (14,1): Pending cell + stashed GET request.
        entries.append((14, 1, { (_: UInt32, _: UInt16, arg: FluxValue, signals: inout SignalStore) in
            guard case let .record(fields) = arg, !fields.isEmpty else { throw VmError.typeMismatch(offset: 0) }
            guard case let .str(urlId) = fields[0].value else { throw VmError.typeMismatch(offset: 0) }
            let id = signals.allocateCell()
            signals.markPending(id)
            store.put(id, HttpRequest(method: "GET", urlId: urlId, bodyId: nil, parseJson: false))
            return id
        }))
        // Http.getJson(url) (14,2): same shape, response parsed to JSON.
        entries.append((14, 2, { (_: UInt32, _: UInt16, arg: FluxValue, signals: inout SignalStore) in
            guard case let .record(fields) = arg, !fields.isEmpty else { throw VmError.typeMismatch(offset: 0) }
            guard case let .str(urlId) = fields[0].value else { throw VmError.typeMismatch(offset: 0) }
            let id = signals.allocateCell()
            signals.markPending(id)
            store.put(id, HttpRequest(method: "GET", urlId: urlId, bodyId: nil, parseJson: true))
            return id
        }))
        // Http.postJson(url, body) (14,3): POST with a JSON body (body id optional).
        entries.append((14, 3, { (_: UInt32, _: UInt16, arg: FluxValue, signals: inout SignalStore) in
            guard case let .record(fields) = arg, !fields.isEmpty else { throw VmError.typeMismatch(offset: 0) }
            guard case let .str(urlId) = fields[0].value else { throw VmError.typeMismatch(offset: 0) }
            let bodyId: UInt32? = fields.count >= 2 ? (fields[1].value).strID : nil
            let id = signals.allocateCell()
            signals.markPending(id)
            store.put(id, HttpRequest(method: "POST", urlId: urlId, bodyId: bodyId, parseJson: true))
            return id
        }))

        return entries
    }

    /// Builds a production `AsyncResolver` that resolves pending `Http` cells.
    /// - Parameters:
    ///   - store: the pending-request store the impls wrote into.
    ///   - transport: the network transport.
    ///   - tableProvider: returns the *current* live string table on each resolve
    ///     (the table is reassigned every frame, so it must not be captured by
    ///     value at construction).
    static func makeHttpResolver(
        store: HttpRequestStore,
        transport: HttpTransport,
        tableProvider: @Sendable @escaping () -> StringTable
    ) -> any AsyncResolver {
        return HttpAsyncResolver(store: store, transport: transport, tableProvider: tableProvider)
    }
}

/// The `Http` async resolver (FLUX-047): settles a parked `Http` cell by
/// performing the real network call and converting the response into a
/// `FluxValue`. Non-`Http` futures pass through untouched.
/// `internal` (not `private`) because `FluxExecutor` constructs it from another
/// file in the same `FluxHost` module.
internal struct HttpAsyncResolver: AsyncResolver {
    let store: HttpRequestStore
    let transport: HttpTransport
    let tableProvider: @Sendable () -> StringTable

    func resolve(_ future: FluxValue) async -> FluxValue {
        guard case let .int(cellId) = future else { return future }
        guard let request = store.get(UInt32(cellId)) else { return future }
        store.remove(UInt32(cellId))
        var table = tableProvider()
        let url = table.lookup(request.urlId) ?? ""
        let body = request.bodyId.flatMap { table.lookup($0) }
        let response = transport.request(method: request.method, url: url, body: body)
        if request.parseJson {
            return FluxValueJsonParser.parse(response)
        }
        // The response text must reach the VM as a resolvable .str; intern it.
        return .str(table.intern(response))
    }
}
