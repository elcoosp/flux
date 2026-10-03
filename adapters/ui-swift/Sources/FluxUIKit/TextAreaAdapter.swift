//  TextAreaAdapter.swift
//  FluxUIKit — `TextArea` → `UITextView` (FLUX-040, Appendix F form family).
//
//  Declarative adapter mapping a Flux `TextArea` node to a `UITextView`
//  (unified tier; AGENTS.md §3.5).
//
//  Props are read by name; the index is the FNV-1a-32 digest of the name
//  masked to `u16` (`Props.propIndex`), derived identically on server and
//  client (AGENTS.md §3.2). Fields:
//  - `value: String = ""` (controlled text)
//  - `onChange: Handler`
//  - `placeholder: Option[String]` (shown as a fading hint when empty)
//  - `maxLines: Option[Int]` (soft cap on scrollable height)
//  - `enabled: Bool = true`
//
//  Editing dispatches `onChange` with the new string as payload, mirroring the
//  `TextInput` contract. The delegate is retained via object association
//  (like `TextInputAdapter`) because `UITextView.delegate` is `weak`.

import UIKit

public final class TextAreaAdapter: FluxAdapter {
    public typealias View = UITextView
    weak var executor: (any FluxExecutor)?
    private var textDelegate: Delegate?

    public init(executor: (any FluxExecutor)? = nil) { self.executor = executor }

    public func create() -> UITextView {
        let view = UITextView()
        view.font = .systemFont(ofSize: 17)
        view.layer.borderWidth = 1
        view.layer.borderColor = UIColor.separator.cgColor
        view.layer.cornerRadius = 6
        let delegate = Delegate()
        delegate.adapter = self
        view.delegate = delegate
        textDelegate = delegate
        objc_setAssociatedObject(view, &Self.associationKey, self, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
        return view
    }

    public func update(_ view: UITextView, from old: Props, to new: Props) {
        if let text = new.getString(named: "value"), view.text != text {
            view.text = text
            // Keep the hint consistent with the new controlled value.
            view.fluxPlaceholderLabel?.isHidden = !text.isEmpty
        }
        // Audit fix: render the hint as a fading overlay label rather than
        // writing it into `view.text`. The previous code made the hint the
        // field's *value*, dispatched it as user payload on the next edit,
        // and diverged from every sibling adapter (which treat placeholder
        // as a hint only).
        if let placeholder = new.getString(named: "placeholder") {
            let label = ensurePlaceholderLabel(on: view)
            label.text = placeholder
            label.isHidden = !view.text.isEmpty
        } else {
            view.fluxPlaceholderLabel?.isHidden = true
        }
        // Audit fix: store ONE height cap constraint and mutate its constant
        // instead of activating a fresh `heightAnchor.constraint` per update
        // (the old code accumulated one constraint per pass, growing without
        // bound as the pipeline re-shipped props).
        if let maxLines = new.getInt(named: "maxLines"), maxLines > 0 {
            let lineHeight = view.font?.lineHeight ?? 20
            let target = CGFloat(maxLines) * lineHeight
            if let existing = view.fluxMaxHeightConstraint {
                existing.constant = target
            } else {
                let cap = view.heightAnchor.constraint(lessThanOrEqualToConstant: target)
                cap.isActive = true
                view.fluxMaxHeightConstraint = cap
            }
        }
        view.isEditable = new.getBool(named: "enabled") ?? true
    }

    public func setChildren(_ children: [AnyObject], on view: UITextView) {}

    public func bindHandler(_ handlerId: FluxHandlerId, to view: UITextView, nodeId: FluxNodeId) {
        (view.delegate as? Delegate)?.bind(handlerId: handlerId, nodeId: nodeId)
    }

    public func destroy(_ view: UITextView) {
        view.delegate = nil
        objc_setAssociatedObject(view, &Self.associationKey, nil, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
    }

    private static var associationKey: UInt8 = 0

    /// Lazily-created placeholder hint shown as an overlay label while the
    /// text view is empty. The hint must NOT be written into `view.text`
    /// (previous behavior) — the delegate would dispatch the hint as user
    /// input and the hint became indistinguishable from typed text.
    private func ensurePlaceholderLabel(on view: UITextView) -> UILabel {
        if let existing = view.fluxPlaceholderLabel { return existing }
        let label = UILabel()
        label.font = view.font
        label.textColor = .placeholderText
        label.numberOfLines = 0
        label.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(label)
        NSLayoutConstraint.activate([
            label.topAnchor.constraint(equalTo: view.topAnchor, constant: 8),
            label.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: 5),
            label.trailingAnchor.constraint(lessThanOrEqualTo: view.trailingAnchor, constant: -5),
        ])
        view.fluxPlaceholderLabel = label
        return label
    }

    /// Forwards edits to the bound `onChange` handler via the weak executor.
    @MainActor
    final class Delegate: NSObject, UITextViewDelegate {
        weak var adapter: TextAreaAdapter?
        var handlerId: FluxHandlerId?
        var nodeId: FluxNodeId?

        func bind(handlerId: FluxHandlerId, nodeId: FluxNodeId) {
            self.handlerId = handlerId
            self.nodeId = nodeId
        }

        func textViewDidChange(_ textView: UITextView) {
            // Keep the placeholder overlay hidden whenever the field has any
            // content — independent of whether a handler is currently bound.
            textView.fluxPlaceholderLabel?.isHidden = !(textView.text?.isEmpty ?? true)
            guard let handlerId, let nodeId, let text = textView.text else { return }
            MainActor.assumeIsolated {
                adapter?.executor?.dispatch(FluxEvent(handlerId: handlerId, nodeId: nodeId, payload: .str(text)))
            }
        }
    }
}


// MARK: - Associated placeholder + height-cap storage

// Stable addresses used as `objc` associated-object keys. The placeholder
// overlay label and the height cap constraint are per-view storage the
// adapter needs on update; the label must never be written to `text`.
private nonisolated(unsafe) var fluxPlaceholderLabelKey: UInt8 = 0
private nonisolated(unsafe) var fluxMaxHeightKey: UInt8 = 0

extension UITextView {
    @MainActor
    var fluxPlaceholderLabel: UILabel? {
        get { objc_getAssociatedObject(self, &fluxPlaceholderLabelKey) as? UILabel }
        set { objc_setAssociatedObject(self, &fluxPlaceholderLabelKey, newValue, .OBJC_ASSOCIATION_RETAIN_NONATOMIC) }
    }

    @MainActor
    var fluxMaxHeightConstraint: NSLayoutConstraint? {
        get { objc_getAssociatedObject(self, &fluxMaxHeightKey) as? NSLayoutConstraint }
        set { objc_setAssociatedObject(self, &fluxMaxHeightKey, newValue, .OBJC_ASSOCIATION_RETAIN_NONATOMIC) }
    }
}
