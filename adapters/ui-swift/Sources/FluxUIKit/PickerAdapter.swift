//  PickerAdapter.swift
//  FluxUIKit — `Picker` → `UIPickerView` (FLUX-040, Appendix F form family).
//
//  Declarative adapter mapping a Flux `Picker` node to a `UIPickerView`
//  (unified tier; AGENTS.md §3.5).
//
//  Props are read by name; the index is the FNV-1a-32 digest of the name
//  masked to `u16` (`Props.propIndex`), derived identically on server and
//  client (AGENTS.md §3.2). Fields:
//  - `value: Int = 0` (controlled selected index)
//  - `onChange: Handler`
//  - `items: List[String]` (candidate labels)
//  - `enabled: Bool = true`
//
//  Selecting a row dispatches `onChange` with the new index as payload. The
//  picker's delegate/dataSource is retained via object association (like
//  `TextInputAdapter`) because `UIPickerView` holds them `weak`.

import UIKit

public final class PickerAdapter: FluxAdapter {
    public typealias View = UIPickerView
    weak var executor: (any FluxExecutor)?
    private var source: Source?

    public init(executor: (any FluxExecutor)? = nil) { self.executor = executor }

    public func create() -> UIPickerView {
        let picker = UIPickerView()
        let source = Source()
        source.adapter = self
        picker.delegate = source
        picker.dataSource = source
        self.source = source
        // Keep this adapter + its source alive as long as the picker is alive.
        objc_setAssociatedObject(picker, &Self.associationKey, self, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
        return picker
    }

    public func update(_ view: UIPickerView, from old: Props, to new: Props) {
        source?.items = new.getList(named: "items")?.compactMap { if case .str(let s) = $0 { s } else { nil } } ?? []
        let selected = new.getInt(named: "value") ?? 0
        if view.selectedRow(inComponent: 0) != Int(selected) {
            view.selectRow(Int(selected), inComponent: 0, animated: false)
        }
        view.isUserInteractionEnabled = new.getBool(named: "enabled") ?? true
    }

    public func setChildren(_ children: [AnyObject], on view: UIPickerView) {}

    public func bindHandler(_ handlerId: FluxHandlerId, to view: UIPickerView, nodeId: FluxNodeId) {
        source?.bind(handlerId: handlerId, nodeId: nodeId)
    }

    public func destroy(_ view: UIPickerView) {
        view.delegate = nil
        view.dataSource = nil
        objc_setAssociatedObject(view, &Self.associationKey, nil, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
    }

    private static var associationKey: UInt8 = 0

    /// Supplies rows and forwards selection to the bound `onChange` handler.
    @MainActor
    final class Source: NSObject, UIPickerViewDelegate, UIPickerViewDataSource {
        weak var adapter: PickerAdapter?
        var items: [String] = []
        var handlerId: FluxHandlerId?
        var nodeId: FluxNodeId?

        func bind(handlerId: FluxHandlerId, nodeId: FluxNodeId) {
            self.handlerId = handlerId
            self.nodeId = nodeId
        }

        func numberOfComponents(in pickerView: UIPickerView) -> Int { 1 }
        func pickerView(_ pickerView: UIPickerView, numberOfRowsInComponent component: Int) -> Int { items.count }

        func pickerView(_ pickerView: UIPickerView, titleForRow row: Int, forComponent component: Int) -> String? {
            items[row]
        }

        func pickerView(_ pickerView: UIPickerView, didSelectRow row: Int, inComponent component: Int) {
            guard let handlerId, let nodeId else { return }
            MainActor.assumeIsolated {
                adapter?.executor?.dispatch(FluxEvent(handlerId: handlerId, nodeId: nodeId, payload: .int(Int64(row))))
            }
        }
    }
}
