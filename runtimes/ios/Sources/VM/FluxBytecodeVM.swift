//  FluxBytecodeVM.swift
//  Native Swift implementation of the Flux VM (Appendix E).
//
//  This interpreter is a behavioral mirror of `flux-vm-ref` (the Rust oracles,
//  FLUX-005): the two implementations must produce identical observable state on
//  every golden ISA vector under `/tests/isa-vectors/`. The semantics here
//  incorporate the resolutions in ADR-0021 (HALT is free; r15 decrements as
//  instructions run), ADR-0023 (integer DIV/MOD by zero raises `DivByZero`;
//  float DIV by zero is IEEE ±inf) and ADR-0024 (`GET_FIELD` on `Null` raises
//  `NullDereference`, other non-records raise `TypeMismatch`).

import Foundation

/// The signal graph a handler reads from and writes to.
protocol SignalStore {
    /// Returns the current value of `id`, or `nil` if unbound.
    func read(_ id: UInt32) -> VMValue?
    /// Writes `value` into `id`.
    mutating func write(_ id: UInt32, _ value: VMValue)
    /// Returns every written signal as a sorted `(id, value)` list.
    func snapshot() -> [(UInt32, VMValue)]
}

/// In-memory `SignalStore` used by tests, the reconciler and the dev server.
///
/// A value type (not a class) so it can cross queue boundaries safely: the
/// executor reassigns its copy after each handler dispatch.
struct InMemorySignals: SignalStore {
    private var store: [UInt32: VMValue]

    init(store: [UInt32: VMValue] = [:]) {
        self.store = store
    }

    func read(_ id: UInt32) -> VMValue? { store[id] }

    mutating func write(_ id: UInt32, _ value: VMValue) { store[id] = value }

    func snapshot() -> [(UInt32, VMValue)] {
        store.map { ($0.key, $0.value) }.sorted { $0.0 < $1.0 }
    }
}

/// Result of running a handler to completion.
struct VmOutcome {
    /// Final values of all signal cells that were written, sorted by id.
    let signals: [(UInt32, VMValue)]
    /// Final values of the 16 registers (r0 = entry payload, r15 = remaining gas).
    let registers: [VMValue]
    /// Number of non-`HALT` instructions executed (ADR-0021).
    let gasUsed: UInt32
}

/// The native Flux bytecode VM.
enum FluxBytecodeVM {
    /// The handler entry gas budget (Appendix E §E.3).
    static let entryGas: UInt32 = 100_000

    /// Runs `bytecode` to completion against `signals`, with `payload` in `r0`.
    ///
    /// - Throws: a `VMError` when the handler faults (gas exhaustion, bad
    ///   dispatch, type error, out-of-bounds access, null dereference, or
    ///   division by zero).
    static func run(
        _ bytecode: [UInt8],
        signals: inout SignalStore,
        payload: VMValue
    ) throws -> VmOutcome {
        let program = try Instruction.decode(bytecode)
        let offsets = program.map { $0.offset }
        var regs = [VMValue](repeating: .null, count: 16)
        regs[0] = payload
        var gas = entryGas
        regs[15] = .int(Int64(gas))
        var ip = 0

        while ip < program.count {
            let instr = program[ip]
            let op = instr.opCode
            if op == .halt { break }
            if gas == 0 {
                throw VMError.gasExhausted(offset: instr.offset)
            }
            gas -= 1
            // Mirror the live gas budget into r15 (Appendix E §E.3; ADR-0021).
            regs[15] = .int(Int64(gas))
            let nextIP = ip + 1

            let reg = { (r: UInt8) -> VMValue in regs[Int(r)] }

            switch op {
            case .halt:
                break

            case .nop:
                break

            case .readSignal:
                let dst = instr.u8(0)
                let id = instr.u32(1)
                regs[Int(dst)] = signals.read(id) ?? .null

            case .writeSignal:
                let id = instr.u32(0)
                let src = instr.u8(4)
                signals.write(id, reg(src))

            case .addI64, .subI64, .mulI64, .divI64, .modI64:
                let dst = instr.u8(0)
                let a = try requireInt(reg(instr.u8(1)), at: instr.offset)
                let b = try requireInt(reg(instr.u8(2)), at: instr.offset)
                let r: Int64
                switch op {
                case .addI64: r = a &+ b
                case .subI64: r = a &- b
                case .mulI64: r = a &* b
                case .divI64:
                    if b == 0 { throw VMError.divByZero(offset: instr.offset) }
                    r = wrappingDiv(a, b)
                case .modI64:
                    if b == 0 { throw VMError.divByZero(offset: instr.offset) }
                    r = wrappingRem(a, b)
                default:
                    fatalError("unreachable: matched arithmetic set")
                }
                regs[Int(dst)] = .int(r)

            case .negI64:
                let dst = instr.u8(0)
                let v = try requireInt(reg(instr.u8(1)), at: instr.offset)
                regs[Int(dst)] = .int(0 &- v)

            case .eqI64, .ltI64, .gtI64, .lteI64, .gteI64:
                let dst = instr.u8(0)
                let a = try requireInt(reg(instr.u8(1)), at: instr.offset)
                let b = try requireInt(reg(instr.u8(2)), at: instr.offset)
                let r: Bool
                switch op {
                case .eqI64: r = a == b
                case .ltI64: r = a < b
                case .gtI64: r = a > b
                case .lteI64: r = a <= b
                case .gteI64: r = a >= b
                default:
                    fatalError("unreachable: matched integer compare set")
                }
                regs[Int(dst)] = .bool(r)

            case .addF64, .subF64, .mulF64, .divF64:
                let dst = instr.u8(0)
                let a = try requireFloat(reg(instr.u8(1)), at: instr.offset)
                let b = try requireFloat(reg(instr.u8(2)), at: instr.offset)
                let r: Double
                switch op {
                case .addF64: r = a + b
                case .subF64: r = a - b
                case .mulF64: r = a * b
                case .divF64: r = fdiv(a, b)
                default:
                    fatalError("unreachable: matched float arithmetic set")
                }
                regs[Int(dst)] = .float(r)

            case .negF64:
                let dst = instr.u8(0)
                let v = try requireFloat(reg(instr.u8(1)), at: instr.offset)
                regs[Int(dst)] = .float(-v)

            case .eqF64, .ltF64, .gtF64:
                let dst = instr.u8(0)
                let a = try requireFloat(reg(instr.u8(1)), at: instr.offset)
                let b = try requireFloat(reg(instr.u8(2)), at: instr.offset)
                let r: Bool
                switch op {
                case .eqF64: r = (a == b) || (a.isNaN && b.isNaN)
                case .ltF64: r = a < b
                case .gtF64: r = a > b
                default:
                    fatalError("unreachable: matched float compare set")
                }
                regs[Int(dst)] = .bool(r)

            case .i64ToF64:
                let dst = instr.u8(0)
                let v = try requireInt(reg(instr.u8(1)), at: instr.offset)
                regs[Int(dst)] = .float(Double(v))

            case .f64ToI64:
                let dst = instr.u8(0)
                let v = try requireFloat(reg(instr.u8(1)), at: instr.offset)
                regs[Int(dst)] = .int(Int64(v))

            case .andBool:
                let dst = instr.u8(0)
                let x = try requireBool(reg(instr.u8(1)), at: instr.offset)
                let y = try requireBool(reg(instr.u8(2)), at: instr.offset)
                regs[Int(dst)] = .bool(x && y)

            case .orBool:
                let dst = instr.u8(0)
                let x = try requireBool(reg(instr.u8(1)), at: instr.offset)
                let y = try requireBool(reg(instr.u8(2)), at: instr.offset)
                regs[Int(dst)] = .bool(x || y)

            case .notBool:
                let dst = instr.u8(0)
                let v = try requireBool(reg(instr.u8(1)), at: instr.offset)
                regs[Int(dst)] = .bool(!v)

            case .strIntern:
                regs[Int(instr.u8(0))] = .str(instr.u32(1))

            case .strEq:
                let dst = instr.u8(0)
                let x = try requireStr(reg(instr.u8(1)), at: instr.offset)
                let y = try requireStr(reg(instr.u8(2)), at: instr.offset)
                regs[Int(dst)] = .bool(x == y)

            case .strLen:
                let dst = instr.u8(0)
                let id = try requireStr(reg(instr.u8(1)), at: instr.offset)
                // Length is the id's decimal digit count (the oracle has no live
                // string table; this is deterministic and matches it).
                regs[Int(dst)] = .int(Int64(digitCount(id)))

            case .strConcat:
                let dst = instr.u8(0)
                let x = try requireStr(reg(instr.u8(1)), at: instr.offset)
                let y = try requireStr(reg(instr.u8(2)), at: instr.offset)
                let combined = (Int64(x) &* 10_000_000) &+ Int64(y)
                regs[Int(dst)] = .str(UInt32(truncatingIfNeeded: combined))

            case .jump:
                ip = try jumpTarget(instr, nextIP: nextIP, offsets: offsets, delta: instr.i32(0))
                continue

            case .condJump, .condJumpNot:
                let taken = truthy(reg(instr.u8(0)))
                let want = op == .condJump
                if taken == want {
                    ip = try jumpTarget(instr, nextIP: nextIP, offsets: offsets, delta: instr.i32(1))
                    continue
                }

            case .allocRecord:
                let dst = instr.u8(0)
                let count = Int(instr.u16(1))
                var fields: [(UInt16, VMValue)] = []
                fields.reserveCapacity(count)
                for i in 0..<count {
                    fields.append((UInt16(i), .null))
                }
                regs[Int(dst)] = .record(fields)

            case .getField:
                let dst = instr.u8(0)
                let idx = Int(instr.u16(1))
                let field = try getField(reg(instr.u8(3)), idx: idx, at: instr.offset)
                regs[Int(dst)] = field

            case .setField:
                let obj = instr.u8(0)
                let idx = Int(instr.u16(1))
                let val = reg(instr.u8(3))
                try setField(&regs[Int(obj)], idx: idx, value: val, at: instr.offset)

            case .recordEq:
                let dst = instr.u8(0)
                let x = try requireRecord(reg(instr.u8(1)), at: instr.offset)
                let y = try requireRecord(reg(instr.u8(2)), at: instr.offset)
                regs[Int(dst)] = .bool(recordsEqual(x, y))

            case .allocList:
                regs[Int(instr.u8(0))] = .list([])

            case .listPush:
                let list = instr.u8(0)
                let val = reg(instr.u8(1))
                guard case var .list(items) = regs[Int(list)] else {
                    throw VMError.typeMismatch(offset: instr.offset)
                }
                items.append(val)
                regs[Int(list)] = .list(items)

            case .listGet:
                let dst = instr.u8(0)
                let items = try requireList(reg(instr.u8(1)), at: instr.offset)
                let i = Int(instr.u8(2))
                guard i < items.count else {
                    throw VMError.indexOutOfBounds(offset: instr.offset)
                }
                regs[Int(dst)] = items[i]

            case .listLen:
                let items = try requireList(reg(instr.u8(1)), at: instr.offset)
                regs[Int(instr.u8(0))] = .int(Int64(items.count))

            case .listConcat:
                let dst = instr.u8(0)
                let a = try requireList(reg(instr.u8(1)), at: instr.offset)
                let b = try requireList(reg(instr.u8(2)), at: instr.offset)
                regs[Int(dst)] = .list(a + b)

            case .callCap:
                let resultReg = instr.u8(0)
                let capID = instr.u32(1)
                let methodID = instr.u16(5)
                let argsReg = instr.u8(7)
                if capID == 1, methodID == 1 {
                    guard case let .record(fields) = reg(argsReg), !fields.isEmpty else {
                        throw VMError.typeMismatch(offset: instr.offset)
                    }
                    let arg = fields[0].value
                    signals.write(99, arg)
                    regs[Int(resultReg)] = arg
                } else {
                    throw VMError.typeMismatch(offset: instr.offset)
                }

            case .matchTag:
                let val = reg(instr.u8(0))
                let tag = instr.u32(1)
                var matched = false
                if case let .record(fields) = val, let first = fields.first {
                    if case let .int(t) = first.value, t == Int64(tag) {
                        matched = true
                    }
                }
                if matched {
                    ip = try jumpTarget(instr, nextIP: nextIP, offsets: offsets, delta: instr.i32(5))
                    continue
                }

            case .extractField:
                let dst = instr.u8(0)
                let idx = Int(instr.u16(1))
                let field = try getField(reg(instr.u8(3)), idx: idx, at: instr.offset)
                regs[Int(dst)] = field

            case .loadIntConst:
                regs[Int(instr.u8(0))] = .int(instr.i64(1))

            case .loadFloatConst:
                regs[Int(instr.u8(0))] = .float(instr.f64(1))

            case .loadBoolConst:
                regs[Int(instr.u8(0))] = .bool(instr.u8(1) != 0)

            case .loadStrConst:
                regs[Int(instr.u8(0))] = .str(instr.u32(1))

            case .loadNull:
                regs[Int(instr.u8(0))] = .null

            case .mov:
                regs[Int(instr.u8(0))] = reg(instr.u8(1))

            case .gasCheck:
                let budget = instr.u32(0)
                if gas < budget {
                    throw VMError.gasExhausted(offset: instr.offset)
                }
            }

            ip = nextIP
        }

        return VmOutcome(
            signals: signals.snapshot(),
            registers: regs,
            gasUsed: entryGas - gas
        )
    }

    // MARK: - Helpers

    /// IEEE-754 division: `x/0.0` is `±inf` (ADR-0023), never an error.
    private static func fdiv(_ x: Double, _ y: Double) -> Double {
        if y == 0.0 {
            if x.isNaN { return Double.nan }
            return x >= 0.0 ? Double.infinity : -Double.infinity
        }
        return x / y
    }

    private static func truthy(_ v: VMValue) -> Bool {
        switch v {
        case let .bool(b): b
        case let .int(i): i != 0
        default: false
        }
    }

    private static func requireInt(_ v: VMValue, at offset: Int) throws -> Int64 {
        guard case let .int(i) = v else { throw VMError.typeMismatch(offset: offset) }
        return i
    }

    private static func requireFloat(_ v: VMValue, at offset: Int) throws -> Double {
        guard case let .float(f) = v else { throw VMError.typeMismatch(offset: offset) }
        return f
    }

    private static func requireBool(_ v: VMValue, at offset: Int) throws -> Bool {
        guard case let .bool(b) = v else { throw VMError.typeMismatch(offset: offset) }
        return b
    }

    private static func requireStr(_ v: VMValue, at offset: Int) throws -> UInt32 {
        guard case let .str(id) = v else { throw VMError.typeMismatch(offset: offset) }
        return id
    }

    private static func requireList(_ v: VMValue, at offset: Int) throws -> [VMValue] {
        guard case let .list(items) = v else { throw VMError.typeMismatch(offset: offset) }
        return items
    }

    private static func requireRecord(_ v: VMValue, at offset: Int) throws -> [(UInt16, VMValue)] {
        guard case let .record(fields) = v else { throw VMError.typeMismatch(offset: offset) }
        return fields
    }

    /// Structural record equality: same field count, same prop indices in order,
    /// and equal values (recursively).
    private static func recordsEqual(
        _ a: [(UInt16, VMValue)],
        _ b: [(UInt16, VMValue)]
    ) -> Bool {
        guard a.count == b.count else { return false }
        for (lhs, rhs) in zip(a, b) {
            guard lhs.0 == rhs.0, lhs.1 == rhs.1 else { return false }
        }
        return true
    }

    private static func getField(_ obj: VMValue, idx: Int, at offset: Int) throws -> VMValue {
        if case .null = obj {
            throw VMError.nullDereference(offset: offset)
        }
        guard case let .record(fields) = obj else {
            throw VMError.typeMismatch(offset: offset)
        }
        guard idx < fields.count else {
            throw VMError.indexOutOfBounds(offset: offset)
        }
        return fields[idx].value
    }

    private static func setField(_ obj: inout VMValue, idx: Int, value: VMValue, at offset: Int) throws {
        if case .null = obj {
            throw VMError.nullDereference(offset: offset)
        }
        guard case var .record(fields) = obj else {
            throw VMError.typeMismatch(offset: offset)
        }
        guard idx < fields.count else {
            throw VMError.indexOutOfBounds(offset: offset)
        }
        fields[idx].value = value
        obj = .record(fields)
    }

    /// Resolves a relative jump offset (relative to the *next* instruction) to a
    /// program index, or `IndexOutOfBounds` if it lands outside the program.
    private static func jumpTarget(
        _ instr: Instruction,
        nextIP: Int,
        offsets: [Int],
        delta: Int32
    ) throws -> Int {
        guard nextIP < offsets.count else {
            throw VMError.indexOutOfBounds(offset: instr.offset)
        }
        let base = Int64(offsets[nextIP])
        let target = base + Int64(delta)
        guard let t = UInt32(exactly: target),
              let index = offsets.firstIndex(of: Int(t)) else {
            throw VMError.indexOutOfBounds(offset: instr.offset)
        }
        return index
    }
}

/// Returns the decimal digit count of a non-negative integer (0 has 1 digit),
/// matching the oracle's `ilog10 + 1` computation used for `STR_LEN`.
private func digitCount(_ n: UInt32) -> Int {
    if n == 0 { return 1 }
    var n = n
    var count = 0
    while n > 0 {
        n /= 10
        count += 1
    }
    return count
}

/// Wrapping signed division, matching Rust's `wrapping_div` (the only
/// overflow case is `Int64.min / -1`, which yields `Int64.min`).
private func wrappingDiv(_ x: Int64, _ y: Int64) -> Int64 {
    if x == .min, y == -1 { return .min }
    return x / y
}

/// Wrapping signed remainder, matching Rust's `wrapping_rem`.
private func wrappingRem(_ x: Int64, _ y: Int64) -> Int64 {
    if x == .min, y == -1 { return 0 }
    return x % y
}
