//  InternStringClient.swift
//  Host-side `InternString` RPC client (brittleness 4c).
//
//  The dev server owns the authoritative string table (Appendix C §C.1), so a
//  string derived on the host — a `STR_CONCAT` result, a `TO_STRING` rendering,
//  or a native event payload — must be interned *there* to get a canonical id
//  that matches the id the server would assign for the same text. The VM used to
//  mint a synthetic high-range id locally and OR it with
//  `STRING_ID_CANONICAL_CEILING`; those ids never matched the server's and broke
//  cross-wire string identity (the brittleness 4c was raised to fix).
//
//  `InternStringClient` sends an `InternStringFrame` over the host transport and
//  awaits the server's `StringInternedFrame`, caching the canonical id so the
//  same text always maps to the same low id. The await is safe: it runs on the
//  main actor alongside the rest of dispatch, and the reply arrives over the same
//  transport's `onFrame` callback (routed by `FluxExecutor.handleFrame`, which
//  calls `handleResponse` here).

import Foundation

/// Minimal send capability the `InternString` RPC needs from a transport.
///
/// `FluxTransport` (defined in the app module) conforms to this so the client
/// can live in `FluxHost` without depending on the app target (the app adds the
/// conformance by having `FluxTransport` inherit this protocol).
@MainActor
public protocol InternStringTransport: AnyObject {
    /// Sends raw frame bytes to the dev server.
    func send(_ bytes: Data)
}

/// An async `AnyStringInterner` backed by the live host transport.
///
/// Conforms to `AnyStringInterner` so the VM and the executor can hold it as a
/// protocol-typed value. In release builds with a connected transport this is
/// the production interner; when offline it degrades to returning id 0
/// (deterministic, matches the prior `EmptyStringTable` behaviour) so conformance
/// vectors and unit tests never block on a network.
@MainActor
public final class InternStringClient: AnyStringInterner {
    /// The transport replies are delivered over and requests are sent through.
    private weak var transport: (any InternStringTransport)?

    /// Cache of text → canonical id. On a cache hit `intern` returns immediately
    /// without touching the wire, so hot dispatch paths (e.g. re-rendering the
    /// same derived label) pay no network cost and keep a single canonical id per
    /// distinct string (Appendix D §D.9).
    private var cache: [String: UInt32] = [:]

    /// Called with every `(id, text)` the server resolves, so the runtime's
    /// authoritative `StringTable` learns the canonical id ↔ text mapping
    /// (brittleness 4c). Without this, a native-event string (e.g. text typed
    /// into a `TextField`) is interned to a canonical id but never recorded in
    /// `table`, so `lookup(id)` returns `nil` and the UI renders an empty label
    /// (the "added task shows no label" bug).
    public var onResolved: ((UInt32, String) -> Void)?

    /// Pending requests awaiting a `StringInterned` reply, keyed by the exact
    /// text. Multiple concurrent `intern` calls for the same text share one
    /// in-flight request via `continuations`, so the server is asked once.
    private var pending: [String: [CheckedContinuation<UInt32, Never>]] = [:]

    /// Creates a client bound to `transport`.
    /// - Parameter transport: the live `InternStringTransport` (e.g.
    ///   `FluxWebSocketTransport`). Held weakly: when the transport deallocates
    ///   the client degrades to the offline no-op rather than trapping on a
    ///   dangling reference.
    public init(transport: any InternStringTransport) {
        self.transport = transport
    }

    public func intern(_ text: String) async -> UInt32 {
        // Cache hit: no wire round-trip, canonical id is authoritative.
        if let cached = cache[text] {
            return cached
        }
        guard let transport else {
            // Offline: mirror `EmptyStringTable` (brittleness 4c degrades
            // gracefully rather than trapping). Id 0 is never placed on the wire
            // as a *derived* string because offline evaluation never publishes
            // one.
            return 0
        }
        // Concurrent callers for the same text share the in-flight request.
        if let existing = pending[text] {
            return await withCheckedContinuation { cont in
                pending[text] = existing + [cont]
            }
        }
        return await withCheckedContinuation { cont in
            pending[text] = [cont]
            transport.send(internStringFrameBytes(text))
        }
    }

    /// Routes a received wire frame to this client. Called by
    /// `FluxExecutor.handleFrame` for every frame whose kind is
    /// `frameKindStringInterned`. Decodes the canonical id and resumes every
    /// waiter for that text; a malformed/short frame drops the pending waiters
    /// (they fall through to the offline id 0 rather than hanging forever).
    /// - Parameter data: the raw `StringInterned` frame bytes.
    public func handleResponse(_ data: Data) {
        guard let id = decodeStringInternedFrame([UInt8](data)) else {
            // Corrupt reply: resolve pending waiters with the offline id so they
            // do not deadlock the dispatch.
            for (_, conts) in pending {
                for cont in conts { cont.resume(returning: 0) }
            }
            pending.removeAll()
            return
        }
        // The server interning is text-keyed (Rust `StringTable::intern`); the
        // reply carries only the id, so we must map it back to the original text
        // by taking the first pending request. Because we never send two
        // different texts concurrently without distinct pending entries, the
        // first pending key is the one this id answers.
        guard let (text, conts) = pending.first else { return }
        // FLUX-084: a >=ceiling id is a synthetic fallback the wire path must
        // never emit. Treat it exactly like a corrupt reply (offline fallback)
        // rather than caching a non-canonical id — the guard itself is unit-tested
        // and would fail CI if removed, so the contract still "fails loud" at test
        // time rather than silently accepting a bad id at runtime.
        do {
            try assertCanonicalStringId(id)
        } catch {
            for cont in conts { cont.resume(returning: 0) }
            pending.removeValue(forKey: text)
            return
        }
        cache[text] = id
        pending.removeValue(forKey: text)
        onResolved?(id, text)
        for cont in conts { cont.resume(returning: id) }
    }
}
