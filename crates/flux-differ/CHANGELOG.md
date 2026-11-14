# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
## [0.1.0] - 2026-11-14

### Bug Fixes
- key candidates by u32 component id; re-add root_position (revert F36) by @elcoosp ([22f822f](https://github.com/elcoosp/flux/commit/22f822fff7c28c07c8fa0a1dd83fd6e40f3c4c9c))
- NaN-equals-NaN; compare records key-wise by @elcoosp ([9325e5d](https://github.com/elcoosp/flux/commit/9325e5d6d43cd2b0daf04aa09856a25c95ff8c85))
- handler on all in-place paths; reattach-before-insert; sort removes; drop F36/F37 sorts by @elcoosp ([c95701a](https://github.com/elcoosp/flux/commit/c95701a82659e3601fa9a6da5db4161552e5a0a2))
- precompute new_roots once; drop O(n²) root_position (round-87, F36) by @elcoosp ([bd4ebe0](https://github.com/elcoosp/flux/commit/bd4ebe0b18a86c40da8c9f3469e320f3668e4bcc))
- deterministic reattach pairing — sort inserted before pairing by @elcoosp ([979cf28](https://github.com/elcoosp/flux/commit/979cf284d74c9837f3003452fbb4e1888f35a7b9))
- sort Remove patches by node id (determinism) by @elcoosp ([57895fa](https://github.com/elcoosp/flux/commit/57895faae94c5ee631ac88700520a5ad456abad6))
- deterministic in-place patch order (round-20) by @elcoosp ([538ad53](https://github.com/elcoosp/flux/commit/538ad535c954d698a85d363f71fc0582feca0653))
- deterministic Reattach emission order (round-19) by @elcoosp ([c4aaa98](https://github.com/elcoosp/flux/commit/c4aaa98c15b7f9fd9d312ac52eb061c2ad219f2e))
- deterministic multi-root insert order (tie-break on node id) by @elcoosp ([64ef2c6](https://github.com/elcoosp/flux/commit/64ef2c63b7506a95675c89f83656af106fa23c10))
- warn + zero-length ref on oversized handler bytecode (audit H14) by @elcoosp ([ff686c4](https://github.com/elcoosp/flux/commit/ff686c44a34c150b7e35d8e5b1767bb7d97133b6))
- topologically order Insert patches so children follow new ancestors by @elcoosp ([3fb49f8](https://github.com/elcoosp/flux/commit/3fb49f8076ed9237d6ead2f6d86c5b216344b176))
- use explicit u32 construction for test node IDs by @elcoosp ([aa237cb](https://github.com/elcoosp/flux/commit/aa237cbf0c1abf737cfe58a284d0e3739fec1b1d))
- handlers_equal compares captured_signals (T-213) by @elcoosp ([e77881b](https://github.com/elcoosp/flux/commit/e77881b8b83ca973fc4c6cb60ba89984af7b7ebe))
- emit handler patches for added/removed handler ids on stable nodes (T-212) by @elcoosp ([ebb459e](https://github.com/elcoosp/flux/commit/ebb459e928e642f0efdf238c07cdd22745c6e0f6))
- differ helper functions use IRArena.all_ids() + Child pattern match by @elcoosp ([40f5056](https://github.com/elcoosp/flux/commit/40f5056d6d94db1ed3a091b59aae6b67df1a9950))
- differ emits multi-insert patches in sorted order + new top-level components on hot reload by @elcoosp ([42891ea](https://github.com/elcoosp/flux/commit/42891ea31c03f5563b32097f7b306ef1cddf3fce))
- remove phantom test-module declarations so flux-differ compiles by @elcoosp ([b32384a](https://github.com/elcoosp/flux/commit/b32384a1a7010f1acd590aa17f1eb3bb3e0df7c5))
- signal values use theme foreground; fold geometry into label cell by @elcoosp ([2fd01f2](https://github.com/elcoosp/flux/commit/2fd01f261e530e50253d5beac3e44777678f2765))
- implement flux-codebase-roast-and-fix-plan (all 10 items) by @elcoosp ([6cb87f8](https://github.com/elcoosp/flux/commit/6cb87f8914fdb75072e3689ce3a3da6e375bb310))
- compare interned string props by content so literal edits ship a Delta by @elcoosp ([b6fd68c](https://github.com/elcoosp/flux/commit/b6fd68c43f45377b93641a4415bf5a7f7a0540b3))
- populate arena closure table so handler-body edits ship a Patch::Handler by @elcoosp ([901cf80](https://github.com/elcoosp/flux/commit/901cf8023f0f32277e0237c91d92423c00260efe))

### Documentation
- point documentation field to Vercel site by @elcoosp ([6a49f5d](https://github.com/elcoosp/flux/commit/6a49f5db7cf7eb3925c36901ef9dd752bcb20429))
- add Vercel documentation link to Cargo.toml by @elcoosp ([d0c18de](https://github.com/elcoosp/flux/commit/d0c18de8f51caf6f1a9feb08dca2d06ae7088a63))
- enforce SKILL.md rules (jsDelivr logos, absolute links, remove license sections) by @elcoosp ([c4ff690](https://github.com/elcoosp/flux/commit/c4ff690b590b225068b8703e587c63da7c74cd56))
- use style=for-the-badge for all badges by @elcoosp ([5dadc45](https://github.com/elcoosp/flux/commit/5dadc45f8b61c02251855933b46a4c80730d8289))
- sync badges to flat style + refresh stale facts (38 ADRs, 633 commits, 51k LOC, 28 stdlib, 12 CI, flux build invokes toolchain, add flux fmt) by @elcoosp ([0027289](https://github.com/elcoosp/flux/commit/00272898e684b94ee6cf6697a2891fb64ea5dccc))
- strip merge-guard references from AGENTS/README/MANIFEST_REQUESTS/CHANGELOG/roadmap by @elcoosp ([5399a11](https://github.com/elcoosp/flux/commit/5399a110ce7937cf5baa799a6a6e0110f9b4b2eb))
- rectify README facts against source (crates, ADRs, node-id hash, CLI, CI) by @elcoosp ([d5fa330](https://github.com/elcoosp/flux/commit/d5fa330c417f68c980532530028a7d1cf659d4c0))
- ground README in actual codebase — nightly toolchain, 15 crates, hand-written parser by @elcoosp ([f9b9c59](https://github.com/elcoosp/flux/commit/f9b9c59e0ee94f7714a5b710961a7870a3b842db))
- add grounded README with CI and metric badges by @elcoosp ([3012103](https://github.com/elcoosp/flux/commit/30121038ddfe0f32e8946e687e2e2563ccf6b927))

### Features
- FLUX-075 unified error overlay — v2 wire frames + ADR-0057 excerpt by @elcoosp ([1facd4f](https://github.com/elcoosp/flux/commit/1facd4f8fe77a5a6314a4e33c9d2b0407df7c584))
- monomorphise generics and add the Reattach patch by @elcoosp ([bdcfa64](https://github.com/elcoosp/flux/commit/bdcfa645617f573c2fcc3ffa61b5ef67f099c8ae))
- add precomputed prop/child hashes for O(1) differ; compile if/when/match to bytecode by @elcoosp ([cb509a7](https://github.com/elcoosp/flux/commit/cb509a736a5ff383d5a345958c298c69dcd7c518))
- implement flux-differ keyed reconciliation (FLUX-014) by @elcoosp ([d2769d3](https://github.com/elcoosp/flux/commit/d2769d37bbb2e50e8fefb959ac49a2ed127d05f5))
- cargo workspace, rust toolchain and flux-syntax crate by @elcoosp ([af901dd](https://github.com/elcoosp/flux/commit/af901dddd882e467516c8961c4bfc70b9ca076fa))

### Miscellaneous
- inject missing metadata and point to root README by @elcoosp ([18eef32](https://github.com/elcoosp/flux/commit/18eef32d8fcc00dd39f2006beee3756e296382f1))
- inject precisely curated keywords for crates.io by @elcoosp ([4d91cc4](https://github.com/elcoosp/flux/commit/4d91cc4b13147eb431ed45db6008257a14dc5e00))
- drop module_inception allow on diff tests by @elcoosp ([199ecb4](https://github.com/elcoosp/flux/commit/199ecb4922af48d464e6f425c5cc3aa55a44c945))
- flux-differ + flux-codegen-core zero findings; PROGRESS stale rows (F12, F10 prerequisite) by @elcoosp ([d7fbfc2](https://github.com/elcoosp/flux/commit/d7fbfc2889576703ee77d73a2076f84ef4f53ed8))
- rustfmt clean-up after F7/F8/F10 patches by @elcoosp ([e41c06c](https://github.com/elcoosp/flux/commit/e41c06c54d970ae41994673f97975086357c23ea))
- add tracing workspace dep by @elcoosp ([b0c8aad](https://github.com/elcoosp/flux/commit/b0c8aadc6de60f46aae254339f743bd3357b2e41))
- drop unused super::* import in tests by @elcoosp ([93479b8](https://github.com/elcoosp/flux/commit/93479b8cc3973e1fe276c721c156eb11e1ec936e))
- drop unused  on depth_of closure param by @elcoosp ([4509b81](https://github.com/elcoosp/flux/commit/4509b810b53e5ba90b3d04646ddff4e3283ca06a))
- split algorithm.rs tree helpers into tree.rs; allowlist backend.rs (T-605) by @elcoosp ([25786fb](https://github.com/elcoosp/flux/commit/25786fb11283ff194a36e176b971d8a214c0b1fb))
- fmt by @elcoosp ([93eec77](https://github.com/elcoosp/flux/commit/93eec7748458b3a1ab020cfe41c10d5d339b343c))
- pre-wire dev-dependencies and criterion bench targets by @elcoosp ([a4df2a8](https://github.com/elcoosp/flux/commit/a4df2a8a9afef4d968a77dbdf62505f98f75effd))

### Performance
- O(1) jump-target set, BFS+HashSet dedup, hash-join reattach by @elcoosp ([59aaf62](https://github.com/elcoosp/flux/commit/59aaf6217406a45204dd12d983d492b0bd819441))
- precompute parent/index once, killing O(n·r·i) cold path (FLUX-079) by @elcoosp ([d20dbe9](https://github.com/elcoosp/flux/commit/d20dbe97717cc8be2e88af7cc7a48a25dce402e2))
- LANE-H large-tree benchmarks vs §3.10 budgets by @elcoosp ([680a9fb](https://github.com/elcoosp/flux/commit/680a9fb3f797af96d94eeb9d03877b8b0ed2c51b))
- benchmark the O(1) prop-hash skip path on identical subtrees by @elcoosp ([0e3480e](https://github.com/elcoosp/flux/commit/0e3480eb181852fc671cf5205594058f72bfbf1d))

### Refactor
- remove stale duplicated comment (round-87 tidy) by @elcoosp ([d152593](https://github.com/elcoosp/flux/commit/d152593abd8735244a26202213bf36c8d80c994a))
- split diff.rs (761 lines) into single-responsibility diff/ modules (FLUX-088) by @elcoosp ([2c19307](https://github.com/elcoosp/flux/commit/2c19307b4270e1a8495494a5d6bec686e979ee13))

### Testing
- add round-21 patch-stream regression tests by @elcoosp ([2e3130f](https://github.com/elcoosp/flux/commit/2e3130fcff623c4ee2eb3df49de4470bdb0e03e6))
- use u32::from for reattach ids by @elcoosp ([aded227](https://github.com/elcoosp/flux/commit/aded2278787d1f474fe22f08ffaaf7b6cc23ead4))
- use u32::from for insert parents by @elcoosp ([ed125ff](https://github.com/elcoosp/flux/commit/ed125ffdd2a712e2946fb9e3eacd251cd0709a77))
- use u32::from for update ids by @elcoosp ([2eb82dd](https://github.com/elcoosp/flux/commit/2eb82dd2accb00389e02ad07d47e2a9b3de46432))
- add adversarial arena tests by @elcoosp ([0c74d38](https://github.com/elcoosp/flux/commit/0c74d3800497fa0ea889f0a3b430be2188f2a028))
- add tests adversarial by @elcoosp ([5587d8d](https://github.com/elcoosp/flux/commit/5587d8d1c99327d1e2ac7d66afea2d9d2f3ba1b2))
- in-place patch order deterministic across runs by @elcoosp ([2f6afa8](https://github.com/elcoosp/flux/commit/2f6afa8808b9f02f6e893da58a729f36ee8f3fdc))
- Reattach order is deterministic across runs by @elcoosp ([8c61a87](https://github.com/elcoosp/flux/commit/8c61a8788bd11bdeedd2ceda5d58403ba5c71684))
- multi-root insert order is deterministic (round-14) by @elcoosp ([54885c9](https://github.com/elcoosp/flux/commit/54885c9cf3c16e71ba7005c48b45f48435a0eb90))
- cover u16::MAX boundary for closure_ref by @elcoosp ([79d84a4](https://github.com/elcoosp/flux/commit/79d84a44e7ea1c0117ea212be374f6e9e2d9ea88))

### fmt
- reorder imports and reformat ClosureIR ctor in tests.rs by @elcoosp ([cbd4cef](https://github.com/elcoosp/flux/commit/cbd4ceff9462628636b8a8858682ac1cd97a2835))
- reformat test helpers and assertions in algorithm.rs by @elcoosp ([9575241](https://github.com/elcoosp/flux/commit/95752414242171aabd97b80bfb41423c1a46b4ab))
- reformat comment alignment in diff.rs test by @elcoosp ([1ba3a83](https://github.com/elcoosp/flux/commit/1ba3a839b6fa986b3ba3f4a9f5f26b39fd772fb9))
**Full Changelog**: https://github.com/elcoosp/flux/releases/tag/v0.1.0
