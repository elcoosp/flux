# FLUX-F37 — `field_indices` map is built by the type checker but never read

**Status:** OPEN (fix landed in same commit)
**Found:** round-87 bug hunt (bytecode sweep)
**Severity:** low-medium (dead code with a *misleading* shape: it stores
positional record-field indices keyed by expression `NodeId`, but the
bytecode emitter deliberately keys records by FNV-1a of the field name
(`prop_index_for_name`). A future contributor who reaches for
`IrMetadata.field_indices` to emit `GET_FIELD` would emit the wrong
slot — every field read on a runtime-constructed record would silently
miss.)

## Summary

The type checker records a positional index for every `base.field`
expression:

* `crates/flux-types/src/checker/field.rs:56` (structural `Record` arm)
* `crates/flux-types/src/checker/field.rs:97` (nominal `Named` record arm)

The map is stored on `Checker.field_indices`, forwarded to
`TypedAST.field_indices`, and passed into `IrMetadata.field_indices` at
`crates/flux-ir/src/lower/mod.rs:362` and `:1010`.

It is never read. Every record-field access in `bytecode.rs` computes
the slot with `intern_prop_index(&field.name)` → `prop_index_for_name`
(FNV-1a of the field name), with an explicit comment at
`bytecode.rs:1442`:

    // not use the type checker's positional `field_indices` here.

The choice is deliberate and correct — records are wire-keyed by
`PropIdx`, not by position — but it means the checker-side work that
builds `field_indices` is dead, and the `IrMetadata.field_indices`
field is a trap: it *looks* like the authoritative source for
`GET_FIELD`/`SET_FIELD` slot selection, and reading it would produce
bytecode the VM's `set_field`/`get_field` cannot find.

## Fix

Remove the dead plumbing:

* `IrMetadata.field_indices` field (its only field-reading site was none).
* The two `IrMetadata { field_indices: &self.typed.field_indices, … }`
  constructor calls in `lower/mod.rs`.
* `TypedAST.field_indices` and its forwarding from `Checker.field_indices`.
* The two `self.field_indices.insert(...)` sites in
  `checker/field.rs` (and the associated `u16::try_from(pos)` narrowing
  code, which existed only to fill the field).
* `Checker.field_indices` field itself.

Regression coverage: the existing `flux-parity` / `flux-differ` /
`flux-types` suites still pass, and a record-field read no longer pays
a `u16::try_from(pos)` narrowing on every `base.field` expression.
