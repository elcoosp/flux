# Flux — VS Code Extension

Editor support for the [Flux](https://github.com/elcoosp/flux) UI language: a
thin client over the `flux-lsp` language server (FLUX-024 / FLUX-026).

## Features

- **Syntax highlighting** — a TextMate grammar (`syntaxes/flux.tmLanguage.json`)
  *generated* from `flux-parser`'s keyword map (`crates/flux-parser/src/lexer.rs`)
  so it can never drift from the surface grammar the compiler accepts (ADR-0029).
  `npm run generate-grammar` regenerates it; `npm run check-grammar` asserts the
  round-trip in CI.
- **Diagnostics** — the LSP client drives `flux-lsp` over stdio for
  parse/type-check errors as you type (debounced re-analysis, FLUX-029).
- **Hot-reload status** — a status-bar item subscribes to the dev server's
  telemetry WebSocket (`:7333`, spec §4.1) and shows `compiling` / `reloaded`.
  Best-effort: it stays `idle` when no dev server is running.
- **Run on device** — the `Flux: Run on device` command launches
  `flux dev --ws-host 0.0.0.0` so a physical device on the LAN can attach to the
  app at this machine's IP on `:7331`.

## Requirements

Build the language server first (from the repo root):

```sh
cargo build -p flux-lsp
```

The extension launches the server via the `flux.lspServerPath` setting
(default `flux-lsp` on `PATH`). Point it at your local binary if needed, e.g.
`target/debug/flux-lsp`.

## Develop

```sh
npm install
npm run generate-grammar   # regenerate the grammar from the lexer
npm run check-grammar      # assert round-trip (CI)
npm run compile            # tsc -> dist/
npm run lint               # eslint
npx @vscode/vsce package   # produce flux-<ver>.vsix
```

Press <kbd>F5</kbd> in this folder to launch an Extension Development Host.

## Layout

| Path | Purpose |
| --- | --- |
| `package.json` | extension manifest (language, grammar, commands, settings) |
| `language-configuration.json` | bracket/comment/indent rules |
| `syntaxes/flux.tmLanguage.json` | **generated** TextMate grammar |
| `src/extension.ts` | activation: LSP client, status bar, Run-on-device |
| `src/generate-grammar.mjs` | grammar generator (lexer → tmLanguage) |
| `scripts/check-grammar.mjs` | CI round-trip assertion |
