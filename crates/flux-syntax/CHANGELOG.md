# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
## [0.1.0] - 2026-11-14

### Bug Fixes
- local floor/ceil char-boundary helpers (MSRV 1.86) by @elcoosp ([b1fb6b5](https://github.com/elcoosp/flux/commit/b1fb6b5d52752a9757c3bf199b914ad1c0808fca))
- char-boundary-safe excerpt slicing from spans (T-604.11) by @elcoosp ([2a13535](https://github.com/elcoosp/flux/commit/2a13535d941d1ae4cd319367882c543bdaddad8a))
- Opcode::ALL includes BoolEq (T-215) by @elcoosp ([0db3615](https://github.com/elcoosp/flux/commit/0db361515b580c801a82791dd5282b290fe3420f))
- fold component_id into node ids as 4 raw bytes (T-214) by @elcoosp ([cc6438e](https://github.com/elcoosp/flux/commit/cc6438ee35f90e4d95003755b228d59b2f6b31c5))
- align Opcode::ALL count to declared 61 by @elcoosp ([1c2a88a](https://github.com/elcoosp/flux/commit/1c2a88a0c10830c82a323b962ece334b0244d34e))
- add missing IsNull opcode variant and Null expr arm by @elcoosp ([773da36](https://github.com/elcoosp/flux/commit/773da369e979f3aa6ee0bf4a9c8d6f4038e783c9))

### Documentation
- point documentation field to Vercel site by @elcoosp ([6a49f5d](https://github.com/elcoosp/flux/commit/6a49f5db7cf7eb3925c36901ef9dd752bcb20429))
- add Vercel documentation link to Cargo.toml by @elcoosp ([d0c18de](https://github.com/elcoosp/flux/commit/d0c18de8f51caf6f1a9feb08dca2d06ae7088a63))
- enforce SKILL.md rules (jsDelivr logos, absolute links, remove license sections) by @elcoosp ([c4ff690](https://github.com/elcoosp/flux/commit/c4ff690b590b225068b8703e587c63da7c74cd56))
- F56 XOR-fold cancels duplicate PropIdx entries (round-100) by @elcoosp ([6882ac3](https://github.com/elcoosp/flux/commit/6882ac3ba38eef04f08753dc345e5fb00da154f9))
- use style=for-the-badge for all badges by @elcoosp ([5dadc45](https://github.com/elcoosp/flux/commit/5dadc45f8b61c02251855933b46a4c80730d8289))
- sync badges to flat style + refresh stale facts (38 ADRs, 633 commits, 51k LOC, 28 stdlib, 12 CI, flux build invokes toolchain, add flux fmt) by @elcoosp ([0027289](https://github.com/elcoosp/flux/commit/00272898e684b94ee6cf6697a2891fb64ea5dccc))
- strip merge-guard references from AGENTS/README/MANIFEST_REQUESTS/CHANGELOG/roadmap by @elcoosp ([5399a11](https://github.com/elcoosp/flux/commit/5399a110ce7937cf5baa799a6a6e0110f9b4b2eb))
- rectify README facts against source (crates, ADRs, node-id hash, CLI, CI) by @elcoosp ([d5fa330](https://github.com/elcoosp/flux/commit/d5fa330c417f68c980532530028a7d1cf659d4c0))
- ground README in actual codebase — nightly toolchain, 15 crates, hand-written parser by @elcoosp ([f9b9c59](https://github.com/elcoosp/flux/commit/f9b9c59e0ee94f7714a5b710961a7870a3b842db))
- correct stale doc comments to match implementation by @elcoosp ([c516350](https://github.com/elcoosp/flux/commit/c51635025a822ccc435fdb7675c452bef730c429))
- add grounded README with CI and metric badges by @elcoosp ([3012103](https://github.com/elcoosp/flux/commit/30121038ddfe0f32e8946e687e2e2563ccf6b927))

### Features
- T-331 ForEach row-id derivation via FNV-1a + marker bits, cross-platform by @elcoosp ([117a37d](https://github.com/elcoosp/flux/commit/117a37d7602e019e6851cf9fc150199841197535))
- add content_addressed_id for wire-tree node ids (FLUX-074) by @elcoosp ([2c90351](https://github.com/elcoosp/flux/commit/2c903511aa8927ef88b8900544a6190d83208031))
- FLUX-078 codegen-driven native glue + parity guards by @elcoosp ([bc27967](https://github.com/elcoosp/flux/commit/bc279675a53a301bb195e76b9d26bbb217e1b9b0))
- FLUX-075 unified error overlay — v2 wire frames + ADR-0057 excerpt by @elcoosp ([1facd4f](https://github.com/elcoosp/flux/commit/1facd4f8fe77a5a6314a4e33c9d2b0407df7c584))
- FLUX-072 concise data-driven app surface by @elcoosp ([cbc6b73](https://github.com/elcoosp/flux/commit/cbc6b7390d86f7700cceacb17a592822817ad5cf))
- FLUX-053 optional-chaining bytecode + Null literal by @elcoosp ([0c14e4a](https://github.com/elcoosp/flux/commit/0c14e4a54be7de0a618ac6a01c438ce0087200b8))
- monomorphise generics and add the Reattach patch by @elcoosp ([bdcfa64](https://github.com/elcoosp/flux/commit/bdcfa645617f573c2fcc3ffa61b5ef67f099c8ae))
- first-class async via AWAIT opcode + suspend/resume (ADR-0044) by @elcoosp ([a215a30](https://github.com/elcoosp/flux/commit/a215a304e9a1a4307c218a12cc8b6857842e68f9))
- sealed NodeTag trait for node-id kind tags (3a) by @elcoosp ([e549bdf](https://github.com/elcoosp/flux/commit/e549bdfb187afea354abd0cd657aa9456dd65f48))
- add TO_STRING opcode (0xD0) + prop-thunk string interpolation (ADR-0043) by @elcoosp ([c284a36](https://github.com/elcoosp/flux/commit/c284a36b1627b78e51d5f0a7b43f346740743b3d))
- add canonical compute_node_id as single source of truth (G2/ADR-0027) by @elcoosp ([1d52b57](https://github.com/elcoosp/flux/commit/1d52b57c6071e77503c0325a9a26bb18f826021a))
- implement flux-vm-ref oracle (FLUX-005) passing all 71 golden ISA vectors by @elcoosp ([a079a43](https://github.com/elcoosp/flux/commit/a079a43b4c473bdb6cb975179fe9910cd09f4370))
- add VM opcode vocabulary from Appendix E by @elcoosp ([62bf718](https://github.com/elcoosp/flux/commit/62bf718c6752a23c40845357c6ec812277a4978d))
- cargo workspace, rust toolchain and flux-syntax crate by @elcoosp ([af901dd](https://github.com/elcoosp/flux/commit/af901dddd882e467516c8961c4bfc70b9ca076fa))

### Miscellaneous
- inject missing metadata and point to root README by @elcoosp ([18eef32](https://github.com/elcoosp/flux/commit/18eef32d8fcc00dd39f2006beee3756e296382f1))
- inject precisely curated keywords for crates.io by @elcoosp ([4d91cc4](https://github.com/elcoosp/flux/commit/4d91cc4b13147eb431ed45db6008257a14dc5e00))
- fmt by @elcoosp ([93eec77](https://github.com/elcoosp/flux/commit/93eec7748458b3a1ab020cfe41c10d5d339b343c))
- pre-wire dev-dependencies and criterion bench targets by @elcoosp ([a4df2a8](https://github.com/elcoosp/flux/commit/a4df2a8a9afef4d968a77dbdf62505f98f75effd))

### Performance
- replace blake3 with FNV-1a-32 for NodeId derivation (FLUX-071) by @elcoosp ([3b817f3](https://github.com/elcoosp/flux/commit/3b817f35558452a4edd7516efa149507e1fc1aac))
- add compute_node_id + StringTable::intern micro-benches by @elcoosp ([71c264c](https://github.com/elcoosp/flux/commit/71c264ca124cf6f23d3b48eb9ebcfa894bfcac27))

### Refactor
- use std floor/ceil char-boundary helpers in SourceExcerpt by @elcoosp ([1b8e7cd](https://github.com/elcoosp/flux/commit/1b8e7cd02f9cdb118d1e08e68eb82bc9db2483c1))
- split ids.rs (556 lines) into single-responsibility ids/ modules (FLUX-088) by @elcoosp ([afea5ce](https://github.com/elcoosp/flux/commit/afea5cea55041e7abddeb6f1e64a253d4112b1a4))

### Testing
- remove F56 hash-fields collision test by @elcoosp ([7e817a3](https://github.com/elcoosp/flux/commit/7e817a329402d1c8a42d59c3477e96fbd2df374a))

### fmt
- remove blank line at top of ids tests.rs by @elcoosp ([196a5be](https://github.com/elcoosp/flux/commit/196a5be8b902a5a6e66746af256d5084ccafc94a))
- reformat content_addressed_id calls in node_id test by @elcoosp ([0f95ae2](https://github.com/elcoosp/flux/commit/0f95ae2984c60d7d01e734cd777ca672491915dc))
**Full Changelog**: https://github.com/elcoosp/flux/releases/tag/v0.1.0
