# Manifest Requests

Manifests (`Cargo.toml`, `Package.swift`, `build.gradle.kts`, `settings.gradle.kts`,
`runtimes/ios/project.yml`) are **frozen**: no agent edits them directly. Request a
dependency by appending a row to the table below. `scripts/manifest-steward.sh`
(run weekly by `.github/workflows/manifest-steward.yml`, and runnable locally with
`--dry-run`) applies each row to the real manifest, commits it, and truncates this
file back to the header.

## Directory-collision behaviour

All agents commit directly to `main`, so ownership is enforced after the fact by
`.github/workflows/merge-guard.yml`:

- Each push to `main` computes the set of top-level directories it touched.
- That set is compared against `.github/dir-locks.json`, which records the
  directories touched by the **previous** push.
- Any overlap fails the workflow with
  `merge-guard: directory X collided with the previous push`, naming every
  colliding directory and telling the agent to pull `main`, re-apply on top of
  the previous push, and confirm the other agent is done with that directory.
- On a clean push the guard rewrites `.github/dir-locks.json` with the current
  push's directories and commits it (`[skip ci]`), so the next push is checked
  against this one.

Because manifest edits arrive only through this file, two agents adding
dependencies never collide on a manifest: they collide on `MANIFEST_REQUESTS.md`
rows, which merge cleanly.

## Requests

| crate | dependency | version | reason |
| --- | --- | --- | --- |
| (workspace) | flux-lsp (new member crate) | — | FLUX-024: real `flux-lsp` language server on async-lsp, split out of the thin `flux-cli` JSON emitter (PRD-O deferred follow-up). Add `crates/flux-lsp` to workspace `members` + `flux-lsp = { path = "crates/flux-lsp" }` to `[workspace.dependencies]`. |
| flux-lsp | async-lsp | ^0.2 (latest 0.2.4) | FLUX-024: tower-based async LSP framework; MIT/Apache-2.0, ~1.3M downloads / 517K recent — passes AGENTS.md §1.3 vetting (active, >1000 stars). Drives the stdio server loop. |
| flux-lsp | lsp-types | ^0.97 (latest 0.97.0) | FLUX-024: typed LSP protocol structs (Diagnostic/Range/InitializeParams/CompletionItem) — 33M downloads, canonical LSP type crate. |
| flux-lsp | flux-parser / flux-types / flux-syntax / flux-ir | path | FLUX-024/025: the server reuses the compiler (PRD-O) — never re-implements analysis. |
| flux-cli | flux-types | path | FLUX-025: extend `flux lsp <file>` to run type-checking (PRD-O deferred "flux lsp type-checking (needs a flux-types dependency)"). Reuse existing workspace path dep. |
