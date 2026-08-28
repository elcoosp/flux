//  Font.swift
//  FluxUIKit — font value (Appendix F `Font`).

import UIKit

/// A font in the adapter layer, decoded from a Flux `Font` record.
///
/// Canonical encoding: a float `size` (points) and an optional string
/// `weight`. Field indices are `FontField`. When `size` is absent the system
/// default of 14 points is used.
public struct FluxFount: Sendable, Hashable {
    /// Point size.
    public let size: Double
    /// Weight, mapped to `UIFont.Weight`.
    public let weight: Weight

    /// The named font weights Flux understands.
    public enum Weight: String, Sendable, Hashable {
        /// Ultra light.
        case ultraLight = "ultralight"
        /// Thin.
        case thin = "thin"
        /// Light.
        case light = "light"
        /// Regular (default).
        case regular = "regular"
        /// Medium.
        case medium = "medium"
        /// Semibold.
        case semibold = "semibold"
        /// Bold.
        case bold = "bold"
        /// Heavy.
        case heavy = "heavy"
        /// Black.
        case black = "black"
    }

    /// Construct a font.
    public init(size: Double, weight: Weight = .regular) {
        self.size = size
        self.weight = weight
    }

    /// Decode from a record using `FontField` indices.
    public init?(record: Props) {
        let size = record.getFloat(FontField.size.rawValue) ?? 14
        let weight = record.getString(FontField.weight.rawValue)
            .flatMap(Weight.init(rawValue:)) ?? .regular
        self.init(size: size, weight: weight)
    }

    /// The equivalent UIKit font.
    public var uiFont: UIFont {
        UIFont.systemFont(ofSize: CGFloat(size), weight: weight.uiWeight)
    }
}

extension FluxFount.Weight {
    /// Map to a `UIFont.Weight`.
    var uiWeight: UIFont.Weight {
        switch self {
        case .ultraLight: .ultraLight
        case .thin: .thin
        case .light: .light
        case .regular: .regular
        case .medium: .medium
        case .semibold: .semibold
        case .bold: .bold
        case .heavy: .heavy
        case .black: .black
        }
    }
}

/// Field indices for the canonical `Font` record encoding.
public enum FontField: UInt16 {
    /// Point size.
    case size = 0
    /// Weight name (see `FluxFount.Weight`).
    case weight = 1
}
