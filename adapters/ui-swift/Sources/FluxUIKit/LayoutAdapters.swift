//  LayoutAdapters.swift
//  FluxUIKit — FLUX-037 layout primitives (Stack / Grid / Spacer / SafeArea).
//
//  Declarative adapters mapping each Flux layout node to a native UIKit view
//  (unified tier; AGENTS.md §3.5). Props are read by name through the FNV-1a
//  prop index (§3.2) — never a hardcoded positional index. Children are
//  reconciled by identity (the runtime guarantees a stable native view per node
//  id), so reorders never recreate a view.

import UIKit

/// Recorded data props for a degraded (pre-ADR-0048) layout/overlay adapter.
///
/// The Android kit records each primitive's data props onto a per-node
/// `FluxNativeView` (`FluxNativeView.setProperty`) so the host renderer can
/// consume them. The Swift kit has no `FluxNativeView` type (that is the
/// Android host's internal store), so the same data is recorded here onto the
/// native `UIView` via a documented associated object — mirroring the Android
/// recorded-property shape exactly (FLUX-077 parity). The keys match the
/// Android `PROP_*` constants; the native *presentation* of that data stays
/// gated on ADR-0048 (degraded container form).
enum FluxRecordedProp {
    /// `Stack`/`Grid` inter-child spacing (Android `PROP_GAP` = `gap`).
    static let gap = "gap"
    /// `Grid` column count (Android `PROP_COLUMNS` = `columns`).
    static let columns = "columns"
    /// `Spacer` grow weight (Android `PROP_FLEX` = `flex`).
    static let flex = "flex"
    /// `SafeArea` inset edges (Android `PROP_EDGES` = `edges`).
    static let edges = "edges"
    /// `ScrollView` scroll axis (Android `PROP_ORIENTATION` = `orientation`).
    static let orientation = "orientation"
    /// `Modal`/`Sheet`/`Dialog` onDismiss handler id (Android `PROP_ON_DISMISS`).
    static let onDismiss = "onDismiss"
    /// `Animate` signal handler id (Android `PROP_SIGNAL`).
    static let signal = "signal"
    /// `Animate` easing curve (Android `PROP_CURVE`).
    static let curve = "curve"
    /// `Animate` duration in seconds (Android `PROP_DURATION`).
    static let duration = "duration"
}

extension UIView {
    /// The recorded data-prop bag for degraded adapters (FLUX-077 parity).
    ///
    /// `nil` until a degraded adapter records its first prop; the reconciler
    /// reads these back when the native presentation layer is wired (ADR-0048).
    private static var recordedPropsKey: UInt8 = 0

    var fluxRecordedProps: [String: Any] {
        get { objc_getAssociatedObject(self, &UIView.recordedPropsKey) as? [String: Any] ?? [:] }
        set { objc_setAssociatedObject(self, &UIView.recordedPropsKey, newValue, .OBJC_ASSOCIATION_RETAIN) }
    }

    /// Records `value` under `key`, replacing any prior entry.
    func fluxRecord(_ key: String, _ value: Any) {
        var bag = fluxRecordedProps
        bag[key] = value
        fluxRecordedProps = bag
    }
}

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
        let gap = new.getFloat(named: "gap") ?? 0
        if Double(view.spacing) != gap { view.spacing = CGFloat(gap) }
        // Recorded for parity with Android `StackAdapter.PROP_GAP` so the host
        // presentation layer (ADR-0048) reads the same data the Compose host does.
        view.fluxRecord(FluxRecordedProp.gap, gap)
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
        let gap = new.getFloat(named: "gap") ?? 0
        if Double(view.spacing) != gap { view.spacing = CGFloat(gap) }
        let columns = new.getInt(named: "columns") ?? 2
        // Recorded for parity with Android `GridAdapter.PROP_COLUMNS` /
        // `PROP_GAP` so the host layout reads the same row-major data Compose does.
        view.fluxRecord(FluxRecordedProp.columns, columns)
        view.fluxRecord(FluxRecordedProp.gap, gap)
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
        let flex = new.getFloat(named: "flex") ?? 1
        // Recorded for parity with Android `SpacerAdapter.PROP_FLEX` — the grow
        // weight the host layout consumes (ADR-0048). No native view mutation is
        // needed for the degraded Spacer beyond hosting the blank.
        view.fluxRecord(FluxRecordedProp.flex, flex)
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
        // `edges` selects which insets to apply; recorded for parity with
        // Android `SafeAreaAdapter.PROP_EDGES` so the host layout reads the same
        // edge set. Absent edges means "all edges" (degraded form).
        let edges = new.getString(named: "edges")
        view.fluxRecord(FluxRecordedProp.edges, edges as Any)
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
