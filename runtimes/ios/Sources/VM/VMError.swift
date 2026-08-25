//  VMError.swift
//  Fault types for the Flux Swift VM.
//
//  Every error carries the byte offset of the offending instruction. The kinds
//  are a superset of Appendix E §E.6: `divByZero` is added by ADR-0023 (integer
//  division by zero must fail rather than panic) and `nullDereference` vs
//  `typeMismatch` for `GET_FIELD` is resolved by ADR-0024.

import Foundation

/// Why a handler invocation terminated without producing a value.
enum VMErrorKind: Equatable {
    /// The 100,000-instruction gas budget was exhausted (Appendix E §E.3).
    case gasExhausted
    /// An index (list/record/string) fell outside its bounds.
    case indexOutOfBounds
    /// A field access was performed on `Null` (ADR-0024).
    case nullDereference
    /// The dispatch byte was not a valid opcode.
    case invalidDispatch
    /// Operand types were not what the (monomorphized) opcode expected.
    case typeMismatch
    /// Integer division or remainder by zero (ADR-0023).
    case divByZero
    /// A handler exceeded the per-dispatch allocation budget (§NFR-SEC-003 /
    /// ADR-0015). The VM bounds total heap it may allocate (records, lists)
    /// so a runaway closure cannot exhaust device memory.
    case memoryExhausted
}

extension VMErrorKind {
    /// The string name emitted in diagnostics and matched by tests.
    var name: String {
        switch self {
        case .gasExhausted: "GasExhausted"
        case .indexOutOfBounds: "IndexOutOfBounds"
        case .nullDereference: "NullDereference"
        case .invalidDispatch: "InvalidDispatch"
        case .typeMismatch: "TypeMismatch"
        case .divByZero: "DivByZero"
        case .memoryExhausted: "MemoryExhausted"
        }
    }
}

/// A VM fault with its location in the bytecode.
struct VMError: Error, Equatable {
    /// The category of fault.
    let kind: VMErrorKind
    /// Byte offset of the offending instruction in the program.
    let offset: Int

    static func gasExhausted(offset: Int) -> VMError { VMError(kind: .gasExhausted, offset: offset) }
    static func indexOutOfBounds(offset: Int) -> VMError { VMError(kind: .indexOutOfBounds, offset: offset) }
    static func nullDereference(offset: Int) -> VMError { VMError(kind: .nullDereference, offset: offset) }
    static func invalidDispatch(offset: Int) -> VMError { VMError(kind: .invalidDispatch, offset: offset) }
    static func typeMismatch(offset: Int) -> VMError { VMError(kind: .typeMismatch, offset: offset) }
    static func divByZero(offset: Int) -> VMError { VMError(kind: .divByZero, offset: offset) }
    static func memoryExhausted(offset: Int) -> VMError { VMError(kind: .memoryExhausted, offset: offset) }
}
