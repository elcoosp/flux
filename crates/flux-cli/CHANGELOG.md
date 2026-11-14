# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
## [0.1.0] - 2026-11-14

### Bug Fixes
- report stdlib parse-check ok only when script exists (revert F49) by @elcoosp ([be10f4f](https://github.com/elcoosp/flux/commit/be10f4f6478dba2e923e1b63b847a3b76c029c04))
- hoist F69 test to module level (F69 follow-up) by @elcoosp ([991a57c](https://github.com/elcoosp/flux/commit/991a57c29577ecad5862f4e08c4107386c022424))
- skip SwiftPM .build dir (round-115, F69) by @elcoosp ([74aab70](https://github.com/elcoosp/flux/commit/74aab70e8066ed897d79e908cc0eab8979234777))
- stdlib parse-check reports [missing] when script absent (round-93, F49) by @elcoosp ([2ef7ada](https://github.com/elcoosp/flux/commit/2ef7ada5c6acc6cca56fffb9ecb827905b5b5afb))
- scaffold uses Flux interpolation {count}, not Kotlin ${count} (round-88, F42) by @elcoosp ([d22e297](https://github.com/elcoosp/flux/commit/d22e297987af4fe872e464f0dcc40fa82759212b))
- revert scope creep; honor is_test_file in file/fn-length (F22 cont.) by @elcoosp ([fb13cdc](https://github.com/elcoosp/flux/commit/fb13cdc9f4eda05a24a93df111c82ac0fdfcf09a))
- scanner ignores comments + string literals (F22) by @elcoosp ([f888aac](https://github.com/elcoosp/flux/commit/f888aacf017f56dd360f0804d6066293301b12eb))
- validate project name, use {{PROJECT_NAME}} placeholder (F8) by @elcoosp ([b647804](https://github.com/elcoosp/flux/commit/b6478040830a0a2b494b12386ad919bb39cfc18b))
- flux init interpolates the project name into flux.toml by @elcoosp ([7cabd1f](https://github.com/elcoosp/flux/commit/7cabd1f297e372ac4a5f6aa620ab541d92b2d719))
- derive file ids from fnv1a32 (stable across toolchains) by @elcoosp ([ba09d60](https://github.com/elcoosp/flux/commit/ba09d60356b53a495b43270efda17326bda2a238))
- skip adb daemon banner so it is not reported as a device by @elcoosp ([9c71040](https://github.com/elcoosp/flux/commit/9c71040b2652d67400997ecbf0a9196e6464e9a5))
- skip symlinks and noise dirs to avoid loops and .git walks by @elcoosp ([7a8cc00](https://github.com/elcoosp/flux/commit/7a8cc00938f75c0cd70ad76c7e4c7b79bbb25d87))
- drift print no longer duplicates the approved ceiling by @elcoosp ([4de6ca8](https://github.com/elcoosp/flux/commit/4de6ca84083b5b04b0add6133abab5f0b43f7e4c))
- fn-length check fires inside impl/class bodies (track fn base depth) by @elcoosp ([06d1a95](https://github.com/elcoosp/flux/commit/06d1a9593e3d78bed650657083a11f8ab45c40ad))
- flux lsp uses --no-types (negatable) so the fast path is reachable by @elcoosp ([0d0ab1d](https://github.com/elcoosp/flux/commit/0d0ab1d7c55668c17977e4f6f16adba7836314bd))
- eliminate unwrap/expect in tests to satisfy FLUX-087 gate by @elcoosp ([af01251](https://github.com/elcoosp/flux/commit/af01251e51dd538295bd5fe9417d3815bba745a7))
- per-source output filenames, deterministic entry point, hard error on unreadable source by @elcoosp ([0ea8ed5](https://github.com/elcoosp/flux/commit/0ea8ed5c8c8d1e53c1b7d55898b370893eced996))
- use io::Error::new instead of anyhow in source collection by @elcoosp ([2748977](https://github.com/elcoosp/flux/commit/2748977399ee4b4c7924ee98cebc27a0c5d1df8a))
- Android output always named MainActivity.kt by @elcoosp ([7baf1e1](https://github.com/elcoosp/flux/commit/7baf1e10c2c180461fc90f8ffead06fae5c37102))
- per-source output filenames, deterministic entry point, hard error on unreadable source (T-404) by @elcoosp ([f0c8f22](https://github.com/elcoosp/flux/commit/f0c8f223e0a7dc40af21e9882db583c38e432b2c))
- close toolchain stub before spawn to avoid ETXTBSY in build tests by @elcoosp ([cf28517](https://github.com/elcoosp/flux/commit/cf285173294b15afae74cb76a5673ab6304252bd))
- gate release-leaking debug logs behind #if DEBUG and repair router test by @elcoosp ([d1ffbd8](https://github.com/elcoosp/flux/commit/d1ffbd8dea047e132293b7e8d18cb9443a660070))
- high-contrast tracing palette so flux dev logs are readable on blue terminals by @elcoosp ([86f6c7e](https://github.com/elcoosp/flux/commit/86f6c7e9b43047eec53207dff506b09694526d7c))

### Documentation
- point documentation field to Vercel site by @elcoosp ([6a49f5d](https://github.com/elcoosp/flux/commit/6a49f5db7cf7eb3925c36901ef9dd752bcb20429))
- add Vercel documentation link to Cargo.toml by @elcoosp ([d0c18de](https://github.com/elcoosp/flux/commit/d0c18de8f51caf6f1a9feb08dca2d06ae7088a63))
- enforce SKILL.md rules (jsDelivr logos, absolute links, remove license sections) by @elcoosp ([c4ff690](https://github.com/elcoosp/flux/commit/c4ff690b590b225068b8703e587c63da7c74cd56))
- F48 doctor mod-tests prefix is over-broad (round-92) by @elcoosp ([2512348](https://github.com/elcoosp/flux/commit/2512348e825829e87ea1ecfb77031c4df2d4a396))
- F47 build output name collisions with regression tests (round-91) by @elcoosp ([46c2d87](https://github.com/elcoosp/flux/commit/46c2d87574532110d0e23ced0f289f15e2172e97))
- use style=for-the-badge for all badges by @elcoosp ([5dadc45](https://github.com/elcoosp/flux/commit/5dadc45f8b61c02251855933b46a4c80730d8289))
- sync badges to flat style + refresh stale facts (38 ADRs, 633 commits, 51k LOC, 28 stdlib, 12 CI, flux build invokes toolchain, add flux fmt) by @elcoosp ([0027289](https://github.com/elcoosp/flux/commit/00272898e684b94ee6cf6697a2891fb64ea5dccc))
- strip merge-guard references from AGENTS/README/MANIFEST_REQUESTS/CHANGELOG/roadmap by @elcoosp ([5399a11](https://github.com/elcoosp/flux/commit/5399a110ce7937cf5baa799a6a6e0110f9b4b2eb))
- rectify README facts against source (crates, ADRs, node-id hash, CLI, CI) by @elcoosp ([d5fa330](https://github.com/elcoosp/flux/commit/d5fa330c417f68c980532530028a7d1cf659d4c0))
- ground README in actual codebase — nightly toolchain, 15 crates, hand-written parser by @elcoosp ([f9b9c59](https://github.com/elcoosp/flux/commit/f9b9c59e0ee94f7714a5b710961a7870a3b842db))
- add grounded README with CI and metric badges by @elcoosp ([3012103](https://github.com/elcoosp/flux/commit/30121038ddfe0f32e8946e687e2e2563ccf6b927))

### Features
- codegen source-map sidecar + crash-reporter resolution; F17 closed by @elcoosp ([06100d3](https://github.com/elcoosp/flux/commit/06100d3d7978bb2b0d00755b095ccef140d9f3c0))
- self-contained release builds for counter/todo/router examples by @elcoosp ([ecbcc87](https://github.com/elcoosp/flux/commit/ecbcc87101ba0cb5e4ee0835c6116d4d0b3e0567))
- flux doctor local checks — dependency drift + AGENTS.md rules (FLUX-091) by @elcoosp ([5d3fa87](https://github.com/elcoosp/flux/commit/5d3fa877bcee8cf4b2c3ec270fe92802edd6c3f6))
- FLUX-078 implement flux fmt formatter + --check by @elcoosp ([aa24e4c](https://github.com/elcoosp/flux/commit/aa24e4c516d7b341477c00679c56d1e97404ec36))
- FLUX-068 wire real native-toolchain invocation into release gate by @elcoosp ([d75115e](https://github.com/elcoosp/flux/commit/d75115ee0c0ca4bfdc19dbaa76c79991249169a7))
- flux doctor environment health check (roadmap §5) by @elcoosp ([0160e24](https://github.com/elcoosp/flux/commit/0160e24a51d3d6e88d83ef387a9d68b19a469d1d))
- add opt-in WebSocket pairing token (flux dev --token) (FLUX-071 token) by @elcoosp ([13bcfcc](https://github.com/elcoosp/flux/commit/13bcfcc914f0c0ddf741dc69ceca91276f848559))
- add `flux lsp <file>` subcommand reusing flux-lsp type diagnostics (FLUX-025) by @elcoosp ([4cdbbb4](https://github.com/elcoosp/flux/commit/4cdbbb4ef1afb7183fec240c954ca78c5ac790e1))
- LANE-E release gate — flux build invokes native toolchain and fails on non-zero by @elcoosp ([6947bec](https://github.com/elcoosp/flux/commit/6947bec16ec4683888abf2f3085e985dd73acbbd))
- replace pest with hand-written dream-syntax lexer + recursive-descent parser by @elcoosp ([d7fbb72](https://github.com/elcoosp/flux/commit/d7fbb72c5e809ae395c499bc19ff6462f63faea9))
- update lib by @elcoosp ([5747e08](https://github.com/elcoosp/flux/commit/5747e08d793799406e45a160dcddaa737eb54cff))
- update dev command by @elcoosp ([9d435a9](https://github.com/elcoosp/flux/commit/9d435a9bc19b2676d207642120f071903ad3bca3))
- implement flux binary with init/dev/build/doc subcommands (FLUX-022) by @elcoosp ([c14b9f5](https://github.com/elcoosp/flux/commit/c14b9f5448febc6a1cd820afb21f6cb9ebabf53b))
- cargo workspace, rust toolchain and flux-syntax crate by @elcoosp ([af901dd](https://github.com/elcoosp/flux/commit/af901dddd882e467516c8961c4bfc70b9ca076fa))

### Miscellaneous
- inject missing metadata and point to root README by @elcoosp ([18eef32](https://github.com/elcoosp/flux/commit/18eef32d8fcc00dd39f2006beee3756e296382f1))
- inject precisely curated keywords for crates.io by @elcoosp ([4d91cc4](https://github.com/elcoosp/flux/commit/4d91cc4b13147eb431ed45db6008257a14dc5e00))
- remove stale main.flux fixture (round-88, F43) by @elcoosp ([085f5c5](https://github.com/elcoosp/flux/commit/085f5c5e617363a38c770aceabc6a6ccfcf14103))
- satisfy clippy::manual_find in build_source_map test by @elcoosp ([90e6df7](https://github.com/elcoosp/flux/commit/90e6df7c63ffcf20ac9412aa0001f1561fcedb2f))
- workspace clean under --all-targets -D warnings (F10 prerequisite) by @elcoosp ([81ce6ce](https://github.com/elcoosp/flux/commit/81ce6cebb7fa7a873b4d45ee77a000aec2d7da8c))
- rustfmt clean-up after F7/F8/F10 patches by @elcoosp ([e41c06c](https://github.com/elcoosp/flux/commit/e41c06c54d970ae41994673f97975086357c23ea))
- add flux-syntax as a direct dependency for stable file ids by @elcoosp ([432314e](https://github.com/elcoosp/flux/commit/432314ef8f1791421415158c1c21f86d3909e6ba))
- size-gate fuzz exclusion + fix false positives by @elcoosp ([f5517c0](https://github.com/elcoosp/flux/commit/f5517c03dce88dcc09ad010d702755140a32c03a))
- add flux-cli -> flux-types path dep, clear manifest requests (FLUX-025) by @elcoosp ([b3305e3](https://github.com/elcoosp/flux/commit/b3305e39087eb475e948023f3045fa1dd0274c5a))
- fmt by @elcoosp ([93eec77](https://github.com/elcoosp/flux/commit/93eec7748458b3a1ab020cfe41c10d5d339b343c))
- add main.flux fixture by @elcoosp ([dc92534](https://github.com/elcoosp/flux/commit/dc9253412aa92124db02a99dbbd3b882ae4a42ed))

### Refactor
- make init private and drop SAMPLE_ENTRY re-export by @elcoosp ([14444ab](https://github.com/elcoosp/flux/commit/14444ab7b37c32894c02d45fb6e306342543d11c))
- simplify doctor panic-API scan; drop .build skip and test-file exemption by @elcoosp ([6083f2e](https://github.com/elcoosp/flux/commit/6083f2ead0fcd723a0747b70a8b8cfdd17d8e0f7))
- drop FLUX-F17 source-map sidecar from flux build by @elcoosp ([40656ba](https://github.com/elcoosp/flux/commit/40656baee1018a2a34034406e4b8457c81abd500))
- consolidate module import on single use directive by @elcoosp ([6cb88b7](https://github.com/elcoosp/flux/commit/6cb88b7ff599dcd9bec99ac0e1fe8d913f88b0f3))

### Revert
- restore ${count} sample and drop project-name validation (revert F8/F42) by @elcoosp ([f80d5a2](https://github.com/elcoosp/flux/commit/f80d5a29eefcc27b6a718797461cfa22ba4de8b8))

### Testing
- remove F8 init name-validation test by @elcoosp ([4840170](https://github.com/elcoosp/flux/commit/484017037a5a165c305da77f310ec9844b7042d8))
- remove F42 init-scaffold compile test by @elcoosp ([e93362a](https://github.com/elcoosp/flux/commit/e93362a133b0db76fa557cd2c3302aaddeb4d1aa))
- remove F17 source-map sidecar integration test by @elcoosp ([761f6b3](https://github.com/elcoosp/flux/commit/761f6b34de0042b02a49641e9c796db7282dcca4))
- remove F22/F48/F49/F69 doctor regression tests by @elcoosp ([0c8f624](https://github.com/elcoosp/flux/commit/0c8f6246008033073d521d5df1a0e2ffd18df0ee))
- add main.flux scaffold fixture (main.flux) by @elcoosp ([1e5e720](https://github.com/elcoosp/flux/commit/1e5e720355913ab9eddd4d082ecf1e114798276c))
- add main.flux scaffold fixture (flux.toml) by @elcoosp ([b2005a6](https://github.com/elcoosp/flux/commit/b2005a61a985717c1d7051e0a167bcae1c15e74c))
- add main.flux scaffold fixture (.fluxignore) by @elcoosp ([ec78403](https://github.com/elcoosp/flux/commit/ec78403ca02e7f68111d2a0f151180bc9ffef1c9))
- add tests adversarial by @elcoosp ([5587d8d](https://github.com/elcoosp/flux/commit/5587d8d1c99327d1e2ac7d66afea2d9d2f3ba1b2))
- match reduced panic-API set (unwrap/expect/panic!) by @elcoosp ([814d60a](https://github.com/elcoosp/flux/commit/814d60ab00fc4887f7d6d7ece8bd35e23b495425))
- flux init rejects unsafe names, writes exact project name (F8) by @elcoosp ([c15cbf0](https://github.com/elcoosp/flux/commit/c15cbf06942ba44acf2ceaf0461c0930473741d8))
- fix integration-test isolation and nightly clippy by @elcoosp ([67af9a1](https://github.com/elcoosp/flux/commit/67af9a1afc62fc8db11d6d97cad66642566d9c92))
- fix CI integration-test isolation and a clippy lint by @elcoosp ([fcacba0](https://github.com/elcoosp/flux/commit/fcacba054a332ecb4f8f969be2828432956761ee))

### fmt
- reformat path join in integration test by @elcoosp ([f884732](https://github.com/elcoosp/flux/commit/f884732850aa40b7174bcc16044f007a3b9c767d))
- reformat skip_while closure in build.rs by @elcoosp ([cc9057a](https://github.com/elcoosp/flux/commit/cc9057a1d2681bf7594b93246cb1526a4b8148c4))
**Full Changelog**: https://github.com/elcoosp/flux/releases/tag/v0.1.0
