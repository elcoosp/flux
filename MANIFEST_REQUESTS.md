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

> All pending requests applied. The `flux-lsp` member crate, the `async-lsp`
> (`^0.2`) and `lsp-types` (`^0.97`) workspace deps, and the `flux-cli` →
> `flux-types` path dep (FLUX-024 / FLUX-025) were wired directly into
> `Cargo.toml` (the steward script cannot create a new crate, only append to an
> existing one, and it emits `dep = "version"` literals rather than the
> `*.workspace = true` form the workspace convention requires). This file is
> left with no open rows; the next weekly steward run will simply report
> "no pending requests".
