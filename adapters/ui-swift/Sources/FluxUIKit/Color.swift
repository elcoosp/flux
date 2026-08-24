//  Color.swift
//  FluxUIKit — color value (Appendix F `Color`).

import UIKit

/// A color in the adapter layer, decoded from a Flux `Color` record.
///
/// Canonical encoding (the contract the runtime maps stdlib `Color` into): a
/// record with float fields `r`, `g`, `b` in `0...1` and optional `a` (alpha,
/// default `1`). Field indices are `ColorField`. Channel values are clamped to
/// `0...1` so a corrupt or out-of-range payload cannot produce an invalid
/// `UIColor`.
public struct FluxColor: Sendable, Hashable {
    /// Red channel, `0...1`.
    public let red: Double
    /// Green channel, `0...1`.
    public let green: Double
    /// Blue channel, `0...1`.
    public let blue: Double
    /// Alpha channel, `0...1`.
    public let alpha: Double

    /// Construct a color, clamping each channel into `0...1`.
    public init(red: Double, green: Double, blue: Double, alpha: Double = 1) {
        self.red = min(max(red, 0), 1)
        self.green = min(max(green, 0), 1)
        self.blue = min(max(blue, 0), 1)
        self.alpha = min(max(alpha, 0), 1)
    }

    /// Decode from a record using `ColorField` indices. Returns `nil` when the
    /// required `r`/`g`/`b` channels are absent.
    public init?(record: Props) {
        guard let r = record.getFloat(ColorField.r.rawValue),
              let g = record.getFloat(ColorField.g.rawValue),
              let b = record.getFloat(ColorField.b.rawValue) else { return nil }
        let a = record.getFloat(ColorField.a.rawValue) ?? 1
        self.init(red: r, green: g, blue: b, alpha: a)
    }

    /// The equivalent UIKit color.
    public var uiColor: UIColor {
        UIColor(red: CGFloat(red), green: CGFloat(green), blue: CGFloat(blue), alpha: CGFloat(alpha))
    }
}

/// Field indices for the canonical `Color` record encoding.
public enum ColorField: UInt16 {
    /// Red channel.
    case r = 0
    /// Green channel.
    case g = 1
    /// Blue channel.
    case b = 2
    /// Alpha channel.
    case a = 3
}
