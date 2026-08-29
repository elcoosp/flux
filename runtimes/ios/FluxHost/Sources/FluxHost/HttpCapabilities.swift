//  HttpCapabilities.swift
//  FLUX-047 (the remaining host bodies): `Http` (cap 14) and `Persist` (cap 15).
//
//  `Persist` (15) is **synchronous** — a thin queryable wrapper over the same
//  injected `StorageBackend` the `Storage` capability already uses, plus an
//  enumeration so `query` can return every stored record. Each impl returns the
//  result-cell signal id (ADR-0045) and the value is written into that cell.
//
//  `Http` (14) is **asynchronous** (network). Its impl cannot resolve the URL /
//  body interned ids to text (it runs without a string table), so it allocates a
//  `Pending` result cell, stashes the request in a shared `HttpRequestStore`,
//  and returns the cell id immediately. The executor's `AsyncResolver` — which
//  owns the live `StringTable` — later reads the request, resolves the ids to
//  text, performs the call via `HttpTransport`, and settles the cell. This
//  closes the LANE-C wiring gap (the resolver previously only saw the cell id,
//  never the cap/method/args).

import Foundation

/// A pending `Http` capability request, stashed by a `CALL_CAP` (cap 14) impl
/// and consumed by the executor's async resolver (ADR-0045).
struct HttpRequest {
    /// The HTTP verb (`GET`/`POST`/`PUT`/`DELETE`).
    let method: String
    /// The interned id of the request URL.
    let urlId: UInt32
    /// The interned id of the request body, or `nil` when absent.
    let bodyId: UInt32?
    /// When `true` (cap 14, method 2 = `getJson`) the response body is parsed to
    /// a structured `FluxValue`; otherwise it is returned as a `.str` (the
    /// response text interned into the table).
    let parseJson: Bool
}

/// The shared store of in-flight `Http` requests, keyed by the result-cell id
/// the `CALL_CAP` returned. A reference type so the capability impl (which
/// writes) and the executor's async resolver (which reads + performs the call)
/// share one instance — the registry and the resolver must be constructed from
/// the same `HttpRequestStore`.
///
/// Values are safe to share across the single reactive dispatcher the VM runs on
/// (ADR-0027); no internal concurrency is introduced here.
final class HttpRequestStore: @unchecked Sendable {
    private var pending: [UInt32: HttpRequest] = [:]

    /// Records a pending request for `cellId`.
    func put(_ cellId: UInt32, _ request: HttpRequest) {
        pending[cellId] = request
    }

    /// Returns the pending request for `cellId`, or `nil` if none.
    func get(_ cellId: UInt32) -> HttpRequest? { pending[cellId] }

    /// Removes the pending request for `cellId`.
    func remove(_ cellId: UInt32) {
        pending.removeValue(forKey: cellId)
    }
}

/// The transport seam for `Http` capability resolution (FLUX-047). The
/// executor's async resolver calls `request` with resolved URL/body text and
/// receives the raw response body. The production host supplies
/// `URLSessionHttpTransport`; tests inject a fake to keep the round-trip
/// deterministic without a live server.
protocol HttpTransport: Sendable {
    /// Performs `method` against `url` with optional `body`; returns the response body.
    func request(method: String, url: String, body: String?) -> String
}

/// Production `Http` transport for the iOS host: performs requests over
/// `URLSession` (Foundation). Runs synchronously on the caller's context (the
/// executor's async resolver is already suspended on the reactive loop).
struct URLSessionHttpTransport: HttpTransport {
    func request(method: String, url: String, body: String?) -> String {
        guard let u = URL(string: url) else { return "" }
        var req = URLRequest(url: u)
        req.httpMethod = method
        req.setValue("application/json", forHTTPHeaderField: "Accept")
        if let body {
            req.httpBody = body.data(using: .utf8)
            req.setValue("application/json; charset=utf-8", forHTTPHeaderField: "Content-Type")
        }
        let sem = DispatchSemaphore(value: 0)
        var result = ""
        let task = URLSession.shared.dataTask(with: req) { data, _, _ in
            if let data, let s = String(data: data, encoding: .utf8) { result = s }
            sem.signal()
        }
        task.resume()
        sem.wait()
        return result
    }
}
