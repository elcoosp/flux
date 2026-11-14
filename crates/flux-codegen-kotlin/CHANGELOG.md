# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
## [0.1.0] - 2026-11-14

### Bug Fixes
- preserve named-arg labels in handler calls (round-110, F63) + file F64 by @elcoosp ([0e3dfa8](https://github.com/elcoosp/flux/commit/0e3dfa87455642df2c09cc9e1e2dd4e58f391558))
- treat `_` as a wildcard in match over ADTs (round-86, F27) by @elcoosp ([190d6cc](https://github.com/elcoosp/flux/commit/190d6cc64a83475f2068f46373458928654b6906))
- guard arm emits explicit unsupported marker (was invalid is-name) by @elcoosp ([a8ddb6d](https://github.com/elcoosp/flux/commit/a8ddb6d1e731bc079240d531cb6a7337e858ced5))
- prelude Color+sp imports, generics before fn name, brace-wrap match arms, null positional key by @elcoosp ([0357f9c](https://github.com/elcoosp/flux/commit/0357f9c6364068b95a6724fc6cce425f863e37fa))
- Toggle emits Compose Switch(checked=, onCheckedChange=) by @elcoosp ([229ef7f](https://github.com/elcoosp/flux/commit/229ef7fd49545fb2cdface4b44eb576b1e373cf5))
- emit real Compose shapes for Switch/Checkbox/Slider/TextArea by @elcoosp ([72bc2cb](https://github.com/elcoosp/flux/commit/72bc2cbdb6fb775ce4591ac35f8e1aa535ec888c))
- override named_arg to '=' and use verticalAlignment for Row gap by @elcoosp ([6740d61](https://github.com/elcoosp/flux/commit/6740d612d8407a4b5fd0a00d375d29e86af14d97))
- Router capability renamed to RouterNav (T-604.17) by @elcoosp ([3c594a0](https://github.com/elcoosp/flux/commit/3c594a0497c3a618844253a9db988eb6752e5f8e))
- canonical event-verb vocabulary + onClick alias acceptance (T-604.16) by @elcoosp ([3e98b6f](https://github.com/elcoosp/flux/commit/3e98b6f679b0fe9f6da41b432570e62160025fcd))
- T-403.1 Swift Toggle binding + T-403.7 route state Router guard by @elcoosp ([57509dd](https://github.com/elcoosp/flux/commit/57509ddc2f59c5e17d0c97b88808f53a9c2bcf3a))
- Kotlin zero-field variants use data object (T-403.4) by @elcoosp ([1196be6](https://github.com/elcoosp/flux/commit/1196be6ce048376ca231209cb1119237b9bbfc64))
- Guard pattern handling in Swift emit_match by @elcoosp ([6cca2b1](https://github.com/elcoosp/flux/commit/6cca2b15a4a2aaeed77e57e378fd45d747d37613))
- axis-aware spacing in codegen (container_spacing_axis) by @elcoosp ([7351e66](https://github.com/elcoosp/flux/commit/7351e665bdf341307241190628842379b94711dc))
- STR_LEN panic fix + render_await backend hook by @elcoosp ([517871e](https://github.com/elcoosp/flux/commit/517871e79a152f146779dc906d426d0caaede3f9))
- Kotlin component header uses ', '-separated params (T-403.4) by @elcoosp ([d757d95](https://github.com/elcoosp/flux/commit/d757d958940cf3489b05d52bbff9d143f3cd968a))
- Toggle/Switch backend-split (T-403.1) by @elcoosp ([e3f5a14](https://github.com/elcoosp/flux/commit/e3f5a14eb9e7f99a7353741a7169bf08a1933b05))
- Kotlin prelude imports, LazyColumn ForEach (T-402) by @elcoosp ([e842ad9](https://github.com/elcoosp/flux/commit/e842ad9befbe70d6230b8dbd760c89513cb48d41))
- per-backend string escaping in release codegen (T-401) by @elcoosp ([1e5c2e9](https://github.com/elcoosp/flux/commit/1e5c2e9d8ffe00849c8c4704d5d3c683c3afb86d))
- emit valid Compose for sum types + match by @elcoosp ([6d706c0](https://github.com/elcoosp/flux/commit/6d706c0c26f9264fa54bcd3c8c18cd1b8961eea7))
- gate kotlinc check on explicit Compose classpath by @elcoosp ([dff1b3a](https://github.com/elcoosp/flux/commit/dff1b3ac09f1add23436260789f67d70968fffa8))
- skip kotlinc compile-check when no Android toolchain by @elcoosp ([57ca924](https://github.com/elcoosp/flux/commit/57ca9247c5fd1e8e2d9665a49ecf87598b4f2323))
- emit Button onClick handler body and text label by @elcoosp ([8e0cbff](https://github.com/elcoosp/flux/commit/8e0cbffdec528f47ef4ddeb9ae56fa70d378079a))
- key component bridge under DeclTag(3) to match flux-ir lowering by @elcoosp ([e50e518](https://github.com/elcoosp/flux/commit/e50e518a589bfb07a18e7b7de63d3581776e7553))
- migrate node-id bridge to sealed NodeTag (3a unblock) by @elcoosp ([6f752ad](https://github.com/elcoosp/flux/commit/6f752ad3a61431901bd8e384937187e13f12a17f))

### CI
- add navigation-compose; jvm-target 11; ui.geometry/graphics imports by @elcoosp ([2fda50d](https://github.com/elcoosp/flux/commit/2fda50d6a6548526e5047fcb96165934d45f5d07))
- pass kotlin-compiler-embeddable as second -Xplugin; skip on plugin load failure by @elcoosp ([42f38e7](https://github.com/elcoosp/flux/commit/42f38e7604c945db4740fa7856e2943b427b084a))
- add kotlin-compiler-embeddable to classpath; use -Xplugin= form by @elcoosp ([f8a68d0](https://github.com/elcoosp/flux/commit/f8a68d08cdc5fb5f790e61d9fea9deeb9933a3ff))
- fetch Compose compiler plugin from Maven Central; tolerate missing artifacts by @elcoosp ([b0034e8](https://github.com/elcoosp/flux/commit/b0034e83ab4cd96250ae15d36d0bcd75692fbee2))
- provision Compose from Google Maven; run real kotlinc check without platform-37 by @elcoosp ([8316f25](https://github.com/elcoosp/flux/commit/8316f25a0727c8108059ffdaffcac5b8c0b5d7a0))

### Documentation
- point documentation field to Vercel site by @elcoosp ([6a49f5d](https://github.com/elcoosp/flux/commit/6a49f5db7cf7eb3925c36901ef9dd752bcb20429))
- add Vercel documentation link to Cargo.toml by @elcoosp ([d0c18de](https://github.com/elcoosp/flux/commit/d0c18de8f51caf6f1a9feb08dca2d06ae7088a63))
- enforce SKILL.md rules (jsDelivr logos, absolute links, remove license sections) by @elcoosp ([c4ff690](https://github.com/elcoosp/flux/commit/c4ff690b590b225068b8703e587c63da7c74cd56))
- mark FLUX-F20 CLOSED — Kotlin guard chain landed by @elcoosp ([1695979](https://github.com/elcoosp/flux/commit/1695979739f26b2fb316d2f7546bf7bec5eaf2f0))
- use style=for-the-badge for all badges by @elcoosp ([5dadc45](https://github.com/elcoosp/flux/commit/5dadc45f8b61c02251855933b46a4c80730d8289))
- sync badges to flat style + refresh stale facts (38 ADRs, 633 commits, 51k LOC, 28 stdlib, 12 CI, flux build invokes toolchain, add flux fmt) by @elcoosp ([0027289](https://github.com/elcoosp/flux/commit/00272898e684b94ee6cf6697a2891fb64ea5dccc))
- strip merge-guard references from AGENTS/README/MANIFEST_REQUESTS/CHANGELOG/roadmap by @elcoosp ([5399a11](https://github.com/elcoosp/flux/commit/5399a110ce7937cf5baa799a6a6e0110f9b4b2eb))
- rectify README facts against source (crates, ADRs, node-id hash, CLI, CI) by @elcoosp ([d5fa330](https://github.com/elcoosp/flux/commit/d5fa330c417f68c980532530028a7d1cf659d4c0))
- ground README in actual codebase — nightly toolchain, 15 crates, hand-written parser by @elcoosp ([f9b9c59](https://github.com/elcoosp/flux/commit/f9b9c59e0ee94f7714a5b710961a7870a3b842db))
- add grounded README with CI and metric badges by @elcoosp ([3012103](https://github.com/elcoosp/flux/commit/30121038ddfe0f32e8946e687e2e2563ccf6b927))

### Features
- data class records; safe tween() fallback; drop guard-match path (revert F20/F27) by @elcoosp ([a0d75ef](https://github.com/elcoosp/flux/commit/a0d75efcf5229ad4afdabddc37203ab23ac33950))
- T-402 Kotlin prelude imports + Compose-native Animate + parity recognizer (Phase 4) by @elcoosp ([54311fb](https://github.com/elcoosp/flux/commit/54311fb40e88295bff72ad8e07d4468ad1f98dee))
- native NavigationStack/navigationDestination for Swift and NavController for Kotlin by @elcoosp ([7f834a2](https://github.com/elcoosp/flux/commit/7f834a21ce931d1a4246cd0cd4c2c9dfe85fd13a))
- self-contained release builds for counter/todo/router examples by @elcoosp ([ecbcc87](https://github.com/elcoosp/flux/commit/ecbcc87101ba0cb5e4ee0835c6116d4d0b3e0567))
- FLUX-042 animation primitive + FLUX-043 design-token theme extension by @elcoosp ([1d3b76f](https://github.com/elcoosp/flux/commit/1d3b76f79c5e0271af42a710b367a03a1cefbadd))
- FLUX-038 Modal/Sheet/Dialog overlay container primitives by @elcoosp ([484af6c](https://github.com/elcoosp/flux/commit/484af6c294d3b3bc2b934a81121f0e39d2f070d6))
- register Gesture primitive (FLUX-041, partial) by @elcoosp ([0434908](https://github.com/elcoosp/flux/commit/0434908d6b6438685b677496aef3492b007ebc87))
- register form primitives (FLUX-040, partial) by @elcoosp ([9fc6fc6](https://github.com/elcoosp/flux/commit/9fc6fc6619751fbd48db71a269b5b678bbeb8be4))
- register Stack/Grid/Spacer/SafeArea primitives (FLUX-037) by @elcoosp ([28026bf](https://github.com/elcoosp/flux/commit/28026bfa8dd85e62462e8f980b168a0c5546b026))
- wrap async capability-call handlers in launch/Task (FLUX-064) by @elcoosp ([1a845ec](https://github.com/elcoosp/flux/commit/1a845ecf220f56d29e9ba88deabc1bb31e5ff489))
- emit one specialised native type per generic instantiation (Phase 1c) by @elcoosp ([5cea19d](https://github.com/elcoosp/flux/commit/5cea19de7bd8ee536ae60db70b2beb12685ace4d))
- fully shape the six previously-bare primitives (ADR-0047) by @elcoosp ([12f2d97](https://github.com/elcoosp/flux/commit/12f2d97e0cffb803cc3947d2fcdaddc6d59b5b94))
- replace pest with hand-written dream-syntax lexer + recursive-descent parser by @elcoosp ([d7fbb72](https://github.com/elcoosp/flux/commit/d7fbb72c5e809ae395c499bc19ff6462f63faea9))
- implement Kotlin/Compose release codegen (FLUX-021) by @elcoosp ([a47d496](https://github.com/elcoosp/flux/commit/a47d496aaf01a91907b3ed390a4013c2a9f88c1e))
- cargo workspace, rust toolchain and flux-syntax crate by @elcoosp ([af901dd](https://github.com/elcoosp/flux/commit/af901dddd882e467516c8961c4bfc70b9ca076fa))

### Miscellaneous
- inject missing metadata and point to root README by @elcoosp ([18eef32](https://github.com/elcoosp/flux/commit/18eef32d8fcc00dd39f2006beee3756e296382f1))
- inject precisely curated keywords for crates.io by @elcoosp ([4d91cc4](https://github.com/elcoosp/flux/commit/4d91cc4b13147eb431ed45db6008257a14dc5e00))
- workspace clean under --all-targets -D warnings (F10 prerequisite) by @elcoosp ([81ce6ce](https://github.com/elcoosp/flux/commit/81ce6cebb7fa7a873b4d45ee77a000aec2d7da8c))
- rustfmt clean-up after F7/F8/F10 patches by @elcoosp ([e41c06c](https://github.com/elcoosp/flux/commit/e41c06c54d970ae41994673f97975086357c23ea))
- derive Debug for Kotlin backend marker by @elcoosp ([e843500](https://github.com/elcoosp/flux/commit/e8435000ea6b13251e7f6b395957f2b769c2c340))
- fmt fixes (T-604.7) + regenerate wire test fixtures by @elcoosp ([8218a4b](https://github.com/elcoosp/flux/commit/8218a4b31db48614e32f6269056506f8747c003e))
- update codegen snapshots + IR serde for drift-matrix fixes by @elcoosp ([82a0bde](https://github.com/elcoosp/flux/commit/82a0bdef694b855c6fdedf889745e41295ab77c0))
- fmt by @elcoosp ([93eec77](https://github.com/elcoosp/flux/commit/93eec7748458b3a1ab020cfe41c10d5d339b343c))
- add flux-vm-ref and flux-parity crates for contract v2 by @elcoosp ([b8be8fa](https://github.com/elcoosp/flux/commit/b8be8faca302ce53207cab2307bb9c6acf473374))
- pre-wire dev-dependencies and criterion bench targets by @elcoosp ([a4df2a8](https://github.com/elcoosp/flux/commit/a4df2a8a9afef4d968a77dbdf62505f98f75effd))

### Performance
- add release codegen emit bench by @elcoosp ([f5a4ef7](https://github.com/elcoosp/flux/commit/f5a4ef7eeb6eda19d05d30038b983706baf5bafa))
- replace per-line indent repeat() with precomputed table by @elcoosp ([55a24f0](https://github.com/elcoosp/flux/commit/55a24f09b5ac30982cc27c9869517546dfdca607))

### Refactor
- emit_state_cell accepts router_start (unused) by @elcoosp ([236ad15](https://github.com/elcoosp/flux/commit/236ad1597a0b388442aa970bd79e2abcf6486a68))
- rename primitives/props/caps to RN/Expo surface (FLUX-038) by @elcoosp ([1f1fa4b](https://github.com/elcoosp/flux/commit/1f1fa4b7d44fdcc2467521f110a6519564a48fd8))
- unified data-driven release codegen via flux-codegen-core (ADR-0047) by @elcoosp ([8f5c5d5](https://github.com/elcoosp/flux/commit/8f5c5d560b44805e5894b50b9bcce9328dc75e79))

### Testing
- shared tempdir; add record/option/map/animation-curve tests by @elcoosp ([7993f02](https://github.com/elcoosp/flux/commit/7993f02a1e3747c8a4204cf290e8130038b4d76d))
- remove obsolete F63 named-arg test by @elcoosp ([e1ba04b](https://github.com/elcoosp/flux/commit/e1ba04b81741752d6bdf9ccbde545dec64d4ddeb))
- remove obsolete round-34 handler-call test by @elcoosp ([4b6bb3f](https://github.com/elcoosp/flux/commit/4b6bb3f884a5d55a3b9ba16b319cdab048ab353d))
- remove obsolete F20 guard-match test by @elcoosp ([4183015](https://github.com/elcoosp/flux/commit/418301571e97cbee7f1baa418dffdc43d66e93f8))
- add adversarial text-emission tests by @elcoosp ([eb77ecd](https://github.com/elcoosp/flux/commit/eb77ecdef15220335111e6a93aa677a7e54d9c82))
- add tests adversarial by @elcoosp ([5587d8d](https://github.com/elcoosp/flux/commit/5587d8d1c99327d1e2ac7d66afea2d9d2f3ba1b2))
- awaited handler renders await expr (F19 regression); close F19 by @elcoosp ([99ce300](https://github.com/elcoosp/flux/commit/99ce3002d8ad1bf4acdad085b42042771fca6553))
- per-pid scratch dir for kotlinc flake (concurrent-run race) by @elcoosp ([09d5d0f](https://github.com/elcoosp/flux/commit/09d5d0f84dea74851a2c9baec789945bcd7dec64))
- capability-call handler survives; docs(issues): FLUX-F19 by @elcoosp ([4fb43f5](https://github.com/elcoosp/flux/commit/4fb43f5d2aeafac76640920b25098ec5b9bda31e))
- self-provision Compose classpath from ~/.gradle; match kotlinc version by @elcoosp ([be38134](https://github.com/elcoosp/flux/commit/be38134c935b27a42bc1c01c24694105e2e6dbd0))
- regenerate b3_10_generics snapshot by @elcoosp ([e359a05](https://github.com/elcoosp/flux/commit/e359a05d5c92e1d414ce98d9446c780c63d29dc1))
- regenerate b3_9_state snapshot by @elcoosp ([eede192](https://github.com/elcoosp/flux/commit/eede19220520193153cd5722437390256bf10e7e))
- regenerate b3_8_form snapshot by @elcoosp ([9e61600](https://github.com/elcoosp/flux/commit/9e61600559f6e2a787d6337b1c170954aff1131d))
- regenerate b3_7_optional snapshot by @elcoosp ([5fa366a](https://github.com/elcoosp/flux/commit/5fa366a723a10cb57dd505e5db8fb9b44861162c))
- regenerate b3_6_fetch snapshot by @elcoosp ([8a4f08d](https://github.com/elcoosp/flux/commit/8a4f08d855ae051859c2c4c9f52a258d2cf37c1d))
- regenerate b3_5_conditional snapshot by @elcoosp ([dc37ae2](https://github.com/elcoosp/flux/commit/dc37ae2d27bc3b99877aa09f9d0dfb51fae7fa14))
- regenerate b3_4_router snapshot by @elcoosp ([70a443c](https://github.com/elcoosp/flux/commit/70a443c08155258bb6f65df0335bb8d25df6d52a))
- regenerate b3_3_match snapshot by @elcoosp ([e85a12b](https://github.com/elcoosp/flux/commit/e85a12b952362c1f0ba9d98dafe79388d75c32e1))
- regenerate b3_2_button snapshot by @elcoosp ([6fbe502](https://github.com/elcoosp/flux/commit/6fbe502b213fbef0ab5fb7bea66c4f3043502e50))
- regenerate b3_1_counter snapshot by @elcoosp ([9d7df97](https://github.com/elcoosp/flux/commit/9d7df971b19f3fcd601d41248cfa387068b12e54))
- per-example kotlinc type-check + valid fixtures by @elcoosp ([4fc65ae](https://github.com/elcoosp/flux/commit/4fc65ae4662980a014f5e608732d576c399a85bb))
- accept B.3.10 generics snapshot with <T> clause restored by @elcoosp ([b14519c](https://github.com/elcoosp/flux/commit/b14519c64a63b411eef5a74f7b3b69f7cdefe189))
- add T-401 escaping round-trip tests for Swift and Kotlin backends by @elcoosp ([1fee310](https://github.com/elcoosp/flux/commit/1fee3105bf6a73f81b52c198d82304f12f1492d3))
- accept codegen/parity snapshot drift from FLUX-072 data-driven surface by @elcoosp ([7faca34](https://github.com/elcoosp/flux/commit/7faca3410ced3f975d9fb4db817a0275a00e8c3e))
- pin Image primitive lowers to native binding (FLUX-039, partial) by @elcoosp ([3941519](https://github.com/elcoosp/flux/commit/3941519fb544138cb85f084a656834b3ebb5cab2))
- add provisioning sanity guard in generated_kotlin_parses by @elcoosp ([95e65d3](https://github.com/elcoosp/flux/commit/95e65d3edd96119ae6e911c95f1d54b1374a95fe))
- surface kotlinc stderr on compile-check failure by @elcoosp ([9ea941e](https://github.com/elcoosp/flux/commit/9ea941e8146198e3b0bfa7fe0b014741c09f717b))
- enable Compose compiler plugin via -Xplugin in kotlinc check by @elcoosp ([5f033d4](https://github.com/elcoosp/flux/commit/5f033d4bf059f11013667f8fee1d3c5160714716))

### fmt
- reorder imports in backend_impl.rs by @elcoosp ([247d5d3](https://github.com/elcoosp/flux/commit/247d5d3f818cc84fd8f47a3e0018ff5d47da73e8))
**Full Changelog**: https://github.com/elcoosp/flux/releases/tag/v0.1.0
