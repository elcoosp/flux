//  Instructions.swift
//  Bytecode decoder for the Flux Swift VM.
//
//  Decoding is total: any unassigned opcode byte yields an `invalidDispatch`
//  error rather than an invalid variant, mirroring the reference VM's decoder.
//  Operands are kept as raw little-endian bytes so the interpreter can extract
//  exactly the widths each opcode expects, without a per-instruction
//  allocation in the hot path.

import Foundation

/// A decoded instruction: its opcode and the raw operand bytes that follow it.
struct Instruction {
    /// The decoded opcode.
    let opcode: Opcode
    /// Byte offset of this instruction within the program (for diagnostics).
    let offset: Int
    /// Raw operand bytes (length == `opcode.operandLen`).
    private let operands: [UInt8]

    /// Reads a `u8` operand at `index` (0-based within the operand bytes).
    func u8(_ index: Int) -> UInt8 { operands[index] }

    /// Reads a little-endian `u16` operand starting at `index`.
    func u16(_ index: Int) -> UInt16 {
        UInt16(littleEndianBytes: Array(operands[index..<index + 2]))
    }

    /// Reads a little-endian `u32` operand starting at `index`.
    func u32(_ index: Int) -> UInt32 {
        UInt32(littleEndianBytes: Array(operands[index..<index + 4]))
    }

    /// Reads a little-endian `i32` operand starting at `index`.
    func i32(_ index: Int) -> Int32 {
        Int32(littleEndianBytes: Array(operands[index..<index + 4]))
    }

    /// Reads a little-endian `i64` operand starting at `index`.
    func i64(_ index: Int) -> Int64 {
        Int64(littleEndianBytes: Array(operands[index..<index + 8]))
    }

    /// Reads a little-endian `f64` operand starting at `index`.
    func f64(_ index: Int) -> Double {
        Double(bitPattern: UInt64(littleEndianBytes: Array(operands[index..<index + 8])))
    }

    /// Decodes a whole program into a vector of instructions.
    ///
    /// - Parameter bytes: The flat bytecode buffer.
    /// - Throws: `VmError.invalidDispatch` at the first byte that is not a valid
    ///   opcode, or `VmError.indexOutOfBounds` if the program is truncated.
    static func decode(_ bytes: [UInt8]) throws -> [Instruction] {
        var instrs: [Instruction] = []
        instrs.reserveCapacity(bytes.count / 2)
        var ip = 0
        while ip < bytes.count {
            let byte = bytes[ip]
            guard let op = Opcode(byte: byte) else {
                throw VmError.invalidDispatch(offset: ip)
            }
            let n = op.operandLen
            let start = ip + 1
            let end = start + n
            if end > bytes.count {
                throw VmError.indexOutOfBounds(offset: ip)
            }
            let operands = Array(bytes[start..<end])
            instrs.append(Instruction(opcode: op, offset: ip, operands: operands))
            ip = end
        }
        return instrs
    }
}

private extension FixedWidthInteger {
    /// Builds an integer from a little-endian byte array.
    init(littleEndianBytes bytes: [UInt8]) {
        precondition(bytes.count == MemoryLayout<Self>.size)
        self = bytes.reversed().reduce(0) { acc, byte in
            (acc << 8) | Self(byte)
        }
    }
}
