//  Props.swift
//  FluxUIKit — the prop map contract (Appendix C §C.1 `Props`).

/// A flat map of `(prop_idx, value)` pairs attached to an IR node.
///
/// Mirrors `flux_syntax::Props`: a content-addressed bag of fields. Field
/// access is by `PropIdx` (O(1)). `hash` is a stable content hash used by the
/// runtime's reconciler to skip referentially-equal subtrees.
public struct Props: Sendable, Hashable {
    /// The underlying field map.
    public let fields: [PropIdx: FluxValue]
    private let digest: UInt64

    /// Build a prop map. The content hash is computed up front so equality and
    /// lookups are O(1) later.
    public init(_ fields: [PropIdx: FluxValue] = [:]) {
        self.fields = fields
        var hasher = Hasher()
        for index in fields.keys.sorted() {
            hasher.combine(index)
            // `fields` is a dictionary; the key is guaranteed present.
            hasher.combine(fields[index]!)
        }
        self.digest = UInt64(bitPattern: Int64(hasher.finalize()))
    }

    /// The content hash (BLAKE3 in the Rust IR; a portable hash here).
    public var hash: UInt64 { digest }

    /// Look up a raw value by prop index.
    public func get(_ index: PropIdx) -> FluxValue? { fields[index] }

    /// Resolve a string prop.
    public func getString(_ index: PropIdx) -> String? {
        if case .str(let s) = fields[index] ?? .null { s } else { nil }
    }

    /// Resolve an integer prop.
    public func getInt(_ index: PropIdx) -> Int64? {
        if case .int(let i) = fields[index] ?? .null { i } else { nil }
    }

    /// Resolve a float prop.
    public func getFloat(_ index: PropIdx) -> Double? {
        if case .float(let f) = fields[index] ?? .null { f } else { nil }
    }

    /// Resolve a boolean prop.
    public func getBool(_ index: PropIdx) -> Bool? {
        if case .bool(let b) = fields[index] ?? .null { b } else { nil }
    }

    /// Resolve a handler-reference prop.
    public func getHandler(_ index: PropIdx) -> FluxHandlerId? {
        if case .handlerRef(let h) = fields[index] ?? .null { h } else { nil }
    }

    /// Resolve a record prop (itself a `Props`).
    public func getRecord(_ index: PropIdx) -> Props? {
        if case .record(let p) = fields[index] ?? .null { p } else { nil }
    }

    /// Resolve a color prop (see `FluxColor`).
    public func getColor(_ index: PropIdx) -> FluxColor? {
        getRecord(index).flatMap(FluxColor.init(record:))
    }

    /// Resolve a font prop (see `FluxFount`).
    public func getFont(_ index: PropIdx) -> FluxFount? {
        getRecord(index).flatMap(FluxFount.init(record:))
    }

    public static func == (lhs: Props, rhs: Props) -> Bool { lhs.fields == rhs.fields }

    public func hash(into hasher: inout Hasher) { hasher.combine(digest) }
}
