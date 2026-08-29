//  FluxAdapter.swift
//  FluxUIKit — the adapter protocol (Appendix F) plus the shared handler bridge.

import UIKit

/// A bidirectional bridge between an IR node and a native UIKit view.
///
/// The runtime (FLUX-006) drives an adapter through this lifecycle:
/// `create()` once, then `update(_:from:to:)` on every prop diff,
/// `setChildren(_:on:)` when the child list changes, `bindHandler(_:to:nodeId:)`
/// when a handler closure is (re)bound, and `destroy(_:)` when the node leaves
/// the tree. The **props are the contract** (Appendix F) — both dev and
/// release adapters consume the same `Props`.
///
/// Adapters are `AnyObject` because they hold a `weak` reference to the
/// executor and are retained by the runtime's shadow node for their lifetime.
@MainActor
public protocol FluxAdapter: AnyObject {
    /// The concrete UIKit view (or view-controller) type this adapter manages.
    associatedtype View: AnyObject

    /// Create a fresh, unconfigured view.
    func create() -> View

    /// Apply the diff from `old` to `new` onto `view`.
    func update(_ view: View, from old: Props, to new: Props)

    /// Reconcile `view`'s children to `children` (keyed by native-view identity,
    /// which the runtime guarantees stable per node id). A child is either a
    /// `UIView` (leaf/container) or a `UIViewController` (a `Screen`).
    func setChildren(_ children: [AnyObject], on view: View)

    /// Bind `handlerId` to `view`, scoped to `nodeId`. Subsequent native events
    /// dispatch a `FluxEvent` to the adapter's weak `executor`.
    func bindHandler(_ handlerId: FluxHandlerId, to view: View, nodeId: FluxNodeId)

    /// Tear down any bindings on `view` before it is released.
    func destroy(_ view: View)
}

/// An `NSObject` target that forwards a UIControl action to a weak executor.
///
/// A single instance is created per `bindHandler` call and kept alive by the
/// owning adapter so its `fire()` selector remains reachable. It holds only a
/// `weak` executor, so it can never resurrect a deallocated runtime.
@MainActor
final class HandlerTarget: NSObject {
    weak var executor: (any FluxExecutor)?
    let handlerId: FluxHandlerId
    let nodeId: FluxNodeId
    let payload: () -> FluxValue?

    init(executor: (any FluxExecutor)?, handlerId: FluxHandlerId, nodeId: FluxNodeId, payload: @escaping () -> FluxValue?) {
        self.executor = executor
        self.handlerId = handlerId
        self.nodeId = nodeId
        self.payload = payload
    }

    /// UIControl entry point. Always invoked on the main thread by UIKit, so we
    /// assert main isolation before touching the `@MainActor` executor.
    @objc func fire() {
        MainActor.assumeIsolated {
            #if DEBUG
            NSLog("[FluxRT] HandlerTarget.fire handlerId=\(handlerId) nodeId=\(nodeId) executor=\(executor != nil ? "set" : "nil")")
            #endif
            executor?.dispatch(FluxEvent(handlerId: handlerId, nodeId: nodeId, payload: payload()))
        }
    }
}

/// Surfaces FLUX-044 accessibility props (`label`, `role`, `focusOrder`) onto a
/// UIKit view's accessibility element.
///
/// These props are host-render-only (no wire field); the dev server never sends
/// them as a distinct field, so they are resolved by name from the same FNV-1a
/// index space the server uses for every prop (AGENTS.md §3.2). Missing props
/// are no-ops (degrade to default, never throw — §3.5).
@MainActor
func applyAccessibility(_ props: Props, to view: UIView) {
    if let label = props.getAccessibilityLabel() {
        view.accessibilityLabel = label
    }
    if let role = props.getAccessibilityRole() {
        // UIKit exposes a fixed set of `UIAccessibilityTraits`; unknown roles
        // fall back to the generic `.none` trait rather than throwing.
        view.accessibilityTraits = UIAccessibilityTraits.fromFluxRole(role)
    }
    if let order = props.getAccessibilityFocusOrder() {
        view.accessibilityHint = "focusOrder:\(order)"
    }
}

extension UIAccessibilityTraits {
    /// Maps a Flux `role` string to the closest UIKit accessibility trait.
    static func fromFluxRole(_ role: String) -> UIAccessibilityTraits {
        switch role.lowercased() {
        case "button": return .button
        case "link": return .link
        case "header", "title": return .header
        case "image": return .image
        case "search": return .searchField
        case "text", "textfield": return .staticText
        default: return .none
        }
    }
}
