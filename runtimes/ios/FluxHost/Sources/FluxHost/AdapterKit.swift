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

/// Read-only access to an interned string table, abstracting over the concrete
/// `MaterializationStringTable` so the VM can resolve `STR_LEN` / `STR_CONCAT`
/// without depending on the runtime's concrete type (Appendix E §E.1).
///
/// Derived strings (e.g. a `STR_CONCAT` result inside a prop thunk) are interned
/// *locally* into the same shared table instance via `intern(_:)` — the table is
/// a reference type, so the id the VM mints is the one the kit later resolves.
/// This mirrors the Android host's shared `materializationStrings` resolver and
/// avoids a round-trip to the dev server (brittleness 4c).
protocol StringResolver {
    /// Resolves an interned `StringId` to its text, or `nil` if unknown.
    func lookup(_ id: UInt32) -> String?

    /// Interns a freshly-derived `value`, returning the id it was stored under.
    /// Implementations mint a host-local id above `nextDerivedId` ceiling; the
    /// id need only be resolvable via `lookup` on the same instance.
    mutating func intern(_ value: String) -> UInt32
}

/// A reference-type string table used for prop-thunk materialisation
/// (ADR-0027 T14). A `class` (not a value type) so the VM's `run` interns
/// derived strings (e.g. `STR_CONCAT` results) into the *same* instance the
/// reconciler resolves later — so a thunk that builds a derived label produces a
/// string id the kit can actually look up. This mirrors the Android host's
/// shared `materializationStrings` resolver.
///
/// The same instance is shared between the executor and the reconciler
/// (constructed once in `FluxExecutor.init` and passed to `ShadowTreeReconciler`),
/// so strings interned during VM evaluation (thunk materialisation, native event
/// payloads) are visible to kit prop resolution. Server seeds come from the Init
/// frame's string entries (via `seed` / `store`); host-derived strings get ids
/// from `intern` in the high local range `>= 0xC000_0000` and never cross the
/// wire as canonical ids (brittleness 4c).
public final class MaterializationStringTable: StringResolver, @unchecked Sendable {
    /// The ceiling above which host-local derived ids start and below which the
    /// next counter wraps (see `intern`). Guarded so a long session cannot
    /// silently bleed into the server-assigned range or overflow to zero.
    private static let derivedIdCeiling: UInt32 = 0xFFFF_FFFF
    private var strings: [UInt32: String] = [:]
    private var reverseLookup: [String: UInt32] = [:]
    /// Host-local derived string id counter (above the server seed range).
    private var nextDerivedId: UInt32 = 0xC000_0000

    public init() {}

    /// Seeds the table from a frame's string entries (literals + component names).
    /// Server-assigned ids land here untouched; they are the canonical ids the
    /// kit resolves during materialisation.
    public func seed(_ entries: [StringEntry]) {
        for entry in entries {
            strings[entry.stringId] = entry.value
            reverseLookup[entry.value] = entry.stringId
        }
    }

    public func lookup(_ id: UInt32) -> String? { strings[id] }

    /// Resolves a string to its interned id, or `nil` if unknown.
    public func id(for value: String) -> UInt32? { reverseLookup[value] }

    /// Every known string id (used to discover declared components).
    public var ids: [UInt32] { Array(strings.keys) }

    /// Stores a server-assigned canonical string id, so the reconciler can
    /// resolve it during prop materialisation. Called from the Init frame's
    /// string entries and from native-event payload interning.
    public func store(id: UInt32, value: String) {
        strings[id] = value
        reverseLookup[value] = id
    }

    /// Mints a host-local id for a freshly-derived string (e.g. a `STR_CONCAT`
    /// result or a `TextField` payload), so the kit resolves the same id later.
    /// The id is `>= 0xC000_0000` and never crosses the wire as a canonical id
    /// (brittleness 4c — host-only). If the counter would wrap past its ceiling,
    /// we trap rather than silently colliding with server-assigned ranges.
    public func intern(_ value: String) -> UInt32 {
        if let existing = reverseLookup[value] { return existing }
        let id = nextDerivedId
        // Guard against wrap into the server-assigned range (FLUX-084). After
        // ~1 billion host-derived uniques in a single session we fail loud
        // rather than risk a non-canonical id leaking across the wire.
        if id == MaterializationStringTable.derivedIdCeiling {
            fatalError("MaterializationStringTable.intern: derived id counter exhausted")
        }
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
/// id-based `FluxValue`, interning any resolved string into the shared
/// `MaterializationStringTable`. Used to hand a native event's payload to the VM,
/// which speaks id-based values.
///
/// Native event payloads carry concrete Swift strings (e.g. text typed into a
/// `TextField`). Those are interned locally into the shared table — no round-trip
/// to the dev server — so the interned id is visible to both the VM (immediately,
/// in the same `dispatch`) and the kit on the next reconcile. The id is
/// host-local (`>= 0xC000_0000`) and never crosses the wire as a canonical id
/// (brittleness 4c).
/// - Parameter table: the live shared string table; mutations are visible to the
///   reconciler's `currentTable()` because they share the same instance.
/// - Returns: the equivalent runtime `FluxValue` with interned `.str` ids.
@MainActor
func toRuntime(_ value: FluxUIKit.FluxValue, table: inout MaterializationStringTable) -> FluxValue {
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
        return .str(table.intern(s))
    case let .handlerRef(h):
        return .handlerRef(h)
    case let .list(items):
        return .list(items.map { toRuntime($0, table: &table) })
    case let .record(props):
        var arr: [(propIndex: UInt16, value: FluxValue)] = []
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
    private let table: MaterializationStringTable

    /// Creates a registry from `table`, binding every interned name that is a
    /// known primitive.
    public init(table: MaterializationStringTable) {
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
            // FLUX-048 `WebHost` (sandboxed native web view; see `FluxUIKit.WebHostView`).
            "WebHost": { AnyFluxAdapter(WebHostView(executor: $0)) },
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


