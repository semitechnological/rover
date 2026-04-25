#!/usr/bin/env bash
# build-swiftui.sh — build the Rust dylib and generate Swift UniFFI bindings.
#
# Run once before opening RoverSwiftUI/ in Xcode or running `swift build`.
#
# Usage:
#   ./build-swiftui.sh           # debug
#   RELEASE=1 ./build-swiftui.sh # release

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROFILE="${RELEASE:+release}"
PROFILE="${PROFILE:-debug}"

# eqswift lives as a sibling of the rover repo inside projects/
PROJECTS_DIR="$(cd "${REPO_ROOT}/../../../.." && pwd)"
EQSWIFT="${PROJECTS_DIR}/eqswift"

if [ ! -f "${EQSWIFT}/eq-swift/Cargo.toml" ]; then
    echo "error: eqswift not found at ${EQSWIFT}" >&2
    echo "       Clone it: git clone https://github.com/semitechnological/eqswift ${EQSWIFT}" >&2
    exit 1
fi

echo "▶ Building rover_swift (${PROFILE})…"
cargo build -p rover_swift ${RELEASE:+--release}

DYLIB="${REPO_ROOT}/target/${PROFILE}/librover_swift.dylib"
OUT_DIR="${REPO_ROOT}/RoverSwiftUI/Sources/Generated"
mkdir -p "${OUT_DIR}"

echo "▶ Generating Swift bindings → ${OUT_DIR}"
cargo run \
    --manifest-path "${EQSWIFT}/eq-swift/Cargo.toml" \
    --bin uniffi-bindgen \
    -- generate \
    --library "${DYLIB}" \
    --language swift \
    --out-dir "${OUT_DIR}"

echo "✓ Done. Run: swift build --package-path RoverSwiftUI"
