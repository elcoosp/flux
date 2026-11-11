# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
## [0.1.0] - 2026-11-11

### Bug Fixes
- route Option/Map through per-backend type hooks by @elcoosp ([6e39ba7](https://github.com/elcoosp/flux/commit/6e39ba79e5a809c30b0072c91308837337f26374))
- render generic-call args; delegate record emission to backend by @elcoosp ([ae67042](https://github.com/elcoosp/flux/commit/ae67042314d0e7426848344c8943676d925fa350))
- dev+release render_expr handle Call (F65 follow-up) by @elcoosp ([067195a](https://github.com/elcoosp/flux/commit/067195a8d503ab44da982d7c7c25318eb627462d))
- render nested calls in handler bodies (round-111, F65) by @elcoosp ([5223a67](https://github.com/elcoosp/flux/commit/5223a674cb914bba2fc469a428ef88188219830a))
- preserve named-arg labels in handler calls (round-110, F63) + file F64 by @elcoosp ([0e3dfa8](https://github.com/elcoosp/flux/commit/0e3dfa87455642df2c09cc9e1e2dd4e58f391558))
- normalize Picker/DatePicker + close F24 test body (round-86, F35) by @elcoosp ([4c805bb](https://github.com/elcoosp/flux/commit/4c805bb57cbd20f0ec0e0d098c58e0e8cf73439d))
- normalize Checkbox/Toggle and TextArea release names (round-86, F32) by @elcoosp ([0c66580](https://github.com/elcoosp/flux/commit/0c665803f35af1c4089f20b04d72c8895e2a4578))
- render bare and awaited calls in handlers (no placeholder) by @elcoosp ([5dc6104](https://github.com/elcoosp/flux/commit/5dc6104c39b361c67ba386d977b9b6e4e036fbd7))
- guard-arm label keeps the condition (parity not lossy) by @elcoosp ([23309f5](https://github.com/elcoosp/flux/commit/23309f53ede7df19ff17d2ac9bc4d9946e60fab8))
- preserve generic bounds in native generic clause (round-15) by @elcoosp ([b4d4722](https://github.com/elcoosp/flux/commit/b4d472213bc3722eea01b6d54ebaddca9be1da17))
- render_args skips unhandled Arg variant (was empty fragment) by @elcoosp ([e72f3e6](https://github.com/elcoosp/flux/commit/e72f3e6e25796984285218ef9d2bbc5af1196610))
- render_binary wildcard uses <unknown-binop>; render_string placeholder for new StrPart by @elcoosp ([6c4bfaf](https://github.com/elcoosp/flux/commit/6c4bfaf2f3cfada3b3fc9ddac74786c8501f669d))
- generic fallback keeps <T>; collect_handler scans onPress first by @elcoosp ([d50b701](https://github.com/elcoosp/flux/commit/d50b701a83d2000144dc8e13974a4e1544617ddf))
- canonicalize_expr handles string literals before placeholder collapse (parity) by @elcoosp ([519aa0c](https://github.com/elcoosp/flux/commit/519aa0cfb28f192594f5812dbeca96855ff3d352))
- binop_symbol fallback uses <unknown-binop> sentinel (drift from parity) by @elcoosp ([89b4a4a](https://github.com/elcoosp/flux/commit/89b4a4afae72ce1a77e7254eee9f8de72dffcbbc))
- KOTLIN_TABLE uses 4-space indent per level (was SWIFT copy) by @elcoosp ([9053003](https://github.com/elcoosp/flux/commit/90530035fc706690e4777294d579df6acb100375))
- route form-family Leaf primitives through Backend::form_control by @elcoosp ([97e0d8f](https://github.com/elcoosp/flux/commit/97e0d8f2278817840ccfacbb9dbe3d8129056ec8))
- use Backend::named_arg in user calls; escape Router startDestination by @elcoosp ([a3aa8f0](https://github.com/elcoosp/flux/commit/a3aa8f0d338b68d2177a88127a78f76ea2736617))
- Router capability renamed to RouterNav (T-604.17) by @elcoosp ([3c594a0](https://github.com/elcoosp/flux/commit/3c594a0497c3a618844253a9db988eb6752e5f8e))
- canonical event-verb vocabulary + onClick alias acceptance (T-604.16) by @elcoosp ([3e98b6f](https://github.com/elcoosp/flux/commit/3e98b6f679b0fe9f6da41b432570e62160025fcd))
- T-403.1 Swift Toggle binding + T-403.7 route state Router guard by @elcoosp ([57509dd](https://github.com/elcoosp/flux/commit/57509ddc2f59c5e17d0c97b88808f53a9c2bcf3a))
- resolve component call names in view tree by @elcoosp ([ea3a93b](https://github.com/elcoosp/flux/commit/ea3a93b93eb0432c09aef199d9ac6b5f461a9155))
- handle component call sites and route resolution by @elcoosp ([52f5be8](https://github.com/elcoosp/flux/commit/52f5be850e9a5ee4c6cb69ae10559e7a4c1c17e8))
- Guard pattern handling in Swift emit_match by @elcoosp ([6cca2b1](https://github.com/elcoosp/flux/commit/6cca2b15a4a2aaeed77e57e378fd45d747d37613))
- axis-aware spacing in codegen (container_spacing_axis) by @elcoosp ([7351e66](https://github.com/elcoosp/flux/commit/7351e665bdf341307241190628842379b94711dc))
- STR_LEN panic fix + render_await backend hook by @elcoosp ([517871e](https://github.com/elcoosp/flux/commit/517871e79a152f146779dc906d426d0caaede3f9))
- Toggle/Switch backend-split (T-403.1) by @elcoosp ([e3f5a14](https://github.com/elcoosp/flux/commit/e3f5a14eb9e7f99a7353741a7169bf08a1933b05))
- per-backend string escaping in release codegen (T-401) by @elcoosp ([1e5c2e9](https://github.com/elcoosp/flux/commit/1e5c2e9d8ffe00849c8c4704d5d3c683c3afb86d))
- Router.navigate renders to route state assignment by @elcoosp ([93aebbf](https://github.com/elcoosp/flux/commit/93aebbf1d65d82c5cea3b932de72f1306755b078))
- declare native_gen module so committed native_kit_parity test compiles by @elcoosp ([1cf527e](https://github.com/elcoosp/flux/commit/1cf527e84f790da94ae7065d6fd3cc6fbf1d23e2))
- sync codegen primitive table with dev VM adapter paths (FLUX-078) by @elcoosp ([39c2488](https://github.com/elcoosp/flux/commit/39c2488913783d8dea9d79d46c548c38e9fa1973))

### Documentation
- correct stale doc comments to match implementation by @elcoosp ([c516350](https://github.com/elcoosp/flux/commit/c51635025a822ccc435fdb7675c452bef730c429))

### Features
- add option_type/map_type/emit_record_type backend hooks by @elcoosp ([7c1ebe1](https://github.com/elcoosp/flux/commit/7c1ebe128561e915d976b94dad18b47855748f05))
- codegen source-map sidecar + crash-reporter resolution; F17 closed by @elcoosp ([06100d3](https://github.com/elcoosp/flux/commit/06100d3d7978bb2b0d00755b095ccef140d9f3c0))
- meta_router_start extracts initialRouteName for state-cell seeding by @elcoosp ([3a182d9](https://github.com/elcoosp/flux/commit/3a182d979f4b3c2edd50226a798fa284f843baed))
- add Backend::form_control hook (default None, legacy fallback) by @elcoosp ([1f32086](https://github.com/elcoosp/flux/commit/1f32086e46ffff8c21daefe4ae21d0084ade7983))
- add Backend::named_arg hook (Swift ':' / Kotlin '=') by @elcoosp ([503d95c](https://github.com/elcoosp/flux/commit/503d95c0c269bb3728fd151263f129472edd33c8))
- T-402 Kotlin prelude imports + Compose-native Animate + parity recognizer (Phase 4) by @elcoosp ([54311fb](https://github.com/elcoosp/flux/commit/54311fb40e88295bff72ad8e07d4468ad1f98dee))
- native NavigationStack/navigationDestination for Swift and NavController for Kotlin by @elcoosp ([7f834a2](https://github.com/elcoosp/flux/commit/7f834a21ce931d1a4246cd0cd4c2c9dfe85fd13a))
- self-contained release builds for counter/todo/router examples by @elcoosp ([ecbcc87](https://github.com/elcoosp/flux/commit/ecbcc87101ba0cb5e4ee0835c6116d4d0b3e0567))
- register ScrollView primitive (FLUX-056, PRD-N) by @elcoosp ([2aae398](https://github.com/elcoosp/flux/commit/2aae398d0ab04d4fe1fb601cab12931a5fde46e9))
- FLUX-078 codegen-driven native glue + parity guards by @elcoosp ([bc27967](https://github.com/elcoosp/flux/commit/bc279675a53a301bb195e76b9d26bbb217e1b9b0))
- FLUX-042 animation primitive + FLUX-043 design-token theme extension by @elcoosp ([1d3b76f](https://github.com/elcoosp/flux/commit/1d3b76f79c5e0271af42a710b367a03a1cefbadd))
- FLUX-038 Modal/Sheet/Dialog overlay container primitives by @elcoosp ([484af6c](https://github.com/elcoosp/flux/commit/484af6c294d3b3bc2b934a81121f0e39d2f070d6))
- register Gesture primitive (FLUX-041, partial) by @elcoosp ([0434908](https://github.com/elcoosp/flux/commit/0434908d6b6438685b677496aef3492b007ebc87))
- register form primitives (FLUX-040, partial) by @elcoosp ([9fc6fc6](https://github.com/elcoosp/flux/commit/9fc6fc6619751fbd48db71a269b5b678bbeb8be4))
- register Stack/Grid/Spacer/SafeArea primitives (FLUX-037) by @elcoosp ([28026bf](https://github.com/elcoosp/flux/commit/28026bfa8dd85e62462e8f980b168a0c5546b026))
- deterministic JSON structural parity (Phase 4, roadmap) by @elcoosp ([792cf73](https://github.com/elcoosp/flux/commit/792cf73e31bcc5db4cf436a2d9cb633a63fb090d))
- emit one specialised native type per generic instantiation (Phase 1c) by @elcoosp ([5cea19d](https://github.com/elcoosp/flux/commit/5cea19de7bd8ee536ae60db70b2beb12685ace4d))
- fully shape the six previously-bare primitives (ADR-0047) by @elcoosp ([12f2d97](https://github.com/elcoosp/flux/commit/12f2d97e0cffb803cc3947d2fcdaddc6d59b5b94))

### Miscellaneous
- add must_use to Bridge::components by @elcoosp ([c703e1a](https://github.com/elcoosp/flux/commit/c703e1a20453cf73ab3a446d9dec37162c1252be))
- drop serde_json dependency (source_map module removed) by @elcoosp ([197a014](https://github.com/elcoosp/flux/commit/197a0144887bc0f06068c23f9460f9e21e8e2021))
- flux-differ + flux-codegen-core zero findings; PROGRESS stale rows (F12, F10 prerequisite) by @elcoosp ([d7fbfc2](https://github.com/elcoosp/flux/commit/d7fbfc2889576703ee77d73a2076f84ef4f53ed8))
- rustfmt clean-up after F7/F8/F10 patches by @elcoosp ([e41c06c](https://github.com/elcoosp/flux/commit/e41c06c54d970ae41994673f97975086357c23ea))
- drop unused RecordField import by @elcoosp ([8cbd569](https://github.com/elcoosp/flux/commit/8cbd569a44ac6a72d274afd1a7045f3488f4980c))

### Performance
- memoize indent_prefix slow path instead of leaking per call by @elcoosp ([8ed6c8f](https://github.com/elcoosp/flux/commit/8ed6c8fe0f0f9dafccbcf46e34f4daf3fb3402c3))
- bench the shared structural view-tree walk by @elcoosp ([8c3a526](https://github.com/elcoosp/flux/commit/8c3a526d480b70e5cf9bab06e5186c7f46f3c645))

### Refactor
- delete source_map module (revert F17) by @elcoosp ([5670899](https://github.com/elcoosp/flux/commit/56708998c42d9d7de2bc3d78c86341a10e9e0faa))
- remove source_map re-export by @elcoosp ([c4d70a7](https://github.com/elcoosp/flux/commit/c4d70a75798971406c7707e34a8a3112e112c52e))
- drop render_call; non-router handler calls fall back to placeholder (revert F63/F65) by @elcoosp ([bdba54a](https://github.com/elcoosp/flux/commit/bdba54a7dd2fa725da79d1121621f65c334e9dba))
- normalize_view_name + is_container become the canonical superset tables by @elcoosp ([16c1338](https://github.com/elcoosp/flux/commit/16c13387ccf34277c7b0f8958a9458975b03846b))
- Bridge::components uses BTreeMap for deterministic iteration order by @elcoosp ([84829d7](https://github.com/elcoosp/flux/commit/84829d77fae4da2e85b001cb91cd6e42a1c43ae7))
- Backend::emit_state_cell takes router_start: Option<&str> by @elcoosp ([d501316](https://github.com/elcoosp/flux/commit/d5013165abb80992b9c91952ade525c097c0f483))
- rename primitives/props/caps to RN/Expo surface (FLUX-038) by @elcoosp ([1f1fa4b](https://github.com/elcoosp/flux/commit/1f1fa4b7d44fdcc2467521f110a6519564a48fd8))
- unified data-driven release codegen via flux-codegen-core (ADR-0047) by @elcoosp ([8f5c5d5](https://github.com/elcoosp/flux/commit/8f5c5d560b44805e5894b50b9bcce9328dc75e79))

### Revert
- drop F32/F35 normalizations and guard-cond pattern label by @elcoosp ([932c6cc](https://github.com/elcoosp/flux/commit/932c6ccee784fe62f53da06e13f69c1a0364af77))

### Testing
- add emit_record_type stubs to mock backends by @elcoosp ([e72d288](https://github.com/elcoosp/flux/commit/e72d2882756836cf05d91ce5aca123dcd13b48b4))
- add adversarial render_string tests (hostile StrPart) by @elcoosp ([312abc6](https://github.com/elcoosp/flux/commit/312abc68d2dea5020286ef63695a4a1fd5d99ddc))
- add tests adversarial by @elcoosp ([5587d8d](https://github.com/elcoosp/flux/commit/5587d8d1c99327d1e2ac7d66afea2d9d2f3ba1b2))
- update stub emit_state_cell signatures to Option<&str> by @elcoosp ([8e09c43](https://github.com/elcoosp/flux/commit/8e09c43ec74d16fcd8c5650c0a649bb7bae29be0))
- add T-401 escaping round-trip tests for Swift and Kotlin backends by @elcoosp ([1fee310](https://github.com/elcoosp/flux/commit/1fee3105bf6a73f81b52c198d82304f12f1492d3))

### fmt
- reorder imports in view_tree.rs by @elcoosp ([f3cf9cc](https://github.com/elcoosp/flux/commit/f3cf9cce52ffec3330b84f9cdeaccc2912021c6c))
- reorder imports in emitter.rs by @elcoosp ([7ff99ce](https://github.com/elcoosp/flux/commit/7ff99ce9a1b546dd36583cb9958f7a78db7af244))
**Full Changelog**: https://github.com/elcoosp/flux/releases/tag/v0.1.0
