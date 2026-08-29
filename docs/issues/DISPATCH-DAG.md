# Flux FLUX-0XX Dispatch DAG — parallel agents on `main`

Purpose: a dependency-aware wave plan so N parallel agents each own **disjoint
source files** and never conflict while all committing to `main`. PRD work is
already done; this dispatches the 46 follow-up issues (FLUX-024 … FLUX-070).

Read with `00-FOLLOWUP-INDEX.md`.

---

## 0. Three rules that make this safe on `main`

1. **Shared meta files are NOT an agent's slot.** Every issue wants to touch
   `CHANGELOG.md`, `AGENTS.md`, `MANIFEST_REQUESTS.md`, and `Cargo.toml`. Those
   are shared-index hazards (AGENTS.md §1.3, §4.2). They are owned by the
   **orchestrator**, completed in serial *merge passes* between waves — never
   counted as parallel work. Agents that need a manifest entry FILE it into
   `MANIFEST_REQUESTS.md` and stop; the orchestrator applies the manifest edit
   once per wave.

2. **Only three files are true hard conflicts** (same exact file, two
   independent issues). Everything else in the "collision" list only shares a
   *directory* and is resolvable by assigning each agent its own new files:

   | File | Conflicting issues | Resolution |
   |---|---|---|
   | `crates/flux-ir/src/lower/bytecode.rs` | FLUX-063, FLUX-070 | sequence: 063 → 070 |
   | `stdlib/capabilities.flux` | FLUX-045, FLUX-070 | sequence: 045 → 070 |
   | `crates/flux-types/src/capabilities.rs` | FLUX-045, FLUX-070 | sequence: 045 → 070 |

   (FLUX-070 is the async-lowering work that must sit *after* both 063's
   lowering fix and 045's capability-IDL change — so it naturally lands in a
   later wave.)

3. **Commit discipline per AGENTS.md §4.2.** Each agent commits with
   `git commit --only <its files> -m "…"` — never `git add -A`. Agents never
   touch shared meta files directly (rule 1).

---

## 1. Lane → directory ownership (the parallel backbone)

Each lane owns disjoint *directories*, so any two issues in different lanes can
always run in the same wave regardless of logical dependencies.

| Lane | Directory ownership | Issues |
|---|---|---|
| LANE-O | `crates/flux-lsp/`, `crates/flux-cli/src/lsp.rs`, `editors/vscode/` | 024–029 |
| LANE-R | `website/` (Astro + Starlight, locales en/es/fr — **already exists & committed**; LANE-R work ADDs to it, does not create a new site), `scripts/`(docs-only) | 030–036 |
| LANE-N | `adapters/ui-kotlin/`, `adapters/ui-swift/`, `crates/flux-codegen-swift/`, `crates/flux-codegen-kotlin/`, `stdlib/` (per-primitive new files) | 037–044, 052 |
| LANE-C/Q | `crates/flux-types/`, `crates/flux-vm-ref/`, `crates/flux-devserver/`, `runtimes/*/host/`, `stdlib/capabilities.flux` | 045–049, 047, 064, 070 |
| LANE-L | `crates/flux-parser/`, `crates/flux-ir/` (new lowering files), `stdlib/` (syntax) | 051, 053, 054, 055 |
| LANE-H | `benches/`, `docs/` (perf only) | 056, 057 |
| LANE-P | `crates/flux-devtools-ui/` | 058–062 |
| LANE-F | `crates/flux-ir/src/lower/`, `crates/flux-types/src/capabilities.rs`, `crates/flux-devserver/src/server/` | 063, 070 |
| LANE-J | `adapters/ui-swift/FluxUIKit/`, `runtimes/`, `platforms/`, `scripts/`(ci) | 065, 066 |
| LANE-M | `scripts/`(ci), `docs/`(adr) | 067 |
| LANE-G | `scripts/`(dist), `platforms/`, `docs/` | 068 |
| LANE-U | `apps/`, `docs/` | 069 |

**Within a lane, file-disjointness note:** LANE-N issues each create their *own*
adapter component files, codegen templates, and stdlib `.flux` module — assign
explicit per-primitive globs so no two agents touch the same file. The
dependency edges inside LANE-N (037→038/043/044 etc.) are *integration*
soft-deps, not file conflicts; relax them for dispatch and gate integration on
the lane lead's review.

---

## 2. Dependency edges (logical DAG)

```
FLUX-024 ──► FLUX-025 ──► FLUX-027
   ├──► FLUX-026
   └──► FLUX-029
FLUX-030 ──► FLUX-031, FLUX-032, FLUX-033, FLUX-036 ──► FLUX-069
FLUX-034 ──► (parity harness, external FLUX-023)
FLUX-035 ──► (host crash reporting)
FLUX-037 ──► FLUX-038, FLUX-043, FLUX-044
FLUX-039 ──► FLUX-043
FLUX-040/041/042/043/044 ──► FLUX-044 (a11y folds in last)
FLUX-065 ──► FLUX-042, FLUX-066        (iOS convergence decision first)
FLUX-038 ──► FLUX-052, FLUX-044
FLUX-045 ──► FLUX-046 ──► FLUX-048
FLUX-045 ──► FLUX-049 ──► FLUX-055
FLUX-064 ──► FLUX-047                  (AsyncResolver merged before HTTP cap)
FLUX-063 ──► FLUX-070                  (bytecode.rs fix before async lowering)
FLUX-045 ──► FLUX-070                  (capabilities.flux/rs before async IDL)
FLUX-056 ──► FLUX-057
FLUX-058/059/060/061 ──► FLUX-062 ──► FLUX-069
FLUX-036, FLUX-062, FLUX-068 ──► FLUX-069
FLUX-068 ──► FLUX-069
```

External (already-done) prerequisites, NOT dispatched here: PRD-L (grammar
frozen), PRD-K (span-threaded `FluxError`), PRD-Q (capability contract),
PRD-S (rustc-grade diagnostics), PRD-N (ScrollView template), ADR-0048
(convergence decision), ADR-0050 (protocol versioning), FLUX-023 (parity
harness). These are assumed landed; verify before dispatching dependent waves.

---

## 3. Waves (dependency-level; lanes further relax same-wave conflicts)

Wave 0 (no preds): 024, 030, 034, 035, **037**, 039, 040, 041, **045**, 051,
053, 054, 056, 058, 059, 060, 061, **063**, **064**, **065**, 067, 068
Wave 1: 025, 026, 029, 031, 032, 033, 036, 042, 043, 046, 047, 049, 057, 062, 066, **070**
Wave 2: 027, 038, 048, 055, 069
Wave 3: 044, 052

Because lanes own disjoint dirs, the *practical* parallel plan is the lane
grouping below, which collapses the above into fewer, bigger concurrent rounds
while honoring every hard edge and the 3 file conflicts.

---

## 4. Dispatch plan (actionable — what to launch, what each agent owns)

Round 1 — independent lane leads (all can launch simultaneously):
- **FLUX-024** (LANE-O): scaffold `crates/flux-lsp/`. Lane lead for 025/026/027/029.
- **FLUX-030** (LANE-R): docs website base + i18n-drift checker.
- **FLUX-034** (LANE-R): headless `.flux` test framework over parity.
- **FLUX-035** (LANE-R): release crash reporting (host).
- **FLUX-037** (LANE-N lead): Stack/Grid/Spacer/SafeArea — sets primitive file pattern.
- **FLUX-039** (LANE-N): Image caching.
- **FLUX-040** (LANE-N): form primitives.
- **FLUX-041** (LANE-N): gestures.
- **FLUX-045** (LANE-C/Q lead): six capabilities + `capabilities.flux`. Lane lead for 046/048/049/055/070.
- **FLUX-051** (LANE-L): list comprehension.
- **FLUX-053** (LANE-L): nullable/optional chaining.
- **FLUX-054** (LANE-L): structural vs nominal typing.
- **FLUX-056** (LANE-H): large-list bench.
- **FLUX-058** (LANE-P lead): signal-graph edges.
- **FLUX-059** (LANE-P): timeline/flamegraph.
- **FLUX-060** (LANE-P): network inspector.
- **FLUX-061** (LANE-P): multi-device.
- **FLUX-063** (LANE-F): `flux-ir` lowering gap fix (`bytecode.rs`).
- **FLUX-064** (LANE-C/Q): merge host `resume` call sites / async halves.
- **FLUX-065** (LANE-J lead): iOS convergence decision + ADR-0048 Phase 0/1.
- **FLUX-067** (LANE-M): mutation testing + compat matrix.
- **FLUX-068** (LANE-G): `flux build` toolchain invoke + distribution.

Round 2 — after Round-1 preds land (and the 3 file-conflict preds: 063, 045):
- **FLUX-025, 026, 029** (LANE-O): each adds a feature to `crates/flux-lsp/`
  in its own file (server pipeline / vscode ext / incremental). 027 waits for
  025's type pipeline.
- **FLUX-031, 032, 033** (LANE-R): depend on 030.
- **FLUX-036** (LANE-R): depends on 030 + 034 (parity).
- **FLUX-042** (LANE-N): animation — depends on 065 decision + 037.
- **FLUX-043** (LANE-N): theming — depends on 037 + 039.
- **FLUX-046** (LANE-C/Q): depends on 045.
- **FLUX-047** (LANE-C/Q): depends on 064 (AsyncResolver merged).
- **FLUX-049** (LANE-C/Q): depends on 045.
- **FLUX-057** (LANE-H): depends on 056.
- **FLUX-062** (LANE-P): depends on 058/059/060/061.
- **FLUX-066** (LANE-J): depends on 065.
- **FLUX-070** (LANE-F): depends on 063 (`bytecode.rs`) + 045
  (`capabilities.flux`/`capabilities.rs`) — the 3 conflict files are now settled.

Round 3 — tail of chains:
- **FLUX-027** (LANE-O): depends on 025.
- **FLUX-038** (LANE-N): depends on 037 + 042.
- **FLUX-048** (LANE-C/Q): depends on 046.
- **FLUX-055** (LANE-L): depends on 049 + 054.
- **FLUX-069** (LANE-U): depends on 036 + 062 + 068 (1.0 evidence gate).

Round 4 — cross-cutting fold-in:
- **FLUX-044** (LANE-N): a11y props — last, folds into every primitive
  (037–043). Runs after all LANE-N primitives exist; touches their adapter
  files (owned by those now-merged agents — so 044 is a *reviewed* follow-on,
  not parallel).
- **FLUX-052** (LANE-N): slot/children composition — depends on 038 (containers).

---

## 5. Orchestrator merge passes (serial, between rounds)

After each round's agents report green (their own `cargo nextest`/platform
suites), the orchestrator:
1. Applies all `MANIFEST_REQUESTS.md` entries filed that round in one commit.
2. Adds the per-issue `CHANGELOG.md` / `AGENTS.md` entries (one squashed
   commit, its own `--only` files) — never interleaved with agent source.
3. Re-runs `cargo check` + the workspace's `git diff --cached --name-only`
   guard before the next round launches.

This keeps the shared-index clean: agents never touch meta files; the
orchestrator touches them exactly once per round.

---

## 6. Conflict-resolution checklist for the dispatcher

- [ ] 070 is NOT in Round 1 (waits for 063 + 045 to free `bytecode.rs`,
      `capabilities.flux`, `capabilities.rs`).
- [ ] LANE-N agents (037,039,040,041,042,043,038) each get explicit per-primitive
      file globs in `adapters/ui-*`, `crates/flux-codegen-*`, `stdlib/` so no
      two write the same file. 044/052 run last as fold-ins.
- [ ] LANE-O 025/026/027/029 split `crates/flux-lsp/` by file (server.rs,
      typecheck.rs, goto.rs, incremental.rs) — no shared module.
- [ ] CHANGELOG/AGENTS/MANIFEST/Cargo.toml edits are orchestrator-only.
- [ ] Every agent commits `--only` its own files; no `git add -A`.
- [ ] External prereqs (PRD-L/K/Q/S/N, ADR-0048/0050, FLUX-023) verified landed
      before the dependent round launches.
