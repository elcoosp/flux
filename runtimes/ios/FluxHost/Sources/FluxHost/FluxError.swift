//  FluxError.swift
//  iOS mirror of PRD-K's `FluxError` + `SourceSpan` (crates/flux-types/src/error.rs),
//  consumed by the native error overlay (FLUX-028) and crash reporter (FLUX-035).
//
//  One error shape across host + DevTools (PRD-O user story 8). The wire carries a
//  span-bearing error field (PRD-K); this struct is what the host decodes it into.
//  ADR-0049 does not rename these (they are new iOS-native types, not mirrors of a
//  host type that already drifted).
//
//  FLUX-075 / ADR-0057 widen this to the eight-value `FluxErrorKind` taxonomy and
//  add `excerpt` (a server-computed `path:line:col` + snippet) so a fault is
//  traceable to `.flux` source on-device without a round-trip.

import Foundation

/// A server-computed source excerpt (ADR-0057) ready for presentation: the
/// resolved file path plus the cited line/column and the offending source line.
public struct FluxErrorExcerpt: Equatable, Sendable {
    /// Resolved source-file path (e.g. `src/Counter.flux`).
    public let path: String
    /// 1-based line within `path`.
    public let line: UInt32
    /// 1-based column within the cited line.
    public let column: UInt32
    /// The cited source line, trimmed.
    public let snippet: String

    public init(path: String, line: UInt32, column: UInt32, snippet: String) {
        self.path = path
        self.line = line
        self.column = column
        self.snippet = snippet
    }
}

/// A source location in a `.flux` file, decoded from a wire `Span` (PRD-K).
public struct SourceSpan: Equatable, Sendable {
    /// The interned source-file id (resolve through the string table).
    public let fileID: UInt32
    /// 1-based line, or 0 when unknown.
    public let line: UInt32
    /// 1-based column, or 0 when unknown.
    public let column: UInt32

    public init(fileID: UInt32, line: UInt32, column: UInt32) {
        self.fileID = fileID
        self.line = line
        self.column = column
    }
}

/// The category of a Flux fault, mirroring `VmErrorKind` + wire/host variants.
public enum FluxErrorKind: String, Equatable, Sendable {
    case parse = "ParseError"
    case type = "TypeError"
    case wire = "WireError"
    case vm = "VmError"
    case runtime = "RuntimeError"
    case capability = "CapabilityError"
    case compile = "CompileError"
    case server = "ServerError"
}

/// A Flux fault with a human-readable message, a category, an optional
/// highlighted source span, a presentation-ready excerpt, and a formatted
/// dispatch stack (PRD-K + FLUX-028 + ADR-0057).
public struct FluxError: Equatable, Sendable {
    /// What went wrong (what/why/how from PRD-K).
    public let message: String
    /// The fault category.
    public let kind: FluxErrorKind
    /// The highlighted `.flux` source span, when available.
    public let span: SourceSpan?
    /// A presentation-ready source excerpt (ADR-0057), when the server shipped one.
    public let excerpt: FluxErrorExcerpt?
    /// A formatted stack through handler dispatch (telemetry `call_sites`).
    public let callSites: [String]

    public init(
        message: String,
        kind: FluxErrorKind,
        span: SourceSpan? = nil,
        excerpt: FluxErrorExcerpt? = nil,
        callSites: [String] = []
    ) {
        self.message = message
        self.kind = kind
        self.span = span
        self.excerpt = excerpt
        self.callSites = callSites
    }

    /// A one-line summary used by the crash reporter and logs.
    public var summary: String {
        "\(kind.rawValue): \(message)"
    }
}
