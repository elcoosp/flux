//  DebugMetadata.swift
//  DevTools native-view inspection (spec §7, ADR-0031).
//
//  Extends `FluxAdapter` with an optional `#if DEBUG` hook so DevTools can read
//  layout frames and native properties of a rendered view without coupling the
//  tool to `UIView` internals. Placed in its own file so it never touches the
//  in-flight `AdapterKit.swift` / `ContainerAdapter.swift`.

import UIKit

/// Debug metadata for a native view instance, surfaced to DevTools.
public struct NativeViewDebugMetadata {
    /// The view's layout frame in its superview's coordinate space.
    public let frame: CGRect
    /// The view's background color, if set.
    public let backgroundColor: UIColor?
    /// The view's accessibility label, if set.
    public let accessibilityLabel: String?

    public init(frame: CGRect, backgroundColor: UIColor?, accessibilityLabel: String?) {
        self.frame = frame
        self.backgroundColor = backgroundColor
        self.accessibilityLabel = accessibilityLabel
    }
}

#if DEBUG
extension FluxAdapter {
    /// Returns debug metadata for a native view instance, or `nil` if the
    /// adapter does not support inspection.
    ///
    /// The default implementation returns `nil`; adapters override this to
    /// expose their view's layout frame and native properties (spec §7.1).
    func inspectDebugState(of view: View) -> NativeViewDebugMetadata? {
        nil
    }
}

extension ContainerAdapter {
    /// Inspects the container `UIView`'s frame, background, and label (spec §7.2).
    public func inspectDebugState(of view: UIView) -> NativeViewDebugMetadata? {
        NativeViewDebugMetadata(
            frame: view.frame,
            backgroundColor: view.backgroundColor,
            accessibilityLabel: view.accessibilityLabel
        )
    }
}
#endif
