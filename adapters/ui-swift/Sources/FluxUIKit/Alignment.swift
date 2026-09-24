//  Alignment.swift
//  FluxUIKit — alignment value (Appendix F `Alignment`).

import UIKit

/// A horizontal alignment in the adapter layer, decoded from a Flux
/// `Alignment` record.
///
/// Canonical encoding (Appendix F `Alignment`, audit D6): a positional record
/// `{0: Int}` where 0=start, 1=center, 2=end — the same positional-record
/// contract `Color` uses, so the IR lowers it as a real value (ADTs lower to
/// `Null` in the dev oracle).
public struct FluxAlignment: Sendable, Hashable {
    /// The horizontal alignment bias.
    public enum Horizontal: Int64, Sendable, Hashable {
        /// Leading edge (start).
        case start = 0
        /// Centered.
        case center = 1
        /// Trailing edge (end).
        case end = 2
    }

    /// The resolved horizontal alignment.
    public let horizontal: Horizontal

    /// Construct with a default of `.start`.
    public init(horizontal: Horizontal = .start) { self.horizontal = horizontal }

    /// Decode from a record using `AlignmentField` indices. The record's
    /// positional slot 0 holds the Int alignment value (audit D6).
    public init?(record: Props) {
        let v = record.getInt(AlignmentField.horizontal.rawValue)
            .flatMap(Horizontal.init(rawValue:)) ?? .start
        self.init(horizontal: v)
    }

    /// The equivalent `NSTextAlignment` (for `UILabel`).
    public var textAlignment: NSTextAlignment {
        switch horizontal {
        case .start: .left
        case .center: .center
        case .end: .right
        }
    }

    /// The equivalent `UIStackView.Alignment` (for `Column`/`Row`).
    public var stackAlignment: UIStackView.Alignment {
        switch horizontal {
        case .start: .leading
        case .center: .center
        case .end: .trailing
        }
    }
}

/// Field indices for the canonical `Alignment` record encoding.
public enum AlignmentField: UInt16 {
    /// Horizontal bias.
    case horizontal = 0
}
