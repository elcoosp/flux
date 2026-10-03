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
        view.view.subviews.forEach { $0.removeFromSuperview() }
        // A `Screen` contractually hosts a single content subtree. The old
        // loop pinned every child to the same four edges, so multiple children
        // overdrew each other with no warning. Preserve the pinned single-child
        // shape (which the ScreenAdapter relies on for top-hugging), and fall
        // back to a vertical stack when the runtime delivers more than one.
        let hosted: UIView?
        if views.count == 1 {
            hosted = views[0]
        } else if views.count > 1 {
            let stack = UIStackView(arrangedSubviews: views)
            stack.axis = .vertical
            stack.alignment = .fill
            stack.distribution = .fill
            stack.translatesAutoresizingMaskIntoConstraints = false
            hosted = stack
        } else {
            hosted = nil
        }
        guard let hosted = hosted else { return }
        view.view.addSubview(hosted)
        hosted.setContentHuggingPriority(.required, for: .vertical)
        let bottom = hosted.bottomAnchor.constraint(equalTo: view.view.bottomAnchor)
        bottom.priority = .defaultLow
        NSLayoutConstraint.activate([
            hosted.leadingAnchor.constraint(equalTo: view.view.leadingAnchor),
            hosted.trailingAnchor.constraint(equalTo: view.view.trailingAnchor),
            hosted.topAnchor.constraint(equalTo: view.view.topAnchor),
            bottom,
        ])
    }

    public func bindHandler(_ handlerId: FluxHandlerId, to view: UIViewController, nodeId: FluxNodeId) {}

    public func destroy(_ view: UIViewController) {
        view.view.subviews.forEach { $0.removeFromSuperview() }
    }
}
