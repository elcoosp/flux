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
        // Rebuild the child list from scratch: remove every current subview, then
        // add the resolved children in order, pinned to the container edges.
        view.subviews.forEach { $0.removeFromSuperview() }
        for v in views {
            view.addSubview(v)
            // Fill WIDTH (mirrors Android's `fillMaxWidth`) but HUG the content
            // HEIGHT so the child stays top-anchored instead of being stretched to
            // the container's full height. The bottom edge is pinned at LOW
            // priority: the high-priority top pin + the child's required vertical
            // hugging keep the child packed at the top. Pinning bottom at required
            // would fight the hug and UIKit would resolve the ambiguity by pushing
            // the content to the bottom (the "big gap, content at the bottom" bug).
            v.setContentHuggingPriority(.required, for: .vertical)
            let bottom = v.bottomAnchor.constraint(equalTo: view.bottomAnchor)
            bottom.priority = .defaultLow
            NSLayoutConstraint.activate([
                v.leadingAnchor.constraint(equalTo: view.leadingAnchor),
                v.trailingAnchor.constraint(equalTo: view.trailingAnchor),
                v.topAnchor.constraint(equalTo: view.topAnchor),
                bottom,
            ])
        }
    }

    public func bindHandler(_ handlerId: FluxHandlerId, to view: UIView, nodeId: FluxNodeId) {}

    public func destroy(_ view: UIView) {
        view.subviews.forEach { $0.removeFromSuperview() }
    }
}
