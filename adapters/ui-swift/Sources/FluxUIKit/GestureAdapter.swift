//  GestureAdapter.swift
//  FluxUIKit — `Gesture` → `UIView` + `UIGestureRecognizer` (FLUX-041, Appendix F gesture family).
//
//  Declarative adapter mapping a Flux `Gesture` wrapper to a plain `UIView`
//  that hosts its child subtree and attaches one `UIGestureRecognizer` selected
//  by the `kind` prop (unified tier; AGENTS.md §3.5). `kind` is one of
//  `longPress` / `swipe` / `drag` / `pinch`; the matching recognizer fires
//  `onGesture` through the weak executor (reusing the `onClick` handler contract).
//  `threshold` is the activation delta for the continuous recognizers (drag/pinch).
//
//  Props are read by name; the index is the FNV-1a-32 digest of the name masked
//  to `u16` (`Props.propIndex`), derived identically on server and client
//  (AGENTS.md §3.2). The native recognizer attach/detach is host-side.
//
//  Children are reconciled by stable view identity (the runtime guarantees one
//  native view per node id), so reorder/patch never recreates a child and drops
//  its internal state — mirroring the Android `GestureAdapter` keyed reconciliation.

import UIKit

public final class GestureAdapter: FluxAdapter {
    public typealias View = UIView
    weak var executor: (any FluxExecutor)?
    private var recognizer: UIGestureRecognizer?

    public init(executor: (any FluxExecutor)? = nil) { self.executor = executor }

    public func create() -> UIView {
        let view = UIView()
        view.translatesAutoresizingMaskIntoConstraints = false
        // Create the environment up front so `update`/`bindHandler` can attach
        // threshold + handler target without nil-guards.
        view.gestureEnvironment = GestureEnvironment()
        return view
    }

    public func update(_ view: UIView, from old: Props, to new: Props) {
        let kind = new.getString(named: "kind") ?? "longPress"
        attachRecognizer(kind: kind, to: view)
        // `threshold` is host-render-only metadata for continuous recognizers;
        // we record it on the view's gesture environment for the host to read.
        if let threshold = new.getFloat(named: "threshold") {
            view.gestureEnvironment?.threshold = threshold
        }
    }

    public func setChildren(_ children: [AnyObject], on view: UIView) {
        let views = children.compactMap { $0 as? UIView }
        reconcileSubviews(views, on: view)
    }

    public func bindHandler(_ handlerId: FluxHandlerId, to view: UIView, nodeId: FluxNodeId) {
        recognizer?.removeTarget(nil, action: nil)
        let target = HandlerTarget(executor: executor, handlerId: handlerId, nodeId: nodeId) { nil }
        recognizer?.addTarget(target, action: #selector(HandlerTarget.fire))
        // Retain the target on the view's gesture environment so it survives.
        view.gestureEnvironment?.handlerTarget = target
    }

    public func destroy(_ view: UIView) {
        if let recognizer { view.removeGestureRecognizer(recognizer) }
        self.recognizer = nil
        view.gestureEnvironment = nil
        view.subviews.forEach { $0.removeFromSuperview() }
    }

    /// Attaches (or re-attaches) the recognizer matching `kind`, preserving the
    /// already-bound handler target across kind changes.
    private func attachRecognizer(kind: String, to view: UIView) {
        if let existing = recognizer {
            // Only swap if the kind (and thus recognizer class) actually changed.
            if recognizerClass(for: kind) == type(of: existing) { return }
            view.removeGestureRecognizer(existing)
            self.recognizer = nil
        }
        let new = makeRecognizer(kind: kind)
        view.addGestureRecognizer(new)
        self.recognizer = new
    }

    private func makeRecognizer(kind: String) -> UIGestureRecognizer {
        switch kind {
        case "swipe": return UISwipeGestureRecognizer()
        case "drag": return UIPanGestureRecognizer()
        case "pinch": return UIPinchGestureRecognizer()
        case "longPress", "longpress": return UILongPressGestureRecognizer()
        default: return UILongPressGestureRecognizer()
        }
    }

    private func recognizerClass(for kind: String) -> AnyClass {
        switch kind {
        case "swipe": return UISwipeGestureRecognizer.self
        case "drag": return UIPanGestureRecognizer.self
        case "pinch": return UIPinchGestureRecognizer.self
        default: return UILongPressGestureRecognizer.self
        }
    }
}

/// Reconcile a plain `UIView`'s subviews to `target` by identity.
///
/// Child views are keyed by the runtime's stable node id (their object identity
/// here), so this performs the minimal insert/remove and reorders in place —
/// never recreating a view that already exists, which would drop its state.
@MainActor
private func reconcileSubviews(_ target: [UIView], on container: UIView) {
    let current = container.subviews
    let targetSet = Set(target)
    for stale in current where !targetSet.contains(stale) {
        stale.removeFromSuperview()
    }
    var index = 0
    for child in target {
        if child.superview !== container {
            container.insertSubview(child, at: min(index, container.subviews.count))
        } else if container.subviews.firstIndex(of: child) != index {
            child.removeFromSuperview()
            container.insertSubview(child, at: min(index, container.subviews.count))
        }
        index += 1
    }
}

// MARK: - Gesture environment attach storage

// Stable address used as an `objc` associated-object key. Marked `nonisolated(unsafe)`
// because it is a global shared mutable singleton whose access is externally
// synchronized by the Objective-C association runtime (it is never read/written
// except as a key pointer); Swift 6 strict concurrency otherwise flags it.
private nonisolated(unsafe) var gestureEnvKey: UInt8 = 0

/// Per-view bag holding the continuous-gesture `threshold` and the retained
/// `HandlerTarget`, kept alive for the lifetime of the gesture view.
@MainActor
final class GestureEnvironment {
    var threshold: Double = 0.0
    var handlerTarget: HandlerTarget?
}

extension UIView {
    /// Lazily-created gesture environment for a `Gesture` view.
    @MainActor
    var gestureEnvironment: GestureEnvironment? {
        get { objc_getAssociatedObject(self, &gestureEnvKey) as? GestureEnvironment }
        set { objc_setAssociatedObject(self, &gestureEnvKey, newValue, .OBJC_ASSOCIATION_RETAIN_NONATOMIC) }
    }
}
