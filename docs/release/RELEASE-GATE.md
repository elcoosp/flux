# Flux 1.0 Release Gate (FLUX-069 / PRD-U)

This document is the **machine-enforced** half of the 1.0 cut. The roadmap
(`docs/roadmaps/flux-roadmap-to-1.0.md` §1) defines 1.0 as *five evidence
criteria*, not a feature checklist, and PRD-U is the terminal gate that verifies
that evidence and freezes the contracts. FLUX-069 implements the parts of that
gate that **can** be enforced in-repo/CI; the human-evidence parts (dogfood
report, beta time-to-fix data, bug-bash log) are the `1.0-evidence.md` checklist
in this directory, filled by humans, gating the manual `v1.0.0` tag.

## What is already in place (component gates, not invented here)

Each release-relevant quality dimension already has a CI job (FLUX-011 / LANE-M
/ PRD-J / PRD-T lineage). The release gate **reuses** them rather than
duplicating their logic:

| Dimension | Existing workflow | Hard? |
|---|---|---|
| Rust fmt/clippy/test/doc | `rust-check.yml` | yes |
| iOS build+test + adapter SPM test | `ios-check.yml` | yes |
| Android gradle test + kotlinc codegen check | `android-check.yml` | best-effort* |
| Toolchain compat matrix (Xcode/AGP/Kotlin) | `compat-matrix.yml` | best-effort* |
| Render-perf budget (§3.10) on both hosts | `perf-harness.yml` | yes (real measure) |
| save→photon e2e (loopback LANE-H) | `benchmarks.yml` (save-photon) | yes (passed=false) |
| Wire robustness fuzz (attacker bytes) | `wire-fuzz.yml` | yes |
| Mutation testing (flux-differ/flux-vm-ref) | `mutation-testing.yml` | informational* |
| `flux build` invokes native toolchain (release gate) | `flux-cli/src/build.rs` (FLUX-068) | yes |

\* best-effort / informational by design (runner image limits, triage window) —
they surface drift but don't red the whole tree.

## What FLUX-069 adds (the new, blocking pieces)

1. **`.github/workflows/release-gate.yml`** — a `release-gate` job that fans out
   to all the hard component gates above (so a single "release-gate" status
   check represents the whole quality bar) **plus** a new hard
   `contract-freeze` job.
2. **`contract-freeze` job** runs
   `scripts/release-gate/check-contract-freeze.sh`, which enforces the PRD-U
   contract freeze: the wire protocol version (`PROTOCOL_VERSION`, Appendix D)
   and adapter contract version (§3.5) must be **consistent across the Rust
   server and both native hosts**, matching the frozen values in
   `docs/release/contract-versions.toml`. A mismatch is a build-breaking change
   and must be deliberate (ADR + coordinated three-site bump per ADR-0056).
3. **`docs/release/contract-versions.toml`** — the frozen contract manifest
   (single source of truth). Optional `pin = "<tag>"` freezes the gate to a
   1.0-RC / 1.0 tag.
4. **`docs/release/1.0-evidence.md`** — the five §1 criteria as a checklist with
   evidence pointers; the manual `v1.0.0` tag is cut only when all five are met
   with evidence.

## How to bump a contract version (breaking change procedure)

1. File an ADR describing the wire/adapter change (AGENTS.md §3.3 — no new
   opcodes/wire fields without an ADR).
2. Land the coordinated change at all three sites:
   - `crates/flux-ir-serde/src/frame.rs` (`PROTOCOL_VERSION`)
   - `runtimes/android/host/.../FrameDeserializer.kt` (`PROTOCOL_VERSION` +
     `SUPPORTED_VERSIONS`)
   - `runtimes/ios/FluxHost/.../FrameDeserializer.swift` (`protocolVersion`) +
     `HelloFrame.swift` emit
   - both kits' `adapterContractVersion` / `ADAPTER_CONTRACT_VERSION`
3. Bump `docs/release/contract-versions.toml` to match.
4. The `contract-freeze` job turns green again.

## Local verification

```bash
# contract freeze (must be PASS before any 1.0 tag)
bash scripts/release-gate/check-contract-freeze.sh

# pin to a release tag (optional), then run on that ref
bash scripts/release-gate/check-contract-freeze.sh --strict-manifest
```

## Reusable pattern (for future release gates in this repo)

This gate is the template for any "aggregate N existing component gates behind
one status check" CI task. Two gotchas that are not caught by `yaml` lint:

1. **A reusable callee must declare `on: workflow_call:`.** A workflow called
   via `jobs.<name>.uses: ./.github/workflows/<callee>.yml` fails the caller at
   *dispatch* ("workflow does not have a workflow_call trigger") unless the
   callee's `on:` block includes `workflow_call:`. Add it **additively** so the
   callee still runs standalone on push/PR — do not replace its other triggers.
2. **PyYAML coerces the `on:` key to boolean `True`.** When validating workflow
   YAML locally with `yaml.safe_load`, read triggers via `doc.get(True) or
   doc.get("on")` — `doc.get("on")` returns `None`. GitHub's own parser is fine.

Local validation recipe (run before committing a new umbrella workflow):

```python
import yaml
callees = ["rust-check.yml","ios-check.yml","android-check.yml","compat-matrix.yml",
           "perf-harness.yml","wire-fuzz.yml","benchmarks.yml"]
rg = yaml.safe_load(open(".github/workflows/release-gate.yml"))
on = rg.get(True) or rg.get("on")            # PyYAML coerces `on` key -> True
assert "push" in on and "pull_request" in on
for f in callees:
    d = yaml.safe_load(open(f".github/workflows/{f}"))
    o = d.get(True) or d.get("on")
    assert "workflow_call" in o, f"{f} missing workflow_call"
used = {j["uses"] for j in rg["jobs"].values() if "uses" in j}
for f in callees:
    assert f"./.github/workflows/{f}" in used
print("ALL CALLEES REFERENCED + workflow_call present: OK")
```

The contract-freeze script itself is the template for a frozen-constant gate: a
`docs/.../contract-versions.toml` manifest is the single source of truth, and a
bash script greps each implementation site for its literal and compares. Prove
it BOTH ways before committing — run green, then `sed -i` bump one site and
assert FAIL, restore, re-confirm green.
