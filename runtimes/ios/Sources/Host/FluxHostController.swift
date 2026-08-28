//  FluxHostController.swift
//  The render-mount shell (FA-RENDER Phase A): presents the reconciler's
//  native UIKit tree on screen.
//
//  The executor owns the signal graph, VM and reconciler and drives the real
//  `FluxUIKit` adapters (FLUX-016), producing a tree of real `UIView`s keyed
//  by stable node id. This controller is a thin presentation layer: it mounts
//  the executor's current `rootView` and re-presents it whenever the reconciler
//  builds or updates the native tree. It owns no graph or transport of its own.

import Foundation
import UIKit
import FluxHost

/// A `UIViewController` that mounts the reconciler's root `UIView` and keeps it
/// in sync as frames arrive from the executor.
///
/// The mount survives the reconciler's per-dispatch updates because the
/// reconciler mutates the existing native views in place (Appendix C §C.1 node
/// id stability); this controller only adds the root view once and then leaves
/// the subtree to the adapters. The error overlay (Appendix E §E.6) is drawn
/// by the SwiftUI `FluxRootView` above this controller, so a VM/wire fault
/// never crashes the host.
@MainActor
final class FluxHostController: UIViewController {
    /// The executor whose reconciled tree this controller presents.
    private let executor: FluxRuntime

    /// Creates a controller bound to `executor`.
    init(executor: FluxRuntime) {
        self.executor = executor
        super.init(nibName: nil, bundle: nil)
    }

    @available(*, unavailable, message: "FluxHostController is constructed programmatically")
    required init?(coder: NSCoder) {
        fatalError("init(coder:) is unavailable")
    }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .systemBackground
        // Re-present whenever the reconciler builds or updates the native tree.
        // The closure is `@MainActor`, matching `onTreeChanged`, so touching the
        // view hierarchy here is isolated to the main actor (P1 confinement).
        executor.onTreeChanged = { [weak self] in
            self?.presentRoot()
        }
        presentRoot()
    }

    /// Mounts the executor's current root view, filling the controller's view.
    ///
    /// The root view is added exactly once; subsequent reconciliations mutate the
    /// subtree in place, so an already-mounted root is left untouched (no
    /// re-parenting churn, no dropped view state). Only when the tree has no
    /// mounted root (first frame) is the view attached.
    private func presentRoot() {
        guard let root = executor.rootView else {
            #if DEBUG
            NSLog("[FluxRT] presentRoot: rootView is nil, nothing to mount")
            #endif
            return
        }
        #if DEBUG
        NSLog("[FluxRT] presentRoot: mounting root \(root)")
        #endif
        if root.superview === view { return }
        view.subviews.forEach { $0.removeFromSuperview() }
        root.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(root)
        NSLayoutConstraint.activate([
            root.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            root.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            root.topAnchor.constraint(equalTo: view.topAnchor),
            root.bottomAnchor.constraint(equalTo: view.bottomAnchor),
        ])
    }

    /// Detaches the presentation hook (called on `onDisappear`); the next mount
    /// re-establishes it. Does not tear down the executor's graph — that is the
    /// session's responsibility.
    func detach() {
        executor.onTreeChanged = nil
    }
}
