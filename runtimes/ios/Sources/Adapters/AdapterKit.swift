//  AdapterKit.swift
//  Native adapter bridge (FLUX-016).
//
//  Wires the real `FluxUIKit` adapter kit into the FLUX-006 runtime. The
//  runtime speaks an id-based value vocabulary (`VMValue` with interned
//  string ids); the kit speaks resolved values (`VMValue` with
//  concrete `String`s). This module is the translation layer plus a
//  `ComponentId -> adapter instance` registry driven by the Init frame's
//  string table.

import Foundation
import FluxUIKit

/// A host-side string table mirroring `flux_syntax::StringTable` (Appendix C).
///
/// The Init frame interns every string — including each component's *name*
/// under its own `ComponentId` — so the runtime resolves a node's
/// `component_id` to its adapter by looking the id up here.
struct StringTable {
    /// The id → string mapping.
    private var strings: [UInt32: String] = [:]

    /// Every known string id (used to discover declared components).
    var ids: [UInt32] { Array(strings.keys) }

    /// Creates an empty table.
    init() {}

    /// Interns `value` under `id`, replacing any prior entry.
    mutating func intern(_ id: UInt32, _ value: String) {
        strings[id] = value
    }

    /// Resolves `id` to its string, or `nil` if unknown.
    func lookup(_ id: UInt32) -> String? {
        strings[id]
    }

    /// Interns `value` (or returns the id of an existing equal string) under a
    /// fresh high-range id, mirroring `id(for:)` but returning the raw id so the
    /// VM can place it into a register. Used by `STR_CONCAT` to publish a newly
    /// concatenated string for later `STR_LEN` / prop resolution.
    mutating func intern(_ value: String) -> UInt32 {
        id(for: value)
    }

    /// Resolves `value` to an existing id, or interns it under a fresh,
    /// high-range id (distinct from the low stdlib component ids) so native
    /// event payloads can be converted back to the runtime's id-based
    /// `VMValue` without colliding with declared strings.
    mutating func id(for value: String) -> UInt32 {
        if let existing = strings.first(where: { $0.value == value })?.key {
            return existing
        }
        // Reserve the high half for reverse-interns to avoid colliding with
        // forward-interns made by the decoder.
        var candidate: UInt32 = 0x8000_0000
        while strings[candidate] != nil { candidate &+= 1 }
        strings[candidate] = value
        return candidate
    }
}

/// Read/append access to an interned string table, abstracting over the concrete
/// `StringTable` so the VM can resolve `STR_LEN` / `STR_CONCAT` without depending
/// on the runtime's concrete type (Appendix E §E.1).
protocol StringResolvable {
    /// Resolves an interned `StringId` to its text, or `nil` if unknown.
    func lookup(_ id: UInt32) -> String?
    /// Interns `value`, returning the id it was stored under.
    mutating func intern(_ value: String) -> UInt32
}

extension StringTable: StringResolvable {}

/// A `StringResolvable` that holds nothing: lookups miss and interning yields
/// the id `0`. Used by offline VM evaluation (the ISA conformance vectors),
/// where `STR_LEN` / `STR_CONCAT` are not exercised, so resolution is a no-op.
struct EmptyStringTable: StringResolvable {
    func lookup(_ id: UInt32) -> String? { nil }
    mutating func intern(_ value: String) -> UInt32 { 0 }
}

/// Translates a runtime `VMValue` (id-based, interned strings) into the
/// kit's resolved `VMValue` using `table` for string ids.
///
/// - Parameter table: the live string table used to resolve `.str` ids.
/// - Returns: the equivalent kit value; unresolved string ids fall back to a
///   debug representation so an adapter never receives a dangling reference.
@MainActor
func toKit(_ value: VMValue, table: StringTable) -> FluxValue {
    switch value {
    case let .int(i):
        return .int(i)
    case let .float(f):
        return .float(f)
    case let .bool(b):
        return .bool(b)
    case .null:
        return .null
    case let .str(id):
        return .str(table.lookup(id) ?? "str(\(id))")
    case let .handlerRef(h):
        return .handlerRef(h)
    case let .list(items):
        return .list(items.map { toKit($0, table: table) })
    case let .record(fields):
        var dict: [UInt16: FluxValue] = [:]
        for field in fields {
            dict[field.propIndex] = toKit(field.value, table: table)
        }
        return .record(Props(dict))
    }
}

/// Builds a kit `Props` map from runtime `Prop`s, resolving strings via `table`.
@MainActor
func kitProps(_ props: [Prop], table: StringTable) -> Props {
    var fields: [UInt16: FluxValue] = [:]
    for p in props {
        fields[p.index] = toKit(p.value, table: table)
    }
    return Props(fields)
}

/// Converts a kit `FluxValue` (resolved strings) back to the runtime's
/// id-based `VMValue`, interning any resolved string through `table`. Used to
/// hand a native event's payload to the VM, which speaks id-based values.
@MainActor
func toRuntime(_ value: FluxValue, table: inout StringTable) -> VMValue {
    switch value {
    case let .int(i):
        return .int(i)
    case let .float(f):
        return .float(f)
    case let .bool(b):
        return .bool(b)
    case .null:
        return .null
    case let .str(s):
        return .str(table.id(for: s))
    case let .handlerRef(h):
        return .handlerRef(h)
    case let .list(items):
        return .list(items.map { toRuntime($0, table: &table) })
    case let .record(props):
        var arr: [(propIndex: UInt16, value: VMValue)] = []
        for (idx, v) in props.fields {
            arr.append((propIndex: idx, value: toRuntime(v, table: &table)))
        }
        return .record(arr)
    }
}

/// A type-erased box around a concrete `FluxAdapter`, hiding the
/// `associatedtype View` so heterogeneous adapters can live in one registry
/// and be driven uniformly by the reconciler.
///
/// Each box wraps a **fresh** adapter instance (the kit's adapters are
/// reference types that retain per-node state, e.g. `TextFieldAdapter`'s
/// delegate), so identity and state are preserved per native view. The executor
/// is injected at creation time via the adapter's public `init(executor:)`
/// (the `executor` property is `internal` to `FluxUIKit`, so it cannot be set
/// from this module after the fact) — see `RegistryFactory` below.
@MainActor
struct AnyFluxAdapter {
    /// Holds the concrete adapter so its operation closures reference a stable
    /// instance for the node's lifetime.
    private final class Holder<A: FluxAdapter> {
        let adapter: A
        init(_ adapter: A) { self.adapter = adapter }
    }

    private let holder: AnyObject
    private let createImpl: () -> AnyObject
    private let updateImpl: (AnyObject, Props, Props) -> Void
    private let setChildrenImpl: (AnyObject, [AnyObject]) -> Void
    private let bindImpl: (AnyObject, FluxHandlerId, FluxNodeId) -> Void
    private let destroyImpl: (AnyObject) -> Void

    /// Wraps a concrete adapter, capturing its `create`/`update`/`setChildren`/
    /// `bindHandler`/`destroy` entry points behind `AnyObject` views.
    init<A: FluxAdapter>(_ adapter: A) {
        let holder = Holder(adapter)
        self.holder = holder
        self.createImpl = { holder.adapter.create() }
        self.updateImpl = { view, old, new in
            guard let v = view as? A.View else { return }
            holder.adapter.update(v, from: old, to: new)
        }
        self.setChildrenImpl = { view, children in
            guard let v = view as? A.View else { return }
            holder.adapter.setChildren(children, on: v)
        }
        self.bindImpl = { view, handlerId, nodeId in
            guard let v = view as? A.View else { return }
            holder.adapter.bindHandler(handlerId, to: v, nodeId: nodeId)
        }
        self.destroyImpl = { view in
            guard let v = view as? A.View else { return }
            holder.adapter.destroy(v)
        }
    }

    /// Creates a fresh native view.
    func create() -> AnyObject { createImpl() }

    /// Applies a prop diff onto an existing view.
    func update(_ view: AnyObject, from old: Props, to new: Props) {
        updateImpl(view, old, new)
    }

    /// Reconciles `view`'s children.
    func setChildren(_ children: [AnyObject], on view: AnyObject) {
        setChildrenImpl(view, children)
    }

    /// Binds `handlerId` to `view`, scoped to `nodeId`.
    func bindHandler(_ handlerId: FluxHandlerId, to view: AnyObject, nodeId: FluxNodeId) {
        bindImpl(view, handlerId, nodeId)
    }

    /// Tears down `view`'s bindings.
    func destroy(_ view: AnyObject) {
        destroyImpl(view)
    }
}

/// A closure that builds a fresh adapter pre-wired to `executor`, used by the
/// registry so each created adapter dispatches native events back to the host
/// coordinator without retaining the runtime.
typealias RegistryFactory = ((any FluxExecutor)?) -> AnyFluxAdapter

/// Resolves a `ComponentId` to a fresh adapter instance, using the Init
/// frame's string table to map each declared component id to its adapter.
///
/// The standard library's seven primitives — `Text`, `Button`, `Column`,
/// `Row`, `TextField`, `Router`, `Screen` — each have a fixed adapter
/// factory; a component id whose name matches one of these is bound.
@MainActor
struct AdapterRegistry {
    /// Factories keyed by `ComponentId`.
    private let factories: [UInt32: RegistryFactory]

    /// Creates a registry from `table`, binding every component id whose
    /// interned name is a known primitive.
    init(table: StringTable) {
        let byName: [String: RegistryFactory] = [
            "Text": { AnyFluxAdapter(TextAdapter(executor: $0)) },
            "Button": { AnyFluxAdapter(ButtonAdapter(executor: $0)) },
            "Column": { AnyFluxAdapter(ColumnAdapter(executor: $0)) },
            "Row": { AnyFluxAdapter(RowAdapter(executor: $0)) },
            "TextField": { AnyFluxAdapter(TextFieldAdapter(executor: $0)) },
            "Router": { AnyFluxAdapter(RouterAdapter(executor: $0)) },
            "Screen": { AnyFluxAdapter(ScreenAdapter(executor: $0)) },
        ]
        var map: [UInt32: RegistryFactory] = [:]
        for id in table.ids {
            if let name = table.lookup(id), let factory = byName[name] {
                map[id] = factory
            }
        }
        self.factories = map
    }

    /// Produces a fresh adapter for `componentId`, wired to `executor`, or
    /// `nil` if the id is unbound.
    func make(for componentId: UInt32, executor: (any FluxExecutor)?) -> AnyFluxAdapter? {
        factories[componentId]?(executor)
    }

    /// Every `ComponentId` this registry can resolve.
    var resolvedComponentIds: [UInt32] { Array(factories.keys) }
}
