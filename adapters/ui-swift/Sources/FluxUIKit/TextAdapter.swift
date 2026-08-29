//  TextAdapter.swift
//  FluxUIKit — `Text` → `UILabel` (Appendix F.1).

import UIKit

/// Declarative adapter mapping a Flux `Text` node to a `UILabel`
/// (unified tier; AGENTS.md §3.5).
///
/// Props are read by name; the index is the FNV-1a-32 digest of the name
/// masked to `u16` (`Props.propIndex`), derived identically on server and
/// client (AGENTS.md §3.2) — never a hardcoded positional index. Fields:
/// - `text: String` (required)
/// - `font: Option[Font]`
/// - `size: Option[Float]`
/// - `color: Option[Color]`
/// - `alignment: Option[Alignment]`
/// - `maxLines: Option[Int]`
/// - `overflow: Option[Overflow]`
public final class TextAdapter: FluxAdapter {
    public typealias View = UILabel
    weak var executor: (any FluxExecutor)?

    public init(executor: (any FluxExecutor)? = nil) { self.executor = executor }

    public func create() -> UILabel {
        let label = UILabel()
        label.numberOfLines = 0
        return label
    }

    public func update(_ view: UILabel, from old: Props, to new: Props) {
        view.text = new.getString(named: "text") ?? old.getString(named: "text")
        applyFont(to: view, props: new)
        if let color = new.getColor(named: "color") { view.textColor = color.uiColor }
        if let align = new.getRecord(named: "alignment").flatMap(FluxAlignment.init(record:)) {
            view.textAlignment = align.textAlignment
        }
        if let maxLines = new.getInt(named: "maxLines") { view.numberOfLines = Int(maxLines) }
    }

    public func setChildren(_ children: [AnyObject], on view: UILabel) {
        // `Text` is a leaf; the runtime never sends children.
    }

    public func bindHandler(_ handlerId: FluxHandlerId, to view: UILabel, nodeId: FluxNodeId) {
        // `Text` has no handlers.
    }

    public func destroy(_ view: UILabel) {}

    private func applyFont(to view: UILabel, props: Props) {
        // `Font` is a positional record: `Font(family, size, weight, style)` —
        // size is field 1. Read the `font` prop by name (FNV-derived index) and
        // the size by its positional record field (AGENTS.md §3.2).
        if let font = props.getRecord(named: "font") {
            let size = font.getFloat(1) ?? 14
            view.font = UIFont.systemFont(ofSize: CGFloat(size))
        } else if view.font == nil {
            view.font = UIFont.systemFont(ofSize: 14)
        }
    }
}
