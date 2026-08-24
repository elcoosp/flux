//  ColumnAdapter.swift
//  FluxUIKit — `Column` → `UIStackView(vertical)` (Appendix F.3).

import UIKit

/// Dev adapter mapping a Flux `Column` node to a vertical `UIStackView`.
///
/// Prop fields (Appendix F.3): `0 gap: Float = 0`, `1 alignment: Option[Alignment]`.
/// Children are reconciled by identity: the runtime guarantees each child's
/// native view is stable per node id, so we match on object identity and only
/// insert/remove to reach the target list — no view is recreated on reorder.
public final class ColumnAdapter: FluxAdapter {
    public typealias View = UIStackView
    weak var executor: (any FluxExecutor)?

    public init(executor: (any FluxExecutor)? = nil) { self.executor = executor }

    public func create() -> UIStackView {
        let stack = UIStackView()
        stack.axis = .vertical
        stack.translatesAutoresizingMaskIntoConstraints = false
        return stack
    }

    public func update(_ view: UIStackView, from old: Props, to new: Props) {
        view.spacing = CGFloat(new.getFloat(0) ?? 0)
        if let align = new.getRecord(1).flatMap(FluxAlignment.init(record:)) {
            view.alignment = align.stackAlignment
        }
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

/// Reconcile a stack's arranged subviews to `target` by identity.
///
/// Child views are keyed by the runtime's stable node id (their object
/// identity here), so this performs the minimal insert/remove and reorders in
/// place — never recreating a view that already exists, which would drop
/// its internal state.
@MainActor
func reconcileChildren(_ target: [UIView], on stack: UIStackView) {
    let current = stack.arrangedSubviews
    let targetSet = Set(target)
    for stale in current where !targetSet.contains(stale) {
        stack.removeArrangedSubview(stale)
        stale.removeFromSuperview()
    }
    var index = 0
    for child in target {
        if child.superview !== stack { stack.insertArrangedSubview(child, at: min(index, stack.arrangedSubviews.count)) }
        else if stack.arrangedSubviews.firstIndex(of: child) != index {
            stack.removeArrangedSubview(child)
            stack.insertArrangedSubview(child, at: min(index, stack.arrangedSubviews.count))
        }
        index += 1
    }
}
