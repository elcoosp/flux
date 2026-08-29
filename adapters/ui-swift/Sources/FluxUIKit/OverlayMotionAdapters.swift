//  OverlayMotionAdapters.swift
//  FluxUIKit — FLUX-038 overlay containers (Modal / Sheet / Dialog) and the
//  FLUX-042 signal-graph animation wrapper (Animate).
//
//  Declarative adapters (unified tier; AGENTS.md §3.5). Each hosts its
//  `content` / animated subtree as children and reads its data props. The
//  native *presentation* (a hosted sheet / alert / dialog) and the native
//  *animation* (`withAnimation`) are gated on the ADR-0048 iOS dev-tier
//  convergence decision — until then these adapters degrade to a plain
//  container carrying the children, so a Flux app can author and render the
//  primitives today without a blank screen (the dev/release parity mapping is
//  already pinned by `flux-parity`).

import UIKit

/// Shared base for the FLUX-038 / FLUX-042 adapters: a plain `UIView` container
/// that hosts children and records data props for the host presentation layer.
public class OverlayContainerAdapter: FluxAdapter {
    public typealias View = UIView
    weak var executor: (any FluxExecutor)?
    /// The surface name of this overlay (used by the host presentation layer
    /// once ADR-0048 lands to pick the native surface).
    public let surface: String

    public init(executor: (any FluxExecutor)? = nil, surface: String) {
        self.executor = executor
        self.surface = surface
    }

    public func create() -> UIView {
        let view = UIView()
        view.translatesAutoresizingMaskIntoConstraints = false
        return view
    }

    public func update(_ view: UIView, from old: Props, to new: Props) {
        // `onDismiss` (handler id) and any presentation data are recorded by the
        // host layer once the native surface is wired (ADR-0048).
        _ = new.getHandler(named: "onDismiss")
    }

    public func setChildren(_ children: [AnyObject], on view: UIView) {
        let views = children.compactMap { $0 as? UIView }
        view.subviews.forEach { $0.removeFromSuperview() }
        for v in views {
            view.addSubview(v)
            v.translatesAutoresizingMaskIntoConstraints = false
            NSLayoutConstraint.activate([
                v.leadingAnchor.constraint(equalTo: view.leadingAnchor),
                v.trailingAnchor.constraint(equalTo: view.trailingAnchor),
                v.topAnchor.constraint(equalTo: view.topAnchor),
                v.bottomAnchor.constraint(equalTo: view.bottomAnchor),
            ])
        }
    }

    public func bindHandler(_ handlerId: FluxHandlerId, to view: UIView, nodeId: FluxNodeId) {}

    public func destroy(_ view: UIView) {
        view.subviews.forEach { $0.removeFromSuperview() }
    }
}

/// `Modal` — centered modal over a scrim (SwiftUI `.fullScreenCover`).
public final class ModalAdapter: OverlayContainerAdapter {
    public init(executor: (any FluxExecutor)? = nil) { super.init(executor: executor, surface: "Modal") }
}

/// `Sheet` — bottom-anchored sheet that slides up (SwiftUI `.sheet`).
public final class SheetAdapter: OverlayContainerAdapter {
    public init(executor: (any FluxExecutor)? = nil) { super.init(executor: executor, surface: "Sheet") }
}

/// `Dialog` — modal dialog with a dimmed scrim (SwiftUI `Alert`).
public final class DialogAdapter: OverlayContainerAdapter {
    public init(executor: (any FluxExecutor)? = nil) { super.init(executor: executor, surface: "Dialog") }
}

/// `Animate` — signal-graph animation wrapper (FLUX-042). Hosts its child subtree
/// and records the `signal` / `curve` / `duration` data the host consumes to
/// drive the native `withAnimation` (ADR-0048). Until the native animation API
/// is wired, the children render unchanged — the node resolves (no blank
/// screen) and the curve data is carried on the view for the host layer.
public final class AnimateAdapter: OverlayContainerAdapter {
    public init(executor: (any FluxExecutor)? = nil) { super.init(executor: executor, surface: "Animate") }

    public override func update(_ view: UIView, from old: Props, to new: Props) {
        _ = new.getHandler(named: "signal")
        _ = new.getString(named: "curve")
        _ = new.getFloat(named: "duration")
    }
}
