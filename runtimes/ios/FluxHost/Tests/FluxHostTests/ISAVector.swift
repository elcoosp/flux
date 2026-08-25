//  ISAVector.swift
//  Fixture model for the ISA conformance test.
//
//  Mirrors the JSON shape documented in `/tests/isa-vectors/README.md`. These
//  structs are test-support only and live in the runtime test target.

import Foundation

/// A value as encoded in an ISA vector JSON file.
struct VecValue: Decodable {
    let type: String
    let value: AnyCodable?

    /// Memberwise initializer for test construction (the default `Decodable`
    /// `init(from:)` cannot be called directly).
    init(type: String, value: AnyCodable?) {
        self.type = type
        self.value = value
    }

    enum CodingKeys: String, CodingKey {
        case type
        case value
    }
}

/// A signal seed (id + value) in an ISA vector.
struct SignalSeed: Decodable {
    let id: UInt32
    let value: VecValue
}

/// The decoded error kind expected by a vector.
enum ExpectedError: String, Decodable {
    case gasExhausted = "GasExhausted"
    case memoryExhausted = "MemoryExhausted"
    case indexOutOfBounds = "IndexOutOfBounds"
    case nullDereference = "NullDereference"
    case invalidDispatch = "InvalidDispatch"
    case typeMismatch = "TypeMismatch"
    case divByZero = "DivByZero"
}

/// A single golden ISA vector.
struct ISAVector: Decodable {
    let name: String
    let description: String
    let bytecodeHex: String
    let initialSignals: [SignalSeed]
    let payload: VecValue?
    let expectedSignals: [SignalSeed]
    let expectedRegisters: [String: VecValue]
    let expectedError: ExpectedError?
    let expectedGasUsed: UInt32

    enum CodingKeys: String, CodingKey {
        case name
        case description
        case bytecodeHex = "bytecode_hex"
        case initialSignals = "initial_signals"
        case payload
        case expectedSignals = "expected_signals"
        case expectedRegisters = "expected_registers"
        case expectedError = "expected_error"
        case expectedGasUsed = "expected_gas_used"
    }
}
