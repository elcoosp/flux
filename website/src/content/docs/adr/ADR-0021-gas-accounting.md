# ADR-0006: Gas accounting — HALT is exempt

**Status:** Accepted
**Date:** 2026-08-24
**Decision Drivers:** Determinism and cross-VM consistency of the golden ISA vectors (contract FLUX-002); resolution of an internal contradiction in Appendix E.

## Context and Problem Statement

Appendix E defines the gas meter as a VM safety mechanism: each instruction costs 1 gas, the handler dispatch budget is 100,000 instructions (§E.3), and exhaustion raises `GasExhausted` (§E.6). The text of §E.6 states the counter is "decremented per instruction," which on its face includes the `HALT` terminator.

However, the two concrete examples in the spec are inconsistent with that literal reading:

- §E.5's worked bytecode for `count = count + 1` is **five** instructions (`READ_SIGNAL`, `LOAD_INT_CONST`, `ADD_I64`, `WRITE_SIGNAL`, `HALT`) and states: *"Total: 21 bytes. At 1 gas per instruction = **4 gas**. Well within budget."* — i.e. 4 gas, not 5.
- The contract's FLUX-002 vector example (§1.5) lists a `READ`/`LOAD`/`ADD`/`WRITE` sequence with `"expected_gas_used": 4`, with no `HALT` present in that snippet.

If `HALT` is charged, §E.5 should report 5 gas and the vector example's asserted 4 would be wrong. Charging `HALT` also has no observable semantics: `HALT` terminates the handler before any further instruction can run, so spending the final gas unit on termination is pure waste and makes loop-bounded `GasExhausted` vectors harder to reason about.

## Considered Options

**Option A — Charge every decoded instruction, including HALT.**
- Pros: Matches the literal "per instruction" phrasing of §E.6.
- Cons: Contradicts §E.5 (would be 5, not 4) and the contract's canonical vector example (would be 5, not 4). Forces every handler to reserve an implicit +1 gas for its terminator.

**Option B — Exempt HALT only (terminator does not consume gas).**
- Pros: Matches both concrete examples (§E.5 = 4, contract example = 4). `HALT` never "executes" a subsequent instruction, so charging it is meaningless. Makes the gas budget equal to the number of *semantic* instructions, which is what the examples intend.
- Cons: Requires a one-line clarification to §E.6 ("per non-terminating instruction").

**Option C — Exempt both HALT and NOP.**
- Pros: Same as B, plus NOP is also "no-op."
- Cons: `NOP` still advances the IP and is a real instruction a compiler may emit; there is no example suggesting it is free. B is the minimal change that resolves the contradiction.

## Decision Outcome

**Chosen: Option B.** Gas is decremented once per *decoded* instruction, with `HALT` (0x00) exempt because it terminates the handler. `GAS_CHECK` (0xC0) is a real instruction and is charged 1 gas before its budget comparison. The handler entry budget in `r15` is 100,000 (§E.3).

Formally, for a handler of executed instructions `I` ending in `HALT`:

```
gas_used = |{ i in I : opcode(i) != HALT }|
GasExhausted raised iff remaining(r15) < 1 at the start of any non-HALT instruction.
```

## Consequences

**Positive:**
- Both concrete examples in Appendix E become self-consistent.
- The golden ISA vectors (FLUX-002) have an unambiguous gas rule to encode against.
- The Rust reference VM (`flux-vm-ref`, FLUX-005), the Swift VM (`FluxBytecodeVM`, FLUX-006), and the Kotlin VM (FLUX-007) all implement one identical rule.

**Negative:**
- §E.6 wording ("decremented per instruction") must be read with the HALT exemption. This ADR is the normative clarification; if Appendix E is edited, §E.6 should say "per non-terminating instruction."

**Neutral:**
- Loop-based `GasExhausted` vectors (e.g. `GAS_CHECK` inside a `JUMP` loop) compute their expected gas as the count of executed non-HALT instructions, which makes hand-authored expected values straightforward.

## References
- Appendix E §E.2 (registers), §E.3 (calling convention), §E.5 (example bytecode), §E.6 (error conditions).
- Contract FLUX-002 (golden ISA vectors) and D6 (vectors exist before any VM work).
- ADR-0002 (production VMs are native Swift/Kotlin; `flux-vm-ref` is the test oracle that must share this rule).
