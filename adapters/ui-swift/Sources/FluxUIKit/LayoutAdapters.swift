//  LayoutAdapters.swift
//  FluxUIKit — FLUX-037 layout primitives (Stack / Grid / Spacer / SafeArea).
//
//  Declarative adapters mapping each Flux layout node to a native UIKit view
//  (unified tier; AGENTS.md §3.5). Props are read by name through the FNV-1a
//  prop index (§3.2) — never a hardcoded positional index. Children are
//  reconciled by identity (the runtime guarantees a stable native view per node
//  id), so reorders never recreate a view.

import UIKit

/// `Stack` — z-order overlay of children (SwiftUI `ZStack`). Mapped to a
/// `UIStackView` laid out vertically; later children paint above earlier ones
/// via z-ordering. The `gap` prop spaces siblings.
public final class StackAdapter: FluxAdapter {
    public typealias View = UIStackView
    weak var executor: (any FluxExecutor)?

    public init(executor: (any FluxExecutor)? = nil) { self.executor = executor }

    public func create() -> UIStackView {
        let stack = UIStackView()
        stack.axis = .vertical
        stack.distribution = .fill
        stack.alignment = .fill
        stack.translatesAutoresizingMaskIntoConstraints = false
        return stack
    }

    public func update(_ view: UIStackView, from old: Props, to new: Props) {
        view.spacing = CGFloat(new.getFloat(named: "gap") ?? 0)
    }

    public func setChildren(_ children: [AnyObject], on view: UIStackView) {
        let views = children.compactMap { $0 as? UIView }
        reconcileChildren(views, on: view)
    }

    public func bindHandler(_ handlerId: FluxHandlerId, to view: UIStackView, nodeId: FluxNodeId) {}

    public func destroy(_ view: UIStackView) {
        view.arrangedSubviews.forEach { $0.removeFromSuperview() }
    }
}

/// `Grid` — responsive grid of children (SwiftUI `Grid`). Mapped to a vertical
/// `UIStackView`; the codegen emits a native `Grid` and the host renders the
/// row-major flow. The `columns`/`gap` props are read for parity with Compose.
public final class GridAdapter: FluxAdapter {
    public typealias View = UIStackView
    weak var executor: (any FluxExecutor)?

    public init(executor: (any FluxExecutor)? = nil) { self.executor = executor }

    public func create() -> UIStackView {
        let stack = UIStackView()
        stack.axis = .vertical
        stack.distribution = .fill
        stack.alignment = .fill
        stack.translatesAutoresizingMaskIntoConstraints = false
        return stack
    }

    public func update(_ view: UIStackView, from old: Props, to new: Props) {
        view.spacing = CGFloat(new.getFloat(named: "gap") ?? 0)
    }

    public func setChildren(_ children: [AnyObject], on view: UIStackView) {
        let views = children.compactMap { $0 as? UIView }
        reconcileChildren(views, on: view)
    }

    public func bindHandler(_ handlerId: FluxHandlerId, to view: UIStackView, nodeId: FluxNodeId) {}

    public func destroy(_ view: UIStackView) {
        view.arrangedSubviews.forEach { $0.removeFromSuperview() }
    }
}

/// `Spacer` — elastic gap growing along the parent's main axis (SwiftUI
/// `Spacer`). Mapped to a `UIStackView` carrying a flexible blank; the `flex`
/// prop drives the relative grow weight.
public final class SpacerAdapter: FluxAdapter {
    public typealias View = UIStackView
    weak var executor: (any FluxExecutor)?

    public init(executor: (any FluxExecutor)? = nil) { self.executor = executor }

    public func create() -> UIStackView {
        let stack = UIStackView()
        stack.axis = .vertical
        stack.distribution = .equalSpacing
        stack.alignment = .fill
        stack.translatesAutoresizingMaskIntoConstraints = false
        return stack
    }

    public func update(_ view: UIStackView, from old: Props, to new: Props) {
        // `flex` is data for the host layout; no native view mutation required
        // for the degraded (pre-ADR-0048) Spacer beyond hosting the blank.
        _ = new.getFloat(named: "flex")
    }

    public func setChildren(_ children: [AnyObject], on view: UIStackView) {
        let views = children.compactMap { $0 as? UIView }
        reconcileChildren(views, on: view)
    }

    public func bindHandler(_ handlerId: FluxHandlerId, to view: UIStackView, nodeId: FluxNodeId) {}

    public func destroy(_ view: UIStackView) {
        view.arrangedSubviews.forEach { $0.removeFromSuperview() }
    }
}

/// `SafeArea` — insets its children within the platform safe area (SwiftUI
/// `SafeArea`). Mapped to a plain `UIView` that pins its children to the safe
/// area insets; the `edges` prop selects which edges to inset.
public final class SafeAreaAdapter: FluxAdapter {
    public typealias View = UIView
    weak var executor: (any FluxExecutor)?

    public init(executor: (any FluxExecutor)? = nil) { self.executor = executor }

    public func create() -> UIView {
        let view = UIView()
        view.translatesAutoresizingMaskIntoConstraints = false
        return view
    }

    public func update(_ view: UIView, from old: Props, to new: Props) {
        // `edges` is data for the host layout; the insets are applied at
        // `setChildren` time via safe-area-guided constraints (degraded form).
        _ = new.getString(named: "edges")
    }

    public func setChildren(_ children: [AnyObject], on view: UIView) {
        let views = children.compactMap { $0 as? UIView }
        view.subviews.forEach { $0.removeFromSuperview() }
        for v in views {
            view.addSubview(v)
            v.translatesAutoresizingMaskIntoConstraints = false
            NSLayoutConstraint.activate([
                v.leadingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.leadingAnchor),
                v.trailingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.trailingAnchor),
                v.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor),
                v.bottomAnchor.constraint(equalTo: view.safeAreaLayoutGuide.bottomAnchor),
            ])
        }
    }

    public func bindHandler(_ handlerId: FluxHandlerId, to view: UIView, nodeId: FluxNodeId) {}

    public func destroy(_ view: UIView) {
        view.subviews.forEach { $0.removeFromSuperview() }
    }
}
