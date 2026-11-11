# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
## [0.1.0] - 2026-11-11

### Bug Fixes
- cap host sessions with LRU eviction (was unbounded) by @elcoosp ([daaca11](https://github.com/elcoosp/flux/commit/daaca11a5d7068ae7c45ccc54a456e992fa875e3))
- timeline slider rebuilds its range when timeline_len changes by @elcoosp ([f1f48f1](https://github.com/elcoosp/flux/commit/f1f48f1d665b2752dd14d0dd01d699ff65a28b32))
- keyboard actions request a repaint (state mutations are invisible to gpui) by @elcoosp ([3a7b32d](https://github.com/elcoosp/flux/commit/3a7b32d1b3de1c9af793763ed7b04f72bd0d67a4))
- VM inspector drops dead registers branch and UI-side ENTRY_GAS magic by @elcoosp ([a84d5fd](https://github.com/elcoosp/flux/commit/a84d5fd1b8f1b2a2452f5e87e97100a550131310))
- second VmState construction supplies entry_gas: None by @elcoosp ([7b187c7](https://github.com/elcoosp/flux/commit/7b187c7eb4c631aa54c86722d44870711b13401a))
- shift scrub_index when the timeline evicts oldest event by @elcoosp ([8de1b21](https://github.com/elcoosp/flux/commit/8de1b21c8562d3129b4d8a40b66856e4eadc80d2))
- search box shrinks with pane width by @elcoosp ([a6f1547](https://github.com/elcoosp/flux/commit/a6f1547b9c77c5240d4f3eb690704022f8dffe7a))
- signal values use theme foreground; fold geometry into label cell by @elcoosp ([2fd01f2](https://github.com/elcoosp/flux/commit/2fd01f261e530e50253d5beac3e44777678f2765))
- remove margins from pane headers, add background color to prevent pane overflow by @elcoosp ([6664f8a](https://github.com/elcoosp/flux/commit/6664f8a5193572a0ab9451f730425e5896f18d40))
- flush pane titles + gpui-component icons (FLUX-059/§4) by @elcoosp ([e5242d4](https://github.com/elcoosp/flux/commit/e5242d40b12d3bbe48bcb34125e1ba2bfbe4fd9e))
- VM gas gauge budget + pane visual hierarchy (FLUX-059/§4) by @elcoosp ([f2e1f8b](https://github.com/elcoosp/flux/commit/f2e1f8b215ee0f4956f6ffa43ce245bb92cb7f65))
- populate Component Tree with names + collapse, align host/dev-server wire by @elcoosp ([dfb80cb](https://github.com/elcoosp/flux/commit/dfb80cbf7b050e082ab765ad0bb8ff4fd538afe8))
- render Component Tree with proven kv_row/rows_column helpers + add tree render test (FLUX-039) by @elcoosp ([1ad4d07](https://github.com/elcoosp/flux/commit/1ad4d0795d9bb0bb997c67a0b0dd1f8629566d49))
- hosts emit ViewMutation so Component Tree shows live node graph (FLUX-039) by @elcoosp ([81c0f43](https://github.com/elcoosp/flux/commit/81c0f43e18635f4fc3d525580fc09d8dda28b0d0))
- polish panes with GroupBox frames + kv_row rows; show empty-state; pad awaiting-host (FLUX-039) by @elcoosp ([275c619](https://github.com/elcoosp/flux/commit/275c6191bf33c0f2f022d0dee1f5d3dcf82b5f2d))
- render title bar via gpui-component TitleBar so it clears macOS traffic lights (FLUX-039) by @elcoosp ([2789ef6](https://github.com/elcoosp/flux/commit/2789ef6b26e75fb1f2b115b7580dcb2d61167d5a))
- port flux-devtools-ui to the pinned gpui API (unblock rust-check) by @elcoosp ([2b1e511](https://github.com/elcoosp/flux/commit/2b1e511d9c343a50af3dd0dd42a1ce8c96cd9746))

### Documentation
- sidebar ADR link points at the main repository, not a personal fork by @elcoosp ([575558f](https://github.com/elcoosp/flux/commit/575558f2112b8bba697adc939dfb401d79679dfd))
- correct stale doc comments to match implementation by @elcoosp ([c516350](https://github.com/elcoosp/flux/commit/c51635025a822ccc435fdb7675c452bef730c429))

### Features
- multi-host picker in the title bar (visible on >1 host) by @elcoosp ([f8409eb](https://github.com/elcoosp/flux/commit/f8409eb6bd3dbb24b99c07dba94df5fa89c2ce5b))
- HostKey::label() for the multi-host picker UI by @elcoosp ([863caf6](https://github.com/elcoosp/flux/commit/863caf60238a782e8359d68dcd113ac8ff3e4b7c))
- perf_record_generation counter for view-layer caches by @elcoosp ([dd9304d](https://github.com/elcoosp/flux/commit/dd9304d3e19285678f7aaaa7de80429452ced9cc))
- right-click signal context menu -> Inspect (FLUX-062/§3) by @elcoosp ([d4ae062](https://github.com/elcoosp/flux/commit/d4ae0623509c12c152dc1bf342d828cabcfd94c9))
- log level Popover filter + clear, semantic theme tokens (FLUX-060/§5/§7) by @elcoosp ([cb930a9](https://github.com/elcoosp/flux/commit/cb930a9c5c83eca7f8e92a0ccd96247de7ab2094))
- keyboard shortcuts + pane-visibility toggles (FLUX-062/§3) by @elcoosp ([df79b04](https://github.com/elcoosp/flux/commit/df79b042aebc12e13a05edb66227aa3bf0298a62))
- turn Signals pane into a clickable dependency DAG by @elcoosp ([0b68295](https://github.com/elcoosp/flux/commit/0b68295b364a7c23489684b247ad5777318b005a))
- add debounced name search box to component tree by @elcoosp ([fbe50ed](https://github.com/elcoosp/flux/commit/fbe50eda82aa7407e28a04b3d3977e5a14bb4239))
- VM opcode badge + gas gauge, theme switch, connect spinner/notification, pane spacing by @elcoosp ([7f57e8e](https://github.com/elcoosp/flux/commit/7f57e8e7b5bc2fbd81176ace4cb61550ebd84984))
- replay last host announce on DevTools connect; unit-tested by @elcoosp ([5a69f8c](https://github.com/elcoosp/flux/commit/5a69f8c443f79a37b0b034f55cf97af9c378d287))
- resizable workspace, square GroupBox panes, icon titles, timeline scrubber, DataTable logs/network, row hover by @elcoosp ([78db5b6](https://github.com/elcoosp/flux/commit/78db5b6be4784915ad035d1fc0ae7ad6504c574e))
- DevTools network inspector + restored log viewer (FLUX-060) by @elcoosp ([f09cfba](https://github.com/elcoosp/flux/commit/f09cfba71a39fb6d11246560c5f08ae436398d96))
- render budget-aware flamegraph from MetricRecord (FLUX-059) by @elcoosp ([d23adcc](https://github.com/elcoosp/flux/commit/d23adccc4dd9492ac189d19e39b0f29ca43f2f16))
- FLUX-075 render unified error overlay + FluxError excerpt by @elcoosp ([f649a26](https://github.com/elcoosp/flux/commit/f649a269ee744e94bdd4bb0afba19259b1e5df5c))
- populate Component Tree from live ViewMutation + instant repaint (FLUX-039) by @elcoosp ([257fc84](https://github.com/elcoosp/flux/commit/257fc84ab3a999e5237d24ec42eb6550d9144bd3))
- render Component Tree as indented hierarchy with parent links (FLUX-039) by @elcoosp ([70e0060](https://github.com/elcoosp/flux/commit/70e0060a5541037f40e1c467fc23d8e214b4cfc2))
- revamp DevTools UI with gpui-component + show host identity (FLUX-039) by @elcoosp ([1fc149d](https://github.com/elcoosp/flux/commit/1fc149d2dddd29ee3b7ce78d63a35347a1e993d7))
- multi-device session map (FLUX-061, partial) by @elcoosp ([b39b423](https://github.com/elcoosp/flux/commit/b39b423f9b5ab6a1eeba136f7efc1af34e052471))
- structured log viewer + fifth pane (FLUX-060, partial) by @elcoosp ([3593f2f](https://github.com/elcoosp/flux/commit/3593f2f784869de7eea744ea88e26e81e4f65467))
- wire live telemetry host->server->gpui and fix gpui repaint-on-update by @elcoosp ([24a0740](https://github.com/elcoosp/flux/commit/24a0740418ee4a477cb82dd9a0a3e6afdca5537e))
- signal-graph dependency-edge rendering (FLUX-058) by @elcoosp ([9db6891](https://github.com/elcoosp/flux/commit/9db6891951b7ec7dfce5354c61cd8894c71b6136))
- ship signal-graph edges, wire run_app ingest loop, headless + live launch proof (PRD-P) by @elcoosp ([8724397](https://github.com/elcoosp/flux/commit/87243979c04d4ce42a4a778db221362363362c47))
- wire protocol + gpui time-travel crate (Phases 1,4,6) by @elcoosp ([1e3ef27](https://github.com/elcoosp/flux/commit/1e3ef27c9fac22d9fc10af93177be5e3ea9531bb))

### Miscellaneous
- mark examples/test-crates as publish = false by @elcoosp ([dbf7254](https://github.com/elcoosp/flux/commit/dbf72549121db11b45575e7112308e92349034a5))
- workspace clean under --all-targets -D warnings (F10 prerequisite) by @elcoosp ([81ce6ce](https://github.com/elcoosp/flux/commit/81ce6cebb7fa7a873b4d45ee77a000aec2d7da8c))
- rustfmt clean-up after F7/F8/F10 patches by @elcoosp ([e41c06c](https://github.com/elcoosp/flux/commit/e41c06c54d970ae41994673f97975086357c23ea))
- signal_graph reads live_snapshot() by @elcoosp ([daac7f1](https://github.com/elcoosp/flux/commit/daac7f1236127915c3d675e419faef0c067b07e3))
- component_tree reads live_snapshot() by @elcoosp ([1c4f8ce](https://github.com/elcoosp/flux/commit/1c4f8ce46a62134f4fb60c485995bda4f8611d71))
- drop unused reconstruct_state import by @elcoosp ([81123de](https://github.com/elcoosp/flux/commit/81123dea6e7b4305faf03e4c10d118b8377b366f))
- allow dead_code on render_pane_rows (kept for tests) by @elcoosp ([f5883b6](https://github.com/elcoosp/flux/commit/f5883b6c24948419b3b18416a4712dc8c686214a))
- drop dead ReconstructedState::paused field by @elcoosp ([a993978](https://github.com/elcoosp/flux/commit/a9939780b4336903baaeeac0afb1fc621175aa54))
- drop on-device assertions on removed session accessors by @elcoosp ([d7517ba](https://github.com/elcoosp/flux/commit/d7517ba9abba268f1e785255b9604e711be7a195))
- drop dead session accessors and their redundant test assertions by @elcoosp ([97818a7](https://github.com/elcoosp/flux/commit/97818a70c2c909c7656e107d2e316effe993b300))
- re-export apply_event from time_travel by @elcoosp ([4222cd5](https://github.com/elcoosp/flux/commit/4222cd5fb24abec316f2358e486eb0fec5e250d0))
- drop duplicate capacity() on TimelineBuffer (already existed) by @elcoosp ([76303f6](https://github.com/elcoosp/flux/commit/76303f6b9e0b96e2d23586ba004c308dcf520318))
- gate devtools eprintln! behind cfg!(debug_assertions); delete per-node tracing::debug! loop in build_init hot path by @elcoosp ([57a8949](https://github.com/elcoosp/flux/commit/57a8949afc873c5c03c614346a63678eb3403956))
- remove unused flux-perf-harness dependency by @elcoosp ([4db4ff2](https://github.com/elcoosp/flux/commit/4db4ff2b1b819b3bf4b0a08684701220e478c91c))
- apply pending manifest requests ([f0a3d2e](https://github.com/elcoosp/flux/commit/f0a3d2eb61ea623b1ba870e4866a49103b1e46e8))

### Performance
- component_tree reads component_name as &str (no clone) by @elcoosp ([03b0700](https://github.com/elcoosp/flux/commit/03b07007d9fdde46ceab3274c7cfb440fee03073))
- ViewFrame::component_name stored as Arc<str> (cheap per-render clone) by @elcoosp ([413a09d](https://github.com/elcoosp/flux/commit/413a09d969905f082677144362decc9ffbb7ec0f))
- checkpoint reconstructed state so deep scrub is O(CHECKPOINT_INTERVAL) by @elcoosp ([0e6b830](https://github.com/elcoosp/flux/commit/0e6b8305a688c7f99531267e79a055689d4ac0e4))
- DeviceSession + legacy mirror use apply_event (kill clone-per-event) by @elcoosp ([5079269](https://github.com/elcoosp/flux/commit/5079269db0bba2f4c2d35071a83cd06c745d5a01))
- TimelineView caches flame rows (skips rebuild on no-op repaint) by @elcoosp ([5829393](https://github.com/elcoosp/flux/commit/5829393135f8e8b92997a9ecaed69bda2c0af98a))
- state_at replays in place (O(n^2) scrub -> O(n)) by @elcoosp ([6990d33](https://github.com/elcoosp/flux/commit/6990d3325666093eec6d3584862e21cea75c49a6))
- extract apply_event for in-place replay (was clone-per-step) by @elcoosp ([73de97c](https://github.com/elcoosp/flux/commit/73de97ca8fdd72313e9b580a3105b196df9516b1))
- flame_rows dedups via HashMap (O(n^2) -> O(n) per render) by @elcoosp ([18e143e](https://github.com/elcoosp/flux/commit/18e143e8bd870423f297931f8f59c8275d434118))
- NetworkDelegate caches one snapshot per render via RefCell by @elcoosp ([29f1d48](https://github.com/elcoosp/flux/commit/29f1d48756fdf02610ccea1efd3ccc6ff3026223))
- LogsDelegate caches one snapshot per render via RefCell by @elcoosp ([1e47dd8](https://github.com/elcoosp/flux/commit/1e47dd829ac91a7fbb6edbe16c2a9fd24ba47147))

### Refactor
- use match for binary message handling by @elcoosp ([a5f1e37](https://github.com/elcoosp/flux/commit/a5f1e37704429b78a5b200a5516d8b5b691a935f))
- build component-tree content via Vec::push by @elcoosp ([5315848](https://github.com/elcoosp/flux/commit/531584826894e37fa009e8a1e3de71dd1b78ebf0))
- iterate cloned host keys in host picker by @elcoosp ([ec5299e](https://github.com/elcoosp/flux/commit/ec5299e43fab0a31e8968d92642175bac0458dc0))
- per-session state (DeviceSession owns timeline+checkpoints) by @elcoosp ([17e2b78](https://github.com/elcoosp/flux/commit/17e2b78acaec9def9067cb4a9121881632992199))
- drop dead lanes_for helper + silence unused import (FLUX-059) by @elcoosp ([58fd6ab](https://github.com/elcoosp/flux/commit/58fd6ab8d619304c8499bf41213736e167ae6a60))
- drop unused log_viewer view from DevTools crate by @elcoosp ([2647f09](https://github.com/elcoosp/flux/commit/2647f09603414ffbcfb3f357c0e4a03981dceb3d))

### Testing
- remove F13.5 in-place fold tests by @elcoosp ([2cf59cd](https://github.com/elcoosp/flux/commit/2cf59cd072077fd40668560939bc711ddd332e56))
- reconstruct in-place fold invariants (F13.5) by @elcoosp ([bf748b9](https://github.com/elcoosp/flux/commit/bf748b95777a843b772b9184a06530785b5428f8))
- on_device_verification collects owned Strings by @elcoosp ([4343508](https://github.com/elcoosp/flux/commit/434350849b66844414e2c36f215ae8a080631199))
- checkpoint correctness across interval boundaries by @elcoosp ([c12f89b](https://github.com/elcoosp/flux/commit/c12f89be78d94d47b39a404e92c93820abca93dd))
- FLUX-062 on-device verification — real DevTools data path end-to-end by @elcoosp ([f457a95](https://github.com/elcoosp/flux/commit/f457a95f7db65d4502a5406f09b4b685b8ff3158))

### style
- add right margin to host badge in title bar by @elcoosp ([824fef2](https://github.com/elcoosp/flux/commit/824fef2565594aaeb6672057654ec02aa26af066))
- small left+top padding on pane titles; drop icon (read as left padding) by @elcoosp ([288d279](https://github.com/elcoosp/flux/commit/288d2790dbca6aeaf42ecd975ab7c54df54e782a))
**Full Changelog**: https://github.com/elcoosp/flux/releases/tag/v0.1.0
