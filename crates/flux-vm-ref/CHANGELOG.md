# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
## [0.1.0] - 2026-11-14

### Bug Fixes
- canonical allocation cost model 8B/slot; Rust counter; F16 closed by @elcoosp ([5e794be](https://github.com/elcoosp/flux/commit/5e794be24089ce9c7d15d981aaf3ab2e8a7e3660))
- LIST_INSERT/REMOVE read index from a register (F1) by @elcoosp ([b6fb2fb](https://github.com/elcoosp/flux/commit/b6fb2fb5e4f961ed66ce6fafff1c411403050fe9))
- signal snapshot reports only written cells (not pre-seeded seeds) by @elcoosp ([5cae0be](https://github.com/elcoosp/flux/commit/5cae0beb6dfff79caa44024000b7433414a0fc85))
- wrapping NEG_I64 + bounded AWAIT cell + strict resume start offset by @elcoosp ([e720639](https://github.com/elcoosp/flux/commit/e7206390428635b0f174d03cc5b8bdb2e92cb97b))
- StrConcat overflow — resolve via StringTable to synthetic_str_id (FLUX-005) by @elcoosp ([5c9f279](https://github.com/elcoosp/flux/commit/5c9f2796f9e30aaa9ced76b09a30015a105c54d0))
- fdiv handles -0.0 sign parity (Swift + Rust oracle) + ISA vector by @elcoosp ([e511edf](https://github.com/elcoosp/flux/commit/e511edfff1aa51bada641a101d30d70695573f16))
- VM oracle correctness — IEEE NaN equality, truthy Int error, StrConcat overflow, AWAIT result_reg by @elcoosp ([11e7e0d](https://github.com/elcoosp/flux/commit/11e7e0d0cb94295fdd0941caa72ab225bad9817b))
- skip non-conformance vector files (foreach_ids.json) in load_vectors by @elcoosp ([f70de4a](https://github.com/elcoosp/flux/commit/f70de4ad49066852f6e45ffe7c671ea07c1c5e50))
- add missing StringTable import to doctest by @elcoosp ([8868354](https://github.com/elcoosp/flux/commit/8868354357168d3b35d16787767f7e29e29b343e))
- expose CapabilityRegistry injection for conformance fakes (T-335.2) by @elcoosp ([6799708](https://github.com/elcoosp/flux/commit/67997085608d161580ee46c344e4d09f6f8f60ba))
- thread &StringTable through VM entry points and fix StrLen (T-335.1) by @elcoosp ([46df0b1](https://github.com/elcoosp/flux/commit/46df0b1a2329b2b80fa7ea5155105565c2a54468))
- thread capability registry through Vm API (T-335.2) by @elcoosp ([f20b93f](https://github.com/elcoosp/flux/commit/f20b93f4a0829a29a956683c16971090a0613883))
- STR_LEN panic fix + render_await backend hook by @elcoosp ([517871e](https://github.com/elcoosp/flux/commit/517871e79a152f146779dc906d426d0caaede3f9))
- STR_LEN returns real length + StorageBackend NaN/corrupt handling by @elcoosp ([89f571e](https://github.com/elcoosp/flux/commit/89f571e51fec09ac0e11d21b9024a59d0d4791fe))
- list-op operand widths aligned between compiler, oracle, Swift (T-303/T-304) by @elcoosp ([6ca0395](https://github.com/elcoosp/flux/commit/6ca0395f60655fb8a3691d271ab1b89d6f6eb1cd))
- canonical record field indices for runtime construction by @elcoosp ([f3ccdc3](https://github.com/elcoosp/flux/commit/f3ccdc329cd6d089c19ee3b3b358376cc4bc144e))

### Documentation
- point documentation field to Vercel site by @elcoosp ([6a49f5d](https://github.com/elcoosp/flux/commit/6a49f5db7cf7eb3925c36901ef9dd752bcb20429))
- add Vercel documentation link to Cargo.toml by @elcoosp ([d0c18de](https://github.com/elcoosp/flux/commit/d0c18de8f51caf6f1a9feb08dca2d06ae7088a63))
- enforce SKILL.md rules (jsDelivr logos, absolute links, remove license sections) by @elcoosp ([c4ff690](https://github.com/elcoosp/flux/commit/c4ff690b590b225068b8703e587c63da7c74cd56))
- use style=for-the-badge for all badges by @elcoosp ([5dadc45](https://github.com/elcoosp/flux/commit/5dadc45f8b61c02251855933b46a4c80730d8289))
- sync badges to flat style + refresh stale facts (38 ADRs, 633 commits, 51k LOC, 28 stdlib, 12 CI, flux build invokes toolchain, add flux fmt) by @elcoosp ([0027289](https://github.com/elcoosp/flux/commit/00272898e684b94ee6cf6697a2891fb64ea5dccc))
- strip merge-guard references from AGENTS/README/MANIFEST_REQUESTS/CHANGELOG/roadmap by @elcoosp ([5399a11](https://github.com/elcoosp/flux/commit/5399a110ce7937cf5baa799a6a6e0110f9b4b2eb))
- rectify README facts against source (crates, ADRs, node-id hash, CLI, CI) by @elcoosp ([d5fa330](https://github.com/elcoosp/flux/commit/d5fa330c417f68c980532530028a7d1cf659d4c0))
- ground README in actual codebase — nightly toolchain, 15 crates, hand-written parser by @elcoosp ([f9b9c59](https://github.com/elcoosp/flux/commit/f9b9c59e0ee94f7714a5b710961a7870a3b842db))
- correct stale doc comments to match implementation by @elcoosp ([c516350](https://github.com/elcoosp/flux/commit/c51635025a822ccc435fdb7675c452bef730c429))
- add grounded README with CI and metric badges by @elcoosp ([3012103](https://github.com/elcoosp/flux/commit/30121038ddfe0f32e8946e687e2e2563ccf6b927))

### Features
- validate register operands per opcode in decode_program by @elcoosp ([69ff409](https://github.com/elcoosp/flux/commit/69ff40930b2c8140bc91ce75a1d7fb746b520323))
- FLUX-072 concise data-driven app surface by @elcoosp ([cbc6b73](https://github.com/elcoosp/flux/commit/cbc6b7390d86f7700cceacb17a592822817ad5cf))
- land FLUX-051/052/053/055 language maturity by @elcoosp ([dd9a2af](https://github.com/elcoosp/flux/commit/dd9a2afaf4de88e73a14444281fa0c1b1fb5de75))
- MLP v2 unified capability bridge — CALL_CAP returns signal-cell id, AWAIT cell-aware (ADR-0045) by @elcoosp ([9cdc470](https://github.com/elcoosp/flux/commit/9cdc4708904cd28a5860b334883feef5390006b4))
- first-class async via AWAIT opcode + suspend/resume (ADR-0044) by @elcoosp ([a215a30](https://github.com/elcoosp/flux/commit/a215a304e9a1a4307c218a12cc8b6857842e68f9))
- add TO_STRING opcode (0xD0) + prop-thunk string interpolation (ADR-0043) by @elcoosp ([c284a36](https://github.com/elcoosp/flux/commit/c284a36b1627b78e51d5f0a7b43f346740743b3d))
- implement flux-vm-ref oracle (FLUX-005) passing all 71 golden ISA vectors by @elcoosp ([a079a43](https://github.com/elcoosp/flux/commit/a079a43b4c473bdb6cb975179fe9910cd09f4370))

### Miscellaneous
- inject missing metadata and point to root README by @elcoosp ([18eef32](https://github.com/elcoosp/flux/commit/18eef32d8fcc00dd39f2006beee3756e296382f1))
- inject precisely curated keywords for crates.io by @elcoosp ([4d91cc4](https://github.com/elcoosp/flux/commit/4d91cc4b13147eb431ed45db6008257a14dc5e00))
- rustfmt clean-up after F7/F8/F10 patches by @elcoosp ([e41c06c](https://github.com/elcoosp/flux/commit/e41c06c54d970ae41994673f97975086357c23ea))
- fmt by @elcoosp ([93eec77](https://github.com/elcoosp/flux/commit/93eec7748458b3a1ab020cfe41c10d5d339b343c))
- add flux-vm-ref and flux-parity crates for contract v2 by @elcoosp ([b8be8fa](https://github.com/elcoosp/flux/commit/b8be8faca302ce53207cab2307bb9c6acf473374))

### Performance
- assert 50-instr handler eval stays under 2ms by @elcoosp ([6ab74e9](https://github.com/elcoosp/flux/commit/6ab74e99b5db745c0a485f7b0388f1b0acef94c2))
- LANE-H large-tree benchmarks vs §3.10 budgets by @elcoosp ([680a9fb](https://github.com/elcoosp/flux/commit/680a9fb3f797af96d94eeb9d03877b8b0ed2c51b))

### Refactor
- drop allocation budget; validate resume state; list index literal (revert F1/F16) by @elcoosp ([e5cd20d](https://github.com/elcoosp/flux/commit/e5cd20deddb18ca8810ee66b46fc1639b042192b))

### Testing
- add register validation and resume-state tests by @elcoosp ([d697f18](https://github.com/elcoosp/flux/commit/d697f186a65985f5cc73cde10c8f188e86f5b4bd))
- use literal list index (revert F1) by @elcoosp ([ca5d9f2](https://github.com/elcoosp/flux/commit/ca5d9f2eb4b35e112dc07988d66a20e291c43646))
- add adversarial bytecode corruption tests by @elcoosp ([e387aed](https://github.com/elcoosp/flux/commit/e387aed88512cd7f36a0f1def106176774e3cc4c))
- add tests adversarial by @elcoosp ([5587d8d](https://github.com/elcoosp/flux/commit/5587d8d1c99327d1e2ac7d66afea2d9d2f3ba1b2))
- T-304 list ops use register-index semantics (F1) by @elcoosp ([27da043](https://github.com/elcoosp/flux/commit/27da043ed3761a040f87c9cacbf35920135764fd))
**Full Changelog**: https://github.com/elcoosp/flux/releases/tag/v0.1.0
