# ADR-0055: In-language `Result[T, E]` / error propagation

- **Status:** Accepted
- **Date:** 2026-08-29
- **Supersedes / related:** PRD-S (deferred in-language Result /
  error-propagation), FLUX-055, ADR-0044 (first-class async result cells),
  ADR-0045 (unified sync/async capability bridge), ADR-0053 (optional
  chaining, sibling surface)

## Context

Capabilities can fail (a denied grant surfaces a typed error, never a crash).
The *runtime* already settles `Ready` / `Pending` / `Error` result cells
(ADR-0044) and the unified capability bridge returns a cell signal id
(ADR-0045). What was missing is the **language surface** to read that error
without a crash and to thread fallible results through ordinary code.

FLUX-055 asked for an ADR + `flux-types` + lower support + a parity trace,
reusing ADR-0044's cell semantics. This ADR records the decided design and
splits it into a landed slice (the `Result[T, E]` type + `Ok`/`Err`
constructors + `match` on `Ok`/`Err`, built on the existing variant/match
machinery) and a follow-up (wiring a denied/failed capability's `Error` cell
to a `Result`, which depends on FLUX-049's typed error envelope and is out of
scope for this landing per the issue).

## Decision

Adopt a built-in algebraic `Result[T, E]` with two variants `Ok(T)` and
`Err(E)`:

```flux
let r: Result[User, AuthError] = Auth.currentUser()
match r {
  Ok(u) => Text(u.name)
  Err(e) => Text("denied: {e}")
}
```

1. **`Result` is a prelude ADT** (`crates/flux-types/src/prelude.rs`), generic
   over `[T, E]`, with variants `Ok(T)` and `Err(E)`. It reuses the existing
   `TcType::Variant` + `MATCH_TAG`/`EXTRACT_FIELD` machinery — no new opcode,
   no new IR node. This keeps `Result` first-class for both construction and
   `match`, exactly like user-declared ADTs.
2. **Construction preserves type arguments.** The handler-bytecode
   constructor path (`apply_callee` in `crates/flux-types/src/checker.rs`)
   previously returned `Named(adt, Vec::new())`, *dropping* the variant's
   payload types. For a generic `Result` that loses `T`/`E` and breaks `match`
   binding. This ADR lands the fix: `Ok(x)` / `Err(e)` unify the call
   arguments against the variant's declared field types (with the ADT's
   generic params `T`/`E` mapped to fresh inference variables), and the
   constructed type is `Variant("Result", [typeof(x), typeof(e)])`. This is a
   general improvement that also fixes any other generic ADT.
3. **`match` on `Ok`/`Err`** already works through `check_exhaustive` +
   `bind_pattern_ty` + the `MATCH_TAG`/`EXTRACT_FIELD` bytecode — the same
   path user ADTs use. `Result`'s two variants make `match` exhaustive; a
   trailing `_` is also accepted.
4. **Capability wiring (follow-up, FLUX-049-gated).** A capability call whose
   result is a `Result` lowers so that an `Error` cell becomes `Err(E)` and a
   `Ready` cell becomes `Ok(T)`. This reuses ADR-0044's cell states and is the
   only piece that depends on FLUX-049's typed error envelope; it is tracked as
   the FLUX-055 capability follow-up and does not block the language surface.
5. **No new VM opcode.** `Result` is an ordinary variant value; construction
   and `match` emit the existing `ALLOC_RECORD` (for the tag+field) and
   `MATCH_TAG`/`EXTRACT_FIELD` opcodes already present in Appendix E.

## Consequences

- Fallible capability results and user `Result`s share one typed,
  matchable vocabulary, and a denied grant becomes `Err(e)` instead of a red
  banner — the ergonomic the issue called for.
- The generic-ADT constructor fix (point 2) is a real compiler improvement
  that also benefits every other generic ADT in the language.
- Negative: `Result` is a built-in, not user-extensible to new variants; that
  is intentional for MLP (a closed error vocabulary per capability).
- Cross-host note: the Kotlin/Swift VMs already handle `MATCH_TAG`/
  `EXTRACT_FIELD` (variant matching), so `Result`/`match` works on-device
  without a new opcode — unlike FLUX-053's `IS_NULL`, which those VMs still
  need to mirror.

## Verification

- `crates/flux-types/tests/typecheck.rs` gains a case asserting
  `let r: Result[Int, String] = Ok(1); match r { Ok(n) => …, Err(_) => … }`
  type-checks and that `Err("x")` unifies `E = String`.
- The `apply_callee` generic-arg preservation is covered by a unit test that
  constructs a generic ADT variant and checks the produced type carries the
  concrete payload types.
- The capability-cell → `Result` wiring is a follow-up gated on FLUX-049 and is
  not asserted by this landing.
