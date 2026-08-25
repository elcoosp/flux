# ADR-0032-devserver-build-accessor: devserver exposes `(LoweredIr, Ast)` per file for `flux build`

- **Status:** Accepted (created 2026-08-25 by the `ADR-0032-devserver-build-accessor` agent, FLUX-019b)
- **Related:** FLUX-019 (devserver), FLUX-022 (cli), FLUX-020 (codegen-swift, DONE),
  ADR-0034 (node-ID bridge), `ADR-0030-codegen-input-contract`, §3.1 / R2 of the
  boundary contract (frozen workspace manifest).

## Context

`flux build --platform ios|android` must drive the release codegen path
(`flux_codegen_swift::codegen(&LoweredIr, &Ast)` and its Kotlin twin). The codegen
inputs are produced by the parse → type-check → lower pipeline — the very pipeline
the **dev server already runs on every file save** (FLUX-019).

The frozen workspace manifest (boundary contract §3.1 / R2) forbids `flux-cli`
from depending on `flux-parser`, `flux-types`, or `flux-ir`. So `cli` cannot
re-run that pipeline itself; it cannot construct a `LoweredIr` or an `Ast`. The
agreed design is therefore: the dev server, which already owns the full pipeline,
exposes a boundary-safe accessor returning the per-file `(path, LoweredIr, Ast)`
bundles from its last good compile. `cli` calls that accessor and forwards the
bundles straight into codegen.

## Decision Drivers

- **Frozen manifest (R2 / §3.1):** `cli` may not take `flux-parser`/`flux-types`/
  `flux-ir` deps, so it cannot build IR or re-parse source. It must obtain the
  codegen inputs from a crate that already has them.
- **Dev server already has the pipeline:** re-running parse → type-check → lower
  in a second crate would duplicate the wire server's logic and risk drift between
  the dev and build paths. The dev server is the single owner of the pipeline.
- **Codegen needs both structure and semantics (see `ADR-0030-codegen-input-contract`):**
  `LoweredIr` carries only numeric `ComponentId`s and drops runtime values
  (string text, generics, `@pure`, prop/state types); the codegen contract is
  `codegen(&LoweredIr, &Ast)`, recovering names/semantics from the `Ast` via the
  ADR-0027 node-ID bridge. The accessor must therefore return `(LoweredIr, Ast)`,
  not generated strings.
- **Dev server must not depend on codegen crates:** returning codegen *output*
  (generated Swift/Kotlin text) would force `flux-devserver` to depend on
  `flux-codegen-swift` / `flux-codegen-kotlin`, violating the dependency direction
  (the codegen crates are downstream of the IR/parser, not upstream of the
  server). The server returns IR + AST; the caller (cli) performs codegen.

## Decision Outcome

Adopt `Pipeline::compiled_sources() -> Vec<(PathBuf, LoweredIr, Ast)>` as the
**single sanctioned path** for `flux build` to reach release-codegen inputs.

1. **Wire path unchanged.** The `Init`/`Delta` frames that ship over WebSocket
   still assemble from the merged single arena (`Pipeline::last_good`). The
   per-file bundle store (`last_sources`) is a *second*, additive view of the
   same compile — the merged arena is never regressed.
2. **Per-file retention.** `compile_tree` now collects
   `Vec<(PathBuf, LoweredIr, Ast)>` — one `(path, lowered, parsed-ast)` per
   tracked `.flux` file — in addition to merging arenas for the wire. The parsed
   `Ast` is retained (previously dropped after lowering) because codegen needs it.
3. **Accessor contract.** `compiled_sources()` returns owned clones (MLP trees are
   small) of `last_sources`, or `Vec::new()` before the first successful compile.
   `LoweredIr` and `flux_parser::Ast` are both `Clone`, so no lifetime escapes the
   borrow of `&self`.
4. **No new dependency.** The dev server does **not** depend on any codegen crate,
   and `cli` adds only a dependency on `flux-devserver` (already permitted) plus
   the codegen crates it targets. The manifest stays inside the R2 freeze.
5. **Naming source of truth.** Node identities in the returned `LoweredIr` and the
   matching nodes reachable from `Ast` are derived via `flux_syntax::compute_node_id`
   per ADR-0027, so a downstream agent can join `ir.arena` and `ast` on identical
   `NodeId`s exactly as `ADR-0030-codegen-input-contract` requires.

The `flux-cli` (FLUX-022) agent MUST:
- obtain codegen inputs exclusively via `DevServer`/`Pipeline::compiled_sources()`
  (or by constructing a `Pipeline`, `set_source`, `compile`, then reading
  `compiled_sources()`) — never by re-declaring `flux-parser`/`flux-types`/`flux-ir`;
- call `flux_codegen_swift::codegen(&ir, &ast)` (and the Kotlin twin) on each
  returned `(path, ir, ast)` bundle, preserving the `ADR-0030-codegen-input-contract`
  signature.

## Consequences

**Positive:**
- `cli` reaches codegen inputs without breaking the frozen manifest.
- The dev and build paths share one pipeline implementation — no drift, no
  duplicated parse/type-check/lower.
- The returned `(LoweredIr, Ast)` bundle is exactly what both codegen backends
  consume, preserving parity (FLUX-023).

**Negative:**
- The dev server now retains an extra `Vec<(PathBuf, LoweredIr, Ast)>` per
  compile. Negligible for MLP project sizes; acceptable per the cloning note in
  the accessor doc.

**Neutral:**
- No existing dev server behavior (Init/Delta/reconnect/error) is altered; the
  12 pre-existing integration tests still pass.

## References
- `ADR-0030-codegen-input-contract` ADR — settles `codegen(&LoweredIr, &Ast)`, not
  `&LoweredIr` alone.
- ADR-0027 (`docs/adr/ADR-0034-ir-node-id-bridge.md`) — single-source `compute_node_id`;
  the naming/join contract between `LoweredIr` and `Ast`.
- Boundary contract §3.1 / R2 — frozen workspace manifest forbidding `cli` deps on
  `flux-parser`/`flux-types`/`flux-ir`.
- `flux-devserver/src/pipeline.rs` — `Pipeline::compiled_sources`, `Pipeline::compile`,
  `Pipeline::compile_tree`, and the `last_sources` field.
