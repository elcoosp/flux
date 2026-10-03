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

    /// The cross-axis constraint pinning the content host to the scroll view's
    /// frame — width-pinning for `vertical`, height-pinning for `horizontal`.
    /// Replaced (not mutated) when the `orientation` prop changes axis.
    private var crossAxisConstraint: NSLayoutConstraint?
    /// Current orientation, so `update` only swaps the cross-axis constraint
    /// when the value actually changes.
    private var currentOrientation: String = "vertical"

    public init(executor: (any FluxExecutor)? = nil) { self.executor = executor }

    public func create() -> UIScrollView {
        let scroll = UIScrollView()
        scroll.translatesAutoresizingMaskIntoConstraints = false
        // A content host that fills the scroll view and carries the children.
        let content = UIView()
        content.translatesAutoresizingMaskIntoConstraints = false
        scroll.addSubview(content)
        // Keep the cross-axis constraint so `update` can swap it when the
        // `orientation` prop switches between vertical and horizontal. The
        // previous code pinned width unconditionally, so "horizontal" could
        // never scroll — a real divergence from the Kotlin adapter which
        // consumes `PROP_ORIENTATION`.
        let widthConstraint = content.widthAnchor.constraint(equalTo: scroll.frameLayoutGuide.widthAnchor)
        NSLayoutConstraint.activate([
            content.leadingAnchor.constraint(equalTo: scroll.contentLayoutGuide.leadingAnchor),
            content.trailingAnchor.constraint(equalTo: scroll.contentLayoutGuide.trailingAnchor),
            content.topAnchor.constraint(equalTo: scroll.contentLayoutGuide.topAnchor),
            content.bottomAnchor.constraint(equalTo: scroll.contentLayoutGuide.bottomAnchor),
            widthConstraint,
        ])
        crossAxisConstraint = widthConstraint
        return scroll
    }

    public func update(_ view: UIScrollView, from old: Props, to new: Props) {
        let orientation = new.getString(named: "orientation") ?? "vertical"
        // Recorded for parity with Android `ScrollViewAdapter.PROP_ORIENTATION`
        // so the host presentation layer (ADR-0048) reads the same scroll-axis
        // data the Compose host does.
        view.fluxRecord(FluxRecordedProp.orientation, orientation)
        if orientation != currentOrientation {
            currentOrientation = orientation
            // Swap the content's cross-axis constraint so the scroll direction
            // matches `orientation`: width-pinned content scrolls vertically,
            // height-pinned content scrolls horizontally.
            crossAxisConstraint?.isActive = false
            if let content = view.subviews.first {
                let newConstraint: NSLayoutConstraint = orientation == "horizontal"
                    ? content.heightAnchor.constraint(equalTo: view.frameLayoutGuide.heightAnchor)
                    : content.widthAnchor.constraint(equalTo: view.frameLayoutGuide.widthAnchor)
                newConstraint.isActive = true
                crossAxisConstraint = newConstraint
                // If the content host currently holds a UIStackView, flip its
                // axis to match. Otherwise leave the (single-child) pin alone.
                if let stack = content.subviews.first as? UIStackView {
                    stack.axis = orientation == "horizontal" ? .horizontal : .vertical
                }
            }
        }
    }

    public func setChildren(_ children: [AnyObject], on view: UIScrollView) {
        guard let content = view.subviews.first else { return }
        let views = children.compactMap { $0 as? UIView }
        // Audit D20/T-316.10: remove only the old arranged subviews, not the
        // content host itself. Removing all subviews (including content) then
        // adding children to the removed content host blanked the scroll view.
        content.subviews.forEach { $0.removeFromSuperview() }
        guard !views.isEmpty else { return }
        // Multiple children pinned to the same four edges would overdraw each
        // other; the scroll view's content is a single flow along the axis, so
        // host two or more in a UIStackView oriented to the current scroll
        // direction. Single-child preserves the previous pinned shape.
        let hosted: UIView
        if views.count == 1 {
            hosted = views[0]
        } else {
            let stack = UIStackView(arrangedSubviews: views)
            stack.axis = currentOrientation == "horizontal" ? .horizontal : .vertical
            stack.alignment = .fill
            stack.distribution = .fill
            hosted = stack
        }
        hosted.translatesAutoresizingMaskIntoConstraints = false
        content.addSubview(hosted)
        NSLayoutConstraint.activate([
            hosted.leadingAnchor.constraint(equalTo: content.leadingAnchor),
            hosted.trailingAnchor.constraint(equalTo: content.trailingAnchor),
            hosted.topAnchor.constraint(equalTo: content.topAnchor),
            hosted.bottomAnchor.constraint(equalTo: content.bottomAnchor),
        ])
    }

    public func bindHandler(_ handlerId: FluxHandlerId, to view: UIScrollView, nodeId: FluxNodeId) {}

    public func destroy(_ view: UIScrollView) {
        view.subviews.forEach { $0.removeFromSuperview() }
    }
}
