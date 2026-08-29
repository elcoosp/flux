---
id: FLUX-024
status: done
lane: LANE-O
phase: "Phase 3"
blocked_by: []
labels:
  - epic
  - prd
  - dx
  - lsp
  - editor
source: CHANGELOG.md §PRD-O (deferred: "the VS Code extension (syntax highlight + LSP client + hot-reload status + 'run on device')")
related_adrs:
  - ADR-0029
  - ADR-0047
---

# FLUX-024: `flux-lsp` crate scaffold on async-lsp

- **Lane:** LANE-O (Phase 3)
- **Depends on:** PRD-L (grammar frozen), PRD-K (span-threaded errors), PRD-S (rustc-grade diagnostics)
- **Source:** `CHANGELOG.md` §PRD-O deferred follow-ups
- **Related ADRs:** ADR-0029 (frozen grammar), ADR-0047 (codegen registry)

## Problem Statement

PRD-O shipped a thin `flux lsp <file>` JSON emitter *inside* `flux-cli`
(`crates/flux-cli/src/lsp.rs`) and explicitly deferred the real LSP server to a
"documented follow-up." That follow-up is this cluster (FLUX-024..029). The
current CLI emitter is a one-shot JSON printer with no LSP transport, no
incremental document sync, and no capability negotiation — it cannot back a real
editor session. The roadmap (Phase 3) calls for a proper `flux-lsp` server
reusing the compiler so editors never disagree with the CLI on a diagnostic.

## Solution

Create a **new workspace crate `flux-lsp`** (`crates/flux-lsp`) that is the real
language server. It is the canonical home for every LSP feature (diagnostics,
go-to-definition, hover, completion, rename — FLUX-025..029). It is **built on
`async-lsp`** (crates.io `0.2.4`, tower-based, MIT/Apache-2.0, ~1.3M
downloads/517K recent — passes AGENTS.md §1.3 vetting) over `tokio`, with
`lsp-types` for the typed protocol structs (see FLUX-024 manifest request).

Design rules (from PRD-O + AGENTS.md):
- **Reuses the compiler.** `flux-lsp` depends on `flux-parser` / `flux-types` /
  `flux-ir`; it NEVER re-implements analysis. Diagnostics must match PRD-S's
  rustc-grade shape so the LSP and the CLI/DevTools never disagree.
- **Frozen grammar only.** Targets the ADR-0029 indentation grammar (PRD-L
  guarantees brace syntax is gone from CI).
- **No new wire fields.** Consumes PRD-K's span-bearing `FluxError` as-is.

## Implementation Decisions

- `Cargo.toml`: `edition = "2024"`, `#![forbid(unsafe_code)]`, same lint gates as
  other crates. Add the `flux-lsp = { path = "crates/flux-lsp" }` member to the
  workspace `Cargo.toml` **members** list (this is a manifest edit — file via
  `MANIFEST_REQUESTS.md` per AGENTS.md §1.3, or land through the orchestrator).
- Dependencies (requested in `MANIFEST_REQUESTS.md`): `async-lsp = "^0.2"`
  (latest `0.2.4`), `lsp-types = "^0.97"` (latest `0.97.0`) for the typed LSP
  protocol structs, `tokio` (workspace), `flux-parser`, `flux-types`,
  `flux-syntax`, `flux-ir` (path deps). If `async-lsp`'s `LanguageServer` trait
  carries its own request/response types, the backend converts between them and
  `lsp-types` (both track the LSP spec, so the mapping is mechanical).
- Transport: async-lsp `stdio` feature over tokio stdin/stdout; `LanguageServer`
  trait implemented as `FluxLspBackend`.
- The existing `flux-cli` `mod lsp` JSON emitter is **kept as a stable contract**:
  `flux-lsp` reuses the same `LspDiagnostic` shape (`line`/`character`/`length`/
  `severity`/`message`/`source`) so the VS Code extension (FLUX-026) and any
  non-LSP consumer keep working. `flux-lsp` is the superset.

## Testing Decisions

- Unit-test the backend's `diagnostics`-from-`ParseError`/`TypeError` mapping
  (span → `lsp_types::Diagnostic` with `Range`, severity, `code`), mirroring the
  existing `flux-cli/src/lsp.rs` `diagnostics_*` tests.
- Integration test: spawn the server over in-memory stdio (async-lsp
  `Router`/`Client`), send `initialize` → `didOpen` a `.flux` fixture with a
  parse error → assert `publishDiagnostics` carries the expected `Range`.
- `cargo nextest`, `cargo clippy -D warnings`, `cargo fmt --check`, `cargo doc`
  all green (no `unwrap`/`expect` in prod).

## Out of Scope

- The VS Code extension client (FLUX-026), go-to-def/hover/completion (FLUX-027),
  on-device overlay (FLUX-028), type-checking diagnostics (FLUX-025). This issue
  is the crate + async-lsp server loop + diagnostics wiring only.

## Further Notes

This is the root of the LSP follow-up cluster. Once landed, the remaining LSP
issues depend on `flux-lsp` existing, not on `flux-cli`.
