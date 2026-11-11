# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
## [0.1.0] - 2026-11-11

### Bug Fixes
- render nested calls in handler bodies (round-111, F65) by @elcoosp ([5223a67](https://github.com/elcoosp/flux/commit/5223a674cb914bba2fc469a428ef88188219830a))
- preserve named-arg labels in handler calls (round-110, F63) + file F64 by @elcoosp ([0e3dfa8](https://github.com/elcoosp/flux/commit/0e3dfa87455642df2c09cc9e1e2dd4e58f391558))
- treat `_` as a wildcard in match over ADTs (round-86, F27) by @elcoosp ([190d6cc](https://github.com/elcoosp/flux/commit/190d6cc64a83475f2068f46373458928654b6906))
- guard arm uses real Swift `where` clause (drops nothing) by @elcoosp ([8dc4bda](https://github.com/elcoosp/flux/commit/8dc4bda9d143c6f0355650d04f25c563a09f9c0f))
- Router route state seeds NavigationPath with initialRouteName by @elcoosp ([70b9816](https://github.com/elcoosp/flux/commit/70b981609f744f4f1a502c20a4f6412be13ace92))
- TextField Binding setter runs the onChange handler (was dropped) by @elcoosp ([7605cf6](https://github.com/elcoosp/flux/commit/7605cf60adc719575f0d324290c9d8708a245d64))
- Toggle emits SwiftUI Toggle(isOn:) without wrapping children by @elcoosp ([4e6ece8](https://github.com/elcoosp/flux/commit/4e6ece87c2439dafb20604b35c67d68f3784643b))
- emit real SwiftUI Binding shapes for Switch/Checkbox/Slider/TextArea by @elcoosp ([1b8276d](https://github.com/elcoosp/flux/commit/1b8276d897f913ec4b3efc13e800a6cb7bafd02f))
- unknown Animate curve falls back to Animation.default by @elcoosp ([d7fada6](https://github.com/elcoosp/flux/commit/d7fada6c52bb1d73d9e964c94e467bddd8f52298))
- Router capability renamed to RouterNav (T-604.17) by @elcoosp ([3c594a0](https://github.com/elcoosp/flux/commit/3c594a0497c3a618844253a9db988eb6752e5f8e))
- canonical event-verb vocabulary + onClick alias acceptance (T-604.16) by @elcoosp ([3e98b6f](https://github.com/elcoosp/flux/commit/3e98b6f679b0fe9f6da41b432570e62160025fcd))
- T-403.1 Swift Toggle binding + T-403.7 route state Router guard by @elcoosp ([57509dd](https://github.com/elcoosp/flux/commit/57509ddc2f59c5e17d0c97b88808f53a9c2bcf3a))
- Guard pattern handling in Swift emit_match by @elcoosp ([6cca2b1](https://github.com/elcoosp/flux/commit/6cca2b15a4a2aaeed77e57e378fd45d747d37613))
- Guard pattern handling in Swift emit_match by @elcoosp ([6dcbe6a](https://github.com/elcoosp/flux/commit/6dcbe6a2be5ca78e5b9efc16d36c2b61e032d0c6))
- axis-aware spacing in codegen (container_spacing_axis) by @elcoosp ([7351e66](https://github.com/elcoosp/flux/commit/7351e665bdf341307241190628842379b94711dc))
- TextField emits mutable Binding instead of constant (T-403.6) by @elcoosp ([a821d0e](https://github.com/elcoosp/flux/commit/a821d0e413eaa62156a5c121a7b58b7ea6e5602e))
- STR_LEN panic fix + render_await backend hook by @elcoosp ([517871e](https://github.com/elcoosp/flux/commit/517871e79a152f146779dc906d426d0caaede3f9))
- per-backend string escaping in release codegen (T-401) by @elcoosp ([1e5c2e9](https://github.com/elcoosp/flux/commit/1e5c2e9d8ffe00849c8c4704d5d3c683c3afb86d))
- Router.navigate renders to route state assignment by @elcoosp ([93aebbf](https://github.com/elcoosp/flux/commit/93aebbf1d65d82c5cea3b932de72f1306755b078))
- resolve -D warnings in devserver/types/codegen-swift by @elcoosp ([b329330](https://github.com/elcoosp/flux/commit/b329330cb292f0ae7ea7a176d17d8e889854a517))
- emit valid SwiftUI for sum types + match by @elcoosp ([fcbd673](https://github.com/elcoosp/flux/commit/fcbd6738179eac9b08f24616c9337bf4dab4f6b0))
- emit Button onClick handler body and text label by @elcoosp ([8e0cbff](https://github.com/elcoosp/flux/commit/8e0cbffdec528f47ef4ddeb9ae56fa70d378079a))
- key component bridge under DeclTag(3) to match flux-ir lowering by @elcoosp ([e50e518](https://github.com/elcoosp/flux/commit/e50e518a589bfb07a18e7b7de63d3581776e7553))
- migrate node-id bridge to sealed NodeTag (3a unblock) by @elcoosp ([6f752ad](https://github.com/elcoosp/flux/commit/6f752ad3a61431901bd8e384937187e13f12a17f))

### Features
- record hook keeps struct; Map -> [K: V]; drop F27 wildcard special-case by @elcoosp ([c9b8642](https://github.com/elcoosp/flux/commit/c9b8642b7f65b86ea57931a66b5dd5778317d744))
- native NavigationStack/navigationDestination for Swift and NavController for Kotlin by @elcoosp ([7f834a2](https://github.com/elcoosp/flux/commit/7f834a21ce931d1a4246cd0cd4c2c9dfe85fd13a))
- self-contained release builds for counter/todo/router examples by @elcoosp ([ecbcc87](https://github.com/elcoosp/flux/commit/ecbcc87101ba0cb5e4ee0835c6116d4d0b3e0567))
- FLUX-075 thread ClosureRef + error frame through lower/codegen by @elcoosp ([aae0a57](https://github.com/elcoosp/flux/commit/aae0a5720eb3ec783c3719a08cb8484e4c03c340))
- FLUX-042 animation primitive + FLUX-043 design-token theme extension by @elcoosp ([1d3b76f](https://github.com/elcoosp/flux/commit/1d3b76f79c5e0271af42a710b367a03a1cefbadd))
- FLUX-038 Modal/Sheet/Dialog overlay container primitives by @elcoosp ([484af6c](https://github.com/elcoosp/flux/commit/484af6c294d3b3bc2b934a81121f0e39d2f070d6))
- wrap async capability-call handlers in launch/Task (FLUX-064) by @elcoosp ([1a845ec](https://github.com/elcoosp/flux/commit/1a845ecf220f56d29e9ba88deabc1bb31e5ff489))
- emit one specialised native type per generic instantiation (Phase 1c) by @elcoosp ([5cea19d](https://github.com/elcoosp/flux/commit/5cea19de7bd8ee536ae60db70b2beb12685ace4d))
- fully shape the six previously-bare primitives (ADR-0047) by @elcoosp ([12f2d97](https://github.com/elcoosp/flux/commit/12f2d97e0cffb803cc3947d2fcdaddc6d59b5b94))
- replace pest with hand-written dream-syntax lexer + recursive-descent parser by @elcoosp ([d7fbb72](https://github.com/elcoosp/flux/commit/d7fbb72c5e809ae395c499bc19ff6462f63faea9))
- generate idiomatic SwiftUI from lowered Flux (FLUX-020) by @elcoosp ([a3b9eb5](https://github.com/elcoosp/flux/commit/a3b9eb5a74fd559de1cc4b8e8585feb617045dcf))
- cargo workspace, rust toolchain and flux-syntax crate by @elcoosp ([af901dd](https://github.com/elcoosp/flux/commit/af901dddd882e467516c8961c4bfc70b9ca076fa))

### Miscellaneous
- rustfmt clean-up after F7/F8/F10 patches by @elcoosp ([e41c06c](https://github.com/elcoosp/flux/commit/e41c06c54d970ae41994673f97975086357c23ea))
- derive Debug for Swift backend marker by @elcoosp ([119b576](https://github.com/elcoosp/flux/commit/119b576651942c0cf81473261be7bd8adc6ab39c))
- fmt by @elcoosp ([93eec77](https://github.com/elcoosp/flux/commit/93eec7748458b3a1ab020cfe41c10d5d339b343c))
- add flux-vm-ref and flux-parity crates for contract v2 by @elcoosp ([b8be8fa](https://github.com/elcoosp/flux/commit/b8be8faca302ce53207cab2307bb9c6acf473374))
- pre-wire dev-dependencies and criterion bench targets by @elcoosp ([a4df2a8](https://github.com/elcoosp/flux/commit/a4df2a8a9afef4d968a77dbdf62505f98f75effd))

### Performance
- add release codegen emit bench by @elcoosp ([a0eab67](https://github.com/elcoosp/flux/commit/a0eab670f9532c2ec86be4849b406cdfccbd4138))
- replace per-line indent repeat() with precomputed table by @elcoosp ([55a24f0](https://github.com/elcoosp/flux/commit/55a24f09b5ac30982cc27c9869517546dfdca607))

### Refactor
- rename primitives/props/caps to RN/Expo surface (FLUX-038) by @elcoosp ([1f1fa4b](https://github.com/elcoosp/flux/commit/1f1fa4b7d44fdcc2467521f110a6519564a48fd8))
- unified data-driven release codegen via flux-codegen-core (ADR-0047) by @elcoosp ([8f5c5d5](https://github.com/elcoosp/flux/commit/8f5c5d560b44805e5894b50b9bcce9328dc75e79))

### Testing
- shared tempdir; add record/option/map tests by @elcoosp ([d73c4da](https://github.com/elcoosp/flux/commit/d73c4dac8457058a13bd0c5af2d59284c0ab85a0))
- remove obsolete F65 nested-call test by @elcoosp ([ddc5b0a](https://github.com/elcoosp/flux/commit/ddc5b0a6e6c56534a8a7a718579fa847a2b7bbf5))
- remove obsolete F63/F64 handler named-arg tests by @elcoosp ([453b8e8](https://github.com/elcoosp/flux/commit/453b8e855b37b5018bc517735ebeba6c1ba13072))
- remove obsolete round-34 handler-call test by @elcoosp ([4ba6012](https://github.com/elcoosp/flux/commit/4ba6012fc69e6b03ed728dfbc0281027dbf21bff))
- add adversarial text-emission tests by @elcoosp ([b799560](https://github.com/elcoosp/flux/commit/b799560f6eec0b4e968c6d7127fd17e77a18d585))
- add tests adversarial by @elcoosp ([5587d8d](https://github.com/elcoosp/flux/commit/5587d8d1c99327d1e2ac7d66afea2d9d2f3ba1b2))
- awaited handler renders await expr (F19 regression); close F19 by @elcoosp ([99ce300](https://github.com/elcoosp/flux/commit/99ce3002d8ad1bf4acdad085b42042771fca6553))
- per-pid scratch dir for swiftc (concurrent-run race) by @elcoosp ([e9b977f](https://github.com/elcoosp/flux/commit/e9b977ff1d9fc6e92c1a6d079e455050f4272764))
- capability-call handler survives; docs(issues): FLUX-F19 by @elcoosp ([4fb43f5](https://github.com/elcoosp/flux/commit/4fb43f5d2aeafac76640920b25098ec5b9bda31e))
- regenerate b3_10_generics snapshot for corrected output by @elcoosp ([78bc988](https://github.com/elcoosp/flux/commit/78bc988a34caaa3f456e59e66170603de9ad1ea4))
- regenerate b3_7_optional snapshot for corrected output by @elcoosp ([5ba2a26](https://github.com/elcoosp/flux/commit/5ba2a262c487efe0fa7d9269657b60de536cf236))
- regenerate b3_6_fetch snapshot for corrected output by @elcoosp ([4e1db92](https://github.com/elcoosp/flux/commit/4e1db9264ba24c99654d39d92dba13c62fc7fdef))
- type-check generated Swift (not just parse) + valid fixtures by @elcoosp ([87e5ca0](https://github.com/elcoosp/flux/commit/87e5ca08624ddf385852ba87712a031b01f5bccd))
- accept B.3.10 generics snapshot with <T> clause restored by @elcoosp ([554441a](https://github.com/elcoosp/flux/commit/554441aa19a5d0b37dc5842486fe56115faa5130))
- router_navigate assertion uses raw string for seeded path by @elcoosp ([17b53e3](https://github.com/elcoosp/flux/commit/17b53e305d44d41aff2023a7eb9253d3abe27a19))
- accept seeded NavigationPath in b3.4_router snapshot by @elcoosp ([d312c9d](https://github.com/elcoosp/flux/commit/d312c9df0dac15472ea50f3c3b1c9e2cdfa7a343))
- add T-401 escaping round-trip tests for Swift and Kotlin backends by @elcoosp ([1fee310](https://github.com/elcoosp/flux/commit/1fee3105bf6a73f81b52c198d82304f12f1492d3))
- accept codegen/parity snapshot drift from FLUX-072 data-driven surface by @elcoosp ([7faca34](https://github.com/elcoosp/flux/commit/7faca3410ced3f975d9fb4db817a0275a00e8c3e))

### fmt
- add trailing newline to codegen.rs by @elcoosp ([3846718](https://github.com/elcoosp/flux/commit/38467185852f461ca159c7f6f4d013e9db50680e))
- reorder imports and reformat TextField Binding by @elcoosp ([c4e0bf8](https://github.com/elcoosp/flux/commit/c4e0bf89cdcf3708168f953263a0f9e38729e8e6))
**Full Changelog**: https://github.com/elcoosp/flux/releases/tag/v0.1.0
