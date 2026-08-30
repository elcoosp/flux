---
id: FLUX-089
status: todo
lane: LANE-CILINT
phase: "Phase 2"
blocked_by: []
labels:
  - ci
  - mutation-testing
  - rust
source: FLUX_PRODUCTION_READINESS_PLAN.md §2.7 (confirm mutation-testing.yml actually covers flux-differ and both VMs).
related_adrs: []
---

# FLUX-089: Ensure mutation testing covers `flux-differ` + both VMs

- **Lane:** LANE-CILINT (Phase 2 — CI hardening)
- **Owner:** CI / tooling
- **Source:** plan §2.7
- **Disjoint from:** every other issue.

## Problem Statement

FLUX-067 already landed `mutation-testing.yml` (`cargo-mutants` over `flux-differ` +
`flux-vm-ref`, informational first) and `compat-matrix.yml`. But the plan (§2.7)
explicitly calls out confirming the mutants job **actually covers `flux-differ` and
the two VMs** — and `flux-vm-ref` is only one of the three interpreters; the Kotlin
and Swift `FluxBytecodeVM` implementations are NOT covered by `cargo-mutants` (it's
a Rust tool). A mutation surviving in the Rust oracle is caught; a parity drift in a
host VM is not.

## Solution

- Confirm `mutation-testing.yml` names `flux-differ` and `flux-vm-ref` explicitly (it
  should already); if not, fix the matrix.
- Add a tri-platform **parity-mutation** gate: run the FLUX-086 ISA-conformance
  vectors against the Kotlin + Swift VMs under their own mutation/coverage tooling
  (Kotlin: `kotlinx`/JaCoCo + a flipped-comparison harness; Swift: a comparable
  approach) so a host-VM divergence is caught, not just a Rust-oracle mutation.
- Promote `mutation-testing.yml` from informational to a gate once the
  surviving-mutant set is triaged (per FLUX-067's stated policy).

## Implementation Decisions

- The Rust half reuses FLUX-067's `cargo-mutants` setup — no new Rust infra.
- The host-VM half is net-new but lives in the existing host check workflows.

## Testing Decisions

- The job lists `flux-differ` + `flux-vm-ref` (Rust) and both host VMs (host) as
  covered; a deliberately-injected host-VM parity bug fails the job.

## Out of Scope

- The ISA vectors themselves (FLUX-086).
- The line-count gate (FLUX-087).
