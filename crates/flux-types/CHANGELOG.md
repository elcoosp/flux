# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
## [0.1.0] - 2026-11-14

### Bug Fixes
- saturating_sub on inverted spans in render by @elcoosp ([ac7cee6](https://github.com/elcoosp/flux/commit/ac7cee606e56e9046d09efbdd0541276186257f5))
- rewrite user-generic ADT variant fields to PARAM_BASE vars by @elcoosp ([5056693](https://github.com/elcoosp/flux/commit/5056693c9ef1e87afa98e43f16842476747a358b))
- treat `_` as a wildcard in match over ADTs (round-86, F27) by @elcoosp ([190d6cc](https://github.com/elcoosp/flux/commit/190d6cc64a83475f2068f46373458928654b6906))
- infer Await as fresh var, not Unit (round-85) by @elcoosp ([b527b8c](https://github.com/elcoosp/flux/commit/b527b8c2033d87b1b06eca3b00fe76b0d455adb9))
- resolve no longer panics on a substitution cycle (F13.3) by @elcoosp ([161820d](https://github.com/elcoosp/flux/commit/161820d0c1566538c41797cf4b292d987d08bda6))
- reject Swift/Kotlin reserved words at declaration sites by @elcoosp ([f140b76](https://github.com/elcoosp/flux/commit/f140b76f282bbab6e52dc7dd1e915458f56e3add))
- reject nested module paths instead of silently globbing by @elcoosp ([9d9ce5f](https://github.com/elcoosp/flux/commit/9d9ce5f4a637f410f579e3a75e1c4583431cdaac))
- try_lookup_value no longer mutates a throwaway supply (round-21) by @elcoosp ([6eccd9a](https://github.com/elcoosp/flux/commit/6eccd9ad383acb35b0565f14ca0fb53e82c79aee))
- checked u16 narrowing for record field indices (audit H14) by @elcoosp ([3faed7c](https://github.com/elcoosp/flux/commit/3faed7cf1a441b69e391ee211502307d2aaad342))
- accept List methods in field-access type inference (FLUX-072) by @elcoosp ([3679e37](https://github.com/elcoosp/flux/commit/3679e37726d974cdde71377ce6c5a8e9cab5c316))
- Router capability renamed to RouterNav (T-604.17) by @elcoosp ([3c594a0](https://github.com/elcoosp/flux/commit/3c594a0497c3a618844253a9db988eb6752e5f8e))
- resolve substitution fixpoint at 64-pass cap (T-604.5) by @elcoosp ([fbe2d0c](https://github.com/elcoosp/flux/commit/fbe2d0c5748a513d8a2c57486d7f9efdd84df88a))
- all-wildcard variant not universal catch-all (T-604.4) by @elcoosp ([6c30aa7](https://github.com/elcoosp/flux/commit/6c30aa70317a20a5718f116242fe585e8c511ae1))
- variant/list field access errors, not silent fresh var (T-604.6) by @elcoosp ([18e696c](https://github.com/elcoosp/flux/commit/18e696c1b2c238f8d9e20464042e3ac19174492b))
- shared generics supply + propagated type errors (T-109/T-110) by @elcoosp ([e75d9ef](https://github.com/elcoosp/flux/commit/e75d9ef1c6631a3d1092c23e415ab97c8fe306a8))
- resolve -D warnings in devserver/types/codegen-swift by @elcoosp ([b329330](https://github.com/elcoosp/flux/commit/b329330cb292f0ae7ea7a176d17d8e889854a517))
- restore FluxError umbrella left half-disabled by LANE-D by @elcoosp ([c758e13](https://github.com/elcoosp/flux/commit/c758e13216787e113b442fbbe9b4185df4c94a3e))
- CALL_CAP emits manifest capability ids, not name hashes by @elcoosp ([8cc2024](https://github.com/elcoosp/flux/commit/8cc202484c8087196825a45eba1ebe9b46495a7d))

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
- expose TypedAST.field_indices by @elcoosp ([f1433c9](https://github.com/elcoosp/flux/commit/f1433c9b35b28c4faa51bf44bd6ba7bba2aae895))
- populate field_indices with u16 checked narrowing by @elcoosp ([d5b7882](https://github.com/elcoosp/flux/commit/d5b7882fdbfd4e8eddc34942c26389344d54551f))
- add field_indices map; resolve loop bounded by subst.len() by @elcoosp ([8f6dc78](https://github.com/elcoosp/flux/commit/8f6dc781d1c437527f8beed88814f73cb21f10cb))
- reserved-identifier list for Swift/Kotlin codegen targets by @elcoosp ([d02fe0e](https://github.com/elcoosp/flux/commit/d02fe0e5f7bdd8bfa6f604e10d18410678bc9d81))
- register ScrollView primitive (FLUX-056, PRD-N) by @elcoosp ([2aae398](https://github.com/elcoosp/flux/commit/2aae398d0ab04d4fe1fb601cab12931a5fde46e9))
- FLUX-078 codegen-driven native glue + parity guards by @elcoosp ([bc27967](https://github.com/elcoosp/flux/commit/bc279675a53a301bb195e76b9d26bbb217e1b9b0))
- implement use module resolution end-to-end by @elcoosp ([5b9fa61](https://github.com/elcoosp/flux/commit/5b9fa6127c5521c303132e7e9b69a35a35ebb757))
- FLUX-072 bind ForEach item to a per-row signal slot by @elcoosp ([4c044a0](https://github.com/elcoosp/flux/commit/4c044a002012a25ff10153b38b1f02219c63a25b))
- FLUX-072 concise data-driven app surface by @elcoosp ([cbc6b73](https://github.com/elcoosp/flux/commit/cbc6b7390d86f7700cceacb17a592822817ad5cf))
- FLUX-047 add Http (cap 14) + Persist (cap 15) to IDL + stdlib by @elcoosp ([bc053f2](https://github.com/elcoosp/flux/commit/bc053f276f30af7a3979224498c8c986bd3682b9))
- FLUX-053 optional-chaining bytecode + Null literal by @elcoosp ([0c14e4a](https://github.com/elcoosp/flux/commit/0c14e4a54be7de0a618ac6a01c438ce0087200b8))
- land FLUX-051/052/053/055 language maturity by @elcoosp ([dd9a2af](https://github.com/elcoosp/flux/commit/dd9a2afaf4de88e73a14444281fa0c1b1fb5de75))
- add WebView (cap 12) + NativeModule (cap 13) with permission mapping (FLUX-048/046) by @elcoosp ([3ece8f0](https://github.com/elcoosp/flux/commit/3ece8f0c2bae6b7b3d5b08bc6edd0744ab7f75b7))
- FLUX-038 Modal/Sheet/Dialog overlay container primitives by @elcoosp ([484af6c](https://github.com/elcoosp/flux/commit/484af6c294d3b3bc2b934a81121f0e39d2f070d6))
- register six concrete native capabilities (FLUX-045, partial) by @elcoosp ([4aa18c2](https://github.com/elcoosp/flux/commit/4aa18c2e63801c696754c626656867515e384f97))
- register Gesture primitive (FLUX-041, partial) by @elcoosp ([0434908](https://github.com/elcoosp/flux/commit/0434908d6b6438685b677496aef3492b007ebc87))
- register form primitives (FLUX-040, partial) by @elcoosp ([9fc6fc6](https://github.com/elcoosp/flux/commit/9fc6fc6619751fbd48db71a269b5b678bbeb8be4))
- register Stack/Grid/Spacer/SafeArea primitives (FLUX-037) by @elcoosp ([28026bf](https://github.com/elcoosp/flux/commit/28026bfa8dd85e62462e8f980b168a0c5546b026))
- structural record typing (FLUX-054 / ADR-0052) by @elcoosp ([9f9e642](https://github.com/elcoosp/flux/commit/9f9e642a79316c1a5b76457e9399176a454293e1))
- optional chaining (?.) ergonomics — FLUX-053 by @elcoosp ([6923144](https://github.com/elcoosp/flux/commit/6923144384a7fe31d2761af58ff6e5ac9127920e))
- register Clipboard (cap 4) and Geolocation (cap 5) capabilities by @elcoosp ([798fd03](https://github.com/elcoosp/flux/commit/798fd036e4c34e8125e19a323e4aaed80a42cce0))
- LANE-I unified FluxError hierarchy + permission gate by @elcoosp ([b972da4](https://github.com/elcoosp/flux/commit/b972da423b3f7f4a6d65b3b5de6adbd83e47d3a9))
- apply Patch::Reattach to preserve instance state (Phase 3h) by @elcoosp ([a667998](https://github.com/elcoosp/flux/commit/a667998f4d4a3cd0f03a7fcbeff45f9faa84c232))
- replace pest with hand-written dream-syntax lexer + recursive-descent parser by @elcoosp ([d7fbb72](https://github.com/elcoosp/flux/commit/d7fbb72c5e809ae395c499bc19ff6462f63faea9))
- update type vocabulary by @elcoosp ([5b4e5b8](https://github.com/elcoosp/flux/commit/5b4e5b86f41e4236a97c3c771eca6375e29e1478))
- FLUX-012 bidirectional type checker + generic instantiation tracking by @elcoosp ([c4ad201](https://github.com/elcoosp/flux/commit/c4ad2013f79260f49285b931845a19ee0064c2e6))
- cargo workspace, rust toolchain and flux-syntax crate by @elcoosp ([af901dd](https://github.com/elcoosp/flux/commit/af901dddd882e467516c8961c4bfc70b9ca076fa))

### Miscellaneous
- inject missing metadata and point to root README by @elcoosp ([18eef32](https://github.com/elcoosp/flux/commit/18eef32d8fcc00dd39f2006beee3756e296382f1))
- inject precisely curated keywords for crates.io by @elcoosp ([4d91cc4](https://github.com/elcoosp/flux/commit/4d91cc4b13147eb431ed45db6008257a14dc5e00))
- remove dead field_indices map (round-87, F37) by @elcoosp ([7a2f424](https://github.com/elcoosp/flux/commit/7a2f424a50d3fd9103e3b6a7d5b9a00d097bcefa))
- rustfmt clean-up after F7/F8/F10 patches by @elcoosp ([e41c06c](https://github.com/elcoosp/flux/commit/e41c06c54d970ae41994673f97975086357c23ea))
- clear pre-existing findings on main (F10 prerequisite) by @elcoosp ([8ce5e35](https://github.com/elcoosp/flux/commit/8ce5e3525279898916592729b8a95935d1995450))
- register the reserved-identifier checker module by @elcoosp ([0807d04](https://github.com/elcoosp/flux/commit/0807d047d220f21205b56ced9234f4dfea6ee698))
- fmt by @elcoosp ([93eec77](https://github.com/elcoosp/flux/commit/93eec7748458b3a1ab020cfe41c10d5d339b343c))
- pre-wire dev-dependencies and criterion bench targets by @elcoosp ([a4df2a8](https://github.com/elcoosp/flux/commit/a4df2a8a9afef4d968a77dbdf62505f98f75effd))

### Performance
- replace blake3 with FNV-1a-32 for NodeId derivation (FLUX-071) by @elcoosp ([3b817f3](https://github.com/elcoosp/flux/commit/3b817f35558452a4edd7516efa149507e1fc1aac))
- add 3ms budget assertion to type-check bench by @elcoosp ([ef89f71](https://github.com/elcoosp/flux/commit/ef89f7150924555f8fa46095e5e613dc6dea47af))

### Refactor
- delete reserved-identifier check module by @elcoosp ([1525771](https://github.com/elcoosp/flux/commit/152577179f9d51fd2aec3e2885682d17e91b6628))
- split capabilities.rs (786 lines) into single-responsibility capabilities/ modules (FLUX-088) by @elcoosp ([9b575d8](https://github.com/elcoosp/flux/commit/9b575d86d765653302bf9e5ceb96681b4ccd211f))
- split kind.rs (387 lines) into single-responsibility kind/ modules (FLUX-088) by @elcoosp ([fd89529](https://github.com/elcoosp/flux/commit/fd895295a938c32e96c1a1810123813aa558c430))
- split error.rs (437 lines) into single-responsibility error/ modules (FLUX-088) by @elcoosp ([d8e1291](https://github.com/elcoosp/flux/commit/d8e1291fc2ce3244438e7e89a945f79b4048da61))
- split checker.rs (1773 lines) into single-responsibility checker/ modules (FLUX-088) by @elcoosp ([736eea2](https://github.com/elcoosp/flux/commit/736eea2101d800e7d8274e2bc288ac08d9111a4a))
- rename primitives/props/caps to RN/Expo surface (FLUX-038) by @elcoosp ([1f1fa4b](https://github.com/elcoosp/flux/commit/1f1fa4b7d44fdcc2467521f110a6519564a48fd8))
- migrate compute_node_id to DeclTag (3b) by @elcoosp ([ad4bc12](https://github.com/elcoosp/flux/commit/ad4bc12a4464b93d9e14facb8696c98d391136cc))
- drop Elided-callee workaround, infer else-block as block by @elcoosp ([ee8ac10](https://github.com/elcoosp/flux/commit/ee8ac10cd9a83be702363ae512eddf54c5b2e6a3))

### Revert
- drop >2-segment use-path rejection by @elcoosp ([f371a3d](https://github.com/elcoosp/flux/commit/f371a3de440851bef01493ec4485d0ffeaafef34))
- drop _ wildcard special-case (revert F27) by @elcoosp ([b9a769a](https://github.com/elcoosp/flux/commit/b9a769ac3faaded316ae470b870ff57a38694905))
- drop explicit Await arm by @elcoosp ([dcc06e0](https://github.com/elcoosp/flux/commit/dcc06e0518102b0612269c4a1ec066bf26b20e35))

### Testing
- add round-21 type regression tests by @elcoosp ([1a6adb4](https://github.com/elcoosp/flux/commit/1a6adb4513c68f7a05a5b5bc9a51262cb827613e))
- remove round-85 await-inference test by @elcoosp ([f49c4f2](https://github.com/elcoosp/flux/commit/f49c4f2b59eb1b068775bc0f555c2ecb20960565))
- add adversarial chain-resolve tests by @elcoosp ([3bac08b](https://github.com/elcoosp/flux/commit/3bac08b98b1ded7c1e4b01d69df16ccafea1ad1b))
- remove reserved-name and nested-use tests by @elcoosp ([2c139c6](https://github.com/elcoosp/flux/commit/2c139c67ca252a7ccc75bd8d6029e16373114f22))
- add tests adversarial by @elcoosp ([5587d8d](https://github.com/elcoosp/flux/commit/5587d8d1c99327d1e2ac7d66afea2d9d2f3ba1b2))
- reserved-identifier rejection regression by @elcoosp ([fa90579](https://github.com/elcoosp/flux/commit/fa905790cded9317902ccae5b8b78cb50d266da2))
- 3-segment use is rejected, not silently globbed by @elcoosp ([1bca0d2](https://github.com/elcoosp/flux/commit/1bca0d286a403fcc4dbcb79b040442a9e244e993))

### fmt
- reformat record ctor source string in typecheck test by @elcoosp ([e38f54c](https://github.com/elcoosp/flux/commit/e38f54cf13123f75170cc65fdf946022880f451e))
**Full Changelog**: https://github.com/elcoosp/flux/releases/tag/v0.1.0
