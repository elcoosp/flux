//  FluxValue.swift
//  Native Swift mirror of `flux_syntax::Value` (Appendix C §C.1, Appendix D §D.5).
//
//  The reference VM (`flux-vm-ref`), the Swift VM (FLUX-006) and the Kotlin VM
//  (FLUX-007) must agree on observable value semantics. These type tags are the
//  wire contract: the deserializer switches on exactly these bytes, so any
//  change here must be reflected in all three implementations and in the golden
//  ISA vectors.

import Foundation

/// A runtime value living in signal cells, prop maps, or VM registers.
///
/// `Str` and `HandlerRef` carry an interned identifier rather than the payload
/// itself; resolve strings through the owning string table and handlers through
/// the closure registry. `Null` doubles as the representation of `Unit`.
public enum FluxValue: Equatable, Sendable, CustomStringConvertible {
    case int(Int64)
    case float(Double)
    case bool(Bool)
    case str(UInt32)
    case handlerRef(UInt32)
    case list([FluxValue])
    case record([(propIndex: UInt16, value: FluxValue)])
    case `null`

    /// Wire type tag, per Appendix D §D.5. The deserializer and serializer
    /// switch on exactly these bytes.
    var tag: UInt8 {
        switch self {
        case .null: 0x00
        case .int: 0x01
        case .float: 0x02
        case .bool: 0x03
        case .str: 0x04
        case .handlerRef: 0x05
        case .list: 0x06
        case .record: 0x07
        }
    }

    /// The interned string id, or `nil` for any other variant.
    public var strID: UInt32? {
        if case let .str(id) = self { id } else { nil }
    }

    /// The integer payload, or `nil` for any other variant.
    public var asInt: Int64? {
        if case let .int(v) = self { v } else { nil }
    }

    /// The float payload, or `nil` for any other variant.
    public var asFloat: Double? {
        if case let .float(v) = self { v } else { nil }
    }

    /// The boolean payload, or `nil` for any other variant.
    public var asBool: Bool? {
        if case let .bool(v) = self { v } else { nil }
    }

    /// The referenced handler id, or `nil` for any other variant.
    var handlerID: UInt32? {
        if case let .handlerRef(id) = self { id } else { nil }
    }

    public var description: String {
        switch self {
        case let .int(v): "\(v)"
        case let .float(v): "\(v)"
        case let .bool(v): "\(v)"
        case let .str(id): "str(\(id))"
        case let .handlerRef(id): "handler(\(id))"
        case .null: "null"
        case let .list(items): "list(\(items))"
        case let .record(fields): "record(\(fields.map { "\($0.propIndex):\($0.value)" }.joined(separator: ",")))"
        }
    }
}

extension FluxValue {
    /// Equality is defined explicitly because Swift cannot auto-synthesize
    /// `Equatable` for a recursive enum (`.list`/`.record` contain `FluxValue`).
    public static func == (lhs: FluxValue, rhs: FluxValue) -> Bool {
        switch lhs {
        case let .int(a):
            if case let .int(b) = rhs { a == b } else { false }
        case let .float(a):
            if case let .float(b) = rhs { a == b } else { false }
        case let .bool(a):
            if case let .bool(b) = rhs { a == b } else { false }
        case let .str(a):
            if case let .str(b) = rhs { a == b } else { false }
        case let .handlerRef(a):
            if case let .handlerRef(b) = rhs { a == b } else { false }
        case .null:
            if case .null = rhs { true } else { false }
        case let .list(a):
            if case let .list(b) = rhs { a == b } else { false }
        case let .record(a):
            if case let .record(b) = rhs {
                a.count == b.count && zip(a, b).allSatisfy {
                    $0.propIndex == $1.propIndex && $0.value == $1.value
                }
            } else { false }
        }
    }
}
