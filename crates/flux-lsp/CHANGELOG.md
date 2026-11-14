# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
## [0.1.0] - 2026-11-14

### Bug Fixes
- did_close bumps version to invalidate in-flight debounced publish (round-89, F45) by @elcoosp ([4960c86](https://github.com/elcoosp/flux/commit/4960c86508604032f295e891acacf1ec79f2e54c))
- track escapes correctly in comment_spans (round-86, F23) by @elcoosp ([63acb10](https://github.com/elcoosp/flux/commit/63acb10ee7b40bfdf6d80b1580c68d358d72d919))
- component/fn names resolve from anywhere in the file by @elcoosp ([5eb12fd](https://github.com/elcoosp/flux/commit/5eb12fdacd98355de5e293fd246b0cdf8db9104a))
- byte-based line_col_at + regression tests (audit §9) by @elcoosp ([f3d7566](https://github.com/elcoosp/flux/commit/f3d756651662e19b52455c59542f6d99dda5dcd6))
- declare position_encoding UTF8 (audit §9) by @elcoosp ([45de5de](https://github.com/elcoosp/flux/commit/45de5de746cbfc8f62e05d218e325bd4eeec5291))
- don't truncate document on unmappable incremental edit by @elcoosp ([bbad413](https://github.com/elcoosp/flux/commit/bbad4137f23b271575a3bd27d7c769d04c0411b1))
- did_open publishes diagnostics on open (real server, not just the test harness) by @elcoosp ([d5dd1d4](https://github.com/elcoosp/flux/commit/d5dd1d4caf730899f906772a71cd8c661a6558ba))
- drop unused DidCloseTextDocumentParams test import by @elcoosp ([7bc570f](https://github.com/elcoosp/flux/commit/7bc570f9c7f09bed1526d98b1a8ed31007fa0502))
- to_lsp_diagnostic_with_text uses UTF-8 byte deltas for range end by @elcoosp ([00a25cb](https://github.com/elcoosp/flux/commit/00a25cb2c267bf416fa9b91815e9f4264879bff9))
- implement did_close and import DidCloseTextDocumentParams by @elcoosp ([18676b3](https://github.com/elcoosp/flux/commit/18676b31d2c882d6372458773101e1af59736422))
- position_to_offset clamps to line end on over-long column by @elcoosp ([f7f2cb0](https://github.com/elcoosp/flux/commit/f7f2cb017e79386af5e6589046399229f3516f8e))
- propagate poisoned mutex as ServerInternalError instead of expect panic (T-604.7) by @elcoosp ([6ef4ae4](https://github.com/elcoosp/flux/commit/6ef4ae49a04674156dda541da175b7f89ef536a5))

### Documentation
- document the UTF-8 byte position contract by @elcoosp ([f53ffad](https://github.com/elcoosp/flux/commit/f53ffadcb11666041d9ec28247dc55a0f62c1aa2))

### Features
- add hover provider by @elcoosp ([f5131f1](https://github.com/elcoosp/flux/commit/f5131f1f48143f7295799437287766b7e6f2e4b6))
- add go-to-definition provider by @elcoosp ([7f4f8d8](https://github.com/elcoosp/flux/commit/7f4f8d896335792bc97360907f5d38c6c2f3bb35))
- add completion provider backed by ADR-0047 primitive registry by @elcoosp ([fa27665](https://github.com/elcoosp/flux/commit/fa276651c2f99715198fdeb8310c95453d5748f6))
- FLUX-029 incremental didChange with debounced re-analysis by @elcoosp ([a949d22](https://github.com/elcoosp/flux/commit/a949d226c8a754a87c1fac5b44159fcd5f262e24))
- add spawnable flux-lsp binary for the FLUX-026 VS Code extension by @elcoosp ([f66fe4b](https://github.com/elcoosp/flux/commit/f66fe4baf548f77f6e88e3e825d1fce9d4ca32cb))
- add type-check diagnostics to flux-lsp (FLUX-025) by @elcoosp ([78eca9a](https://github.com/elcoosp/flux/commit/78eca9a863dae0badafde4470c7194b4b5af159f))
- rebuild Flux grammar from real lexer + add compiler-driven semantic tokens by @elcoosp ([9303ef9](https://github.com/elcoosp/flux/commit/9303ef9ec1c1991984581623ba799ee3b06e7aa0))
- scaffold flux-lsp crate on async-lsp + wire workspace (FLUX-024) by @elcoosp ([892f86f](https://github.com/elcoosp/flux/commit/892f86f45074bd76f24817d007b9223c4baa10d9))

### Miscellaneous
- rustfmt clean-up after F7/F8/F10 patches by @elcoosp ([e41c06c](https://github.com/elcoosp/flux/commit/e41c06c54d970ae41994673f97975086357c23ea))
- fmt fixes (T-604.7) + regenerate wire test fixtures by @elcoosp ([8218a4b](https://github.com/elcoosp/flux/commit/8218a4b31db48614e32f6269056506f8747c003e))

### Refactor
- drop module_scope threading in goto_def index by @elcoosp ([44e8089](https://github.com/elcoosp/flux/commit/44e8089675c0cb92856f0f4e2626d4d91a69baba))
- split goto_def.rs (353 lines) into single-responsibility goto_def/ modules (FLUX-088) by @elcoosp ([9826f0a](https://github.com/elcoosp/flux/commit/9826f0a2ebd07d366d06e13e813bbd5af814b39b))
- consolidate module import on single use directive by @elcoosp ([6cb88b7](https://github.com/elcoosp/flux/commit/6cb88b7ff599dcd9bec99ac0e1fe8d913f88b0f3))

### Revert
- restore simple quote toggle (revert F23) by @elcoosp ([4ab8d39](https://github.com/elcoosp/flux/commit/4ab8d398e277741fed9f3fcb3965c3945e95b64d))
- drop did-close counter bump (revert F45) by @elcoosp ([85ece8f](https://github.com/elcoosp/flux/commit/85ece8f722a4c266b974619ffd0828876e68b5a0))

### Testing
- remove F45 did-close invalidate test by @elcoosp ([87e1ead](https://github.com/elcoosp/flux/commit/87e1ead1307ccb42aefa4e55071f35e4b7fa14c8))
- remove cross-component goto-def test by @elcoosp ([0155742](https://github.com/elcoosp/flux/commit/015574295f165654dafe03470a63ce9c303662c6))
- cross-component go-to-definition regression by @elcoosp ([4b550da](https://github.com/elcoosp/flux/commit/4b550dadb4f686d9a2e2c2285831cbdf1f762938))
- regression — unmappable incremental edit must not truncate by @elcoosp ([79f76fd](https://github.com/elcoosp/flux/commit/79f76fd2bc54f42c34e76ab9efc5fe824d5a9a69))
- bind did_open/did_change results to silence unused-return warnings by @elcoosp ([8738988](https://github.com/elcoosp/flux/commit/8738988cf036ee8c3a5063b33b1c15eb8102cfbb))
- add FLUX-024 loopback integration test for publishDiagnostics by @elcoosp ([fc81c82](https://github.com/elcoosp/flux/commit/fc81c825412ce8c9045414c783849c7f9d74c446))
**Full Changelog**: https://github.com/elcoosp/flux/releases/tag/v0.1.0
