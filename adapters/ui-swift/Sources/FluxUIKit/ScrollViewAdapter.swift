//  ScrollViewAdapter.swift
//  FluxUIKit — FLUX-056 `ScrollView` primitive (PRD-N family).
//
//  Declarative adapter mapping a Flux `ScrollView` node to a native
//  `UIScrollView` (unified tier; AGENTS.md §3.5). The `orientation` prop
//  selects the scroll axis ("vertical" default, "horizontal" otherwise) and is
//  recorded for parity with the Android `ScrollViewAdapter.PROP_ORIENTATION`.
//  Props are read by name through the FNV-1a prop index (§3.2) — never a
//  hardcoded positional index. Children are reconciled by identity (the runtime
//  guarantees a stable native view per node id), so reorders never recreate a
//  view.

import UIKit

/// `ScrollView` — a scrollable viewport for its children (SwiftUI
/// `ScrollView`). Mapped to a `UIScrollView`; the `orientation` prop selects
/// the scroll axis. The children are laid out by the reconciler inside the
/// scroll view's content.
public final class ScrollViewAdapter: FluxAdapter {
    public typealias View = UIScrollView
    weak var executor: (any FluxExecutor)?

    public init(executor: (any FluxExecutor)? = nil) { self.executor = executor }

    public func create() -> UIScrollView {
        let scroll = UIScrollView()
        scroll.translatesAutoresizingMaskIntoConstraints = false
        // A content host that fills the scroll view and carries the children.
        let content = UIView()
        content.translatesAutoresizingMaskIntoConstraints = false
        scroll.addSubview(content)
        NSLayoutConstraint.activate([
            content.leadingAnchor.constraint(equalTo: scroll.contentLayoutGuide.leadingAnchor),
            content.trailingAnchor.constraint(equalTo: scroll.contentLayoutGuide.trailingAnchor),
            content.topAnchor.constraint(equalTo: scroll.contentLayoutGuide.topAnchor),
            content.bottomAnchor.constraint(equalTo: scroll.contentLayoutGuide.bottomAnchor),
            content.widthAnchor.constraint(equalTo: scroll.frameLayoutGuide.widthAnchor),
        ])
        return scroll
    }

    public func update(_ view: UIScrollView, from old: Props, to new: Props) {
        let orientation = new.getString(named: "orientation") ?? "vertical"
        // Recorded for parity with Android `ScrollViewAdapter.PROP_ORIENTATION`
        // so the host presentation layer (ADR-0048) reads the same scroll-axis
        // data the Compose host does.
        view.fluxRecord(FluxRecordedProp.orientation, orientation)
    }

    public func setChildren(_ children: [AnyObject], on view: UIScrollView) {
        guard let content = view.subviews.first else { return }
        let views = children.compactMap { $0 as? UIView }
        view.subviews.forEach { $0.removeFromSuperview() }
        for v in views {
            v.translatesAutoresizingMaskIntoConstraints = false
            content.addSubview(v)
            NSLayoutConstraint.activate([
                v.leadingAnchor.constraint(equalTo: content.leadingAnchor),
                v.trailingAnchor.constraint(equalTo: content.trailingAnchor),
                v.topAnchor.constraint(equalTo: content.topAnchor),
                v.bottomAnchor.constraint(equalTo: content.bottomAnchor),
            ])
        }
    }

    public func bindHandler(_ handlerId: FluxHandlerId, to view: UIScrollView, nodeId: FluxNodeId) {}

    public func destroy(_ view: UIScrollView) {
        view.subviews.forEach { $0.removeFromSuperview() }
    }
}
