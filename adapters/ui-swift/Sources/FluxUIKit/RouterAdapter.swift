//  RouterAdapter.swift
//  FluxUIKit — `Router` → `UIView` hosting a `UINavigationController` (Appendix F.6).
//
//  The router has no props (Appendix F.6). Its `setChildren` receives the list
//  of screen view-controllers and reconciles them onto the navigation stack by
//  **identity**, preserving a screen's view controller (and its subtree state)
//  when it remains present, pushing new ones, and popping ones that disappear.
//  A screen that stays on the stack across a patch is never recreated, so its
//  state is preserved per the router contract.
//
//  CRITICAL (iOS layout): every Flux adapter in the tree must expose a `UIView`
//  so the generic `ContainerAdapter.setChildren` (which only accepts `UIView`
//  children) can attach it. The router therefore returns a `RouterHostView` —
//  a plain `UIView` that embeds a `UINavigationController` as a **child view
//  controller** and pins its view to fill. Returning the `UINavigationController`
//  directly made the generic container drop the router (a `UIViewController` is
//  not a `UIView`), which left the whole tree unmounted and blank.

import UIKit

/// A `UIView` that hosts the router's `UINavigationController` as a child view
/// controller, so the router slots into the uniform `UIView`-based tree.
public final class RouterHostView: UIView {
    /// The embedded navigation controller (public for test introspection).
    public let nav: UINavigationController

    init(nav: UINavigationController) {
        self.nav = nav
        super.init(frame: .zero)
        self.translatesAutoresizingMaskIntoConstraints = false
        backgroundColor = .systemBackground
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) unavailable") }

    /// Pins `nav`'s view to fill `self`. The router returns a `UIView` (not a
    /// `UIViewController`) precisely so the generic `ContainerAdapter` can
    /// attach it; the nav controller's view is added as a plain pinned subview,
    /// which displays the navigation bar and active screen in the dev renderer.
    func embedNavController() {
        guard nav.parent == nil else { return }
        addSubview(nav.view)
        nav.view.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            nav.view.leadingAnchor.constraint(equalTo: leadingAnchor),
            nav.view.trailingAnchor.constraint(equalTo: trailingAnchor),
            nav.view.topAnchor.constraint(equalTo: topAnchor),
            nav.view.bottomAnchor.constraint(equalTo: bottomAnchor),
        ])
    }
}

public final class RouterAdapter: FluxAdapter {
    public typealias View = RouterHostView
    weak var executor: (any FluxExecutor)?

    public init(executor: (any FluxExecutor)? = nil) { self.executor = executor }

    public func create() -> RouterHostView {
        RouterHostView(nav: UINavigationController())
    }

    public func update(_ view: RouterHostView, from old: Props, to new: Props) {}

    public func setChildren(_ children: [AnyObject], on view: RouterHostView) {
        guard let screens = children as? [UIViewController] else {
            // Screens are always UIViewController; a non-VC child is a runtime bug.
            return
        }
        // A `Router` presents exactly ONE screen (the active route); the
        // reconciler already filtered `children` down to that single screen, so
        // the whole nav stack is always replaced, never pushed. Pushing would
        // re-add the screen that is already the nav's root and UIKit's
        // `_sanityCheckPushViewController` aborts the process (SIGABRT) — which is
        // exactly what blanked the iOS app. Replacing the stack swaps the visible
        // screen cleanly with no animation race (mirrors Android's
        // `setChildren(view, listOf(active.id), listOf(active.view))`).
        view.nav.setViewControllers(screens, animated: false)
        // Embed the nav controller's view into the host view (idempotent).
        view.embedNavController()
    }

    public func bindHandler(_ handlerId: FluxHandlerId, to view: RouterHostView, nodeId: FluxNodeId) {}

    public func destroy(_ view: RouterHostView) {
        view.nav.viewControllers.forEach { $0.dismiss(animated: false) }
        view.nav.setViewControllers([], animated: false)
        view.nav.willMove(toParent: nil)
        view.nav.view.removeFromSuperview()
        view.nav.removeFromParent()
    }
}
