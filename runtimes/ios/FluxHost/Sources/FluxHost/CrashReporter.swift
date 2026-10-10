//  CrashReporter.swift
//  FLUX-035 (LANE-R, Phase 8) + FLUX-F17 — release crash reporting.
//
//  A host-native (no webview) reporter that maps a release crash back to the
//  generated component/source, feeding the same `FluxError` shape PRD-K
//  established. RELEASE-ONLY: guarded by `#if !DEBUG` so dev telemetry
//  (ADR-0040) and release crash reporting never mix.
//
//  ADR-0049 does not apply (new iOS-native type).
//
//  `install()` wires `NSSetUncaughtExceptionHandler` so an uncaught release
//  exception lands in `lastCrash` with its `callStackSymbols`. F17 adds
//  resolution of the top frames against an embedded `<stem>.flux.map.json`
//  sidecar (the codegen source map), populating `FluxError.excerpt` with the
//  originating `.flux` file path + 1-based line/col so the on-device overlay
//  and the crash pipeline point at the user's source rather than at
//  generated native symbols.

import Foundation

#if !DEBUG
/// One entry from a `<stem>.flux.map.json` sidecar (schema documented in
/// `docs/issues/FLUX-F17-crash-source-map.md`). Matches the Rust
/// `flux_codegen_core::SourceMapEntry` shape.
private struct CodegenSourceMapEntry: Decodable {
    let native_symbol: String
    let component_name: String
    let source_file: String
    let line: UInt32
    let col: UInt32
}

/// A parsed source-map sidecar. Matches `flux_codegen_core::SourceMap`.
private struct CodegenSourceMap: Decodable {
    let version: UInt32
    let source_file: String
    let component_map: [CodegenSourceMapEntry]
}

/// Captures release-path crashes and renders them into a `FluxError` carrying
/// the component id / source reference (PRD-R §9). Release-only.
public final class CrashReporter: @unchecked Sendable {
    /// The most recent crash, if any, for display/telemetry.
    ///
    /// `@unchecked Sendable`: this is a process-global singleton. `lastCrash`
    /// is only ever written from the installed uncaught-exception / signal
    /// handler, a single serialized entry point, so cross-task shared mutable
    /// state is not a concurrency hazard in practice.
    public private(set) var lastCrash: FluxError?

    public static let shared = CrashReporter()

    /// The embedded source map (nil until `installSourceMap()` has run).
    private var sourceMap: CodegenSourceMap?

    private init() {}

    /// Loads a codegen source-map sidecar (F17) so subsequent crashes resolve
    /// their top frames to `.flux` source. Safe to call multiple times; the
    /// last loaded map wins. Missing or malformed data is silently ignored
    /// (a reporter must never crash the app it reports on).
    ///
    /// - Parameters:
    ///   - name: The sidecar basename (no extension), e.g. `MainActivity` or
    ///     `HelloApp`. The reporter tries `<name>.flux.map.json` in the main
    ///     bundle first, then falls back to walking every bundled
    ///     `.flux.map.json` if the requested name is not found.
    public func installSourceMap(named name: String) {
        sourceMap = loadSourceMap(named: name)
    }

    /// Convenience: loads the first `.flux.map.json` in the main bundle. Use
    /// when the app ships exactly one generated file (the common case).
    public func installSourceMapFromBundle() {
        guard let url = Bundle.main.urls(forResourcesWithExtension: "json", subdirectory: nil)?
            .first(where: { $0.lastPathComponent.hasSuffix(".flux.map.json") })
        else {
            return
        }
        sourceMap = decode(url: url)
    }

    private func loadSourceMap(named name: String) -> CodegenSourceMap? {
        // Try the exact name first.
        if let url = Bundle.main.url(forResource: "\(name).flux.map", withExtension: "json"),
           let map = decode(url: url) {
            return map
        }
        // Fall back to any bundled source-map.
        if let url = Bundle.main.urls(forResourcesWithExtension: "json", subdirectory: nil)?
            .first(where: { $0.lastPathComponent == "\(name).flux.map.json" }),
           let map = decode(url: url) {
            return map
        }
        return nil
    }

    private func decode(url: URL) -> CodegenSourceMap? {
        guard let data = try? Data(contentsOf: url) else { return nil }
        return try? JSONDecoder().decode(CodegenSourceMap.self, from: data)
    }

    /// Records a crash as a `FluxError`. In a full build this is invoked from
    /// the installed `NSSetUncaughtExceptionHandler` / signal handler; here it
    /// is the public entry point the shell wires up. Never throws.
    public func report(message: String, callSites: [String] = []) {
        let excerpt = resolveExcerpt(from: callSites)
        lastCrash = FluxError(
            message: message,
            kind: .runtime,
            excerpt: excerpt,
            callSites: callSites
        )
    }

    /// Walks the top N frames of the crash and returns the first entry whose
    /// `native_symbol` appears in a frame line. Returns nil when no map is
    /// loaded or no frame matches (the fall-through behavior the reporter had
    /// before F17 — the raw `callSites` are always preserved on the error).
    ///
    /// Frames beyond the first few are the SDK/dyld tail; bounding the walk
    /// keeps the reporter fast and the resolution focused on the app code.
    private func resolveExcerpt(from callSites: [String]) -> FluxErrorExcerpt? {
        guard let map = sourceMap else { return nil }
        let topFrames = callSites.prefix(8)
        for frame in topFrames {
            for entry in map.component_map {
                // A frame line looks like
                //   "0   FluxApp  0x0000000104a10b88 Hello.body.getter + 1"
                // so a substring search on the symbol name is the portable
                // check across OS versions.
                if frame.contains(entry.native_symbol) {
                    return FluxErrorExcerpt(
                        path: entry.source_file,
                        line: entry.line,
                        column: entry.col,
                        snippet: ""
                    )
                }
            }
        }
        return nil
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
