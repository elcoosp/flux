//  ContainerAdapter.swift
//  FluxUIKit — `Component` → plain `UIView` container (Appendix F).
//
//  The dev server lowers a top-level component (e.g. `Counter`) to a
//  `Component` root node whose `componentId` is the interned component name.
//  No primitive adapter is registered for that id, so the host registry falls
//  back to this container: it hosts the component's children in a plain
//  `UIView` without interpreting any props. This keeps the reconciler uniform
//  — every node, primitive or component, flows through `registry.make` and
//  `setChildren` (Appendix F).

import UIKit

/// Declarative adapter (unified tier; AGENTS.md §3.5) mapping a Flux
/// `Component` node to a plain `UIView` that simply
/// hosts its children. User components carry no host-native props of their own;
/// their visual content is entirely their descendant primitives.
public final class ContainerAdapter: FluxAdapter {
    public typealias View = UIView
    weak var executor: (any FluxExecutor)?

    public init(executor: (any FluxExecutor)? = nil) { self.executor = executor }

    public func create() -> UIView {
        let view = UIView()
        view.translatesAutoresizingMaskIntoConstraints = false
        return view
    }

    public func update(_ view: UIView, from old: Props, to new: Props) {}

    public func setChildren(_ children: [AnyObject], on view: UIView) {
        let views = children.compactMap { $0 as? UIView }
        // Rebuild the child list from scratch. Multiple children pinned to the
        // same four edges would overdraw each other (the old behavior); host
        // two or more in a vertical UIStackView instead, and preserve the
        // pinned top-hugging single-child shape that ContainerAdapter relies on.
        view.subviews.forEach { $0.removeFromSuperview() }
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
        view.addSubview(hosted)
        // Fill WIDTH but HUG the content HEIGHT (see comment above the pin).
        hosted.setContentHuggingPriority(.required, for: .vertical)
        let bottom = hosted.bottomAnchor.constraint(equalTo: view.bottomAnchor)
        bottom.priority = .defaultLow
        NSLayoutConstraint.activate([
            hosted.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            hosted.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            hosted.topAnchor.constraint(equalTo: view.topAnchor),
            bottom,
        ])
    }

    public func bindHandler(_ handlerId: FluxHandlerId, to view: UIView, nodeId: FluxNodeId) {}

    public func destroy(_ view: UIView) {
        view.subviews.forEach { $0.removeFromSuperview() }
    }
}
