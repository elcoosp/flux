//  Opcodes.swift
//  Native Swift mirror of `flux_syntax::opcode` (Appendix E §E.1).
//
//  The instruction set is intentionally minimal and monomorphized: there is no
//  generic `ADD` with runtime tag dispatch, only type-specific `ADD_I64` and
//  `ADD_F64`. These byte values are a wire contract shared with the Rust
//  reference VM and the Kotlin VM.

import Foundation

/// A decoded VM opcode. Decoding is total: an unassigned byte yields `nil`
/// rather than an invalid variant, so a corrupt or future-versioned frame is
/// reported as a protocol error instead of producing undefined behaviour.
enum Opcode: UInt8, CaseIterable, Equatable {
    case halt = 0x00
    case nop = 0x01

    case readSignal = 0x10
    case writeSignal = 0x11

    case addI64 = 0x20
    case subI64 = 0x21
    case mulI64 = 0x22
    case divI64 = 0x23
    case modI64 = 0x24
    case negI64 = 0x25
    case eqI64 = 0x26
    case ltI64 = 0x27
    case gtI64 = 0x28
    case lteI64 = 0x29
    case gteI64 = 0x2A

    case addF64 = 0x30
    case subF64 = 0x31
    case mulF64 = 0x32
    case divF64 = 0x33
    case negF64 = 0x34
    case eqF64 = 0x35
    case ltF64 = 0x36
    case gtF64 = 0x37
    case i64ToF64 = 0x38
    case f64ToI64 = 0x39

    case andBool = 0x40
    case orBool = 0x41
    case notBool = 0x42

    case strConcat = 0x50
    case strIntern = 0x51
    case strEq = 0x52
    case strLen = 0x53

    case jump = 0x60
    case condJump = 0x61
    case condJumpNot = 0x62

    case allocRecord = 0x70
    case getField = 0x71
    case setField = 0x72
    case recordEq = 0x73

    case allocList = 0x80
    case listPush = 0x81
    case listGet = 0x82
    case listLen = 0x83
    case listConcat = 0x84
    // --- FLUX-072: dynamic-list mutation opcodes (mirror flux-vm-ref). ---
    // These were added to the Rust oracle + compiler but the host VM lacked
    // them, so `tasks.clear()` / `tasks.remove(item)` / `tasks.insert(i, x)`
    // hit an unknown-opcode branch and the list signal never changed on device.
    case listInsert = 0x85
    case listRemove = 0x86
    case listClear = 0x87
    case listRemoveItem = 0x88

    case callCap = 0x90

    case matchTag = 0xA0
    case extractField = 0xA1

    case loadIntConst = 0xB0
    case loadFloatConst = 0xB1
    case loadBoolConst = 0xB2
    case loadStrConst = 0xB3
    case loadNull = 0xB4
    case mov = 0xB5

    case gasCheck = 0xC0

    case toString = 0xD0

    /// Suspends the VM, capturing the continuation (ADR-0044, MLP v2 first-class async).
    /// `AWAIT resultReg(u8), futureReg(u8)`: the handler parks after this instruction;
    /// the executor resumes it via `FluxBytecodeVM.resume`, which deposits the resolved
    /// future value into `r0`.
    case await = 0xE0

    /// The Appendix E mnemonic, e.g. `"ADD_I64"`.
    var mnemonic: String {
        switch self {
        case .halt: "HALT"
        case .nop: "NOP"
        case .readSignal: "READ_SIGNAL"
        case .writeSignal: "WRITE_SIGNAL"
        case .addI64: "ADD_I64"
        case .subI64: "SUB_I64"
        case .mulI64: "MUL_I64"
        case .divI64: "DIV_I64"
        case .modI64: "MOD_I64"
        case .negI64: "NEG_I64"
        case .eqI64: "EQ_I64"
        case .ltI64: "LT_I64"
        case .gtI64: "GT_I64"
        case .lteI64: "LTE_I64"
        case .gteI64: "GTE_I64"
        case .addF64: "ADD_F64"
        case .subF64: "SUB_F64"
        case .mulF64: "MUL_F64"
        case .divF64: "DIV_F64"
        case .negF64: "NEG_F64"
        case .eqF64: "EQ_F64"
        case .ltF64: "LT_F64"
        case .gtF64: "GT_F64"
        case .i64ToF64: "I64_TO_F64"
        case .f64ToI64: "F64_TO_I64"
        case .andBool: "AND_BOOL"
        case .orBool: "OR_BOOL"
        case .notBool: "NOT_BOOL"
        case .strConcat: "STR_CONCAT"
        case .strIntern: "STR_INTERN"
        case .strEq: "STR_EQ"
        case .strLen: "STR_LEN"
        case .jump: "JUMP"
        case .condJump: "COND_JUMP"
        case .condJumpNot: "COND_JUMP_NOT"
        case .allocRecord: "ALLOC_RECORD"
        case .getField: "GET_FIELD"
        case .setField: "SET_FIELD"
        case .recordEq: "RECORD_EQ"
        case .allocList: "ALLOC_LIST"
        case .listPush: "LIST_PUSH"
        case .listGet: "LIST_GET"
        case .listLen: "LIST_LEN"
        case .listConcat: "LIST_CONCAT"
        case .listInsert: "LIST_INSERT"
        case .listRemove: "LIST_REMOVE"
        case .listClear: "LIST_CLEAR"
        case .listRemoveItem: "LIST_REMOVE_ITEM"
        case .callCap: "CALL_CAP"
        case .matchTag: "MATCH_TAG"
        case .extractField: "EXTRACT_FIELD"
        case .loadIntConst: "LOAD_INT_CONST"
        case .loadFloatConst: "LOAD_FLOAT_CONST"
        case .loadBoolConst: "LOAD_BOOL_CONST"
        case .loadStrConst: "LOAD_STR_CONST"
        case .loadNull: "LOAD_NULL"
        case .mov: "MOV"
        case .gasCheck: "GAS_CHECK"
        case .toString: "TO_STRING"
        case .await: "AWAIT"
        }
    }

    /// The number of operand bytes that follow this opcode. Adding 1 (the
    /// opcode byte) gives the total instruction width. Derived from the §E.1
    /// width table; lengths are normative (ADR-0022).
    var operandLen: Int {
        switch self {
        case .halt, .nop: 0
        case .loadNull: 1
        case .negI64, .negF64, .i64ToF64, .f64ToI64, .notBool, .strLen,
             .mov, .listLen, .toString: 2
        case .addI64, .subI64, .mulI64, .divI64, .modI64, .eqI64, .ltI64, .gtI64,
             .lteI64, .gteI64, .addF64, .subF64, .mulF64, .divF64, .eqF64, .ltF64,
             .gtF64, .andBool, .orBool, .strConcat, .strEq, .recordEq, .listGet,
             .listConcat: 3
        case .readSignal, .writeSignal, .condJump, .condJumpNot, .strIntern,
             .loadStrConst: 5
        case .allocRecord, .allocList: 3
        case .loadBoolConst, .listPush: 2
        case .listInsert: 4
        case .listRemove: 3
        case .listClear: 1
        case .listRemoveItem: 2
        case .loadIntConst, .loadFloatConst: 9
        case .jump: 4
        case .gasCheck: 4
        case .getField, .setField, .extractField: 4
        case .matchTag: 9
        case .callCap: 8
        case .await: 2
        }
    }

    /// The total instruction width in bytes, including the opcode byte.
    var instructionLen: Int { operandLen + 1 }

    /// Decodes an opcode byte, returning `nil` for any unassigned value.
    init?(byte: UInt8) {
        self.init(rawValue: byte)
    }
}
