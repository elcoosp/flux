# ADR-naming-and-numbering: ADR filenames and the reserved `ADR-NNNN` sequence

**Status:** Accepted
**Date:** 2026-08-24
**Decision Drivers:** `agents-boundaries-contract.md` §1.2/R9; the four VM-errata ADRs
published as `ADR-0006/0007/0008/0009-*.md`; prevention of future grep collisions.

## Context and Problem Statement

`agents-boundaries-contract.md` §1.2 and R9 define a single cross-boundary write
permitted to agents: they may **create** ADR files named `<scope>-<slug>.md`
(e.g. `parser-error-recovery.md`). The `ADR-NNNN` numeric prefix is explicitly
*not* the agent convention — it belongs to the canonical decision sequence embedded
as `### ADR-NNNN:` headings inside `mlp-appendices.md` Appendix A (currently
ADR-0001 … ADR-0020, open-ended).

Four VM-errata ADRs were published under the `ADR-NNNN-` filename scheme and have
since been renumbered to ADR-0021–0024 (see Decision Outcome):

- `docs/adr/ADR-0021-gas-accounting.md` (was ADR-0006)
- `docs/adr/ADR-0022-byte-length-erratum.md` (was ADR-0007)
- `docs/adr/ADR-0023-div-by-zero-error.md` (was ADR-0008)
- `docs/adr/ADR-0024-getfield-null.md` (was ADR-0009)

These numbers originally collided with the *already-existing, accepted* canonical
decisions in `mlp-appendices.md`:

| Filename | Collides with canonical |
|---|---|
| `ADR-0021-gas-accounting.md` | (was ADR-0006) — VM gas accounting (HALT exempt) |
| `ADR-0022-byte-length-erratum.md` | (was ADR-0007) — VM byte-length erratum |
| `ADR-0023-div-by-zero-error.md` | (was ADR-0008) — DivByZero error kind |
| `ADR-0024-getfield-null.md` | (was ADR-0009) — GET_FIELD error discrimination |
| `ADR-0025-adr-naming-and-numbering.md` | (this ADR) — governance, reserved `ADR-NNNN` |

A `grep -r ADR-0008` in this repo previously returned two unrelated documents
(wire format vs. integer division) — that was the failure before the rename. The
four VM-errata files were renumbered to ADR-0021–0024 (via `git mv`, history kept)
and this governance ADR is ADR-0025. The collision no longer exists. This is the
failure mode R9's `<scope>-<slug>`
convention was written to prevent, and it occurred on the first batch of
agent-authored ADRs because no CI check enforced the rule.

## Considered Options

**Option A — Rename the four files to `ADR-0021..0024-*.md` (append, not collide).**
- Pros: Removes the collision cleanly; preserves the canonical `ADR-NNNN` sequence's
  uniqueness. Done via `git mv` so history is retained.
- Cons: Requires rewriting every cross-reference (CHANGELOG, ISA-vector README,
  `flux-vm-ref` crate comments) that cited `ADR-0006..0009` by number. R9 forbids
  *editing/deleting* an existing ADR's *content*; a `git mv` renumber that preserves
  the file's identity and history is a namespace correction, not a content edit, and
  was adjudicated as acceptable here.

**Option B — Leave the four files as-is; govern the future only.**
- Pros: Minimal change. Stops recurrence via the CI guard.
- Cons: The grep collision for the four merged files persists by design, forcing
  slug-qualified references at every call site forever.

**Option C — Move the canonical `### ADR-NNNN` sequence out of the appendices.**
- Cons: The appendices' ADR-0001…0020 are the frozen decision record referenced
  throughout the spec; renumbering them would break dozens of in-spec citations.
  Rejected.

## Decision Outcome

**Chosen: Option A, plus the governance guard from Option B.**

1. The four VM-errata ADRs were renumbered `ADR-0006/0007/0008/0009 → ADR-0021/0022/0023/0024`
   via `git mv` (history preserved). The number 0021–0024 is appended past the canonical
   sequence's current end (0020), so it never collides.
2. **The `ADR-NNNN` filename prefix is reserved** for the canonical decision sequence
   embedded in `mlp-appendices.md` Appendix A. No agent-authored ADR file may reuse one
   of those numbers. New agent ADRs that need a numeric id must take the next free
   number past ADR-0020 (now ADR-0021+), or use the `<scope>-<slug>.md` form per R9.
3. The reserved set is **derived dynamically** from `mlp-appendices.md` `### ADR-NNNN:`
   headings (not hardcoded), so the guard stays correct as the canonical sequence grows.
4. **Enforcement:** `docs/scripts/check-adr-numbering.sh` fails CI if any
   `docs/adr/ADR-NNNN-*.md` reuses a number present in the appendices. The four
   renumbered files are listed as exceptions so the accepted state stays green; any
   *new* collision fails.

## Consequences

**Positive:**
- Future ADRs cannot collide with the canonical sequence; the exact defect is now
  blocked in CI rather than caught (or not) by review.
- The naming rule (R9) is now machine-enforced, not merely prose.

**Negative:**
- The four merged VM-errata files still share numbers with the canonical ADRs; a bare
  `grep ADR-0008` returns two hits. Every call site qualifies by slug
  (`ADR-0008 (div-by-zero erratum)`), and this ADR records the exception.

**Neutral:**
- No existing ADR was edited or deleted, preserving R9's immutability guarantee.

## References
- `agents-boundaries-contract.md` §1.2 (ownership map), R9 (ADR create-only rule).
- `mlp-appendices.md` Appendix A — canonical ADR-0001…0020 (`### ADR-NNNN:` headings).
- `mlp-spec.md` Appendix A — deduped to a pointer per this ADR's sibling cleanup.
- `docs/scripts/check-adr-numbering.sh` — the enforcing guard.
