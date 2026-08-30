//  AdapterKit.swift
//  Native adapter bridge (FLUX-016).
//
//  Wires the real `FluxUIKit` adapter kit into the FLUX-006 runtime. The
//  runtime speaks an id-based value vocabulary (`FluxValue` with interned
//  string ids); the kit speaks resolved values (`FluxValue` with
//  concrete `String`s). This module is the translation layer plus a
//  `ComponentId -> adapter instance` registry driven by the Init frame's
//  string table.

import Foundation
import FluxUIKit

/// A host-side string table mirroring `flux_syntax::StringTable` (Appendix C).
///
/// The Init frame interns every string — including each component's *name*
/// under its own `ComponentId` — so the runtime resolves a node's
/// `component_id` to its adapter by looking the id up here. The ids stored here
/// are always *server-assigned* (the Init frame seeds them), never synthesized
/// on the host: the host publishes any freshly-derived string via the async
/// `AnyStringInterner` RPC, which returns a canonical id `< stringIdCanonicalCeiling`.
/// The former high-range `synthetic_str_id` fallback is retired (brittleness 4c).
public struct StringTable: Sendable {
    /// The id → string mapping.
    private var strings: [UInt32: String] = [:]
    /// The string → id reverse index (Perf #7 / R4). Lets `id(for:)` resolve a
    /// native event's string payload in O(1) instead of scanning `strings`.
    private var reverseLookup: [String: UInt32] = [:]

    /// Counter for host-local derived string ids (above the server seed range).
    private var nextDerivedId: UInt32 = 0xC000_0000

    /// Every known string id (used to discover declared components).
    var ids: [UInt32] { Array(strings.keys) }

    /// Creates an empty table.
    public init() {}

    /// Interns `value` under `id`, replacing any prior entry. Only ever called
    /// with server-assigned ids (the Init frame's seeds), never to mint a new id.
    public mutating func intern(_ id: UInt32, _ value: String) {
        strings[id] = value
        reverseLookup[value] = id
    }

    /// Resolves `id` to its string, or `nil` if unknown.
    public func lookup(_ id: UInt32) -> String? {
        strings[id]
    }

    /// Resolves `value` to an already-interned id, or `nil` if it has never been
    /// interned. Pure cache lookup — the host never *generates* a fresh id here;
    /// publishing a new string is the `AnyStringInterner` RPC's job (brittleness 4c).
    func id(for value: String) -> UInt32? {
        reverseLookup[value]
    }
}

/// Read-only access to an interned string table, abstracting over the concrete
/// `StringTable` so the VM can resolve `STR_LEN` / `STR_CONCAT` without depending
/// on the runtime's concrete type (Appendix E §E.1).
///
/// Derived strings (e.g. a `STR_CONCAT` result inside a prop thunk) are interned
/// *locally* into this same table via `intern(_:)` — the table is a reference
/// type during materialisation, so the id the VM mints is the one the kit later
/// resolves. This mirrors the Android host's shared `materializationStrings`
/// resolver and avoids a round-trip to the dev server (brittleness 4c).
protocol StringResolver {
    /// Resolves an interned `StringId` to its text, or `nil` if unknown.
    func lookup(_ id: UInt32) -> String?

    /// Interns a freshly-derived `value`, returning the id it was stored under.
    /// Implementations mint a host-local id; the id need only be resolvable via
    /// `lookup` on the same instance.
    mutating func intern(_ value: String) -> UInt32
}

extension StringTable: StringResolver {
    mutating func intern(_ value: String) -> UInt32 {
        if let existing = id(for: value) { return existing }
        // Mint above the server's seed range; collisions are astronomically
        // unlikely for derived strings within a single frame.
        let id = nextDerivedId
        nextDerivedId = nextDerivedId &+ 1
        strings[id] = value
        reverseLookup[value] = id
        return id
    }
}

/// A reference-type string table used for prop-thunk materialisation
/// (ADR-0027 T14). Unlike the value-type `StringTable`, a `class` lets the VM's
/// `run` intern derived strings (e.g. `STR_CONCAT` results) into the *same*
/// instance the reconciler resolves later — so a thunk that builds a derived
/// label produces a string id the kit can actually look up. This mirrors the
/// Android host's shared `materializationStrings` resolver.
///
/// Resolution is read-only here; the canonical id for a freshly-derived string
/// is produced by the `AnyStringInterner` RPC, never synthesized locally.
final class MaterializationStringTable: StringResolver {
    private var strings: [UInt32: String] = [:]
    private var reverseLookup: [String: UInt32] = [:]
    /// Host-local derived string id counter (above the server seed range).
    private var nextDerivedId: UInt32 = 0xC000_0000

    init() {}

    /// Seeds the table from a frame's string entries (literals + component names).
    func seed(_ entries: [StringEntry]) {
        for entry in entries {
            strings[entry.stringId] = entry.value
            reverseLookup[entry.value] = entry.stringId
        }
    }

    func lookup(_ id: UInt32) -> String? { strings[id] }

    /// Mints a host-local id for a freshly-derived string (e.g. a `STR_CONCAT`
    /// result), so the kit resolves the same id later.
    func intern(_ value: String) -> UInt32 {
        if let existing = reverseLookup[value] { return existing }
        let id = nextDerivedId
        nextDerivedId = nextDerivedId &+ 1
        strings[id] = value
        reverseLookup[value] = id
        return id
    }
}

/// A `StringResolver` that holds nothing: lookups miss. Used by offline VM
/// evaluation (the ISA conformance vectors), where `STR_LEN` / `STR_CONCAT` are
/// not exercised, so resolution is a no-op.
struct EmptyStringTable: StringResolver {
    func lookup(_ id: UInt32) -> String? { nil }
    mutating func intern(_ value: String) -> UInt32 { 0 }
}

/// Translates a runtime `FluxValue` (id-based, interned strings) into the
/// kit's resolved `FluxUIKit.FluxValue` using `table` for string ids.
///
/// - Parameter table: the live string table used to resolve `.str` ids.
/// - Returns: the equivalent kit value; unresolved string ids fall back to a
///   debug representation so an adapter never receives a dangling reference.
@MainActor
func toKit(_ value: FluxValue, table: any StringResolver) -> FluxUIKit.FluxValue {
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
        var dict: [UInt16: FluxUIKit.FluxValue] = [:]
        for field in fields {
            dict[field.propIndex] = toKit(field.value, table: table)
        }
        return .record(Props(dict))
    }
}

/// Builds a kit `Props` map from runtime `Prop`s, resolving strings via `table`.
@MainActor
func kitProps(_ props: [Prop], table: any StringResolver) -> Props {
    var fields: [UInt16: FluxUIKit.FluxValue] = [:]
    for p in props {
        fields[p.index] = toKit(p.value, table: table)
    }
    return Props(fields)
}

/// Content hash of raw runtime props (Perf R2), computed WITHOUT resolving
/// interned strings through the string table. It folds each prop's index and the
/// raw `FluxValue` payload, so two prop sets that differ only in string *resolution*
/// but share the same `FluxValue` ids hash identically — which is the correct
/// comparison domain for "did this node's props change": the adapter ultimately
/// renders the `FluxValue`, not the resolved Swift string. Hashing the raw values
/// also avoids the per-prop string-table walk that `kitProps(_:).hash` would do.
func propHash(_ props: [Prop]) -> UInt64 {
    var h: UInt64 = 0xcbf2_9ce4_8422_2325
    for p in props {
        h = fnv1aMix(h, UInt64(p.index))
        h = fnv1aMix(h, rawValueHash(p.value))
    }
    return h
}

/// One FNV-1a mixing step.
private func fnv1aMix(_ h: UInt64, _ v: UInt64) -> UInt64 {
    (h ^ v) &* 0x0000_0100_0000_01b3
}

/// Stable raw hash of a `FluxValue` that does not consult the string table (R2).
private func rawValueHash(_ v: FluxValue) -> UInt64 {
    switch v {
    case .null:
        return 0
    case let .int(n):
        return fnv1aMix(1, UInt64(bitPattern: n))
    case let .float(d):
        return fnv1aMix(2, d.bitPattern)
    case let .bool(b):
        return fnv1aMix(3, b ? 1 : 0)
    case let .str(id):
        return fnv1aMix(4, UInt64(id))
    case let .handlerRef(id):
        return fnv1aMix(5, UInt64(id))
    case let .list(items):
        var h: UInt64 = 6
        for item in items { h = fnv1aMix(h, rawValueHash(item)) }
        return h
    case let .record(fields):
        var h: UInt64 = 7
        for field in fields {
            h = fnv1aMix(h, UInt64(field.propIndex))
            h = fnv1aMix(h, rawValueHash(field.value))
        }
        return h
    }
}

/// Converts a kit `FluxUIKit.FluxValue` (resolved strings) back to the runtime's
/// id-based `FluxValue`, interning any resolved string through `interner`. Used to
/// hand a native event's payload to the VM, which speaks id-based values.
///
/// Native event payloads carry concrete Swift strings (e.g. text typed into a
/// `TextField`). Those must be interned through the dev server's authoritative
/// string table to receive a canonical id `< stringIdCanonicalCeiling`, exactly
/// like every other string the host publishes — the local `synthetic_str_id`
/// fallback is retired (brittleness 4c). The call is `async` and never blocks the
/// UI thread (see `FluxExecutor.dispatch`).
/// - Parameter interner: the host's `InternString` RPC client (or the offline
///   `NoOpStringInterner` when no live transport is attached).
/// - Returns: the equivalent runtime `FluxValue` with canonical `.str` ids.
@MainActor
func toRuntime(_ value: FluxUIKit.FluxValue, interner: any AnyStringInterner) async -> FluxValue {
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
        return .str(await interner.intern(s))
    case let .handlerRef(h):
        return .handlerRef(h)
    case let .list(items):
        return .list(await items.asyncMap { await toRuntime($0, interner: interner) })
    case let .record(props):
        var arr: [(propIndex: UInt16, value: FluxValue)] = []
        for (idx, v) in props.fields {
            arr.append((propIndex: idx, value: await toRuntime(v, interner: interner)))
        }
        return .record(arr)
    }
}

/// A type-erased box around a concrete `FluxAdapter`, hiding the
/// `associatedtype View` so heterogeneous adapters can live in one registry
/// and be driven uniformly by the reconciler.
///
/// Each box wraps a **fresh** adapter instance (the kit's adapters are
/// reference types that retain per-node state, e.g. `TextInputAdapter`'s
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
typealias RegistryFactory = ((any FluxUIKit.FluxExecutor)?) -> AnyFluxAdapter

/// Resolves a `ComponentId` to a fresh adapter instance.
///
/// The wire carries component ids that are the server's interned *component
/// names* (Appendix D §D.3). A primitive such as `Text` is interned to some
/// `ComponentId` on the server; that id is content-dependent and collides with
/// the host's own hardcoded primitive ids, so we must resolve the id → *name*
/// through the Init frame's string table and look the adapter up by name. The
/// registry is therefore keyed by name, and `make` translates the incoming id
/// to a name before binding.
@MainActor
public struct AdapterRegistry {
    /// Factories keyed by component name.
    private let byName: [String: RegistryFactory]
    /// The string table used to resolve a `ComponentId` to its name.
    private let table: StringTable

    /// Creates a registry from `table`, binding every interned name that is a
    /// known primitive.
    public init(table: StringTable) {
        self.byName = [
            "Text": { AnyFluxAdapter(TextAdapter(executor: $0)) },
            "Button": { AnyFluxAdapter(ButtonAdapter(executor: $0)) },
            "Column": { AnyFluxAdapter(ColumnAdapter(executor: $0)) },
            "Row": { AnyFluxAdapter(RowAdapter(executor: $0)) },
            "TextInput": { AnyFluxAdapter(TextInputAdapter(executor: $0)) },
            "Image": { AnyFluxAdapter(ImageAdapter(executor: $0)) },
            "Router": { AnyFluxAdapter(RouterAdapter(executor: $0)) },
            "Screen": { AnyFluxAdapter(ScreenAdapter(executor: $0)) },
            // FLUX-037 layout primitives.
            "Stack": { AnyFluxAdapter(StackAdapter(executor: $0)) },
            "Grid": { AnyFluxAdapter(GridAdapter(executor: $0)) },
            "Spacer": { AnyFluxAdapter(SpacerAdapter(executor: $0)) },
            "SafeArea": { AnyFluxAdapter(SafeAreaAdapter(executor: $0)) },
            // FLUX-038 overlay containers + FLUX-042 animation wrapper (degraded
            // container form; native presentation/animation gated on ADR-0048).
            "Modal": { AnyFluxAdapter(ModalAdapter(executor: $0)) },
            "Sheet": { AnyFluxAdapter(SheetAdapter(executor: $0)) },
            "Dialog": { AnyFluxAdapter(DialogAdapter(executor: $0)) },
            "Animate": { AnyFluxAdapter(AnimateAdapter(executor: $0)) },
            // FLUX-040 form primitives (PRD-N family).
            "Switch": { AnyFluxAdapter(SwitchAdapter(executor: $0)) },
            // FLUX-077 — `Toggle` (data-driven two-state control, FLUX-072).
            "Toggle": { AnyFluxAdapter(ToggleAdapter(executor: $0)) },
            "Checkbox": { AnyFluxAdapter(CheckboxAdapter(executor: $0)) },
            "Slider": { AnyFluxAdapter(SliderAdapter(executor: $0)) },
            "Picker": { AnyFluxAdapter(PickerAdapter(executor: $0)) },
            "DatePicker": { AnyFluxAdapter(DatePickerAdapter(executor: $0)) },
            "TextArea": { AnyFluxAdapter(TextAreaAdapter(executor: $0)) },
            // FLUX-041 gesture primitive (PRD-N family).
            "Gesture": { AnyFluxAdapter(GestureAdapter(executor: $0)) },
            // FLUX-056 `ScrollView` (PRD-N family).
            "ScrollView": { AnyFluxAdapter(ScrollViewAdapter(executor: $0)) },
        ]
        self.table = table
    }

    /// Produces a fresh adapter for `componentId`, wired to `executor`, or
    /// `nil` if the id is unbound.
    ///
    /// Unbound ids are user-defined `Component` nodes (e.g. `Counter`): the
    /// dev server lowers them to `Component` roots with no primitive adapter,
    /// and the reconciler renders those as plain containers (see
    /// `ShadowTreeReconciler`). `make` itself returns `nil` for unbound ids;
    /// the reconciler supplies the container fallback so a typo'd primitive is
    /// surfaced (B8) rather than silently swallowed.
    func make(for componentId: UInt32, executor: (any FluxUIKit.FluxExecutor)?) -> AnyFluxAdapter? {
        guard let name = table.lookup(componentId), let factory = byName[name] else { return nil }
        return factory(executor)
    }

    /// Produces a fresh adapter for a component resolved to `name` by the
    /// caller (typically via the frame-synced string table), wired to
    /// `executor`, or `nil` if the name is unbound.
    func make(named name: String, executor: (any FluxUIKit.FluxExecutor)?) -> AnyFluxAdapter? {
        guard let factory = byName[name] else { return nil }
        return factory(executor)
    }

    /// Every component name this registry can resolve.
    var resolvedComponentIds: [UInt32] {
        byName.keys.compactMap { table.id(for: $0) }
    }
}

/// Maps each element of `sequence` through the `async` `transform`, preserving
/// `Sequence`-scoped async map, used by `toRuntime` to intern event-payload
/// strings without blocking the UI thread.
extension Sequence {
    @MainActor
    fileprivate func asyncMap<T>(_ transform: @MainActor (Element) async -> T) async -> [T] {
        var result: [T] = []
        result.reserveCapacity(underestimatedCount)
        for element in self {
            result.append(await transform(element))
        }
        return result
    }
}
