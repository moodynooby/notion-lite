#!/usr/bin/env bash
set -euo pipefail

# Universal Client APT Repository Setup Script for Notion Lite
# Configures the modern deb822 source format (Debian 12+ / Ubuntu 24.04+ / Ubuntu 26.04+)
# Compliant with apt-packaging-guide.md

REPO_URL="${1:-https://moodynooby.github.io/Notion/apt-repo}"
REPO_NAME="notion-lite"

echo "Configuring ${REPO_NAME} APT repository..."

# 1. Clean up legacy .list files to prevent duplicate warnings
sudo rm -f "/etc/apt/sources.list.d/${REPO_NAME}.list" \
           "/etc/apt/sources.list.d/notion.list" \
           "/etc/apt/sources.list.d/lotion-rs.list"

# 2. Check if repository provides a GPG keyring
if curl -fsSL "${REPO_URL}/keyring.gpg" -o /tmp/repo-key.gpg 2>/dev/null && [ -s /tmp/repo-key.gpg ]; then
    sudo mkdir -p /etc/apt/keyrings
    # Install with mode 0644 so the _apt sandbox user can read it
    sudo install -m 644 /tmp/repo-key.gpg "/etc/apt/keyrings/${REPO_NAME}.gpg"
    rm -f /tmp/repo-key.gpg

    sudo tee "/etc/apt/sources.list.d/${REPO_NAME}.sources" > /dev/null << EOF
Types: deb
URIs: ${REPO_URL}/
Suites: stable
Components: main
Architectures: amd64 arm64
Signed-By: /etc/apt/keyrings/${REPO_NAME}.gpg
EOF
else
    # Fallback for unsigned testing repositories (omit Signed-By entirely to avoid warnings)
    sudo tee "/etc/apt/sources.list.d/${REPO_NAME}.sources" > /dev/null << EOF
Types: deb
URIs: ${REPO_URL}/
Suites: stable
Components: main
Architectures: amd64 arm64
Trusted: yes
EOF
fi

sudo chmod 644 "/etc/apt/sources.list.d/${REPO_NAME}.sources"
echo "Repository configured. Updating package lists..."

# Perform targeted update to accelerate refresh without re-downloading entire system index
sudo apt-get update -o Dir::Etc::sourcelist="sources.list.d/${REPO_NAME}.sources" -o Dir::Etc::sourceparts="-" -o APT::Get::List-Cleanup="0"

echo ""
echo "Notion Lite repository configured successfully!"
echo "You can now install Notion Lite by running:"
echo "    sudo apt install ${REPO_NAME}"
