//  RowAdapter.swift
//  FluxUIKit — `Row` → `UIStackView(horizontal)` (Appendix F.4).

import UIKit

/// Dev adapter mapping a Flux `Row` node to a horizontal `UIStackView`.
///
/// Prop fields (Appendix F.4): `0 gap: Float = 0`, `1 alignment: Option[Alignment]`.
/// Child reconciliation reuses the shared `reconcileChildren` keyed-by-identity
/// algorithm from `ColumnAdapter` so both containers preserve view state across
/// reorderings.
public final class RowAdapter: FluxAdapter {
    public typealias View = UIStackView
    weak var executor: (any FluxExecutor)?

    public init(executor: (any FluxExecutor)? = nil) { self.executor = executor }

    public func create() -> UIStackView {
        let stack = UIStackView()
        stack.axis = .horizontal
        stack.translatesAutoresizingMaskIntoConstraints = false
        return stack
    }

    public func update(_ view: UIStackView, from old: Props, to new: Props) {
        view.spacing = CGFloat(new.getFloat(named: "gap") ?? 0)
        if let align = new.getRecord(named: "alignment").flatMap(FluxAlignment.init(record:)) {
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
