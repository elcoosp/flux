# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
## [0.1.0] - 2026-11-11

### Bug Fixes
- empty right trace renders; use file line for length mismatch by @elcoosp ([7229c2b](https://github.com/elcoosp/flux/commit/7229c2b5cbad32ff74b23fbe186b48ed4128ea30))
- close F62 test body before F68 (F68 follow-up) by @elcoosp ([c5d67f5](https://github.com/elcoosp/flux/commit/c5d67f596a2f6876e4061bf78f4aa245e40400c2))
- dev+release render_expr handle Call (F65 follow-up) by @elcoosp ([067195a](https://github.com/elcoosp/flux/commit/067195a8d503ab44da982d7c7c25318eb627462d))
- close F58 test body (F62 follow-up) by @elcoosp ([2e51ecc](https://github.com/elcoosp/flux/commit/2e51eccb334968c02d85285dfd04958df0a66a12))
- close picker_date_picker test body (F58 follow-up) by @elcoosp ([60798e0](https://github.com/elcoosp/flux/commit/60798e05fc639700cf04acf4c333f1ffa6f0cb30))
- remove stray trailing brace; F25-F35 tests now actually run by @elcoosp ([f21095a](https://github.com/elcoosp/flux/commit/f21095a965d2cf55417438c4b0423aa1e0d38fac))
- normalize Picker/DatePicker release names (round-86, F35) by @elcoosp ([621fe12](https://github.com/elcoosp/flux/commit/621fe125e80282c9c0e5473c013c92e76a479a7e))
- normalize Checkbox/Toggle and TextArea release names (round-86, F32) by @elcoosp ([0c66580](https://github.com/elcoosp/flux/commit/0c665803f35af1c4089f20b04d72c8895e2a4578))
- restore placeholder collapse in composite walker (round-86, F24 follow-up) by @elcoosp ([3951b13](https://github.com/elcoosp/flux/commit/3951b13f30265d875abd5c5269ffef521030ad94))
- walk composite expressions in canonicalize_release_expr (round-86, F24 follow-up) by @elcoosp ([879c259](https://github.com/elcoosp/flux/commit/879c25985c47400409a8cac0e4a201b1273273d3))
- unescape release-side string literals (round-86, F24) by @elcoosp ([c9347cc](https://github.com/elcoosp/flux/commit/c9347ccfbbedfe6e6d20a662e21405b61fbc40ee))
- drop alignment/color/label from RECOGNISED (round-86, F22) by @elcoosp ([f616383](https://github.com/elcoosp/flux/commit/f616383d5b00b9045135fa6f229a7253136ae612))
- strict RFC 8259 number parser + \b/\f escapes (F23 cont.) by @elcoosp ([969493c](https://github.com/elcoosp/flux/commit/969493c93fa4fe6563da4d3b723f891eafb24581))
- JSON canonicalizer escapes control chars + key backslashes (F23) by @elcoosp ([0cbc4f2](https://github.com/elcoosp/flux/commit/0cbc4f27ca3388b1c1240aa1f0ffd54e09ce6b6f))
- handle UnsupportedVariant in WireErrorKind; clean comment by @elcoosp ([fb78cab](https://github.com/elcoosp/flux/commit/fb78cab0530bb34bfa9fe87ae322a5663e7bf186))
- guard-arm label keeps the condition (mirror codegen-core) by @elcoosp ([94547b6](https://github.com/elcoosp/flux/commit/94547b6a4b839136b084a9d6b310a9437e4ea2b9))
- propagate parse_body error from parse_items (no end=0 hang) by @elcoosp ([28093a5](https://github.com/elcoosp/flux/commit/28093a5bba9d0dbdfc7120e04c768792e1b7dcc2))
- propagate parse_body error from parse_for_each (no end=0 hang) by @elcoosp ([ffeefbb](https://github.com/elcoosp/flux/commit/ffeefbbe1f6f9b1fdb2fa5afd30b9e4b3cd8b446))
- cover WireError::DuplicateHandlerId (F5 follow-up) by @elcoosp ([1b7c8e4](https://github.com/elcoosp/flux/commit/1b7c8e4d5971f17938fc43c2e547eae63dd82d2e))
- report the extra frame's file line; guard empty-trace render by @elcoosp ([407c6ff](https://github.com/elcoosp/flux/commit/407c6ff49b196e5c06a817ff795a8018170ebd15))
- Swift interpolation \(x) canonicalizes to {x}, matching Kotlin by @elcoosp ([71a46b1](https://github.com/elcoosp/flux/commit/71a46b1509c36442ec18ccdd053ac8d2c4b356b0))
- norm_cond must not terminate a string on an escaped quote by @elcoosp ([043264f](https://github.com/elcoosp/flux/commit/043264f4ca76afba7f7e0f5c6aad61d2bdd0cc19))
- canonicalize Kotlin null key to Swift \.self by @elcoosp ([81d2d3e](https://github.com/elcoosp/flux/commit/81d2d3ef8991ab806fd156eef835c253a8e81b5a))
- recognizer skips generic clause before fn name by @elcoosp ([4b59757](https://github.com/elcoosp/flux/commit/4b597574eb020d9d1f5f2d409f2d2c7734ffaa8d))
- tokenize <> as delimiters for generic clauses by @elcoosp ([edf8f0a](https://github.com/elcoosp/flux/commit/edf8f0ac3e580d18b6e9bca1f44d468c19e5ed00))
- ReferenceDecoder kind-dispatches so valid non-Error/Hello frames are Accepted by @elcoosp ([6f6ef1c](https://github.com/elcoosp/flux/commit/6f6ef1cc55fb53608baabf94c2495f82612bc077))
- binop_symbol fallback matches codegen <unknown-binop> sentinel by @elcoosp ([a18ad22](https://github.com/elcoosp/flux/commit/a18ad22ac4d665cf6926aff762ab24f80728e01a))
- Divergence::render walks frame indices, not file line numbers by @elcoosp ([579a53b](https://github.com/elcoosp/flux/commit/579a53b146c5e06d592749a07f730206d63c2f13))
- trace JSON parser decodes UTF-8 correctly, handles \uXXXX, no off-by-one by @elcoosp ([c6a5764](https://github.com/elcoosp/flux/commit/c6a576466383140fa75ff4a37b88c24c22e46913))
- tokenizer tracks backslash escapes so strings with \" don't break the brace map by @elcoosp ([9726c18](https://github.com/elcoosp/flux/commit/9726c1886b0e8af99aee12c17b7e504657b628af))
- validate Delta handler_count against decoded closure count (T-316.6, FLUX-005) by @elcoosp ([2ff7e2c](https://github.com/elcoosp/flux/commit/2ff7e2cc37daf5ab5a88c41243a181f269711096))
- Router capability renamed to RouterNav (T-604.17) by @elcoosp ([3c594a0](https://github.com/elcoosp/flux/commit/3c594a0497c3a618844253a9db988eb6752e5f8e))
- encode Alignment as positional record {0: Int} (0=start,1=center,2=end) by @elcoosp ([cdd8760](https://github.com/elcoosp/flux/commit/cdd8760fec68fb30a67aaacffea0a81560c7d20e))
- land native_kit_parity.rs — the adapter-drift gate FLUX-078 claimed (T-504) by @elcoosp ([d1c356b](https://github.com/elcoosp/flux/commit/d1c356b893371c1ecfbee0f58906ce3b0968bcbe))
- parity harness compares view props + fixes equivalence holes (T-501,T-502) by @elcoosp ([4ec8f81](https://github.com/elcoosp/flux/commit/4ec8f81a05074c63b1e2c13836169001448f8159))
- parity harness compares view props, not just structure (T-501) by @elcoosp ([d0a08ef](https://github.com/elcoosp/flux/commit/d0a08efe0370de58baaa9159c1409dfb846a2e6b))
- correct Int→Float literal in test fixtures by @elcoosp ([8633db6](https://github.com/elcoosp/flux/commit/8633db60a2b1071d5f807bcce354a5bdba421519))
- Router.navigate renders to route state assignment by @elcoosp ([93aebbf](https://github.com/elcoosp/flux/commit/93aebbf1d65d82c5cea3b932de72f1306755b078))
- FLUX-042/043 dev/release tree alignment by @elcoosp ([166aea1](https://github.com/elcoosp/flux/commit/166aea1bd55148145f99c27e1d112f7f066b3727))
- dev reducer reads Screen route + else branches; accept 6 baselines by @elcoosp ([6825e2d](https://github.com/elcoosp/flux/commit/6825e2daecb2e0078c8f25581f6434e49e7c451f))
- emit Button onClick handler body and text label by @elcoosp ([8e0cbff](https://github.com/elcoosp/flux/commit/8e0cbffdec528f47ef4ddeb9ae56fa70d378079a))

### Documentation
- file F68 (generic fn monomorphic across call sites) (round-114) by @elcoosp ([2990822](https://github.com/elcoosp/flux/commit/29908229ae45d38592ea20debcb1094ba615c587))
- file F62 (nominal record literal init) (round-109) by @elcoosp ([8e6bdd2](https://github.com/elcoosp/flux/commit/8e6bdd27b971a88789f7bfff28aebdf5f8524069))
- file F58 (named-record destructuring binds whole record) (round-103) by @elcoosp ([c0097cc](https://github.com/elcoosp/flux/commit/c0097cc78a38c75e837a799ba7740a599770355e))
- file F52 (generic ADT pattern fields not substituted) (round-96) by @elcoosp ([3faa6d6](https://github.com/elcoosp/flux/commit/3faa6d6c64d618b7a627c464ee49a6c60b6233f9))
- file FLUX-F38 (lowercase null/nil) with ignored regression (round-87) by @elcoosp ([1755735](https://github.com/elcoosp/flux/commit/17557350c4edf347a9822cd2787f5d05d647391b))
- file F33 (Theme flatten) + F34 (Gesture) with ignored regressions (round-86) by @elcoosp ([7b84880](https://github.com/elcoosp/flux/commit/7b848804f214582b15a72966401c00cb8494c968))
- file F33 (Theme flatten) + F34 (Gesture) with ignored regressions (round-86) by @elcoosp ([b1f4502](https://github.com/elcoosp/flux/commit/b1f45024cd5e29c59f5d4f60cbc115e826f3585f))
- file FLUX-F31 (subscript silently splits) with ignored regression (round-86) by @elcoosp ([976f5fb](https://github.com/elcoosp/flux/commit/976f5fbeca95712e9a817c797ab6d69e0696e761))
- file FLUX-F30 (trait bounds unenforced) with ignored regressions (round-86) by @elcoosp ([aac5d89](https://github.com/elcoosp/flux/commit/aac5d89fe1ab734758035b2783b44b1ef1c11441))
- file FLUX-F29 (assign to non-signal binding) with ignored regression (round-86) by @elcoosp ([17e92f7](https://github.com/elcoosp/flux/commit/17e92f70118ea12fdbfc14f7de834087bb216cda))
- file FLUX-F28 (match-arm bindings not in scope) with ignored regression (round-86) by @elcoosp ([96f36d5](https://github.com/elcoosp/flux/commit/96f36d5a891127f11296ff5630bf13f6795dab79))
- file FLUX-F26 (batch/untrack in handler) with ignored regression (round-86) by @elcoosp ([fdc8cde](https://github.com/elcoosp/flux/commit/fdc8cde6028817ed584aa2cd12b0ef941209f40d))
- file FLUX-F25 (match recognizer gap) with ignored regression (round-86) by @elcoosp ([2fdd7a8](https://github.com/elcoosp/flux/commit/2fdd7a87fb1ff8f0b42ed7d17b440f96f8e11c3c))
- FLUX-056 ScrollView primitive landed; benchmark leg still blocked by @elcoosp ([fdccdb7](https://github.com/elcoosp/flux/commit/fdccdb7d974b9138570ea941bd8a4431c0cb9c5b))

### Features
- T-402 Kotlin prelude imports + Compose-native Animate + parity recognizer (Phase 4) by @elcoosp ([54311fb](https://github.com/elcoosp/flux/commit/54311fb40e88295bff72ad8e07d4468ad1f98dee))
- native NavigationStack/navigationDestination for Swift and NavController for Kotlin by @elcoosp ([7f834a2](https://github.com/elcoosp/flux/commit/7f834a21ce931d1a4246cd0cd4c2c9dfe85fd13a))
- FLUX-082 LANE-PARITY — flux-parity for persistence, cache eviction, error frames by @elcoosp ([e534888](https://github.com/elcoosp/flux/commit/e534888bd4d77df337a23e33fc81520c3d779325))
- FLUX-078 codegen-driven native glue + parity guards by @elcoosp ([bc27967](https://github.com/elcoosp/flux/commit/bc279675a53a301bb195e76b9d26bbb217e1b9b0))
- FLUX-037/038/042 layout, overlay, and animation primitives by @elcoosp ([e82311e](https://github.com/elcoosp/flux/commit/e82311e308a9784ebed739ad8deb930051e3c062))
- FLUX-038 Modal/Sheet/Dialog overlay container primitives by @elcoosp ([484af6c](https://github.com/elcoosp/flux/commit/484af6c294d3b3bc2b934a81121f0e39d2f070d6))
- headless .flux app-test harness (FLUX-034) by @elcoosp ([542c65c](https://github.com/elcoosp/flux/commit/542c65cb8d16e54b3befd63c37b082566d62e01f))
- deterministic JSON structural parity (Phase 4, roadmap) by @elcoosp ([792cf73](https://github.com/elcoosp/flux/commit/792cf73e31bcc5db4cf436a2d9cb633a63fb090d))
- fully shape the six previously-bare primitives (ADR-0047) by @elcoosp ([12f2d97](https://github.com/elcoosp/flux/commit/12f2d97e0cffb803cc3947d2fcdaddc6d59b5b94))
- replace pest with hand-written dream-syntax lexer + recursive-descent parser by @elcoosp ([d7fbb72](https://github.com/elcoosp/flux/commit/d7fbb72c5e809ae395c499bc19ff6462f63faea9))
- add router navigation example + parity test for route prop by @elcoosp ([609d10e](https://github.com/elcoosp/flux/commit/609d10e0d57de5731b80f311aef333b39ebcb248))
- implement T16 trace-diff tooling + golden corpus (FLUX-023) by @elcoosp ([14595c3](https://github.com/elcoosp/flux/commit/14595c3d09f145720cf457fdd645dfa6e79ccc17))
- implement flux-parity crate (FLUX-023) with dev-AST + release codegen structural parity by @elcoosp ([8a129a8](https://github.com/elcoosp/flux/commit/8a129a80c27dc6d17d29179d4f93e613d79c0744))

### Miscellaneous
- workspace clean under --all-targets -D warnings (F10 prerequisite) by @elcoosp ([81ce6ce](https://github.com/elcoosp/flux/commit/81ce6cebb7fa7a873b4d45ee77a000aec2d7da8c))
- rustfmt clean-up after F7/F8/F10 patches by @elcoosp ([e41c06c](https://github.com/elcoosp/flux/commit/e41c06c54d970ae41994673f97975086357c23ea))
- map WireError::LengthExceedsU32 to MalformedBytecode by @elcoosp ([c20c7f5](https://github.com/elcoosp/flux/commit/c20c7f5d6aee2157f26ff907ba3ba0d5e4f9c744))
- handle WireError::LengthExceedsU16 in ReferenceDecoder mapping by @elcoosp ([4eaa8de](https://github.com/elcoosp/flux/commit/4eaa8de7c46a33f2e323b314a662d33bd016d8c9))
- add flux-vm-ref and flux-parity crates for contract v2 by @elcoosp ([b8be8fa](https://github.com/elcoosp/flux/commit/b8be8faca302ce53207cab2307bb9c6acf473374))

### Refactor
- use canonicalize_expr; drop parse_for_each error propagation by @elcoosp ([1783312](https://github.com/elcoosp/flux/commit/17833123f93fd75c4ff846664bbfade2349a2425))
- use canonicalize_expr; drop parse_items error propagation by @elcoosp ([136b5b8](https://github.com/elcoosp/flux/commit/136b5b81e11f023a16c933d7e6affc84142f3be4))
- drop DuplicateHandlerId/UnsupportedVariant mapping by @elcoosp ([4fffcd5](https://github.com/elcoosp/flux/commit/4fffcd5d5679967a696e57b13ae841b9cb958a78))
- delegate view-name/container tables to codegen-core (single source of truth) by @elcoosp ([b40f625](https://github.com/elcoosp/flux/commit/b40f62527f7d73b31cb5d2f484b6c6460efd5941))
- rename primitives/props/caps to RN/Expo surface (FLUX-038) by @elcoosp ([1f1fa4b](https://github.com/elcoosp/flux/commit/1f1fa4b7d44fdcc2467521f110a6519564a48fd8))
- unified data-driven release codegen via flux-codegen-core (ADR-0047) by @elcoosp ([8f5c5d5](https://github.com/elcoosp/flux/commit/8f5c5d560b44805e5894b50b9bcce9328dc75e79))

### Revert
- limited string escaping in canonicalizer (drop control-char handling) by @elcoosp ([f87cf4a](https://github.com/elcoosp/flux/commit/f87cf4ad52cd0f25b827192a571ed3f832d9f345))
- drop guard-cond label; restore RECOGNISED list (revert F22) by @elcoosp ([268fa98](https://github.com/elcoosp/flux/commit/268fa9842ead331432e15032f4268fe4c29978f9))
- drop escaped-quote scan in norm_cond by @elcoosp ([4c7e2e1](https://github.com/elcoosp/flux/commit/4c7e2e1e610d5994c2be41efb6af292b50a694cf))
- drop canonicalize_release_expr and Call render (revert F24/F65) by @elcoosp ([f706c3c](https://github.com/elcoosp/flux/commit/f706c3c759888825a8d99a60c487c38190f58ee4))

### Testing
- update platform snapshot (Call -> placeholder) by @elcoosp ([b7cc0b3](https://github.com/elcoosp/flux/commit/b7cc0b3aca4aa6a0b211de08f4837569480ece21))
- restore label/color prop expectations (revert F22) by @elcoosp ([c0cfd62](https://github.com/elcoosp/flux/commit/c0cfd62d22b8705bf2e61404a9ceeeed8f3789d6))
- remove obsolete F22/F24/F25/F26 parity tests by @elcoosp ([0741c22](https://github.com/elcoosp/flux/commit/0741c22396211a9e7f5dd26bb7606ac7d37dda2b))
- F30 bound-violated gap asserts the actual (lowering) error by @elcoosp ([f271e35](https://github.com/elcoosp/flux/commit/f271e356d288f65d48e2cb9d04540442a78880f7))
- regenerate b34_lifecycle snapshot by @elcoosp ([a48fe5d](https://github.com/elcoosp/flux/commit/a48fe5d8be4a475c19f7a893d77e5bd70b879304))
- pin FLUX-056 ScrollView dev/release mapping by @elcoosp ([ef30853](https://github.com/elcoosp/flux/commit/ef3085354841e0fd831c8cfc4cb04561689c654b))
- accept codegen/parity snapshot drift from FLUX-072 data-driven surface by @elcoosp ([7faca34](https://github.com/elcoosp/flux/commit/7faca3410ced3f975d9fb4db817a0275a00e8c3e))
- pin B.3 lowering regression gate (FLUX-063) by @elcoosp ([0eec8ff](https://github.com/elcoosp/flux/commit/0eec8ff3e42fc9ad3805f69bfa587c2ed6a4c193))
- LANE-B parity gate pins positional-Screen route-prop blind spot by @elcoosp ([c625a02](https://github.com/elcoosp/flux/commit/c625a02cea43a718e9b6d3903db4baaf5a7d5d20))
- end-to-end async suspension over the AwaitSuspend/Resume wire by @elcoosp ([4c2d5de](https://github.com/elcoosp/flux/commit/4c2d5de4c4ce1b3d57c757a66e7f92fd25df2a00))
- fail hard on lowering errors, drop LowererUnsupported (5) by @elcoosp ([0eb4591](https://github.com/elcoosp/flux/commit/0eb4591106a7db3919f02bd0a039db116159eef6))
- add insta snapshots for the ten B.3 parity tests by @elcoosp ([4dbde09](https://github.com/elcoosp/flux/commit/4dbde099a55690044df950789b64cc594d7517ba))

### fmt
- reorder imports in props_compare test by @elcoosp ([303e9c1](https://github.com/elcoosp/flux/commit/303e9c1144b9c943da54b224c0f4e75631f4405f))
- reformat while condition in collect_value by @elcoosp ([6188f49](https://github.com/elcoosp/flux/commit/6188f49701234eb77ec5034fd93c25feb8979b37))
- reformat extract_swift_props call in kotlin_views.rs by @elcoosp ([d4234fb](https://github.com/elcoosp/flux/commit/d4234fb2de2f937799baf6a3aa6efcb3eab3ba21))
- reformat pattern match and closure in equivalence.rs by @elcoosp ([0be0645](https://github.com/elcoosp/flux/commit/0be0645771f2bb6be2698e3c1bd50adfd3734ba8))

### style
- apply rustfmt to capability_manifest and stdlib_parse test files by @elcoosp ([7c4b881](https://github.com/elcoosp/flux/commit/7c4b88151ce4e3ab87131ef45fbaa753780d578a))
**Full Changelog**: https://github.com/elcoosp/flux/releases/tag/v0.1.0
