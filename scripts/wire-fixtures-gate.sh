#!/usr/bin/env bash
# T-505: three-decoder wire-fixture version gate (FLUX-083).
#
# Decodes every `fixtures/wire/*.bin` through all THREE host decoders
# (Rust flux-ir-serde, Kotlin FrameDeserializer, Swift FrameDeserializer)
# and exits non-zero if any decoder disagrees.
#
# Each decoder has its own fixture-loading test (see README.md). This script
# runs them and checks they all exit 0. When a native toolchain is missing,
# the corresponding decoder is reported as SKIPPED (not a failure) so CI on
# a minimal runner still catches drift in the decoders it *can* run.
set -euo pipefail

REPO="$(cd "$(dirname "$0")/.." && pwd)"
FIXTURES="$REPO/fixtures/wire"
EXIT=0
RUST_OK=0
KOTLIN_OK=0
SWIFT_OK=0

# ---------------------------------------------------------------------------
# 0. Fixture presence — the three decoder files must exist (T-505 source).
# ---------------------------------------------------------------------------
for f in init_v2.bin delta_v2.bin unsupported-version.bin; do
    if [ ! -f "$FIXTURES/$f" ]; then
        echo "FAIL: fixtures/wire/$f missing" >&2
        EXIT=1
    fi
done
if [ "$EXIT" -ne 0 ]; then
    echo "wire-fixtures-gate: FAIL (missing fixtures)"
    exit "$EXIT"
fi
echo "[ok] all three fixture binaries present"

# ---------------------------------------------------------------------------
# 1. Rust decoder  —  crates/flux-ir-serde  (fixtures_golden.rs)
# ---------------------------------------------------------------------------
if command -v cargo >/dev/null 2>&1; then
    echo "--- Rust (flux-ir-serde) ---"
    if cargo test -p flux-ir-serde --test fixtures_golden 2>&1 | tail -5; then
        echo "[ok] Rust fixture test"
        RUST_OK=1
    else
        echo "FAIL: Rust fixture test" >&2
        EXIT=1
    fi
else
    echo "SKIP: cargo not installed (Rust decoder not run)"
fi

# ---------------------------------------------------------------------------
# 2. Kotlin decoder  —  runtimes/android/host  (FrameDeserializerTest)
# ---------------------------------------------------------------------------
if [ -x "$REPO/runtimes/android/gradlew" ]; then
    echo "--- Kotlin (host FrameDeserializerTest) ---"
    if (cd "$REPO/runtimes/android" && ./gradlew :host:testDebugUnitTest \
        --tests "dev.flux.host.wire.FrameDeserializerTest" \
        --console=plain 2>&1 | tail -5); then
        echo "[ok] Kotlin host fixture test"
        KOTLIN_OK=1
    else
        echo "FAIL: Kotlin host fixture test" >&2
        EXIT=1
    fi
else
    echo "SKIP: gradlew missing (Kotlin decoder not run)"
fi

# ---------------------------------------------------------------------------
# 3. Swift decoder  —  runtimes/ios/FluxHost  (WireDecodeTests)
# ---------------------------------------------------------------------------
if command -v xcodebuild >/dev/null 2>&1; then
    echo "--- Swift (FluxHost WireDecodeTests) ---"
    if (cd "$REPO/runtimes/ios/FluxHost" && xcodebuild test \
        -scheme FluxHost \
        -destination 'platform=macOS' \
        -only-testing FluxHostTests.WireDecodeTests \
        2>&1 | tail -5); then
        echo "[ok] Swift fixture test"
        SWIFT_OK=1
    else
        echo "FAIL: Swift fixture test" >&2
        EXIT=1
    fi
else
    echo "SKIP: xcodebuild not installed (Swift decoder not run)"
fi

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo "--- summary ---"
echo "Rust:    $([ $RUST_OK -eq 1 ] && echo PASS || echo 'NOT RUN')"
echo "Kotlin:  $([ $KOTLIN_OK -eq 1 ] && echo PASS || echo 'NOT RUN')"
echo "Swift:   $([ $SWIFT_OK -eq 1 ] && echo PASS || echo 'NOT RUN')"

if [ "$EXIT" -eq 0 ]; then
    ran=0
    [ "$RUST_OK" -eq 1 ] && ran=1
    [ "$KOTLIN_OK" -eq 1 ] && ran=1
    [ "$SWIFT_OK" -eq 1 ] && ran=1
    if [ "$ran" -eq 0 ]; then
        echo "SKIP: no native toolchain available"
        exit 0
    fi
    echo "wire-fixtures-gate: PASS (all available decoders agree)"
else
    echo "wire-fixtures-gate: FAIL"
fi
exit "$EXIT"
