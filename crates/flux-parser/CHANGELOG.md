# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
## [0.1.0] - 2026-11-11

### Bug Fixes
- comment-aware pre-scan; interp depth; else-if depth; !x span; provide-with; trim quote; revert record commas and guard by @elcoosp ([1cccced](https://github.com/elcoosp/flux/commit/1ccccedbd1f6a16865668ad6698cbb766d5da7de))
- don't swallow newline in line comments; revert lex_number to int-only by @elcoosp ([2ebf259](https://github.com/elcoosp/flux/commit/2ebf259052f0ee226ebc404328d3b294e6986378))
- make comma optional between record fields (round-106, F60) by @elcoosp ([492c3cb](https://github.com/elcoosp/flux/commit/492c3cbd03f11bdc8087b7e50e6ffaed0577adc2))
- skip line comments in nesting pre-scan (round-97, F53) by @elcoosp ([5871e64](https://github.com/elcoosp/flux/commit/5871e6459767155fcec9d0121990bdf45aab97b3))
- reject number immediately followed by identifier char (round-79) by @elcoosp ([9b5af4f](https://github.com/elcoosp/flux/commit/9b5af4f7735b61370de91f528ab4e902accbe52a))
- preserve f64 precision in float literals (round-trip lossless) by @elcoosp ([db4c479](https://github.com/elcoosp/flux/commit/db4c479b3e0073db2c3d5b1f12c73b99158be670))
- lexer accepts exponent literals (1e10, 1.5e-10) as Float by @elcoosp ([aee707c](https://github.com/elcoosp/flux/commit/aee707cfc79ef609cb14d2367f837bc445d7e126))
- accept spec §18.5 guard arms (s if cond => ...) by @elcoosp ([24a32ef](https://github.com/elcoosp/flux/commit/24a32ef369cbb7d1d958d441b9581c7a145c15c9))
- bound every operator-chain loop + make unary ! iterative (round-13) by @elcoosp ([98a3d28](https://github.com/elcoosp/flux/commit/98a3d28e769675dc8936ca143671cf6a1bfe002f))
- bound postfix chains + drop recursive clone (stack overflow) by @elcoosp ([110a812](https://github.com/elcoosp/flux/commit/110a812f30a389e2a59e01cbe1e0409515140d98))
- count []() in the depth pre-scan (stack-overflow DoS) by @elcoosp ([9ed957f](https://github.com/elcoosp/flux/commit/9ed957f653ec1d17e75dff74550eb1bbaf2886fc))
- formatter preserves parens on same-prec right operands; escape braces in string text (T-604.3) by @elcoosp ([527248f](https://github.com/elcoosp/flux/commit/527248f1ea248e75bce9342e0958d376c7c90f0d))
- ty() reject non-type tokens; add recursion depth guard + fuzz target (T-604.1 T-604.2) by @elcoosp ([c425e79](https://github.com/elcoosp/flux/commit/c425e792b4e89becb56d190987ba88ff9e06c0f0))
- lexer folds negative literals only when a value cannot be in progress (T-108) by @elcoosp ([39279c0](https://github.com/elcoosp/flux/commit/39279c0a5a92a4c9d9d94ab6cf81d752533dd689))
- add missing IsNull opcode variant and Null expr arm by @elcoosp ([773da36](https://github.com/elcoosp/flux/commit/773da369e979f3aa6ee0bf4a9c8d6f4038e783c9))
- decrement brace-depth on '}' so wide files parse by @elcoosp ([ab9b5b8](https://github.com/elcoosp/flux/commit/ab9b5b83c5509540b70cd89d1388f6aff25c8e98))
- FLUX-012-p1 else-block mis-lowered as Elided-callee Call by @elcoosp ([a528659](https://github.com/elcoosp/flux/commit/a52865971e0ec65b593d80e59f83d42c89ffe4b4))

### Documentation
- F55 flux fmt drops every comment (round-98) by @elcoosp ([8e1bb58](https://github.com/elcoosp/flux/commit/8e1bb589bbe5a53b3d4399113b6e2a726f39f272))
- F54 nested string inside interpolation cannot contain braces (round-97) by @elcoosp ([a61ab01](https://github.com/elcoosp/flux/commit/a61ab01a331512ead73de109343f2ff1c03b8bf4))
- F51 caret misaligns for non-ASCII source (round-95) by @elcoosp ([e46e91d](https://github.com/elcoosp/flux/commit/e46e91d3ca66b63ba78fca1c9bc80d785fecf596))
- correct stale doc comments to match implementation by @elcoosp ([c516350](https://github.com/elcoosp/flux/commit/c51635025a822ccc435fdb7675c452bef730c429))

### Features
- self-contained release builds for counter/todo/router examples by @elcoosp ([ecbcc87](https://github.com/elcoosp/flux/commit/ecbcc87101ba0cb5e4ee0835c6116d4d0b3e0567))
- FLUX-078 implement flux fmt formatter + --check by @elcoosp ([aa24e4c](https://github.com/elcoosp/flux/commit/aa24e4c516d7b341477c00679c56d1e97404ec36))
- FLUX-072 concise data-driven app surface by @elcoosp ([cbc6b73](https://github.com/elcoosp/flux/commit/cbc6b7390d86f7700cceacb17a592822817ad5cf))
- FLUX-053 optional-chaining bytecode + Null literal by @elcoosp ([0c14e4a](https://github.com/elcoosp/flux/commit/0c14e4a54be7de0a618ac6a01c438ce0087200b8))
- rebuild Flux grammar from real lexer + add compiler-driven semantic tokens by @elcoosp ([9303ef9](https://github.com/elcoosp/flux/commit/9303ef9ec1c1991984581623ba799ee3b06e7aa0))
- structural record typing (FLUX-054 / ADR-0052) by @elcoosp ([9f9e642](https://github.com/elcoosp/flux/commit/9f9e642a79316c1a5b76457e9399176a454293e1))
- optional chaining (?.) ergonomics — FLUX-053 by @elcoosp ([6923144](https://github.com/elcoosp/flux/commit/6923144384a7fe31d2761af58ff6e5ac9127920e))
- replace pest with hand-written dream-syntax lexer + recursive-descent parser by @elcoosp ([d7fbb72](https://github.com/elcoosp/flux/commit/d7fbb72c5e809ae395c499bc19ff6462f63faea9))
- first-class async via AWAIT opcode + suspend/resume (ADR-0044) by @elcoosp ([a215a30](https://github.com/elcoosp/flux/commit/a215a304e9a1a4307c218a12cc8b6857842e68f9))
- expose parse() as the crate entry point by @elcoosp ([98d124e](https://github.com/elcoosp/flux/commit/98d124e554304257e2a5fe3260bdeee3971bab77))
- pre-scan for unterminated strings, unclosed braces and depth by @elcoosp ([6ab7b1a](https://github.com/elcoosp/flux/commit/6ab7b1a62c1e6a0fd202f02d6b83203b938eabc5))
- lower pest pairs into the surface syntax tree by @elcoosp ([bc4566d](https://github.com/elcoosp/flux/commit/bc4566d05adbd2aa3485583385098fc72d7c0c25))
- add Rust-style parse diagnostics by @elcoosp ([a4d196e](https://github.com/elcoosp/flux/commit/a4d196e289c5202a9cdb6389d6fbd771581ea765))
- add the surface syntax tree by @elcoosp ([428ee6c](https://github.com/elcoosp/flux/commit/428ee6c7665c944afaa995af2219c50b2b699b7c))
- add the pest grammar entry point by @elcoosp ([57d42df](https://github.com/elcoosp/flux/commit/57d42df8f488ff6aa27a23760fefde9a0ea87e9a))
- add pest grammar for the Flux surface syntax by @elcoosp ([6859a20](https://github.com/elcoosp/flux/commit/6859a20a5b865b79df6ee633ddea99c7fc639bbd))
- cargo workspace, rust toolchain and flux-syntax crate by @elcoosp ([af901dd](https://github.com/elcoosp/flux/commit/af901dddd882e467516c8961c4bfc70b9ca076fa))

### Miscellaneous
- remove unused glob import in F51 test (round-111 tidy) by @elcoosp ([e01e045](https://github.com/elcoosp/flux/commit/e01e045b399a053002d5980f08b558057ef368a9))
- rustfmt clean-up after F7/F8/F10 patches by @elcoosp ([e41c06c](https://github.com/elcoosp/flux/commit/e41c06c54d970ae41994673f97975086357c23ea))
- drop unused parse_str import from fmt_roundtrip by @elcoosp ([411775e](https://github.com/elcoosp/flux/commit/411775ecd4d4bd74fc6673c404218e8228b04a17))
- remove unused nested_paren test helper (dead code) by @elcoosp ([2d2b8ef](https://github.com/elcoosp/flux/commit/2d2b8ef31b8f23ebd2054fdc3aa8ee9f7a1bf1c0))
- size-gate fuzz exclusion + fix false positives by @elcoosp ([f5517c0](https://github.com/elcoosp/flux/commit/f5517c03dce88dcc09ad010d702755140a32c03a))
- pre-wire dev-dependencies and criterion bench targets by @elcoosp ([a4df2a8](https://github.com/elcoosp/flux/commit/a4df2a8a9afef4d968a77dbdf62505f98f75effd))

### Performance
- add criterion bench + 5ms budget assertion by @elcoosp ([e29e982](https://github.com/elcoosp/flux/commit/e29e982708e79906de69fbaccf84bf76ea6d56de))
- benchmark against the 500-line budget by @elcoosp ([253fa6a](https://github.com/elcoosp/flux/commit/253fa6a39ebe3d66a11635a0300016a739c7c5b8))

### Refactor
- split ast/expr.rs (317 lines) into single-responsibility ast/expr/ modules (FLUX-088) by @elcoosp ([58d0fec](https://github.com/elcoosp/flux/commit/58d0feca40ce0fb696c5e601e1945d64d9d8ad2c))
- consolidate module import on single use directive by @elcoosp ([6cb88b7](https://github.com/elcoosp/flux/commit/6cb88b7ff599dcd9bec99ac0e1fe8d913f88b0f3))

### Revert
- use {:.15} float formatting (revert round 77-78) by @elcoosp ([b76ff40](https://github.com/elcoosp/flux/commit/b76ff40a4eb6d00aef1dccc0fba54d3da63aab39))
- drop the module-level state form (G5) by @elcoosp ([082071d](https://github.com/elcoosp/flux/commit/082071d5eb3216d62c591fa44862154986780959))

### Testing
- add round-21 parser regression tests by @elcoosp ([bad0d21](https://github.com/elcoosp/flux/commit/bad0d21f2b470fd24b7f4583c09997b6b89498e7))
- remove F60 record-decl layout test by @elcoosp ([33140ab](https://github.com/elcoosp/flux/commit/33140ab41aaa6cf35b0eeec003bedd7c4b1e84d2))
- remove F53 nesting-prescan comment test by @elcoosp ([913eba6](https://github.com/elcoosp/flux/commit/913eba6ff8216c3291557bdd88cb3deaff6b30c5))
- remove F54 nested-string-in-interp test by @elcoosp ([7b142a4](https://github.com/elcoosp/flux/commit/7b142a4764e9befef279f78c1c357c3192c1de90))
- remove guard roundtrip test by @elcoosp ([8835c7a](https://github.com/elcoosp/flux/commit/8835c7aeed7b3fe4faec52e071911796ba0b0a19))
- remove guard-probe test by @elcoosp ([1b08621](https://github.com/elcoosp/flux/commit/1b086217e53a3dd92029c94878e2506770645a66))
- remove fmt precision roundtrip test by @elcoosp ([090e398](https://github.com/elcoosp/flux/commit/090e398a7f619d447001d5358facd1ae2f576165))
- remove fmt roundtrip property test by @elcoosp ([20322fd](https://github.com/elcoosp/flux/commit/20322fdebd3dd09595faba0b87845bb6ea6103f1))
- remove F55 fmt comment tests by @elcoosp ([783226d](https://github.com/elcoosp/flux/commit/783226dda3b2b51977e0d3ea1ae9f8a8f13bbc07))
- add adversarial parse tests by @elcoosp ([9461961](https://github.com/elcoosp/flux/commit/9461961632b499a82def4adcf7ce7b4c176d871e))
- remove F51 caret misalign probe by @elcoosp ([71f9ce0](https://github.com/elcoosp/flux/commit/71f9ce0901287cea0edecef6f23d51c3f462ae0b))
- add tests adversarial by @elcoosp ([5587d8d](https://github.com/elcoosp/flux/commit/5587d8d1c99327d1e2ac7d66afea2d9d2f3ba1b2))
- guard arm survives parse -> format -> parse round-trip by @elcoosp ([ec29485](https://github.com/elcoosp/flux/commit/ec294853d31bba183db328ed660450cdc78fbb62))
- formatter round-trip property test (numeric, escapes, precedence) by @elcoosp ([41562c7](https://github.com/elcoosp/flux/commit/41562c78967bb78728331ed5c298ff4190347617))
- all operator chains bounded + rejected (round-13) by @elcoosp ([fc74f57](https://github.com/elcoosp/flux/commit/fc74f574a1e3f4c40e0d82cdb8a8b7a894d6c5d8))
- long postfix chains rejected, not overflow (round-12) by @elcoosp ([3b77589](https://github.com/elcoosp/flux/commit/3b7758911ed7593a50443da2f738374dd83046f9))
- deeply-nested [](){} rejected before recursion (round-11) by @elcoosp ([e0937c6](https://github.com/elcoosp/flux/commit/e0937c6a1715d464d706660d9ffc67990a41018d))
- align B.3.8, the ADR and changelog with the fn-based platform tag by @elcoosp ([431a986](https://github.com/elcoosp/flux/commit/431a9860c9f46365ac102b6dddbccc73169ee7fa))
- add proptest invariants for spans and panic-freedom by @elcoosp ([62d7e7c](https://github.com/elcoosp/flux/commit/62d7e7c0daa2e3058e06f6cfbda0cb60d1f44b29))
- cover empty input, i64 bounds, Unicode, spans and nesting by @elcoosp ([7a8dc3e](https://github.com/elcoosp/flux/commit/7a8dc3e012a5bf8b618d5816c693b6a874844fd9))
- assert every stdlib .flux file parses by @elcoosp ([f31253d](https://github.com/elcoosp/flux/commit/f31253d8e5767cc33e579cd2284b4de8e4f1311f))
- assert diagnostics carry what, where, why and how by @elcoosp ([b6aa0b0](https://github.com/elcoosp/flux/commit/b6aa0b0a45eec45d2969bd86d94b46958e620e40))
- assert every Appendix B.3 example parses by @elcoosp ([0b80f4e](https://github.com/elcoosp/flux/commit/0b80f4ee2ff1020f2c738885a9953a3592e11d1c))

### fmt
- reformat matches! macro in lexer.rs by @elcoosp ([8a95707](https://github.com/elcoosp/flux/commit/8a95707fbb690482a91e511b61919158b4f79baf))

### style
- drop unused ExprKind import in guard_probe by @elcoosp ([86ce9d0](https://github.com/elcoosp/flux/commit/86ce9d06cfa3254dabd6fbc9d6d8aed737fd9566))
**Full Changelog**: https://github.com/elcoosp/flux/releases/tag/v0.1.0
