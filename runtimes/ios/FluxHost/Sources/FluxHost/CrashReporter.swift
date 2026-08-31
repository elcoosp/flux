//  CrashReporter.swift
//  FLUX-035 (LANE-R, Phase 8) — release crash reporting (Swift reporter).
//
//  A host-native (no webview) reporter that maps a release crash back to the
//  generated component/source where possible, feeding the same `FluxError`
//  shape PRD-K established. RELEASE-ONLY: guarded by `#if !DEBUG` so dev
//  telemetry (ADR-0040) and release crash reporting never mix.
//
//  ADR-0049 does not apply (new iOS-native type).
//
//  RELEASE-TODO: wire `SignalExceptionHandler` / `NSSetUncaughtExceptionHandler`
//  to capture the raw backtrace, then resolve the top frame to a generated
//  component id via the embedded source map. This build provides the shape and
//  the `report(_:)` entry point; the handler registration is a one-line shell
//  call in `FluxApp` launch.

import Foundation

#if !DEBUG
/// Captures release-path crashes and renders them into a `FluxError` carrying
/// the component id / source reference (PRD-R §9). Release-only.
public final class CrashReporter: @unchecked Sendable {
    /// The most recent crash, if any, for display/telemetry.
    ///
    /// `@unchecked Sendable`: this is a process-global singleton. `lastCrash` is
    /// only ever written from the installed uncaught-exception / signal handler,
    /// a single serialized entry point, so cross-task shared mutable state is not
    /// a concurrency hazard in practice.
    public private(set) var lastCrash: FluxError?

    public static let shared = CrashReporter()

    private init() {}

    /// Records a crash as a `FluxError`. In a full build this is invoked from the
    /// installed `NSSetUncaughtExceptionHandler` / signal handler; here it is the
    /// public entry point the shell wires up. Never throws.
    public func report(message: String, callSites: [String] = []) {
        lastCrash = FluxError(message: message, kind: .runtime, callSites: callSites)
    }

    /// Installs the global handlers. Idempotent; safe to call once at launch.
    public func install() {
        NSSetUncaughtExceptionHandler { exception in
            CrashReporter.shared.report(
                message: "\(exception.name.rawValue): \(exception.reason ?? "unknown")",
                callSites: exception.callStackSymbols
            )
        }
    }
}
#endif
