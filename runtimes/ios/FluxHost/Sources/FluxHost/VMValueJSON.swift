//  FluxValueJSON.swift
//  JSON (de)serialization for `FluxValue`, used to persist `Storage` values to
//  `UserDefaults` (Appendix D §D.5 wire shape, expressed as JSON). Kept
//  dependency-free (Foundation only) so the pure runtime package stays
//  platform-neutral.

import Foundation

/// JSON (de)serialization for `FluxValue`.
enum FluxValueJSON {
    /// Encodes `value` to JSON `Data`.
    /// - Throws: `VmError.typeMismatch` if the value cannot be represented.
    static func encode(_ value: FluxValue) throws -> Data {
        try JSONSerialization.data(withJSONObject: box(value))
    }

    /// Decodes `value` from JSON `Data`.
    /// - Throws: `VmError.typeMismatch` if the bytes are not a valid encoding.
    static func decode(_ data: Data) throws -> FluxValue {
        try unbox(try JSONSerialization.jsonObject(with: data))
    }

    // MARK: - encode

    private static func box(_ v: FluxValue) -> Any {
        switch v {
        case .null:
            return ["t": "null"]
        case let .int(n):
            return ["t": "int", "v": NSNumber(value: n)]
        case let .float(d):
            return ["t": "float", "v": NSNumber(value: d)]
        case let .bool(b):
            return ["t": "bool", "v": NSNumber(value: b)]
        case let .str(id):
            return ["t": "str", "v": NSNumber(value: UInt64(id))]
        case let .handlerRef(id):
            return ["t": "handler", "v": NSNumber(value: UInt64(id))]
        case let .list(items):
            return ["t": "list", "v": items.map(box)]
        case let .record(fields):
            return ["t": "rec", "v": fields.map {
                ["i": NSNumber(value: UInt64($0.propIndex)), "v": box($0.value)]
            }]
        }
    }

    // MARK: - decode

    private static func unbox(_ obj: Any) throws -> FluxValue {
        guard let dict = obj as? [String: Any], let raw = dict["t"] as? String else {
            throw VmError.typeMismatch(offset: 0)
        }
        switch raw {
        case "null":
            return .null
        case "int":
            return .int(try int64(dict["v"]))
        case "float":
            return .float(try double(dict["v"]))
        case "bool":
            return .bool(try bool(dict["v"]))
        case "str":
            return .str(try u32(dict["v"]))
        case "handler":
            return .handlerRef(try u32(dict["v"]))
        case "list":
            guard let arr = dict["v"] as? [Any] else { throw VmError.typeMismatch(offset: 0) }
            return .list(try arr.map { try unbox($0) })
        case "rec":
            guard let arr = dict["v"] as? [[String: Any]] else { throw VmError.typeMismatch(offset: 0) }
            return .record(try arr.map {
                let i = try u16($0["i"])
                let v = try unbox($0["v"] as Any)
                return (i, v)
            })
        default:
            throw VmError.typeMismatch(offset: 0)
        }
    }

    private static func num(_ v: Any?) throws -> NSNumber {
        guard let n = v as? NSNumber else { throw VmError.typeMismatch(offset: 0) }
        return n
    }

    private static func int64(_ v: Any?) throws -> Int64 { try num(v).int64Value }
    private static func double(_ v: Any?) throws -> Double { try num(v).doubleValue }
    private static func bool(_ v: Any?) throws -> Bool { try num(v).boolValue }
    private static func u32(_ v: Any?) throws -> UInt32 { UInt32(truncatingIfNeeded: try num(v).int64Value) }
    private static func u16(_ v: Any?) throws -> UInt16 { UInt16(truncatingIfNeeded: try num(v).int64Value) }
}
