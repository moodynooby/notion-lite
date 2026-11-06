#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
MANIFEST="${PROJECT_ROOT}/src-tauri/Cargo.toml"
VERSION="$(grep -m1 '^version = ' "$MANIFEST" | cut -d '"' -f2)"

if ! command -v cargo >/dev/null 2>&1; then
    echo "Error: Rust and Cargo are required to build the RPM." >&2
    exit 1
fi

if ! cargo tauri --help >/dev/null 2>&1; then
    echo "Error: Tauri CLI is required. Install it with: cargo install tauri-cli --version '^2.0'" >&2
    exit 1
fi

TARGET="${TARGET:-$(rustc -vV | sed -n 's/^host: //p')}"
BUNDLE_DIR="${PROJECT_ROOT}/src-tauri/target/${TARGET}/release/bundle/rpm"
DIST_DIR="${PROJECT_ROOT}/dist"

echo "Building RPM package for notion-lite v${VERSION} (${TARGET})..."
(cd "${PROJECT_ROOT}/src-tauri" && cargo tauri build --target "$TARGET" --bundles rpm)

if [ ! -d "$BUNDLE_DIR" ]; then
    echo "Error: RPM bundle directory not found: $BUNDLE_DIR" >&2
    exit 1
fi

mapfile -t RPM_PACKAGES < <(find "$BUNDLE_DIR" -maxdepth 1 -type f -name '*.rpm' -print | sort)
if [ "${#RPM_PACKAGES[@]}" -ne 1 ]; then
    echo "Error: Expected one RPM in ${BUNDLE_DIR}, found ${#RPM_PACKAGES[@]}." >&2
    exit 1
fi

mkdir -p "$DIST_DIR"
cp -f "${RPM_PACKAGES[0]}" "$DIST_DIR/"
cp -f "${RPM_PACKAGES[0]}" "${DIST_DIR}/notion-lite.rpm"

echo "Successfully built: ${DIST_DIR}/$(basename "${RPM_PACKAGES[0]}")"
