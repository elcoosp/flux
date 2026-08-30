#!/usr/bin/env bash
# check-contract-freeze.sh — FLUX-069 / PRD-U release-gate contract freeze.
#
# Reads the frozen contract versions from docs/release/contract-versions.toml
# and asserts that every implementation site in the tree agrees with them:
#
#   1. WIRE PROTOCOL VERSION consistency (ADR-0056 fail-closed handshake):
#        * Rust server  : crates/flux-ir-serde/src/frame.rs  (PROTOCOL_VERSION)
#        * Android host : FrameDeserializer.PROTOCOL_VERSION  (and SUPPORTED_VERSIONS)
#        * iOS host     : FrameDeserializer.protocolVersion   (and HelloFrame emit)
#      All three MUST equal the frozen `wire` value, and that value MUST be a
#      member of the host `supported` set on each native host.
#
#   2. ADAPTER CONTRACT VERSION consistency (AGENTS.md §3.5):
#        * iOS kit     : FluxUIKit.adapterContractVersion
#        * Android kit : FluxUiKit.ADAPTER_CONTRACT_VERSION
#        * (release codegen emits the same vocabulary; the kit constant is the
#          contract surface the host reads props through.)
#
#   3. OPTIONAL MANIFEST PIN (only enforced when the manifest file sets it):
#      docs/release/contract-versions.toml may declare `pin = "<tag>`; when set,
#      the running CI ref MUST equal that tag (a 1.0-RC / 1.0 tag). This lets the
#      release manager freeze the gate at a tag without editing CI on every bump.
#
# Exit status: 0 when every check passes, 1 on the first inconsistency (the
# workflow treats this as the blocking release gate). The script never modifies
# source — it only reads and compares.
#
# Usage: bash scripts/release-gate/check-contract-freeze.sh [--strict-manifest]

set -uo pipefail

STRICT_MANIFEST=0
if [ "${1:-}" = "--strict-manifest" ]; then
  STRICT_MANIFEST=1
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

MANIFEST="docs/release/contract-versions.toml"
if [ ! -f "$MANIFEST" ]; then
  echo "::error::contract manifest missing: $MANIFEST"
  exit 1
fi

# --- parse manifest with a tiny tolerant TOML reader (no external deps) ---
wire_ver="$(grep -E '^wire[[:space:]]*=' "$MANIFEST" | head -1 | sed -E 's/^wire[[:space:]]*=[[:space:]]*"?([^"]+)"?[[:space:]]*$/\1/')"
adapter_ver="$(grep -E '^adapter[[:space:]]*=' "$MANIFEST" | head -1 | sed -E 's/^adapter[[:space:]]*=[[:space:]]*"?([^"]+)"?[[:space:]]*$/\1/')"
pin_tag="$(grep -E '^pin[[:space:]]*=' "$MANIFEST" | head -1 | sed -E 's/^pin[[:space:]]*=[[:space:]]*"?([^"]*)"?[[:space:]]*$/\1/')"

fail=0
report() { # $1 = kind (OK/FAIL), $2 = message
  if [ "$1" = "OK" ]; then
    echo "  [ok]   $2"
  else
    echo "  [FAIL] $2"
    fail=1
  fi
}

echo "== Contract freeze check (frozen: wire=$wire_ver adapter=$adapter_ver) =="

# ---- 1. wire protocol version ----
rust_wire="$(grep -oE 'pub const PROTOCOL_VERSION: u8 = [0-9]+' crates/flux-ir-serde/src/frame.rs | grep -oE '[0-9]+$')"
android_wire="$(grep -oE 'public const val PROTOCOL_VERSION: UByte = 0x[0-9A-Fa-f]+u' runtimes/android/host/src/main/kotlin/dev/flux/host/wire/FrameDeserializer.kt | grep -oE '0x[0-9A-Fa-f]+')"
ios_wire="$(grep -oE 'static let protocolVersion: UInt8 = 0x[0-9A-Fa-f]+' runtimes/ios/FluxHost/Sources/FluxHost/FrameDeserializer.swift | grep -oE '0x[0-9A-Fa-f]+')"

if [ "$rust_wire" = "$wire_ver" ]; then
  report OK "Rust server PROTOCOL_VERSION == $wire_ver"
else
  report FAIL "Rust server PROTOCOL_VERSION is '$rust_wire', frozen manifest says '$wire_ver' (crates/flux-ir-serde/src/frame.rs)"
fi

if [ "$android_wire" = "0x$(printf '%02X' "$wire_ver")" ]; then
  report OK "Android host PROTOCOL_VERSION == $wire_ver"
else
  report FAIL "Android host PROTOCOL_VERSION is '$android_wire', frozen manifest says '0x$(printf '%02X' "$wire_ver")' (FrameDeserializer.kt)"
fi

if [ "$ios_wire" = "0x$(printf '%02X' "$wire_ver")" ]; then
  report OK "iOS host protocolVersion == $wire_ver"
else
  report FAIL "iOS host protocolVersion is '$ios_wire', frozen manifest says '0x$(printf '%02X' "$wire_ver")' (FrameDeserializer.swift)"
fi

# host supported-version set must include the frozen wire version
android_supported="$(grep -oE 'setOf\([^)]*\)' runtimes/android/host/src/main/kotlin/dev/flux/host/wire/FrameDeserializer.kt | head -1)"
if echo "$android_supported" | grep -q "0x$(printf '%02X' "$wire_ver")u"; then
  report OK "Android SUPPORTED_VERSIONS includes $wire_ver"
else
  report FAIL "Android SUPPORTED_VERSIONS ($android_supported) does not include 0x$(printf '%02X' "$wire_ver")u (ADR-0056)"
fi

# iOS HelloFrame must emit the frozen wire version on the wire
if grep -qE "data.append\(0x$(printf '%02X' "$wire_ver")\) // protocol version" runtimes/ios/FluxHost/Sources/FluxHost/HelloFrame.swift; then
  report OK "iOS HelloFrame emits wire version $wire_ver"
else
  report FAIL "iOS HelloFrame.swift does not emit 0x$(printf '%02X' "$wire_ver") as the protocol version byte"
fi

# ---- 2. adapter contract version ----
ios_adapter="$(grep -oE 'adapterContractVersion = [0-9]+' adapters/ui-swift/Sources/FluxUIKit/FluxUIKit.swift | grep -oE '[0-9]+$')"
android_adapter="$(grep -oE 'ADAPTER_CONTRACT_VERSION: Int = [0-9]+' adapters/ui-kotlin/src/main/kotlin/dev/flux/ui/FluxUiKit.kt | grep -oE '[0-9]+$')"

if [ "$ios_adapter" = "$adapter_ver" ]; then
  report OK "iOS adapterContractVersion == $adapter_ver"
else
  report FAIL "iOS adapterContractVersion is '$ios_adapter', frozen manifest says '$adapter_ver' (FluxUIKit.swift)"
fi

if [ "$android_adapter" = "$adapter_ver" ]; then
  report OK "Android ADAPTER_CONTRACT_VERSION == $adapter_ver"
else
  report FAIL "Android ADAPTER_CONTRACT_VERSION is '$android_adapter', frozen manifest says '$adapter_ver' (FluxUiKit.kt)"
fi

# ---- 3. optional manifest pin ----
if [ -n "$pin_tag" ]; then
  ref="${GITHUB_REF_NAME:-${GIT_REF:-}}"
  if [ -z "$ref" ]; then
    if [ "$STRICT_MANIFEST" -eq 1 ]; then
      report FAIL "pin='$pin_tag' set but no CI ref available (GITHUB_REF_NAME/GIT_REF) and --strict-manifest passed"
    else
      report OK "pin='$pin_tag' declared; ref not resolvable in this env (non-CI skip)"
    fi
  elif [ "$ref" = "$pin_tag" ]; then
    report OK "running ref '$ref' matches pinned tag '$pin_tag'"
  else
    report FAIL "running ref '$ref' does NOT match pinned release tag '$pin_tag' (docs/release/contract-versions.toml)"
  fi
else
  report OK "no pin declared in manifest (gate runs on every ref)"
fi

echo "== contract freeze check: $([ "$fail" -eq 0 ] && echo PASS || echo FAILED) =="
exit $fail
