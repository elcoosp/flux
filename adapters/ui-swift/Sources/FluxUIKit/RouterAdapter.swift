//  RouterAdapter.swift
//  FluxUIKit — `Router` → `UINavigationController` (Appendix F.6).

import UIKit

/// Dev adapter mapping a Flux `Router` node to a `UINavigationController`.
///
/// The router has no props (Appendix F.6). Its `setChildren` receives the list
/// of screen view-controllers and reconciles them onto the navigation stack by
/// **identity**, preserving a screen's view controller (and its subtree state)
/// when it remains present, pushing new ones, and popping ones that disappear.
/// A screen that stays on the stack across a patch is never recreated, so its
/// state is preserved per the router contract.
public final class RouterAdapter: FluxAdapter {
    public typealias View = UINavigationController
    weak var executor: (any FluxExecutor)?

    public init(executor: (any FluxExecutor)? = nil) { self.executor = executor }

    public func create() -> UINavigationController {
        UINavigationController()
    }

    public func update(_ view: UINavigationController, from old: Props, to new: Props) {}

    public func setChildren(_ children: [AnyObject], on view: UINavigationController) {
        guard let screens = children as? [UIViewController] else {
            // Screens are always UIViewController; a non-VC child is a runtime bug.
            return
        }
        reconcileStack(screens, on: view)
    }

    public func bindHandler(_ handlerId: FluxHandlerId, to view: UINavigationController, nodeId: FluxNodeId) {}

    public func destroy(_ view: UINavigationController) {
        view.viewControllers.forEach { $0.dismiss(animated: false) }
        view.setViewControllers([], animated: false)
    }

    /// Push/pop to match `target`, keeping existing view controllers alive.
    private func reconcileStack(_ target: [UIViewController], on nav: UINavigationController) {
        let current = nav.viewControllers
        let targetSet = Set(target)
        let toPop = current.filter { !targetSet.contains($0) }
        guard toPop.isEmpty else {
            // Pop every screen that left the list, preserving the rest in order.
            // We mutate synchronously (`animated: false`) so the reconciler's
            // result is immediately observable by the runtime, with no animation
            // race.
            let kept = current.filter { targetSet.contains($0) }
            nav.setViewControllers(kept, animated: false)
            return
        }
        // No removals: append any new screens in order after the existing stack.
        let existing = current.last.map { [$0] } ?? []
        let additions = target.dropFirst(existing.count)
        guard !additions.isEmpty else { return }
        // First new screen animates the push; the rest are appended without
        // animation so the final stack matches the target immediately.
        nav.pushViewController(additions.first!, animated: false)
        for screen in additions.dropFirst() {
            nav.pushViewController(screen, animated: false)
        }
    }
}
