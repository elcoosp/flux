//  FluxCrashReporter.swift
//  FLUX-081 (LANE-ISTORAGE) — observable error channel for recoverable host
//  failures.
//
//  Storage encode/decode failures must be *observable*, never silently
//  swallowed (AGENTS.md §2.2). `FluxCrashReporter` is the host's lightweight
//  error channel: every recorded error is logged via `os_log` at `.error`
//  level so the failure surfaces in the device console / crash pipeline
//  instead of vanishing as silent data loss.
//
//  It is a process-global singleton. `@unchecked Sendable`: the only mutable
//  state is the test/inspection buffer below, guarded by an internal serial
//  queue, so cross-task shared mutation is not a concurrency hazard in
//  practice. ADR-0049 does not apply (new iOS-native type, debug+release).

import Foundation
import os.log

/// Observability sink for recoverable host errors (FLUX-081).
final class FluxCrashReporter: @unchecked Sendable {
    /// The shared reporter.
    static let shared = FluxCrashReporter()

    private let log = OSLog(subsystem: "dev.flux.host", category: "storage")
    private let queue = DispatchQueue(label: "dev.flux.crashreporter")
    private var _lastDescription: String?

    private init() {}

    /// Records a recoverable error to the host error channel. Never throws and
    /// is safe to call from any thread.
    func record(_ error: Error) {
        let description = error.localizedDescription
        os_log(.error, log: log, "%{public}@", description)
        queue.sync { self._lastDescription = description }
    }

    /// The description of the most recently recorded error, or `nil`
    /// (test / inspection hook).
    var lastRecordedDescription: String? {
        queue.sync { _lastDescription }
    }

    /// Clears the recorded-error buffer (test helper).
    func reset() {
        queue.sync { _lastDescription = nil }
    }
}
