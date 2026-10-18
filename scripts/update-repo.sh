#!/usr/bin/env bash
set -euo pipefail

# Comprehensive APT Repository Indexing & Signing Script for Notion Lite
# Compliant with modern APT specifications (Debian 12+ / Ubuntu 24.04+ / Ubuntu 26.04+)

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

# Resolve REPO_DIR to an absolute canonical path so relative inputs work seamlessly
REPO_INPUT="${1:-${PROJECT_ROOT}/apt-repo}"
mkdir -p "$REPO_INPUT"
REPO_DIR="$(cd "$REPO_INPUT" && pwd)"

SUITE="stable"
COMPONENT="main"
ARCHS=("amd64" "arm64")
GPG_KEY_ID="${GPG_KEY_ID:-${2:-}}"

echo "Updating APT repository in ${REPO_DIR}..."

# Ensure directory structure
mkdir -p "${REPO_DIR}/pool/${COMPONENT}"

# 1. Copy versioned deb packages to pool/main if present in dist/
if [ -d "${PROJECT_ROOT}/dist" ]; then
    if compgen -G "${PROJECT_ROOT}/dist/*_*_*.deb" > /dev/null; then
        cp -f "${PROJECT_ROOT}/dist/"*_*_*.deb "${REPO_DIR}/pool/${COMPONENT}/"
    elif compgen -G "${PROJECT_ROOT}/dist/*.deb" > /dev/null; then
        cp -f "${PROJECT_ROOT}/dist/"*.deb "${REPO_DIR}/pool/${COMPONENT}/"
    fi
fi

deb_count=$(find "${REPO_DIR}/pool/${COMPONENT}" -maxdepth 1 -name "*.deb" 2>/dev/null | wc -l)
if [ "$deb_count" -eq 0 ]; then
    echo "Notice: No .deb packages found in ${REPO_DIR}/pool/${COMPONENT}."
    echo "Tip: Run ./scripts/build-deb.sh first to build a package before updating the repo."
fi

# 2. Index packages for each architecture (running in subshell from within REPO_DIR)
(
    cd "${REPO_DIR}"
    for arch in "${ARCHS[@]}"; do
        target_dir="dists/${SUITE}/${COMPONENT}/binary-${arch}"
        mkdir -p "$target_dir"
        
        echo "Scanning packages for ${arch}..."
        dpkg-scanpackages --multiversion --arch "$arch" "pool/${COMPONENT}" /dev/null > "${target_dir}/Packages"
        gzip -k -f -9 "${target_dir}/Packages"
        xz -k -f -9 "${target_dir}/Packages"

        # Generate Acquire-By-Hash indices for atomic, race-free caching
        by_hash_dir="${target_dir}/by-hash/SHA256"
        mkdir -p "$by_hash_dir"
        for index_file in "${target_dir}"/Packages*; do
            if [ -f "$index_file" ] && [[ "$index_file" != *by-hash* ]]; then
                h=$(sha256sum "$index_file" | cut -d' ' -f1)
                cp -f "$index_file" "${by_hash_dir}/${h}"
            fi
        done
    done
)

# 3. Generate Release file with Acquire-By-Hash: yes
release_file="${REPO_DIR}/dists/${SUITE}/Release"
cat << EOF > "$release_file"
Origin: Notion Lite
Label: Notion Lite Repository
Suite: ${SUITE}
Codename: ${SUITE}
Architectures: ${ARCHS[*]}
Components: ${COMPONENT}
Description: Official APT repository for Notion Lite desktop client
Date: $(date -u -R)
Acquire-By-Hash: yes
SHA256:
EOF

(
    cd "${REPO_DIR}/dists/${SUITE}"
    find "${COMPONENT}" -type f \( -name "Packages*" \) ! -path "*/by-hash/*" | sort | while read -r file; do
        size=$(wc -c < "$file")
        hash=$(sha256sum "$file" | cut -d' ' -f1)
        echo " ${hash} ${size} ${file}" >> "Release"
    done

    echo "SHA512:" >> "Release"
    find "${COMPONENT}" -type f \( -name "Packages*" \) ! -path "*/by-hash/*" | sort | while read -r file; do
        size=$(wc -c < "$file")
        hash=$(sha512sum "$file" | cut -d' ' -f1)
        echo " ${hash} ${size} ${file}" >> "Release"
    done
)

# 4. Sign Release if GPG key is present
if [ -n "$GPG_KEY_ID" ]; then
    echo "Signing with GPG key: ${GPG_KEY_ID}..."
    gpg --batch --yes --armor --default-key "$GPG_KEY_ID" --detach-sign -o "${release_file}.gpg" "$release_file"
    gpg --batch --yes --clearsign --default-key "$GPG_KEY_ID" -o "${REPO_DIR}/dists/${SUITE}/InRelease" "$release_file"
    gpg --batch --yes --armor --export "$GPG_KEY_ID" > "${REPO_DIR}/keyring.gpg"
    echo "Repository successfully signed with InRelease and Release.gpg."
else
    echo "No GPG_KEY_ID provided; repository generated unsigned."
    # Remove old signature artifacts if any to prevent mismatched signature warnings
    rm -f "${release_file}.gpg" "${REPO_DIR}/dists/${SUITE}/InRelease"
fi

echo "APT repository update complete at ${REPO_DIR}"
