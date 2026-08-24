//  Alignment.swift
//  FluxUIKit — alignment value (Appendix F `Alignment`).

import UIKit

/// A horizontal alignment in the adapter layer, decoded from a Flux
/// `Alignment` record.
///
/// Canonical encoding: a string `horizontal` field with value `"start"`,
/// `"center"`, or `"end"`. Field index is `AlignmentField.horizontal`.
public struct FluxAlignment: Sendable, Hashable {
    /// The horizontal alignment bias.
    public enum Horizontal: String, Sendable, Hashable {
        /// Leading edge.
        case start = "start"
        /// Centered.
        case center = "center"
        /// Trailing edge.
        case end = "end"
    }

    /// The resolved horizontal alignment.
    public let horizontal: Horizontal

    /// Construct with a default of `.start`.
    public init(horizontal: Horizontal = .start) { self.horizontal = horizontal }

    /// Decode from a record using `AlignmentField` indices.
    public init?(record: Props) {
        let h = record.getString(AlignmentField.horizontal.rawValue)
            .flatMap(Horizontal.init(rawValue:)) ?? .start
        self.init(horizontal: h)
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
