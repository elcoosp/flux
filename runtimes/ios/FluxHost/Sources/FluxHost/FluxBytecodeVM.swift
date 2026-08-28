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
    func read(_ id: UInt32) -> FluxValue?
    /// Writes `value` into `id`.
    mutating func write(_ id: UInt32, _ value: FluxValue)
    /// Allocates a fresh, unbound signal id for a new capability result cell
    /// (ADR-0045). Drawn from a high ceiling so it never collides with the low,
    /// fixed ids handlers and golden vectors use (e.g. 99).
    mutating func allocateCell() -> UInt32
    /// Returns the reactive `CellState` of `id`, defaulting to `.ready`.
    func cellState(_ id: UInt32) -> CellState
    /// Marks `id` as `.pending` (an async capability has started).
    mutating func markPending(_ id: UInt32)
    /// Resolves `id` to `value`, marking it `.ready` (an async capability finished).
    mutating func resolveCell(_ id: UInt32, _ value: FluxValue)
    /// Returns every written signal as a sorted `(id, value)` list.
    func snapshot() -> [(UInt32, FluxValue)]
}

/// In-memory `SignalStore` used by tests, the reconciler and the dev server.
///
/// A value type (not a class) so it can cross queue boundaries safely: the
/// executor reassigns its copy after each handler dispatch.
struct InMemorySignals: SignalStore {
    private var store: [UInt32: FluxValue]
    /// Reactive state of each cell (ADR-0045). A `write`/ `resolveCell` resolves a
    /// cell to `.ready`; an async capability marks its cell `.pending` until the
    /// host resolves it.
    private var cellStates: [UInt32: CellState]
    /// Monotonic id source for `allocateCell`, drawn from a high ceiling so it
    /// never collides with fixed ids like 99.
    private var nextCell: UInt32 = 1_000_000

    init(store: [UInt32: FluxValue] = [:]) {
        self.store = store
        self.cellStates = [:]
    }

    func read(_ id: UInt32) -> FluxValue? { store[id] }

    mutating func write(_ id: UInt32, _ value: FluxValue) {
        store[id] = value
        cellStates[id] = .ready
    }

    mutating func allocateCell() -> UInt32 {
        nextCell &+= 1
        return nextCell
    }

    func cellState(_ id: UInt32) -> CellState {
        cellStates[id] ?? .ready
    }

    mutating func markPending(_ id: UInt32) {
        cellStates[id] = .pending
    }

    mutating func resolveCell(_ id: UInt32, _ value: FluxValue) {
        store[id] = value
        cellStates[id] = .ready
    }

    func snapshot() -> [(UInt32, FluxValue)] {
        store.map { ($0.key, $0.value) }.sorted { $0.0 < $1.0 }
    }
}

/// Result of running a handler to completion.
struct VmOutcome {
    /// Final values of all signal cells that were written, sorted by id.
    let signals: [(UInt32, FluxValue)]
    /// Final values of the 16 registers (r0 = entry payload, r15 = remaining gas).
    let registers: [FluxValue]
    /// Number of non-`HALT` instructions executed (ADR-0021).
    let gasUsed: UInt32
}

/// The captured continuation of a suspended handler (ADR-0044, MLP v2 async).
///
/// The VM is a flat register machine with no call stack, so a suspend is exactly its
/// live interpreter state: the next instruction offset, the register file, the
/// remaining gas, and the snapshot of signal cells written before the `AWAIT`.
/// `FluxBytecodeVM.resume` re-enters the interpreter at `resumeOffset` with the
/// delivered value placed in `r0`. This is a value type so it can cross the executor's
/// background→main queue boundary safely.
struct SuspendState {
    /// The original bytecode program, re-decoded on resume so the tail can be
    /// executed from `resumeOffset` without the caller retaining the bytes.
    let program: [UInt8]
    /// Byte offset of the instruction to execute on resume (the byte after `AWAIT`).
    let resumeOffset: UInt32
    /// Register file at the point of suspension.
    let registers: [FluxValue]
    /// Remaining gas at the point of suspension; continues decrementing on resume.
    let gasRemaining: UInt32
    /// Signal cells written before the suspend, replayed into the graph on resume.
    let signals: [(UInt32, FluxValue)]
    /// The register holding the awaited future handle at suspension. The executor
    /// reads `registers[futureReg]` to obtain the future to resolve (ADR-0044).
    let futureReg: UInt8
}

/// The result of a resumable handler dispatch (ADR-0044).
enum RunResult {
    /// The handler ran to `HALT`.
    case halt(VmOutcome)
    /// The handler suspended at an `AWAIT`; resume it with `FluxBytecodeVM.resume`.
    case suspended(SuspendState)
}

/// The native Flux bytecode VM.
enum FluxBytecodeVM {
    /// The handler entry gas budget (Appendix E §E.3).
    static let entryGas: UInt32 = 100_000

    /// The per-dispatch allocation budget in bytes (§NFR-SEC-003 / ADR-0015).
    /// A closure may allocate at most this many bytes across `ALLOC_RECORD` /
    /// `ALLOC_LIST` / `LIST_PUSH` before the VM raises `MemoryExhausted`. This
    /// bounds runaway handlers so a single bad closure cannot exhaust device
    /// memory. 16 MiB per handler invocation.
    static let allocationBudget: UInt64 = 16_000_000

    /// Runs `bytecode` to completion against `signals`, with `payload` in `r0`.
    ///
    /// - Parameters:
    ///   - stringTable: resolves the interned `StringId`s referenced by
    ///     `STR_LEN` / `STR_CONCAT` (Appendix E §E.1). Defaults to an empty
    ///     table, which yields `MemoryExhausted`-free but unresolved strings.
    ///   - capRegistry: routes `CALL_CAP` (capability) invocations by
    ///     `(capId, methodId)` to native/dev implementations (G4). Defaults to
    ///     the `CapabilityRegistry.dev` placeholder table.
    /// - Throws: a `VmError` when the handler faults (gas exhaustion, memory
    ///   exhaustion, bad dispatch, type error, out-of-bounds access, null
    ///   dereference, or division by zero).
    /// Runs `bytecode` to completion against `signals`, with `payload` in `r0`.
    ///
    /// Generic over `S: SignalStore` so the host can pass its concrete signal
    /// graph by reference and avoid re-boxing into an `any SignalStore` existential
    /// on the dispatch hot path (R3 / Perf review). Decodes the bytecode then runs
    /// the already-decoded instruction stream.
    static func run<S: SignalStore>(
        _ bytecode: [UInt8],
        signals: inout S,
        payload: FluxValue,
        stringTable: any StringResolver = EmptyStringTable(),
        capRegistry: CapabilityRegistry = .dev
    ) throws -> VmOutcome {
        let program = try Instruction.decode(bytecode)
        return try run(program, signals: &signals, payload: payload, stringTable: stringTable, capRegistry: capRegistry)
    }

    /// Runs an already-decoded instruction stream (R3). The executor caches the
    /// `[Instruction]` per handler at registration time and reuses it across
    /// dispatches, so this is the hot path that avoids re-decoding every tap.
    static func run<S: SignalStore>(
        _ program: [Instruction],
        signals: inout S,
        payload: FluxValue,
        stringTable: any StringResolver = EmptyStringTable(),
        capRegistry: CapabilityRegistry = .dev
    ) throws -> VmOutcome {
        let offsets = program.map { $0.offset }
        var regs = [FluxValue](repeating: .null, count: 16)
        regs[0] = payload
        var gas = entryGas
        regs[15] = .int(Int64(gas))
        // Running allocation counter, checked against `allocationBudget`.
        var allocated: UInt64 = 0
        // Bind the string table to a `var` so `STR_CONCAT` can intern new text.
        var stringTable = stringTable
        var ip = 0

        while ip < program.count {
            let instr = program[ip]
            let op = instr.opcode
            if op == .halt { break }
            if gas == 0 {
                throw VmError.gasExhausted(offset: instr.offset)
            }
            gas -= 1
            // Mirror the live gas budget into r15 (Appendix E §E.3; ADR-0021).
            regs[15] = .int(Int64(gas))
            let nextIP = ip + 1

            let reg = { (r: UInt8) -> FluxValue in regs[Int(r)] }

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
                    if b == 0 { throw VmError.divByZero(offset: instr.offset) }
                    r = wrappingDiv(a, b)
                case .modI64:
                    if b == 0 { throw VmError.divByZero(offset: instr.offset) }
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
                // Byte length of the resolved string (Appendix E §E.1). When the
                // string table is absent (no frame in scope), fall back to the
                // decimal digit count of the id so the value stays deterministic
                // and the golden conformance vectors still hold.
                let len: Int
                if let resolved = stringTable.lookup(id) {
                    len = resolved.utf8.count
                } else {
                    len = digitCount(id)
                }
                regs[Int(dst)] = .int(Int64(len))

            case .strConcat:
                let dst = instr.u8(0)
                let x = try requireStr(reg(instr.u8(1)), at: instr.offset)
                let y = try requireStr(reg(instr.u8(2)), at: instr.offset)
                guard let a = stringTable.lookup(x), let b = stringTable.lookup(y) else {
                    // Without a live string table we cannot concatenate concrete
                    // text; the closure is being evaluated outside a frame (e.g.
                    // a conformance vector), where this opcode is not exercised.
                    throw VmError.memoryExhausted(offset: instr.offset)
                    }
                    let combined = a + b
                    let newId = stringTable.intern(combined)
                    regs[Int(dst)] = .str(newId)

            case .toString:
                let dst = instr.u8(0)
                let src = reg(instr.u8(1))
                let rendered = renderForToString(src, table: stringTable)
                regs[Int(dst)] = .str(stringTable.intern(rendered))

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
                // Each field reserves two words of storage (a `UInt16` prop index
                // plus a `FluxValue` tagged union). Bounds the total against the
                // per-dispatch allocation budget (§NFR-SEC-003 / ADR-0015).
                allocated &+= UInt64(count) &* 16
                if allocated > allocationBudget {
                    throw VmError.memoryExhausted(offset: instr.offset)
                }
                var fields: [(UInt16, FluxValue)] = []
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
                // A freshly allocated list reserves one word for its header.
                allocated &+= 8
                if allocated > allocationBudget {
                    throw VmError.memoryExhausted(offset: instr.offset)
                }
                regs[Int(instr.u8(0))] = .list([])

            case .listPush:
                let list = instr.u8(0)
                let val = reg(instr.u8(1))
                // Pushing one element grows the backing storage by one word.
                allocated &+= 8
                if allocated > allocationBudget {
                    throw VmError.memoryExhausted(offset: instr.offset)
                }
                guard case var .list(items) = regs[Int(list)] else {
                    throw VmError.typeMismatch(offset: instr.offset)
                }
                items.append(val)
                regs[Int(list)] = .list(items)

            case .listGet:
                let dst = instr.u8(0)
                let items = try requireList(reg(instr.u8(1)), at: instr.offset)
                let i = Int(instr.u8(2))
                guard i < items.count else {
                    throw VmError.indexOutOfBounds(offset: instr.offset)
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
                guard let impl = capRegistry.lookup(capID, methodID) else {
                    // No registered implementation for this (capId, methodId):
                    // the loop below only raises on a *known but invalid* shape,
                    // so an unregistered capability is a type error at the call
                    // site (the MLP defines no such capability).
                    throw VmError.typeMismatch(offset: instr.offset)
                }
                do {
                    // The capability signature is `inout any SignalStore`, so box
                    // the concrete store only for the duration of the call (R3:
                    // the rest of the loop uses `S` directly, avoiding the
                    // existential on the hot path), then copy the writes back.
                    var boxed: any SignalStore = signals
                    let cellId = try impl(capID, methodID, reg(argsReg), &boxed)
                    signals = boxed as! S
                    // Unified sync/async contract (ADR-0045): the impl creates a
                    // result cell and returns its signal id; `resultReg` receives that
                    // id. A sync method has already written `Ready` into it; an async
                    // method has left it `Pending` for the host to resolve later.
                    regs[Int(resultReg)] = .int(Int64(cellId))
                } catch let err as VmError {
                    throw err
                } catch {
                    throw VmError.typeMismatch(offset: instr.offset)
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
                    throw VmError.gasExhausted(offset: instr.offset)
                }
            default:
                // v1 handlers never emit `AWAIT`; treat any unassigned opcode as
                // a malformed program (ADR-0044). v2 async runs through `execTailWith`.
                throw VmError.invalidDispatch(offset: instr.offset)
            }

            ip = nextIP
            #if DEBUG
            fluxDevtoolsEmit(.vmStep(bytecodeOffset: UInt32(instr.offset), opcode: instr.opcode.rawValue, registers: regs, gasRemaining: gas))
            #endif
        }

        return VmOutcome(
            signals: signals.snapshot(),
            registers: regs,
            gasUsed: entryGas - gas
        )
    }

    /// Runs `bytecode` with resumable semantics, returning either a final `VmOutcome`
    /// or a `RunResult.suspended` continuation at the first `AWAIT` (ADR-0044).
    ///
    /// This is the v2 entry point for async-capable handlers. The v1 `run` is a thin
    /// wrapper that asserts the handler never suspends; it delegates here and converts a
    /// `suspended` result into the v1 `invalidDispatch` fault so v1 callers see no change.
    ///
    /// The loop mirrors `run` exactly; the only divergence is the `await` case, which
    /// captures the continuation and returns instead of stepping. Keep the two in
    /// lockstep — any opcode added to `run` must also be added here.
    static func runResumable<S: SignalStore>(
        _ bytecode: [UInt8],
        signals: inout S,
        payload: FluxValue,
        stringTable: any StringResolver = EmptyStringTable(),
        capRegistry: CapabilityRegistry = .dev
    ) -> Result<RunResult, VmError> {
        let program: [Instruction]
        do { program = try Instruction.decode(bytecode) }
        catch let err as VmError { return .failure(err) }
        catch { return .failure(.invalidDispatch(offset: 0)) }
        return execTail(program, signals: &signals, startOffset: 0, payload: payload,
                        stringTable: stringTable, capRegistry: capRegistry, programBytes: bytecode)
    }

    /// Continues a suspended handler (ADR-0044), delivering `value` as the awaited result.
    ///
    /// Replays the captured signal writes, then re-enters the interpreter at
    /// `state.resumeOffset` with `value` placed in `r0`.
    static func resume<S: SignalStore>(
        _ state: SuspendState,
        signals: inout S,
        value: FluxValue,
        stringTable: any StringResolver = EmptyStringTable(),
        capRegistry: CapabilityRegistry = .dev
    ) -> Result<RunResult, VmError> {
        for (id, v) in state.signals { signals.write(id, v) }
        let program: [Instruction]
        do { program = try Instruction.decode(state.program) }
        catch let err as VmError { return .failure(err) }
        catch { return .failure(.invalidDispatch(offset: 0)) }
        var regs = state.registers
        regs[0] = value
        // `execTail` takes the register file by value; rebuild via a `runResumable`-style
        // re-entry would clobber the captured regs, so call the shared helper with them.
        return execTailWith(program, signals: &signals, startOffset: Int(state.resumeOffset),
                            registers: regs, gas: state.gasRemaining, payload: value,
                            stringTable: stringTable, capRegistry: capRegistry, programBytes: state.program)
    }

    /// Shared interpreter tail used by `run`, `runResumable` and `resume` (ADR-0044).
    ///
    /// Runs from `startOffset` until `HALT` or `AWAIT`. Mirrors the opcode semantics of
    /// `run` exactly; the `await` case returns `RunResult.suspended` with the offset of
    /// the following instruction. `programBytes` is retained in the suspend state so the
    /// executor can resume without retaining the program itself.
    private static func execTail<S: SignalStore>(
        _ program: [Instruction],
        signals: inout S,
        startOffset: Int,
        payload: FluxValue,
        stringTable: any StringResolver,
        capRegistry: CapabilityRegistry,
        programBytes: [UInt8]
    ) -> Result<RunResult, VmError> {
        var regs = [FluxValue](repeating: .null, count: 16)
        regs[0] = payload
        let gas = entryGas
        regs[15] = .int(Int64(gas))
        return execTailWith(program, signals: &signals, startOffset: startOffset,
                             registers: regs, gas: gas, payload: payload,
                             stringTable: stringTable, capRegistry: capRegistry, programBytes: programBytes)
    }

    /// Core interpreter tail: runs from `startOffset` with a caller-supplied live
    /// `registers`/`gas` (used by `resume` to restore the captured continuation).
    private static func execTailWith<S: SignalStore>(
        _ program: [Instruction],
        signals: inout S,
        startOffset: Int,
        registers: [FluxValue],
        gas: UInt32,
        payload: FluxValue,
        stringTable: any StringResolver,
        capRegistry: CapabilityRegistry,
        programBytes: [UInt8]
    ) -> Result<RunResult, VmError> {
        let offsets = program.map { $0.offset }
        guard let startIndex = offsets.firstIndex(of: startOffset) else {
            return .failure(.invalidDispatch(offset: startOffset))
        }
        var regs = registers
        var gas = gas
        var allocated: UInt64 = 0
        var stringTable = stringTable
        var ip = startIndex

        do {
        while ip < program.count {
            let instr = program[ip]
            let op = instr.opcode
            if op == .halt { break }
            if gas == 0 { return .failure(.gasExhausted(offset: instr.offset)) }
            gas -= 1
            regs[15] = .int(Int64(gas))
            let nextIP = ip + 1

            let reg = { (r: UInt8) -> FluxValue in regs[Int(r)] }

            switch op {
            case .halt:
                break

            case .await:
                // Unified sync/async bridge (ADR-0045): `futureReg` holds the register
                // containing the result-cell signal id returned by CALL_CAP. Park only
                // while the cell is `.pending`; a `.ready` cell continues with its value
                // in `r0` (one re-entry, no real park); a `.error` cell faults the handler.
                let cellId: UInt32
                switch regs[Int(instr.u8(1))] {
                case .int(let n): cellId = UInt32(truncatingIfNeeded: n)
                default: return .failure(.typeMismatch(offset: instr.offset))
                }
                switch signals.cellState(cellId) {
                case .ready:
                    regs[0] = signals.read(cellId) ?? .null
                case .pending:
                    let written = signals.snapshot()
                    return .success(.suspended(SuspendState(
                        program: programBytes,
                        resumeOffset: UInt32(instr.offset) + UInt32(instr.opcode.instructionLen),
                        registers: regs,
                        gasRemaining: gas,
                        signals: written,
                        futureReg: instr.u8(1)
                    )))
                case .error:
                    return .failure(.typeMismatch(offset: instr.offset))
                }

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
                    if b == 0 { return .failure(.divByZero(offset: instr.offset)) }
                    r = wrappingDiv(a, b)
                case .modI64:
                    if b == 0 { return .failure(.divByZero(offset: instr.offset)) }
                    r = wrappingRem(a, b)
                default:
                    return .failure(.invalidDispatch(offset: instr.offset))
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
                    return .failure(.invalidDispatch(offset: instr.offset))
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
                    return .failure(.invalidDispatch(offset: instr.offset))
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
                    return .failure(.invalidDispatch(offset: instr.offset))
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
                let len: Int
                if let resolved = stringTable.lookup(id) {
                    len = resolved.utf8.count
                } else {
                    len = digitCount(id)
                }
                regs[Int(dst)] = .int(Int64(len))

            case .strConcat:
                let dst = instr.u8(0)
                let x = try requireStr(reg(instr.u8(1)), at: instr.offset)
                let y = try requireStr(reg(instr.u8(2)), at: instr.offset)
                guard let a = stringTable.lookup(x), let b = stringTable.lookup(y) else {
                    return .failure(.memoryExhausted(offset: instr.offset))
                }
                let combined = a + b
                let newId = stringTable.intern(combined)
                regs[Int(dst)] = .str(newId)

            case .toString:
                let dst = instr.u8(0)
                let src = reg(instr.u8(1))
                let rendered = renderForToString(src, table: stringTable)
                regs[Int(dst)] = .str(stringTable.intern(rendered))

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
                allocated &+= UInt64(count) &* 16
                if allocated > allocationBudget { return .failure(.memoryExhausted(offset: instr.offset)) }
                var fields: [(UInt16, FluxValue)] = []
                fields.reserveCapacity(count)
                for i in 0..<count { fields.append((UInt16(i), .null)) }
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
                allocated &+= 8
                if allocated > allocationBudget { return .failure(.memoryExhausted(offset: instr.offset)) }
                regs[Int(instr.u8(0))] = .list([])

            case .listPush:
                let list = instr.u8(0)
                let val = reg(instr.u8(1))
                allocated &+= 8
                if allocated > allocationBudget { return .failure(.memoryExhausted(offset: instr.offset)) }
                guard case var .list(items) = regs[Int(list)] else {
                    return .failure(.typeMismatch(offset: instr.offset))
                }
                items.append(val)
                regs[Int(list)] = .list(items)

            case .listGet:
                let dst = instr.u8(0)
                let items = try requireList(reg(instr.u8(1)), at: instr.offset)
                let i = Int(instr.u8(2))
                guard i < items.count else { return .failure(.indexOutOfBounds(offset: instr.offset)) }
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
                guard let impl = capRegistry.lookup(capID, methodID) else {
                    return .failure(.typeMismatch(offset: instr.offset))
                }
                do {
                    var boxed: any SignalStore = signals
                    let cellId = try impl(capID, methodID, reg(argsReg), &boxed)
                    signals = boxed as! S
                    // Unified sync/async contract (ADR-0045): the impl returns the
                    // result-cell signal id; `resultReg` receives it.
                    regs[Int(resultReg)] = .int(Int64(cellId))
                } catch let err as VmError {
                    return .failure(err)
                } catch {
                    return .failure(.typeMismatch(offset: instr.offset))
                }

            case .matchTag:
                let val = reg(instr.u8(0))
                let tag = instr.u32(1)
                var matched = false
                if case let .record(fields) = val, let first = fields.first {
                    if case let .int(t) = first.value, t == Int64(tag) { matched = true }
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
                if gas < budget { return .failure(.gasExhausted(offset: instr.offset)) }
            }
            ip = nextIP
        }
        } catch let err as VmError {
            return .failure(err)
        } catch {
            return .failure(.invalidDispatch(offset: 0))
        }

        return .success(.halt(VmOutcome(
            signals: signals.snapshot(),
            registers: regs,
            gasUsed: entryGas - gas
        )))
    }

    /// The opcode → contiguous integer index used by `runViaDispatchTable`.
    /// Swift already lowers an `enum` `switch` to a jump table, so this index
    /// map is the explicit form of the same dispatch; it exists so the perf
    /// review's "LuaJIT-style closure table" hypothesis can be measured
    /// directly (see Perf #9). The canonical evaluator remains `run`.
    private static let opcodeIndex: [Opcode: Int] = Dictionary(
        uniqueKeysWithValues: Opcode.allCases.enumerated().map { ($0.element, $0.offset) }
    )

    /// Experimental dispatch-table evaluator (Perf #9). Behaviorally identical
    /// to `run`; dispatches through an `Int`-tagged `switch` keyed by
    /// `opcodeIndex` rather than the enum directly, so the cost of the two
    /// dispatch styles can be compared under a micro-benchmark. Retained only if
    /// measurement shows it is faster than the native enum `switch`; otherwise
    /// `run` stays canonical.
    static func runViaDispatchTable(
        _ bytecode: [UInt8],
        signals: inout SignalStore,
        payload: FluxValue,
        stringTable: any StringResolver = EmptyStringTable(),
        capRegistry: CapabilityRegistry = .dev
    ) throws -> VmOutcome {
        let program = try Instruction.decode(bytecode)
        let offsets = program.map { $0.offset }
        var regs = [FluxValue](repeating: .null, count: 16)
        regs[0] = payload
        var gas = entryGas
        regs[15] = .int(Int64(gas))
        var allocated: UInt64 = 0
        var stringTable = stringTable
        var ip = 0

        while ip < program.count {
            let instr = program[ip]
            let tag = opcodeIndex[instr.opcode]!
            if instr.opcode == .halt { break }
            if gas == 0 {
                throw VmError.gasExhausted(offset: instr.offset)
            }
            gas -= 1
            regs[15] = .int(Int64(gas))
            let nextIP = ip + 1
            let reg = { (r: UInt8) -> FluxValue in regs[Int(r)] }

            switch tag {
            case opcodeIndex[.halt]!: break
            case opcodeIndex[.nop]!: break
            case opcodeIndex[.readSignal]!:
                let dst = instr.u8(0); let id = instr.u32(1)
                regs[Int(dst)] = signals.read(id) ?? .null
            case opcodeIndex[.writeSignal]!:
                let id = instr.u32(0); let src = instr.u8(4)
                signals.write(id, reg(src))
            case opcodeIndex[.addI64]!, opcodeIndex[.subI64]!, opcodeIndex[.mulI64]!, opcodeIndex[.divI64]!, opcodeIndex[.modI64]!:
                let dst = instr.u8(0)
                let a = try requireInt(reg(instr.u8(1)), at: instr.offset)
                let b = try requireInt(reg(instr.u8(2)), at: instr.offset)
                let r: Int64
                switch instr.opcode {
                case .addI64: r = a &+ b
                case .subI64: r = a &- b
                case .mulI64: r = a &* b
                case .divI64:
                    if b == 0 { throw VmError.divByZero(offset: instr.offset) }
                    r = wrappingDiv(a, b)
                case .modI64:
                    if b == 0 { throw VmError.divByZero(offset: instr.offset) }
                    r = wrappingRem(a, b)
                default: fatalError("unreachable")
                }
                regs[Int(dst)] = .int(r)
            case opcodeIndex[.negI64]!:
                let dst = instr.u8(0)
                let v = try requireInt(reg(instr.u8(1)), at: instr.offset)
                regs[Int(dst)] = .int(0 &- v)
            case opcodeIndex[.eqI64]!, opcodeIndex[.ltI64]!, opcodeIndex[.gtI64]!, opcodeIndex[.lteI64]!, opcodeIndex[.gteI64]!:
                let dst = instr.u8(0)
                let a = try requireInt(reg(instr.u8(1)), at: instr.offset)
                let b = try requireInt(reg(instr.u8(2)), at: instr.offset)
                let r: Bool
                switch instr.opcode {
                case .eqI64: r = a == b
                case .ltI64: r = a < b
                case .gtI64: r = a > b
                case .lteI64: r = a <= b
                case .gteI64: r = a >= b
                default: fatalError("unreachable")
                }
                regs[Int(dst)] = .bool(r)
            case opcodeIndex[.addF64]!, opcodeIndex[.subF64]!, opcodeIndex[.mulF64]!, opcodeIndex[.divF64]!:
                let dst = instr.u8(0)
                let a = try requireFloat(reg(instr.u8(1)), at: instr.offset)
                let b = try requireFloat(reg(instr.u8(2)), at: instr.offset)
                let r: Double
                switch instr.opcode {
                case .addF64: r = a + b
                case .subF64: r = a - b
                case .mulF64: r = a * b
                case .divF64: r = fdiv(a, b)
                default: fatalError("unreachable")
                }
                regs[Int(dst)] = .float(r)
            case opcodeIndex[.negF64]!:
                let dst = instr.u8(0)
                let v = try requireFloat(reg(instr.u8(1)), at: instr.offset)
                regs[Int(dst)] = .float(-v)
            case opcodeIndex[.eqF64]!, opcodeIndex[.ltF64]!, opcodeIndex[.gtF64]!:
                let dst = instr.u8(0)
                let a = try requireFloat(reg(instr.u8(1)), at: instr.offset)
                let b = try requireFloat(reg(instr.u8(2)), at: instr.offset)
                let r: Bool
                switch instr.opcode {
                case .eqF64: r = (a == b) || (a.isNaN && b.isNaN)
                case .ltF64: r = a < b
                case .gtF64: r = a > b
                default: fatalError("unreachable")
                }
                regs[Int(dst)] = .bool(r)
            case opcodeIndex[.i64ToF64]!:
                let dst = instr.u8(0)
                let v = try requireInt(reg(instr.u8(1)), at: instr.offset)
                regs[Int(dst)] = .float(Double(v))
            case opcodeIndex[.f64ToI64]!:
                let dst = instr.u8(0)
                let v = try requireFloat(reg(instr.u8(1)), at: instr.offset)
                regs[Int(dst)] = .int(Int64(v))
            case opcodeIndex[.andBool]!:
                let dst = instr.u8(0)
                let x = try requireBool(reg(instr.u8(1)), at: instr.offset)
                let y = try requireBool(reg(instr.u8(2)), at: instr.offset)
                regs[Int(dst)] = .bool(x && y)
            case opcodeIndex[.orBool]!:
                let dst = instr.u8(0)
                let x = try requireBool(reg(instr.u8(1)), at: instr.offset)
                let y = try requireBool(reg(instr.u8(2)), at: instr.offset)
                regs[Int(dst)] = .bool(x || y)
            case opcodeIndex[.notBool]!:
                let dst = instr.u8(0)
                let v = try requireBool(reg(instr.u8(1)), at: instr.offset)
                regs[Int(dst)] = .bool(!v)
            case opcodeIndex[.strIntern]!:
                regs[Int(instr.u8(0))] = .str(instr.u32(1))
            case opcodeIndex[.strEq]!:
                let dst = instr.u8(0)
                let x = try requireStr(reg(instr.u8(1)), at: instr.offset)
                let y = try requireStr(reg(instr.u8(2)), at: instr.offset)
                regs[Int(dst)] = .bool(x == y)
            case opcodeIndex[.strLen]!:
                let dst = instr.u8(0)
                let id = try requireStr(reg(instr.u8(1)), at: instr.offset)
                let len: Int
                if let resolved = stringTable.lookup(id) { len = resolved.utf8.count } else { len = digitCount(id) }
                regs[Int(dst)] = .int(Int64(len))
            case opcodeIndex[.strConcat]!:
                let dst = instr.u8(0)
                let x = try requireStr(reg(instr.u8(1)), at: instr.offset)
                let y = try requireStr(reg(instr.u8(2)), at: instr.offset)
                guard let a = stringTable.lookup(x), let b = stringTable.lookup(y) else {
                    throw VmError.memoryExhausted(offset: instr.offset)
                }
                let combined = a + b
                let newId = stringTable.intern(combined)
                regs[Int(dst)] = .str(newId)
            case opcodeIndex[.toString]!:
                let dst = instr.u8(0)
                let src = reg(instr.u8(1))
                let rendered = renderForToString(reg(instr.u8(1)), table: stringTable)
                regs[Int(dst)] = .str(stringTable.intern(rendered))
            case opcodeIndex[.jump]!:
                ip = try jumpTarget(instr, nextIP: nextIP, offsets: offsets, delta: instr.i32(0))
                continue
            case opcodeIndex[.condJump]!, opcodeIndex[.condJumpNot]!:
                let taken = truthy(reg(instr.u8(0)))
                let want = instr.opcode == .condJump
                if taken == want {
                    ip = try jumpTarget(instr, nextIP: nextIP, offsets: offsets, delta: instr.i32(1))
                    continue
                }
            case opcodeIndex[.allocRecord]!:
                let dst = instr.u8(0)
                let count = Int(instr.u16(1))
                allocated &+= UInt64(count) &* 16
                if allocated > allocationBudget { throw VmError.memoryExhausted(offset: instr.offset) }
                var fields: [(UInt16, FluxValue)] = []
                fields.reserveCapacity(count)
                for i in 0..<count { fields.append((UInt16(i), .null)) }
                regs[Int(dst)] = .record(fields)
            case opcodeIndex[.getField]!:
                let dst = instr.u8(0)
                let idx = Int(instr.u16(1))
                let field = try getField(reg(instr.u8(3)), idx: idx, at: instr.offset)
                regs[Int(dst)] = field
            case opcodeIndex[.setField]!:
                let obj = instr.u8(0)
                let idx = Int(instr.u16(1))
                let val = reg(instr.u8(3))
                try setField(&regs[Int(obj)], idx: idx, value: val, at: instr.offset)
            case opcodeIndex[.recordEq]!:
                let dst = instr.u8(0)
                let x = try requireRecord(reg(instr.u8(1)), at: instr.offset)
                let y = try requireRecord(reg(instr.u8(2)), at: instr.offset)
                regs[Int(dst)] = .bool(recordsEqual(x, y))
            case opcodeIndex[.allocList]!:
                allocated &+= 8
                if allocated > allocationBudget { throw VmError.memoryExhausted(offset: instr.offset) }
                regs[Int(instr.u8(0))] = .list([])
            case opcodeIndex[.listPush]!:
                let list = instr.u8(0)
                let val = reg(instr.u8(1))
                allocated &+= 8
                if allocated > allocationBudget { throw VmError.memoryExhausted(offset: instr.offset) }
                guard case var .list(items) = regs[Int(list)] else { throw VmError.typeMismatch(offset: instr.offset) }
                items.append(val)
                regs[Int(list)] = .list(items)
            case opcodeIndex[.listGet]!:
                let dst = instr.u8(0)
                let items = try requireList(reg(instr.u8(1)), at: instr.offset)
                let i = Int(instr.u8(2))
                guard i < items.count else { throw VmError.indexOutOfBounds(offset: instr.offset) }
                regs[Int(dst)] = items[i]
            case opcodeIndex[.listLen]!:
                let items = try requireList(reg(instr.u8(1)), at: instr.offset)
                regs[Int(instr.u8(0))] = .int(Int64(items.count))
            case opcodeIndex[.listConcat]!:
                let dst = instr.u8(0)
                let a = try requireList(reg(instr.u8(1)), at: instr.offset)
                let b = try requireList(reg(instr.u8(2)), at: instr.offset)
                regs[Int(dst)] = .list(a + b)
            case opcodeIndex[.callCap]!:
                let resultReg = instr.u8(0)
                let capID = instr.u32(1)
                let methodID = instr.u16(5)
                let argsReg = instr.u8(7)
                guard let impl = capRegistry.lookup(capID, methodID) else {
                    throw VmError.typeMismatch(offset: instr.offset)
                }
                do {
                    let result = try impl(capID, methodID, reg(argsReg), &signals)
                    regs[Int(resultReg)] = .int(Int64(result))
                } catch let err as VmError { throw err } catch { throw VmError.typeMismatch(offset: instr.offset) }
            case opcodeIndex[.matchTag]!:
                let val = reg(instr.u8(0))
                let tag2 = instr.u32(1)
                var matched = false
                if case let .record(fields) = val, let first = fields.first {
                    if case let .int(t) = first.value, t == Int64(tag2) { matched = true }
                }
                if matched {
                    ip = try jumpTarget(instr, nextIP: nextIP, offsets: offsets, delta: instr.i32(5))
                    continue
                }
            case opcodeIndex[.extractField]!:
                let dst = instr.u8(0)
                let idx = Int(instr.u16(1))
                let field = try getField(reg(instr.u8(3)), idx: idx, at: instr.offset)
                regs[Int(dst)] = field
            case opcodeIndex[.loadIntConst]!:
                regs[Int(instr.u8(0))] = .int(instr.i64(1))
            case opcodeIndex[.loadFloatConst]!:
                regs[Int(instr.u8(0))] = .float(instr.f64(1))
            case opcodeIndex[.loadBoolConst]!:
                regs[Int(instr.u8(0))] = .bool(instr.u8(1) != 0)
            case opcodeIndex[.loadStrConst]!:
                regs[Int(instr.u8(0))] = .str(instr.u32(1))
            case opcodeIndex[.loadNull]!:
                regs[Int(instr.u8(0))] = .null
            case opcodeIndex[.mov]!:
                regs[Int(instr.u8(0))] = reg(instr.u8(1))
            case opcodeIndex[.gasCheck]!:
                let budget = instr.u32(0)
                if gas < budget { throw VmError.gasExhausted(offset: instr.offset) }
            default:
                fatalError("unknown opcode tag \(tag)")
            }

            ip = nextIP
            #if DEBUG
            fluxDevtoolsEmit(.vmStep(bytecodeOffset: UInt32(instr.offset), opcode: instr.opcode.rawValue, registers: regs, gasRemaining: gas))
            #endif
        }

        return VmOutcome(signals: signals.snapshot(), registers: regs, gasUsed: entryGas - gas)
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

    private static func truthy(_ v: FluxValue) -> Bool {
        switch v {
        case let .bool(b): b
        case let .int(i): i != 0
        default: false
        }
    }

    private static func requireInt(_ v: FluxValue, at offset: Int) throws -> Int64 {
        guard case let .int(i) = v else { throw VmError.typeMismatch(offset: offset) }
        return i
    }

    private static func requireFloat(_ v: FluxValue, at offset: Int) throws -> Double {
        guard case let .float(f) = v else { throw VmError.typeMismatch(offset: offset) }
        return f
    }

    private static func requireBool(_ v: FluxValue, at offset: Int) throws -> Bool {
        guard case let .bool(b) = v else { throw VmError.typeMismatch(offset: offset) }
        return b
    }

    private static func requireStr(_ v: FluxValue, at offset: Int) throws -> UInt32 {
        guard case let .str(id) = v else { throw VmError.typeMismatch(offset: offset) }
        return id
    }

    private static func requireList(_ v: FluxValue, at offset: Int) throws -> [FluxValue] {
        guard case let .list(items) = v else { throw VmError.typeMismatch(offset: offset) }
        return items
    }

    private static func requireRecord(_ v: FluxValue, at offset: Int) throws -> [(UInt16, FluxValue)] {
        guard case let .record(fields) = v else { throw VmError.typeMismatch(offset: offset) }
        return fields
    }

    /// Structural record equality: same field count, same prop indices in order,
    /// and equal values (recursively).
    private static func recordsEqual(
        _ a: [(UInt16, FluxValue)],
        _ b: [(UInt16, FluxValue)]
    ) -> Bool {
        guard a.count == b.count else { return false }
        for (lhs, rhs) in zip(a, b) {
            guard lhs.0 == rhs.0, lhs.1 == rhs.1 else { return false }
        }
        return true
    }

    private static func getField(_ obj: FluxValue, idx: Int, at offset: Int) throws -> FluxValue {
        if case .null = obj {
            throw VmError.nullDereference(offset: offset)
        }
        guard case let .record(fields) = obj else {
            throw VmError.typeMismatch(offset: offset)
        }
        guard idx < fields.count else {
            throw VmError.indexOutOfBounds(offset: offset)
        }
        return fields[idx].value
    }

    private static func setField(_ obj: inout FluxValue, idx: Int, value: FluxValue, at offset: Int) throws {
        if case .null = obj {
            throw VmError.nullDereference(offset: offset)
        }
        guard case var .record(fields) = obj else {
            throw VmError.typeMismatch(offset: offset)
        }
        guard idx < fields.count else {
            throw VmError.indexOutOfBounds(offset: offset)
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
            throw VmError.indexOutOfBounds(offset: instr.offset)
        }
        let base = Int64(offsets[nextIP])
        let target = base + Int64(delta)
        guard let t = UInt32(exactly: target),
              let index = offsets.firstIndex(of: Int(t)) else {
            throw VmError.indexOutOfBounds(offset: instr.offset)
        }
        return index
    }
}

/// Renders `value` as the text `TO_STRING` (0xD0, ADR-0043) produces.
///
/// The rendering is a cross-runtime contract: the Rust oracle, this VM and the
/// Kotlin VM must produce byte-identical text for the same value, because a
/// node's materialised props are compared against the release codegen output in
/// the parity suite. An integral `Float` keeps one fractional digit (`1.0`), and
/// a `Str` resolves through `table` (falling back to its id when the table has
/// no entry, which only happens outside a live frame).
func renderForToString(_ value: FluxValue, table: any StringResolver) -> String {
    switch value {
    case let .int(i):
        return String(i)
    case let .float(f):
        return f.isFinite && f == f.rounded() ? String(format: "%.1f", f) : String(f)
    case let .bool(b):
        return b ? "true" : "false"
    case let .str(id):
        return table.lookup(id) ?? String(id)
    case let .handlerRef(id):
        return "handler(\(id))"
    case let .list(items):
        return "[" + items.map { renderForToString($0, table: table) }.joined(separator: ", ") + "]"
    case let .record(fields):
        let rendered = fields.map { "\($0.propIndex): \(renderForToString($0.value, table: table))" }
        return "{" + rendered.joined(separator: ", ") + "}"
    case .null:
        return "null"
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
