//  TextInputAdapter.swift
//  FluxUIKit — `TextInput` → `UITextField` (Appendix F.5).
//
//  Declarative adapter mapping a Flux `TextInput` node to a `UITextField`
//  (unified tier; AGENTS.md §3.5).

import UIKit
//
//  Props are read by name (never a hardcoded positional index, which would
//  desync from the server's FNV-1a(name) wire layout — see AGENTS.md §3.2);
//  the index is the FNV-1a-32 digest of the name masked to `u16`
//  (`Props.propIndex`), derived identically on server and client. Fields:
//  - `text: String = ""` (controlled value)
//  - `onChangeText: Handler`
//  - `placeholder: Option[String]`
//  - `ref: Option[Ref]` (unused in dev; the view itself is the ref)
//  - `enabled: Bool = true`
//  - `secureTextEntry: Bool = false`
//  - `keyboardType: Option[String]`
//
//  Editing changes dispatch `onChangeText` with the new text as the payload, so
//  the runtime's handler can write the bound signal. A `UITextFieldDelegate`
//  relays the edit. The field retains this adapter via object association so
//  the adapter outlives `create()`; the delegate holds the executor `weak`, so
//  there is no retain cycle back to the runtime.

public final class TextInputAdapter: FluxAdapter {
    public typealias View = UITextField
    weak var executor: (any FluxExecutor)?
    /// Strongly retains the delegate; `UITextField.delegate` is weak, so without
    /// this the delegate would be released the moment `create()` returns.
    private var textDelegate: Delegate?

    public init(executor: (any FluxExecutor)? = nil) { self.executor = executor }

    public func create() -> UITextField {
        let field = UITextField()
        field.borderStyle = .roundedRect
        let delegate = Delegate()
        delegate.adapter = self
        field.delegate = delegate
        textDelegate = delegate
        // Keep this adapter alive as long as the field is alive.
        objc_setAssociatedObject(field, &Self.associationKey, self, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
        return field
    }

    public func update(_ view: UITextField, from old: Props, to new: Props) {
        if let text = new.getString(named: "text"), view.text != text { view.text = text }
        view.placeholder = new.getString(named: "placeholder")
        view.isEnabled = new.getBool(named: "enabled") ?? true
        view.isSecureTextEntry = new.getBool(named: "secureTextEntry") ?? false
    }

    public func setChildren(_ children: [AnyObject], on view: UITextField) {}

    public func bindHandler(_ handlerId: FluxHandlerId, to view: UITextField, nodeId: FluxNodeId) {
        (view.delegate as? Delegate)?.bind(handlerId: handlerId, nodeId: nodeId)
    }

    public func destroy(_ view: UITextField) {
        view.delegate = nil
        objc_setAssociatedObject(view, &Self.associationKey, nil, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
    }

    private static var associationKey: UInt8 = 0

    /// Forwards edits to the bound `onChangeText` handler via the weak executor.
    @MainActor
    final class Delegate: NSObject, UITextFieldDelegate {
        weak var adapter: TextInputAdapter?
        var handlerId: FluxHandlerId?
        var nodeId: FluxNodeId?

        func bind(handlerId: FluxHandlerId, nodeId: FluxNodeId) {
            self.handlerId = handlerId
            self.nodeId = nodeId
        }

        func textFieldDidChangeSelection(_ textField: UITextField) {
            guard let handlerId, let nodeId, let text = textField.text else { return }
            MainActor.assumeIsolated {
                adapter?.executor?.dispatch(FluxEvent(handlerId: handlerId, nodeId: nodeId, payload: .str(text)))
            }
        }
    }
}
