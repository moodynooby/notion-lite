#!/usr/bin/env bash
set -euo pipefail

# Comprehensive Debian Packaging Script for Notion Lite
# Compliant with apt-packaging-guide.md (Debian 12+ / Ubuntu 24.04+ / Ubuntu 26.04+)

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

PKG_NAME="notion-lite"
if [ -z "${VERSION:-}" ]; then
    VERSION=$(grep -m1 '^version = ' "${PROJECT_ROOT}/src-tauri/Cargo.toml" | cut -d '"' -f2 || echo "0.3.25")
fi

# Architecture detection
HOST_ARCH="$(uname -m)"
case "$HOST_ARCH" in
    x86_64) ARCH="amd64" ;;
    aarch64|arm64) ARCH="arm64" ;;
    armv7l) ARCH="armhf" ;;
    *) ARCH="$HOST_ARCH" ;;
esac
ARCH="${TARGET_ARCH:-$ARCH}"

# Locate compiled binary
BINARY_PATH="${1:-}"
if [ -z "$BINARY_PATH" ]; then
    if [ -f "${PROJECT_ROOT}/src-tauri/target/release/${PKG_NAME}" ]; then
        BINARY_PATH="${PROJECT_ROOT}/src-tauri/target/release/${PKG_NAME}"
    elif [ -f "${PROJECT_ROOT}/src-tauri/target/${HOST_ARCH}-unknown-linux-gnu/release/${PKG_NAME}" ]; then
        BINARY_PATH="${PROJECT_ROOT}/src-tauri/target/${HOST_ARCH}-unknown-linux-gnu/release/${PKG_NAME}"
    elif [ -f "${PROJECT_ROOT}/src-tauri/target/debug/${PKG_NAME}" ]; then
        BINARY_PATH="${PROJECT_ROOT}/src-tauri/target/debug/${PKG_NAME}"
        echo "Note: Using debug build at ${BINARY_PATH}"
    else
        echo "Building release binary using cargo..."
        cargo build --release --manifest-path "${PROJECT_ROOT}/src-tauri/Cargo.toml"
        BINARY_PATH="${PROJECT_ROOT}/src-tauri/target/release/${PKG_NAME}"
    fi
fi

if [ ! -f "$BINARY_PATH" ]; then
    echo "Error: Binary not found at $BINARY_PATH" >&2
    exit 1
fi

STAGING_DIR="${PROJECT_ROOT}/build/deb-staging"
DIST_DIR="${PROJECT_ROOT}/dist"

echo "Assembling Debian package for ${PKG_NAME} v${VERSION} (${ARCH})..."
rm -rf "$STAGING_DIR"
mkdir -p "$STAGING_DIR/DEBIAN"
mkdir -p "$STAGING_DIR/usr/bin"
mkdir -p "$STAGING_DIR/usr/share/applications"
mkdir -p "$STAGING_DIR/usr/share/pixmaps"
mkdir -p "$STAGING_DIR/usr/lib/${PKG_NAME}"

# 1. Install binary
install -m 755 "$BINARY_PATH" "$STAGING_DIR/usr/bin/${PKG_NAME}"

# 2. Install desktop entry
install -m 644 "${PROJECT_ROOT}/src-tauri/assets/notion-lite.desktop" "$STAGING_DIR/usr/share/applications/${PKG_NAME}.desktop"

# 3. Install icons across all standard hicolor dimensions and pixmaps
ICON_DIR="${PROJECT_ROOT}/src-tauri/assets/icons"
for size in 16 32 48 64 128 256 512; do
    target_icon_dir="$STAGING_DIR/usr/share/icons/hicolor/${size}x${size}/apps"
    mkdir -p "$target_icon_dir"
    if [ -f "${ICON_DIR}/${size}x${size}.png" ]; then
        install -m 644 "${ICON_DIR}/${size}x${size}.png" "${target_icon_dir}/${PKG_NAME}.png"
    fi
done

# Standard pixmap fallback (required by many desktop environments)
if [ -f "${ICON_DIR}/256x256.png" ]; then
    install -m 644 "${ICON_DIR}/256x256.png" "$STAGING_DIR/usr/share/pixmaps/${PKG_NAME}.png"
fi

# 4. Install assets & i18n resources
cp -r "${PROJECT_ROOT}/src-tauri/assets" "$STAGING_DIR/usr/lib/${PKG_NAME}/"
cp -r "${PROJECT_ROOT}/src-tauri/i18n" "$STAGING_DIR/usr/lib/${PKG_NAME}/"

# 5. Generate control file (strict dpkg spec)
INSTALLED_SIZE=$(du -sk "$STAGING_DIR/usr" | cut -f1)
cat << EOF > "$STAGING_DIR/DEBIAN/control"
Package: ${PKG_NAME}
Version: ${VERSION}
Section: utils
Priority: optional
Architecture: ${ARCH}
Maintainer: Notion Lite Contributors <dev@notion-lite.local>
Depends: libc6 (>= 2.34), libwebkit2gtk-4.1-0, libgtk-3-0, libayatana-appindicator3-1, librsvg2-common, ca-certificates
Recommends: xdg-utils
Installed-Size: ${INSTALLED_SIZE}
Homepage: https://github.com/moodynooby/Notion
Description: Native lightweight client for Notion.so
 Notion Lite is a fast, memory-efficient native Rust desktop client for
 Notion.so, built with Tauri v2. Features Zero-Trust security, LiteBox
 sandboxing, multi-tab interface, and custom theming.
EOF

# 6. Install postinst (idempotent, clean deb822 migration, cache update)
install -m 755 "${PROJECT_ROOT}/src-tauri/scripts/postinst.sh" "$STAGING_DIR/DEBIAN/postinst"

# 7. Install postrm
install -m 755 "${PROJECT_ROOT}/src-tauri/scripts/postrm.sh" "$STAGING_DIR/DEBIAN/postrm"

# 8. Build .deb package
mkdir -p "$DIST_DIR"
OUTPUT_DEB="${DIST_DIR}/${PKG_NAME}_${VERSION}_${ARCH}.deb"
dpkg-deb --build --root-owner-group "$STAGING_DIR" "$OUTPUT_DEB"
cp -f "$OUTPUT_DEB" "${DIST_DIR}/${PKG_NAME}_${ARCH}.deb"

echo "Successfully built: ${OUTPUT_DEB}"
dpkg-deb --info "$OUTPUT_DEB"
