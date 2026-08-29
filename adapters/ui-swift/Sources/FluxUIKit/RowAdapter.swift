//  RowAdapter.swift
//  FluxUIKit — `Row` → `UIStackView(horizontal)` (Appendix F.4).

import UIKit

/// Declarative adapter mapping a Flux `Row` node to a horizontal `UIStackView`
/// (unified tier; AGENTS.md §3.5).
///
/// Props are read by name (`gap`, `alignment`); the index is the FNV-1a-32
/// digest of the name masked to `u16` (`Props.propIndex`), derived identically
/// on server and client (AGENTS.md §3.2) — never a hardcoded positional index.
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
        stack.distribution = .fill
        // Default cross-axis alignment is `.center` (mirrors Android's
        // `Row(fillMaxWidth())` which centers children vertically). The `alignment`
        // prop overrides this in `update`.
        stack.alignment = .center
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
        // Pin every child to its intrinsic size on BOTH axes (see ColumnAdapter:
        // prevents the full-screen parent stretch from distributing slack into the
        // children, so a `Row` packs them leading like Android).
        for child in views {
            child.setContentHuggingPriority(.required, for: .vertical)
            child.setContentHuggingPriority(.required, for: .horizontal)
        }
        reconcileChildren(views, on: view)
    }

    public func bindHandler(_ handlerId: FluxHandlerId, to view: UIStackView, nodeId: FluxNodeId) {}

    public func destroy(_ view: UIStackView) {
        view.arrangedSubviews.forEach { $0.removeFromSuperview() }
    }
}
