//  StorageError.swift
//  Errors surfaced by the storage backends (FLUX-081) when a `FluxValue`
//  cannot be JSON (de)serialized for persistence.
//
//  These are *recoverable* failures: a `put` that fails to encode must not
//  silently no-op (which would make `Storage.set` appear to succeed while
//  storing nothing), and a `get`/`entries` hit on corrupt bytes must surface
//  rather than be skipped unnoticed. Both paths log the error through
//  `RecoverableErrorReporter` and treat the key as absent, matching Android's
//  corrupt-treat-as-absent contract.

import Foundation

/// Errors raised by the storage backends when (de)serializing `FluxValue`.
enum StorageError: LocalizedError {
    /// Encoding `value` for storage `key` failed (e.g. a non-JSON-representable
    /// `FluxValue` such as `float(.nan)`).
    case encodeFailed(key: UInt32, underlying: Error)
    /// Decoding the stored bytes for `key` failed (corrupt / unreadable payload).
    case decodeFailed(key: UInt32, underlying: Error)

    var errorDescription: String? {
        switch self {
        case let .encodeFailed(key, underlying):
            "Storage encode failed for key \(key): \(underlying.localizedDescription)"
        case let .decodeFailed(key, underlying):
            "Storage decode failed for key \(key): \(underlying.localizedDescription)"
        }
    }
}
