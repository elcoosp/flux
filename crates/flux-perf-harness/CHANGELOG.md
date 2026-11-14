# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
## [0.1.0] - 2026-11-14

### Bug Fixes
- LatencyMs::try_from_raw and validating serde (no NaN/inf/negative) by @elcoosp ([6b35b00](https://github.com/elcoosp/flux/commit/6b35b00607d2dc0618b7d775cbe1eb7f0ee51e18))
- exhaustive ceiling match + distinct no-samples verdict by @elcoosp ([649b2cc](https://github.com/elcoosp/flux/commit/649b2ccd7c45ffc0c3097dbb50dfd6b5abfd746d))
- skip dirs in copy_example + epsilon compare for JSON round-trip by @elcoosp ([8fe0f16](https://github.com/elcoosp/flux/commit/8fe0f1612d6b76f7bebaf19ca2939f6804d2db9a))
- encode Alignment as positional record {0: Int} (0=start,1=center,2=end) by @elcoosp ([cdd8760](https://github.com/elcoosp/flux/commit/cdd8760fec68fb30a67aaacffea0a81560c7d20e))

### CI
- wire flux-perf-harness into CI (PRD-J completion) by @elcoosp ([6b5bb75](https://github.com/elcoosp/flux/commit/6b5bb755dbfec7702a8996d5bafc33ca8e322a67))

### Features
- add Budgets::with_ceiling for tree-size-aware overrides by @elcoosp ([dc7e1ad](https://github.com/elcoosp/flux/commit/dc7e1adecf55cc5dd8c4a8798096fb440f773fc9))
- FLUX-066 wire real on-device render-perf measurements into both hosts by @elcoosp ([57c88fe](https://github.com/elcoosp/flux/commit/57c88fe45292852a0d2151e7dce5e8bfdf1deced))
- stand up render-perf harness core (PRD-J, partial) by @elcoosp ([2c1e26b](https://github.com/elcoosp/flux/commit/2c1e26be7a9ca6115285d137ddb8a9fcde563376))

### Miscellaneous
- mark examples/test-crates as publish = false by @elcoosp ([dbf7254](https://github.com/elcoosp/flux/commit/dbf72549121db11b45575e7112308e92349034a5))
- rustfmt clean-up after F7/F8/F10 patches by @elcoosp ([e41c06c](https://github.com/elcoosp/flux/commit/e41c06c54d970ae41994673f97975086357c23ea))

### Performance
- FLUX-073 save→photon loopback e2e harness + LANE-H CI by @elcoosp ([5a23774](https://github.com/elcoosp/flux/commit/5a237745a0180a327f80586b7ed8cf010e0fda02))

### Testing
- JSON deserialize rejects negative/NaN/infinite latency by @elcoosp ([6dd3a6f](https://github.com/elcoosp/flux/commit/6dd3a6f52a6e29f12427d98721fe49c6ebb45297))
**Full Changelog**: https://github.com/elcoosp/flux/releases/tag/v0.1.0
