# LANE-I — Error hierarchy & capability permission gating

**Dispatch:** Wave 1 (independent). Do NOT delegate; Louis runs this in his own session.
**Owned directories (exclusive):**
- `crates/flux-types/src/**` (error types; capability permission model)
- `crates/flux-devserver/src/**` (server-side error framing, asset path-traversal audit)
**Consumed (read-only):** `flux-syntax::Span`, `flux-ir` error types. No VM dispatch edits.

## Context (grounded)
VM faults surface as a red banner on iOS (Appendix E §E.6) — good. But the error taxonomy
is fragmented: `VmErrorKind` (vm-ref), `LoweringError`/`HandlerCompileError` (flux-ir),
`ParseError` (flux-parser), `TypeCheckError` (flux-types) each carry spans but there is no
unified "compile-time vs runtime vs capability" classification, and capability failures are
not permission-gated. AGENTS.md §3.11 wants what/where/why/how on every error. The asset
server (devserver) serves from project root; path-traversal is "already checked" per the
review but there is no test proving it.

## Tasks (TDD)
1. **Unified error doc/enum (flux-types):** add a `FluxError` umbrella classifying
   `Compile(Span, ...)` / `Runtime(Span, VmErrorKind)` / `Capability(capId, methodId, why)`,
   each with what/where/why/how accessors. Keep existing per-crate errors; add `From`
   conversions so the dev server can emit one shape. Do NOT break existing `LoweringError`
   call sites gratuitously — add the conversion, migrate incrementally.
2. **Capability permission gate:** add a `Permission` concept to `stdlib/capabilities.flux`
   (e.g. `Camera` requires `.camera`, `Storage` requires `.storage`). The host checks the
   OS grant BEFORE `CALL_CAP` resolves; a denied permission returns a `Capability` error
   (red banner), never a crash. iOS: `AVCaptureDevice.authorizationStatus` /
   `PHPhotoLibrary`; Android: `ContextCompat.checkSelfPermission`. Initially gate in the
   registry closure (your lane) reading a `PermissionChecker` injected alongside
   `CapabilityStore`.
3. **Asset server path-traversal test:** add a devserver test that requests
   `../../etc/passwd`-style paths and asserts a 403/404, proving the traversal guard.
4. **Error rendering test:** assert a type-mismatch error contains file:line:col + a hint.

## Acceptance gates (DoD)
- `cargo fmt --check` / `cargo clippy -D warnings` / `cargo nextest` / `cargo doc` clean.
- New tests: permission-denied → `Capability` error (not panic); traversal → rejected.
- Every NEW public error item documented with what/where/why/how.
- `git commit --only crates/flux-types/... crates/flux-devserver/...` — no `git add -A`.

## Pitfalls
- Do NOT change `flux-vm-ref/src/vm.rs` `VmErrorKind` discriminants that ISA vectors assert.
  Add a classification layer; keep the VM's error variants stable.
- `git commit --only` your files; the devserver has other agents' WIP in it — do not sweep.
