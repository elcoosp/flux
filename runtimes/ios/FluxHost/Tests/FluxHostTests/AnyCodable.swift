//  AnyCodable.swift
//  Minimal type-erased codable value used only by the ISA vector fixtures.
//
//  The vector JSON encodes values with heterogeneous `value` payloads (ints,
//  floats, bools, strings, nested arrays), so we decode generically and extract
//  what each vector needs. Test-support only.

import Foundation

struct AnyCodable: Decodable {
    let value: Any

    init(_ value: Any) { self.value = value }

    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if container.decodeNil() {
            value = Optional<Any>.none as Any
        } else if let b = try? container.decode(Bool.self) {
            value = b
        } else if let i = try? container.decode(Int64.self) {
            value = i
        } else if let d = try? container.decode(Double.self) {
            value = d
        } else if let s = try? container.decode(String.self) {
            value = s
        } else if let a = try? container.decode([AnyCodable].self) {
            value = a
        } else {
            value = Optional<Any>.none as Any
        }
    }

    var asDouble: Double? {
        if let d = value as? Double { d }
        else if let i = value as? Int64 { Double(i) }
        else if let s = value as? String {
            switch s {
            case "inf": Double.infinity
            case "-inf": -Double.infinity
            case "nan": Double.nan
            default: nil
            }
        } else { nil }
    }

    var asInt64: Int64? { value as? Int64 }
    var asBool: Bool? { value as? Bool }
    var asString: String? { value as? String }
    var asArray: [AnyCodable]? { value as? [AnyCodable] }
}
