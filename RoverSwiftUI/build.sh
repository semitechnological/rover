#!/usr/bin/env bash
# build.sh — build the Rust dylib and generate Swift UniFFI bindings.
#
# Run this once before opening the Swift package in Xcode or running swift build.
#
# Usage:
#   ./RoverSwiftUI/build.sh           # debug build
#   RELEASE=1 ./RoverSwiftUI/build.sh # release build

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROFILE="${RELEASE:+release}"
PROFILE="${PROFILE:-debug}"

echo "▶ Building rover_swift (${PROFILE})…"
if [ "${PROFILE}" = "release" ]; then
    cargo build -p rover_swift --release --manifest-path "${REPO_ROOT}/Cargo.toml"
else
    cargo build -p rover_swift --manifest-path "${REPO_ROOT}/Cargo.toml"
fi

DYLIB="${REPO_ROOT}/target/${PROFILE}/librover_swift.dylib"
OUT_DIR="${REPO_ROOT}/RoverSwiftUI/Sources/Generated"
mkdir -p "${OUT_DIR}"

echo "▶ Generating Swift bindings → ${OUT_DIR}"
# Use the uniffi-bindgen binary from the eqswift workspace.
EQSWIFT="${REPO_ROOT}/../../../../../../eqswift"
cargo run \
    --manifest-path "${EQSWIFT}/eq-swift/Cargo.toml" \
    --bin uniffi-bindgen \
    -- generate \
    --library "${DYLIB}" \
    --language swift \
    --out-dir "${OUT_DIR}"

echo "✓ Done. Open RoverSwiftUI/ in Xcode or run: swift build --package-path RoverSwiftUI"
