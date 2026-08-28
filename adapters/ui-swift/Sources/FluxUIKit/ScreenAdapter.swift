//  ScreenAdapter.swift
//  FluxUIKit — `Screen` → `UIViewController` (Appendix F.7).

import UIKit

/// Declarative adapter mapping a Flux `Screen` node to a `UIViewController`
/// (unified tier; AGENTS.md §3.5).
///
/// A screen has no props of its own (see AGENTS.md §3.5); its single child is the
/// screen's content, hosted in the view controller's root view. Because the
/// runtime reuses the same `UIViewController` instance across patches (keyed by
/// node id), a screen's navigation state and its content's state survive router
/// push/pop and hot-swaps.
public final class ScreenAdapter: FluxAdapter {
    public typealias View = UIViewController
    weak var executor: (any FluxExecutor)?

    public init(executor: (any FluxExecutor)? = nil) { self.executor = executor }

    public func create() -> UIViewController {
        let vc = UIViewController()
        vc.view.backgroundColor = .systemBackground
        return vc
    }

    public func update(_ view: UIViewController, from old: Props, to new: Props) {}

    public func setChildren(_ children: [AnyObject], on view: UIViewController) {
        let views = children.compactMap { $0 as? UIView }
        // Host the screen's content as the single root subview.
        view.view.subviews.forEach { $0.removeFromSuperview() }
        for child in views {
            child.translatesAutoresizingMaskIntoConstraints = false
            view.view.addSubview(child)
            NSLayoutConstraint.activate([
                child.leadingAnchor.constraint(equalTo: view.view.leadingAnchor),
                child.trailingAnchor.constraint(equalTo: view.view.trailingAnchor),
                child.topAnchor.constraint(equalTo: view.view.topAnchor),
                child.bottomAnchor.constraint(equalTo: view.view.bottomAnchor),
            ])
        }
    }

    public func bindHandler(_ handlerId: FluxHandlerId, to view: UIViewController, nodeId: FluxNodeId) {}

    public func destroy(_ view: UIViewController) {
        view.view.subviews.forEach { $0.removeFromSuperview() }
    }
}
