#!/usr/bin/env bash
set -euo pipefail

# Notion Lite Project Integrity & Hygiene Checker
# Mirrors and expands upon .github/workflows/integrity.yml

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${PROJECT_ROOT}"

# Color formatting
if [ -t 1 ]; then
    GREEN="\033[0;32m"
    RED="\033[0;31m"
    YELLOW="\033[1;33m"
    BLUE="\033[0;34m"
    BOLD="\033[1m"
    RESET="\033[0m"
else
    GREEN=""
    RED=""
    YELLOW=""
    BLUE=""
    BOLD=""
    RESET=""
fi

FAILED=0

echo -e "${BOLD}${BLUE}==> Running Notion Lite Project Integrity Checks...${RESET}"

# 1. Required Assets Verification
echo -e "\n${BOLD}[1/6] Verifying required assets...${RESET}"
assets=(
    "src-tauri/assets/icon.png"
    "src-tauri/assets/icon.ico"
    "src-tauri/assets/icon.icns"
    "src-tauri/assets/Banner.png"
    "src-tauri/assets/notion-lite.desktop"
)
for asset in "${assets[@]}"; do
    if [ -f "$asset" ]; then
        echo -e "  ${GREEN}✓${RESET} $asset"
    else
        echo -e "  ${RED}✗ MISSING:${RESET} $asset"
        FAILED=1
    fi
done

# 2. tauri.conf.json Validation
echo -e "\n${BOLD}[2/6] Validating tauri.conf.json...${RESET}"
if python3 -c "
import json, sys
try:
    with open('src-tauri/tauri.conf.json') as f:
        data = json.load(f)
    assert 'productName' in data, 'Missing productName'
    assert 'version' in data, 'Missing version'
    print(f'  ✓ tauri.conf.json is valid (product: {data[\"productName\"]}, version: {data[\"version\"]})')
except Exception as e:
    print(f'  ✗ Invalid tauri.conf.json: {e}', file=sys.stderr)
    sys.exit(1)
"; then
    :
else
    FAILED=1
fi

# 3. Version Parity Check
echo -e "\n${BOLD}[3/6] Checking version parity (Cargo.toml == tauri.conf.json)...${RESET}"
CARGO_VER=$(grep -m1 '^version' src-tauri/Cargo.toml | sed -E 's/.*"([^"]+)".*/\1/')
TAURI_VER=$(python3 -c "import json; print(json.load(open('src-tauri/tauri.conf.json'))['version'])")

echo -e "  Cargo.toml version:       ${BOLD}$CARGO_VER${RESET}"
echo -e "  tauri.conf.json version:  ${BOLD}$TAURI_VER${RESET}"

if [ "$CARGO_VER" = "$TAURI_VER" ]; then
    echo -e "  ${GREEN}✓${RESET} Versions match exactly"
else
    echo -e "  ${RED}✗ Version mismatch! Update both src-tauri/Cargo.toml and src-tauri/tauri.conf.json.${RESET}"
    FAILED=1
fi

# 4. README & Desktop File Checks
echo -e "\n${BOLD}[4/6] Verifying README.md & .desktop specification...${RESET}"
if [ ! -f README.md ]; then
    echo -e "  ${RED}✗ README.md is missing!${RESET}"
    FAILED=1
else
    README_LINES=$(wc -l < README.md)
    if [ "$README_LINES" -lt 20 ]; then
        echo -e "  ${RED}✗ README.md too short (${README_LINES} lines, expected >= 20)${RESET}"
        FAILED=1
    else
        echo -e "  ${GREEN}✓${RESET} README.md present (${README_LINES} lines)"
    fi
fi

DESKTOP="src-tauri/assets/notion-lite.desktop"
if [ ! -f "$DESKTOP" ]; then
    echo -e "  ${RED}✗ Desktop file missing: $DESKTOP${RESET}"
    FAILED=1
else
    DESKTOP_OK=1
    for field in "Name=" "Exec=" "Icon=" "Type=" "Categories="; do
        if grep -q "^${field}" "$DESKTOP"; then
            :
        else
            echo -e "  ${RED}✗ Missing field '${field}' in $DESKTOP${RESET}"
            DESKTOP_OK=0
            FAILED=1
        fi
    done
    if [ "$DESKTOP_OK" -eq 1 ]; then
        echo -e "  ${GREEN}✓${RESET} notion-lite.desktop contains all required desktop entries"
    fi
fi

# 5. Shell Script Syntax Check
echo -e "\n${BOLD}[5/6] Validating shell script syntax (bash -n)...${RESET}"
SHELL_SCRIPTS=$(find scripts src-tauri/scripts -maxdepth 2 -type f -name "*.sh" 2>/dev/null || true)
SHELL_OK=1
for sh_file in $SHELL_SCRIPTS; do
    if bash -n "$sh_file" 2>/dev/null; then
        echo -e "  ${GREEN}✓${RESET} $sh_file"
    else
        echo -e "  ${RED}✗ Syntax error in: $sh_file${RESET}"
        bash -n "$sh_file" || true
        SHELL_OK=0
        FAILED=1
    fi
done

# 6. Syntax Validation (JSON, TOML, YAML)
echo -e "\n${BOLD}[6/6] Validating JSON, TOML, and YAML configurations...${RESET}"
if python3 -c "
import glob, json, sys

# Check JSON files
json_files = [f for f in glob.glob('**/*.json', recursive=True) if not f.startswith(('target/', 'src-tauri/target/'))]
for jf in json_files:
    try:
        with open(jf, 'r', encoding='utf-8') as f:
            json.load(f)
    except Exception as e:
        print(f'  ✗ Invalid JSON in {jf}: {e}', file=sys.stderr)
        sys.exit(1)
print(f'  ✓ {len(json_files)} JSON files validated')

# Check TOML files
try:
    import tomllib
    toml_files = [f for f in glob.glob('**/*.toml', recursive=True) if not f.startswith(('target/', 'src-tauri/target/'))]
    for tf in toml_files:
        try:
            with open(tf, 'rb') as f:
                tomllib.load(f)
        except Exception as e:
            print(f'  ✗ Invalid TOML in {tf}: {e}', file=sys.stderr)
            sys.exit(1)
    print(f'  ✓ {len(toml_files)} TOML files validated')
except ImportError:
    pass

# Check YAML files
try:
    import yaml
    yaml_files = glob.glob('.github/**/*.yml', recursive=True) + glob.glob('.pre-commit-config.yaml', recursive=True)
    for yf in yaml_files:
        try:
            with open(yf, 'r', encoding='utf-8') as f:
                yaml.safe_load(f)
        except Exception as e:
            print(f'  ✗ Invalid YAML in {yf}: {e}', file=sys.stderr)
            sys.exit(1)
    print(f'  ✓ {len(yaml_files)} YAML files validated')
except ImportError:
    pass
"; then
    :
else
    FAILED=1
fi

echo ""
if [ "$FAILED" -eq 0 ]; then
    echo -e "${GREEN}${BOLD}✓ All integrity checks passed successfully!${RESET}"
    exit 0
else
    echo -e "${RED}${BOLD}✗ Integrity checks failed. Please resolve the errors above before committing.${RESET}"
    exit 1
fi
