# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
## [0.1.0] - 2026-11-11

### Bug Fixes
- local floor/ceil char-boundary helpers (MSRV 1.86) by @elcoosp ([b1fb6b5](https://github.com/elcoosp/flux/commit/b1fb6b5d52752a9757c3bf199b914ad1c0808fca))
- char-boundary-safe excerpt slicing from spans (T-604.11) by @elcoosp ([2a13535](https://github.com/elcoosp/flux/commit/2a13535d941d1ae4cd319367882c543bdaddad8a))
- Opcode::ALL includes BoolEq (T-215) by @elcoosp ([0db3615](https://github.com/elcoosp/flux/commit/0db361515b580c801a82791dd5282b290fe3420f))
- fold component_id into node ids as 4 raw bytes (T-214) by @elcoosp ([cc6438e](https://github.com/elcoosp/flux/commit/cc6438ee35f90e4d95003755b228d59b2f6b31c5))
- align Opcode::ALL count to declared 61 by @elcoosp ([1c2a88a](https://github.com/elcoosp/flux/commit/1c2a88a0c10830c82a323b962ece334b0244d34e))
- add missing IsNull opcode variant and Null expr arm by @elcoosp ([773da36](https://github.com/elcoosp/flux/commit/773da369e979f3aa6ee0bf4a9c8d6f4038e783c9))

### Documentation
- F56 XOR-fold cancels duplicate PropIdx entries (round-100) by @elcoosp ([6882ac3](https://github.com/elcoosp/flux/commit/6882ac3ba38eef04f08753dc345e5fb00da154f9))
- correct stale doc comments to match implementation by @elcoosp ([c516350](https://github.com/elcoosp/flux/commit/c51635025a822ccc435fdb7675c452bef730c429))

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
