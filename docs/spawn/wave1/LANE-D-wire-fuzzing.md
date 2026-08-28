# LANE-D — Wire robustness & fuzzing (untrusted-frame hardening)

**Dispatch:** Wave 1 (independent). Do NOT delegate; Louis runs this in his own session.
**Owned directories (exclusive):**
- `crates/flux-ir-serde/src/**`
- `/.github/workflows/**` (add a `cargo fuzz` / fuzz CI job — CI lane; see contract R for
  `/scripts`/`/.github` ownership = `ci` agent; coordinate or fold into this file)
**Consumed (read-only):** `flux-syntax::Value`/`Opcode`/`Span`; `flux-ir` arena types.
No VM dispatch edits.

## Context (grounded)
The wire decoder (`Frame::from_*_bytes`, `HelloFrame::from_hello_bytes`) is hand-rolled
binary parsing. The dev server already rejects malformed *source* with an Error frame and
keeps the previous good tree, and the handshake rejects unknown frame types with a
version-mismatch Error frame. But: (a) there is NO fuzz target on the decoder, so
out-of-range jump/opcode offsets, truncated varints, or over-long frames are untested;
(b) the VM's per-op bounds are partially asserted (e.g. `expect_ints`) but a crafted
frame could still drive a `Jump` past the code blob. Gas (16 MiB cap) + `MemoryExhausted`
exist; extend with a hard frame-size ceiling.

## Tasks (TDD)
1. **Add `cargo fuzz` target** `fuzz/fuzz_targets/decode_frame.rs` exercising
   `Frame::from_init_bytes` / `from_delta_bytes` / `from_hello_bytes` with arbitrary bytes.
   Assert: NEVER panics — every input either returns `Ok` or a typed `Err`, never `unwrap()`
   inside the decoder on attacker bytes.
2. **Bytecode bound validation** (in `flux-ir-serde` decode, before the VM runs it):
   when decoding a handler closure blob, validate that every `Jump`/relative-offset target
   lies within `[0, len)` and every opcode's operand width is satisfied; otherwise return
   `Err(WireError::MalformedBytecode)` rather than letting the VM index OOB. The VM may
   keep its own guard, but the DECODER must not produce an out-of-range program.
3. **Frame-size ceiling:** reject an `Init`/`Delta` whose declared length exceeds a const
   `MAX_FRAME_BYTES` (e.g. 64 MiB) — defense in depth beyond the 16 MiB per-dispatch alloc cap.
4. **CI job:** a `cargo fuzz` build+run (linux) that crashes if the target panics; wired
   into `.github/workflows` (linux runner; native suites stay on mac/windows).

## Acceptance gates (DoD)
- `cargo fuzz build` + 60s `cargo fuzz run` finds no panic (libFuzzer on linux).
- New unit tests in `crates/flux-ir-serde/tests/` assert: truncated frame → `Err`,
  out-of-range jump target → `Err`, oversized frame → `Err`.
- `cargo fmt --check` / `cargo clippy -D warnings` / `cargo nextest` / `cargo doc` clean.
- `git commit --only crates/flux-ir-serde/...` (+ workflow file) — no `git add -A`.

## Pitfalls
- Do NOT weaken the `unreachable!()` arms in `flux-vm-ref/src/vm.rs` — they are inside
  already-filtered match arms (provably unreachable). Your bound checks belong in the
  *decoder*, not by deleting VM safety.
- Keep the existing `PROTOCOL_VERSION` Error-frame behavior intact (session.rs) — extend,
  don't replace.
