---
title: Troubleshooting
description: Diagnose Flux errors by class — compile, runtime (VM), and capability — with the message shape, root cause, and fix for each.
---

Every error Flux surfaces — in the dev server log, the on-device red banner
([FLUX-028](https://github.com/elcoosp/flux/tree/main/docs/adr)), or the CLI —
is classified by the unified [`FluxError`](https://github.com/elcoosp/flux/blob/main/crates/flux-types/src/error.rs)
taxonomy. This guide is keyed to that taxonomy so it cannot drift: the section
list is generated from the error sources and asserted in CI.

`FluxError` has three classes, each carrying a `what` / `where` / `why` / `how`
payload ([AGENTS.md §3.11](https://github.com/elcoosp/flux/blob/main/AGENTS.md)):

- **Compile** — a parse / type-check / lowering failure with a source span.
- **Runtime** — a VM fault during handler evaluation (classified by `VmErrorKind`).
- **Capability** — a `CALL_CAP` that was gated and denied by the host.

Below, each class and each VM fault variant gets a section with the typical
message shape, the root cause, and the fix.

## Compile errors

Typical shape:

```
[compile:Parse] expected `Int`, got `String` (hint: `count` was inferred as Int at line 18)
[compile:Type] ...
[compile:Lower] ...
```

Compile errors carry a `phase` (`Parse` / `Type` / `Lower`), a `message`, a
`span` (file:line:col), and an optional `hint`.

| Phase | What it means | Common cause | Fix |
|---|---|---|---|
| `Parse` | The lexer/parser rejected the source (indentation layout, keyword, or brace mismatch). | Mixing indentation and brace syntax, or a missing indent under a `compo`/view call. | Follow the indentation grammar ([ADR-0029](https://github.com/elcoosp/flux/tree/main/docs/adr)); run `flux dev` and read the caret. |
| `Type` | The type checker rejected an expression. | A signal used as the wrong type, or a handler whose body doesn't match its prop type. | Read the `hint` — it names the previously-inferred type and the location. |
| `Lower` | The typed AST could not be lowered to the reactive IR. | A construct not yet supported by the MLP lowering pass. | Simplify the expression; check `flux doc` for the supported surface. |

**General fix path:** open the file/line/col in the `span`, read the `hint`,
and re-express the construct. Compile errors are deterministic — re-running
`flux dev` reproduces them exactly.

## Runtime (VM) errors

Typical shape:

```
[runtime] gas budget exhausted at offset 42
[runtime] division by zero at offset 17
```

Runtime errors wrap a stable `VmErrorKind` ([flux-vm-ref/src/error.rs](https://github.com/elcoosp/flux/blob/main/crates/flux-vm-ref/src/error.rs))
so the fault code is load-bearing and asserted by the ISA vectors. Each variant:

### gas budget exhausted
- **Root cause:** a handler ran for more than 100,000 instructions (Appendix E §E.3).
- **Fix:** break the work into smaller handlers, or remove an accidental infinite loop / unbounded recursion.

### memory pool exhausted
- **Root cause:** the 16 MiB frame memory pool was exhausted by large literals or deep frames.
- **Fix:** reduce the size of data held in handler scope; page large lists through a capability instead of inlining.

### index out of bounds
- **Root cause:** a list / record / string index fell outside its bounds.
- **Fix:** guard the index (bounds-check before access); this is a logic bug in the handler.

### null dereference
- **Root cause:** a `GET_FIELD` was performed on `Null` ([ADR-0024](https://github.com/elcoosp/flux/tree/main/docs/adr)).
- **Fix:** check the value for `Null` before field access, or ensure the upstream signal is initialized.

### invalid opcode dispatch
- **Root cause:** the dispatch byte was not a valid opcode — usually a corrupt or stale patch frame.
- **Fix:** restart `flux dev`; if it recurs, it's a wire/lowering bug — file an issue with the frame bytes.

### type mismatch
- **Root cause:** operand types were not what the monomorphized opcode expected.
- **Fix:** ensure both operands share the expected type (`ADD_I64` needs two `Int`, `ADD_F64` two `Float`).

### division by zero
- **Root cause:** integer division or remainder by zero ([ADR-0023](https://github.com/elcoosp/flux/tree/main/docs/adr) — must fail, not panic).
- **Fix:** guard the divisor before the `/` or `%` operation.

**General fix path:** the `offset` points at the offending instruction; the
fault is deterministic and reproducible, so the handler bytecode around that
offset tells you which Flux expression lowered to it.

## Capability errors

Typical shape:

```
[capability] `Storage.removeItem` (2/3): the OS grant `.storage` was not authorized for this capability call
```

Capability errors ([CapabilityError](https://github.com/elcoosp/flux/blob/main/crates/flux-types/src/error.rs))
carry the numeric `cap_id` / `method_id`, the resolved `cap_name` / `method_name`
when registered, the required OS permission, and the `why`. They **never crash**
— they render as a red banner and the denied call simply doesn't run.

- **Root cause:** a `CALL_CAP` reached a capability that is gated behind an OS
  permission which the user has not granted (e.g. Camera, Location, Notifications).
- **Fix:** request and await the permission before calling the capability. The
  host surfaces a system prompt for the user to grant it; re-invoke the handler
  after the grant resolves.

The stable capability ids are documented in
[stdlib/capabilities.flux](https://github.com/elcoosp/flux/blob/main/stdlib/capabilities.flux)
(cap 1 = Camera, 2 = Storage, 3 = Router, 4 = Clipboard, 5 = Location, and up
through 11). Capability ids are derived deterministically — never hand-assigned
([AGENTS.md §3.4](https://github.com/elcoosp/flux/blob/main/AGENTS.md)).

## Still stuck?

- Read [Dev vs Release](/concepts/dev-vs-release/) to understand which tier you're in.
- Inspect [The Wire](/concepts/the-wire/) if frames look corrupt.
- For state that won't update, see the [State management](/guides/state-management/) guide.
