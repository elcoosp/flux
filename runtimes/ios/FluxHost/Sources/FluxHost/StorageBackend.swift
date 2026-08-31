//  StorageBackend.swift
//  Persistence backends for stateful capabilities (e.g. `Storage`), injected
//  into `CapabilityStore` so tests can use an in-memory store while dev/release
//  builds persist to `UserDefaults` (Appendix E §E.1, ADR-0045).
//
//  The backend is the seam between the capability registry (pure logic) and
//  the platform's durable storage. The MLP dev/test path registers an
//  `InMemoryStorageBackend`; the app shell registers a
//  `UserDefaultsStorageBackend` so `Storage.set`/`get` survive process
//  restarts. Values are JSON-encoded via `FluxValueJSON` (Appendix D §D.5 shape).

import Foundation

/// A persistence backend for stateful capabilities.
///
/// Implementations map an interned string id (`Storage` key) to a `FluxValue`.
/// `put(_:nil)` clears the key. Conforming types must be safe to share across
/// the single reactive dispatcher the VM runs on (ADR-0027); the reference
/// implementations hold no internal concurrency of their own.
protocol StorageBackend: Sendable {
    /// Records `value` for `key`; `nil` clears it.
    func put(_ key: UInt32, _ value: FluxValue?)
    /// Reads the value for `key`, or `nil` if absent.
    func get(_ key: UInt32) -> FluxValue?

    /// Enumerates every stored entry (FLUX-047 `Persist.query`).
    func entries() -> [UInt32: FluxValue]
}

/// In-memory backend: the MLP dev/test default.
///
/// Values live only for the lifetime of the store; dropping the store drops its
/// contents. Used by the unit tests and the headless VM.
final class InMemoryStorageBackend: @unchecked Sendable, StorageBackend {
    private var storage: [UInt32: FluxValue] = [:]

    func put(_ key: UInt32, _ value: FluxValue?) {
        if let value { storage[key] = value } else { storage.removeValue(forKey: key) }
    }

    func get(_ key: UInt32) -> FluxValue? { storage[key] }

    func entries() -> [UInt32: FluxValue] { storage }
}

/// `UserDefaults`-backed backend: real persistence for dev/release builds.
///
/// Values are JSON-encoded (Appendix D §D.5 shape, see `FluxValueJSON`) under a
/// namespaced key `flux.storage.<keyId>` so they survive process restarts and
/// never collide with other `UserDefaults` users. An isolated `suite` (e.g. a
/// test-only suite name) scopes the store to a private persistent domain, which
/// is what lets a round-trip test prove the value is read from disk rather than
/// an in-memory cache.
final class UserDefaultsStorageBackend: @unchecked Sendable, StorageBackend {
    private let defaults: UserDefaults
    private let prefix: String

    /// Creates a backend.
    /// - Parameters:
    ///   - suite: an optional `UserDefaults` suite name; when `nil` (or
    ///     unresolvable) the standard domain is used.
    ///   - prefix: the key namespace; defaults to `flux.storage.`.
    init(suite: String? = nil, prefix: String = "flux.storage.") {
        if let suite, let suiteDefaults = UserDefaults(suiteName: suite) {
            self.defaults = suiteDefaults
        } else {
            self.defaults = .standard
        }
        self.prefix = prefix
    }

    private func key(_ id: UInt32) -> String { "\(prefix)\(id)" }

    func put(_ key: UInt32, _ value: FluxValue?) {
        let k = self.key(key)
        guard let value else {
            defaults.removeObject(forKey: k)
            return
        }
        do {
            let encoded = try FluxValueJSON.encode(value)
            defaults.set(encoded, forKey: k)
        } catch {
            FluxCrashReporter.shared.record(StorageError.encodeFailed(key: key, underlying: error))
        }
    }

    func get(_ key: UInt32) -> FluxValue? {
        guard let data = defaults.data(forKey: self.key(key)) else { return nil }
        do {
            return try FluxValueJSON.decode(data)
        } catch {
            FluxCrashReporter.shared.record(StorageError.decodeFailed(key: key, underlying: error))
            return nil
        }
    }

    func entries() -> [UInt32: FluxValue] {
        let dict = defaults.dictionaryRepresentation()
        var result: [UInt32: FluxValue] = [:]
        for (k, v) in dict {
            guard k.hasPrefix(prefix),
                  let id = UInt32(k.dropFirst(prefix.count)),
                  let data = v as? Data else { continue }
            do {
                result[id] = try FluxValueJSON.decode(data)
            } catch {
                FluxCrashReporter.shared.record(StorageError.decodeFailed(key: id, underlying: error))
            }
        }
        return result
    }
}
