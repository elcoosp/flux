//  FluxError.swift
//  iOS mirror of PRD-K's `FluxError` + `SourceSpan` (crates/flux-types/src/error.rs),
//  consumed by the native error overlay (FLUX-028) and crash reporter (FLUX-035).
//
//  One error shape across host + DevTools (PRD-O user story 8). The wire carries a
//  span-bearing error field (PRD-K); this struct is what the host decodes it into.
//  ADR-0049 does not rename these (they are new iOS-native types, not mirrors of a
//  host type that already drifted).

import Foundation

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
    case vm = "VmError"
    case wire = "WireError"
    case runtime = "RuntimeError"
    case capability = "CapabilityError"
}

/// A Flux fault with a human-readable message, a category, an optional
/// highlighted source span, and a formatted dispatch stack (PRD-K + FLUX-028).
public struct FluxError: Equatable, Sendable {
    /// What went wrong (what/why/how from PRD-K).
    public let message: String
    /// The fault category.
    public let kind: FluxErrorKind
    /// The highlighted `.flux` source span, when available.
    public let span: SourceSpan?
    /// A formatted stack through handler dispatch (telemetry `call_sites`).
    public let callSites: [String]

    public init(message: String, kind: FluxErrorKind, span: SourceSpan? = nil, callSites: [String] = []) {
        self.message = message
        self.kind = kind
        self.span = span
        self.callSites = callSites
    }

    /// A one-line summary used by the crash reporter and logs.
    public var summary: String {
        "\(kind.rawValue): \(message)"
    }
}
