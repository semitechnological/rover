#!/usr/bin/env bash
# build-swiftui.sh — build the Rust dylib and generate Swift UniFFI bindings.
#
# Run once before opening RoverSwiftUI/ in Xcode or running `swift build`.
# Must be re-run whenever the Rust FFI surface changes.
#
# Usage:
#   ./build-swiftui.sh           # debug
#   RELEASE=1 ./build-swiftui.sh # release

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROFILE="${RELEASE:+release}"
PROFILE="${PROFILE:-debug}"

# eqswift is a sibling of the rover repo inside the shared projects/ dir.
PROJECTS_DIR="$(cd "${REPO_ROOT}/../../../.." && pwd)"
EQSWIFT="${PROJECTS_DIR}/eqswift"

if [ ! -f "${EQSWIFT}/eq-swift/Cargo.toml" ]; then
    echo "error: eqswift not found at ${EQSWIFT}" >&2
    echo "       Clone: git clone https://github.com/semitechnological/eqswift ${EQSWIFT}" >&2
    exit 1
fi

# ── 1. Build Rust cdylib ────────────────────────────────────────────────────
echo "▶ Building rover_swift (${PROFILE})…"
cargo build -p rover_swift ${RELEASE:+--release}

DYLIB="${REPO_ROOT}/target/${PROFILE}/librover_swift.dylib"

# ── 2. Copy dylib into package-relative lib/ so SPM can find it ─────────────
# Package.swift uses -L lib (relative to RoverSwiftUI/), which is stable
# regardless of where SPM's build directory is.
LIB_DIR="${REPO_ROOT}/RoverSwiftUI/lib"
mkdir -p "${LIB_DIR}"
cp "${DYLIB}" "${LIB_DIR}/librover_swift.dylib"
echo "  Copied dylib → RoverSwiftUI/lib/librover_swift.dylib"

# ── 3. Generate UniFFI Swift bindings into a temp dir ───────────────────────
TMP="$(mktemp -d)"
trap 'rm -rf "${TMP}"' EXIT

echo "▶ Generating Swift bindings…"
cargo run \
    --manifest-path "${EQSWIFT}/eq-swift/Cargo.toml" \
    --bin uniffi-bindgen \
    -- generate \
    --library "${DYLIB}" \
    --language swift \
    --out-dir "${TMP}"

# ── 4. Organise generated files into the SPM target layout ──────────────────
#
#   Sources/rover_swiftFFI/   ← C module target: header + modulemap
#   Sources/RoverCore/        ← Swift target: generated bindings + our code

FFI_DIR="${REPO_ROOT}/RoverSwiftUI/Sources/rover_swiftFFI"
CORE_DIR="${REPO_ROOT}/RoverSwiftUI/Sources/RoverCore"
mkdir -p "${FFI_DIR}" "${CORE_DIR}"

cp "${TMP}/rover_swiftFFI.h"         "${FFI_DIR}/rover_swiftFFI.h"
cp "${TMP}/rover_swiftFFI.modulemap" "${FFI_DIR}/module.modulemap"
cp "${TMP}/rover_swift.swift"        "${CORE_DIR}/rover_swift.swift"

echo "✓ Done. Run: swift build --package-path RoverSwiftUI"
