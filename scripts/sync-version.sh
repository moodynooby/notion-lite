#!/usr/bin/env bash
set -euo pipefail

# Notion Lite Version Synchronization Script
# Synchronizes the semantic version across Cargo.toml, tauri.conf.json, and packaging metadata.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

RAW_VERSION="${1:-}"

if [ -z "$RAW_VERSION" ]; then
    echo "Usage: $0 <version-or-tag>"
    echo "Example: $0 v0.3.25"
    exit 1
fi

# Detect if the input is a valid semantic version (with optional leading 'v')
if [[ ! "$RAW_VERSION" =~ ^v?[0-9]+\.[0-9]+\.[0-9]+ ]]; then
    echo "Notice: '${RAW_VERSION}' is not a semantic version tag (skipping version sync)."
    exit 0
fi

# Strip leading 'v' if present
VERSION="${RAW_VERSION#v}"

echo "Synchronizing project version to ${VERSION}..."

PYTHON_BIN="python3"
if ! command -v python3 &>/dev/null; then
    PYTHON_BIN="python"
fi

$PYTHON_BIN -c "
import re

version = '$VERSION'

# 1. Update src-tauri/Cargo.toml
cargo_path = '${PROJECT_ROOT}/src-tauri/Cargo.toml'
with open(cargo_path, 'r', encoding='utf-8') as f:
    content = re.sub(r'(?m)^version = \".*?\"', f'version = \"{version}\"', f.read(), count=1)
with open(cargo_path, 'w', encoding='utf-8') as f:
    f.write(content)

# 2. Update src-tauri/tauri.conf.json
tauri_path = '${PROJECT_ROOT}/src-tauri/tauri.conf.json'
with open(tauri_path, 'r', encoding='utf-8') as f:
    content = re.sub(r'\"version\": \".*?\"', f'\"version\": \"{version}\"', f.read(), count=1)
with open(tauri_path, 'w', encoding='utf-8') as f:
    f.write(content)

print(f'✓ Synchronized Cargo.toml and tauri.conf.json to {version}')
"
