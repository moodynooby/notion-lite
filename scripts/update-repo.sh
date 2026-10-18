#!/usr/bin/env bash
set -euo pipefail

# Comprehensive APT Repository Indexing & Signing Script for Notion Lite
# Compliant with apt-packaging-guide.md (Debian 12+ / Ubuntu 24.04+ / Ubuntu 26.04+)

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

REPO_DIR="${1:-${PROJECT_ROOT}/apt-repo}"
SUITE="stable"
COMPONENT="main"
ARCHS=("amd64" "arm64")
GPG_KEY_ID="${GPG_KEY_ID:-${2:-}}"

echo "Updating APT repository in ${REPO_DIR}..."

# Ensure directory structure
mkdir -p "${REPO_DIR}/pool/${COMPONENT}"

# 1. Copy versioned deb packages to pool/main if present
if [ -d "${PROJECT_ROOT}/dist" ]; then
    find "${PROJECT_ROOT}/dist" -maxdepth 1 -name "*_*_*.deb" -exec cp -f {} "${REPO_DIR}/pool/${COMPONENT}/" \;
fi

# 2. Index packages for each architecture (running from within REPO_DIR to ensure relative Filename entries)
cd "${REPO_DIR}"
for arch in "${ARCHS[@]}"; do
    target_dir="dists/${SUITE}/${COMPONENT}/binary-${arch}"
    mkdir -p "$target_dir"
    
    echo "Scanning packages for ${arch}..."
    dpkg-scanpackages --multiversion --arch "$arch" "pool/${COMPONENT}" /dev/null > "${target_dir}/Packages"
    gzip -k -f -9 "${target_dir}/Packages"
    xz -k -f -9 "${target_dir}/Packages"
done

# 3. Generate Release file with Acquire-By-Hash: yes
release_file="dists/${SUITE}/Release"
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

cd "dists/${SUITE}"
find "${COMPONENT}" -type f \( -name "Packages*" \) | sort | while read -r file; do
    size=$(wc -c < "$file")
    hash=$(sha256sum "$file" | cut -d' ' -f1)
    echo " ${hash} ${size} ${file}" >> "Release"
done

echo "SHA512:" >> "Release"
find "${COMPONENT}" -type f \( -name "Packages*" \) | sort | while read -r file; do
    size=$(wc -c < "$file")
    hash=$(sha512sum "$file" | cut -d' ' -f1)
    echo " ${hash} ${size} ${file}" >> "Release"
done
cd "${REPO_DIR}"

# 4. Sign Release if GPG key is present
if [ -n "$GPG_KEY_ID" ]; then
    echo "Signing with GPG key: ${GPG_KEY_ID}..."
    gpg --batch --yes --armor --default-key "$GPG_KEY_ID" --detach-sign -o "${release_file}.gpg" "$release_file"
    gpg --batch --yes --clearsign --default-key "$GPG_KEY_ID" -o "dists/${SUITE}/InRelease" "$release_file"
    gpg --batch --yes --armor --export "$GPG_KEY_ID" > "keyring.gpg"
    echo "Repository successfully signed with InRelease and Release.gpg."
else
    echo "No GPG_KEY_ID provided; repository generated unsigned."
    # Remove old signature artifacts if any to prevent mismatched signature warnings
    rm -f "${release_file}.gpg" "dists/${SUITE}/InRelease"
fi

cd "${PROJECT_ROOT}"
echo "APT repository update complete at ${REPO_DIR}"
