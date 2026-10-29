#!/usr/bin/env bash
set -euo pipefail

# Notion Lite Git Hooks Setup Script
# Configures git pre-commit hooks using either native git hooks or the pre-commit framework.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${PROJECT_ROOT}"

# Color formatting
if [ -t 1 ]; then
    GREEN="\033[0;32m"
    YELLOW="\033[1;33m"
    BLUE="\033[0;34m"
    BOLD="\033[1m"
    RESET="\033[0m"
else
    GREEN=""
    YELLOW=""
    BLUE=""
    BOLD=""
    RESET=""
fi

MODE="${1:-native}"

echo -e "${BOLD}${BLUE}==> Configuring Notion Lite Pre-commit Hooks...${RESET}"

if [ "$MODE" = "pre-commit" ] || [ "$MODE" = "--pre-commit" ]; then
    echo "Configuring pre-commit framework (.pre-commit-config.yaml)..."
    if which pre-commit >/dev/null 2>&1; then
        pre-commit install
        echo -e "${GREEN}✓ Installed hooks via pre-commit CLI.${RESET}"
    elif which uvx >/dev/null 2>&1; then
        uvx pre-commit install
        echo -e "${GREEN}✓ Installed hooks via uvx pre-commit.${RESET}"
    else
        echo -e "${YELLOW}pre-commit or uvx not found. Falling back to native git hooks.${RESET}"
        MODE="native"
    fi
fi

if [ "$MODE" = "native" ] || [ "$MODE" = "--native" ]; then
    echo "Configuring zero-dependency native Git hooks (.githooks)..."
    chmod +x "${PROJECT_ROOT}/.githooks/pre-commit"
    chmod +x "${PROJECT_ROOT}/scripts/check-integrity.sh"
    chmod +x "${PROJECT_ROOT}/scripts/check-all.sh"
    git config core.hooksPath .githooks
    echo -e "${GREEN}✓ Configured git core.hooksPath to .githooks${RESET}"
fi

echo ""
echo -e "${GREEN}${BOLD}Pre-commit setup complete!${RESET}"
echo "Every 'git commit' will now verify:"
echo "  1. Rust code formatting (cargo fmt)"
echo "  2. Required assets presence"
echo "  3. Cargo.toml <-> tauri.conf.json version parity"
echo "  4. Desktop file format and README validity"
echo "  5. Shell script syntax (bash -n)"
echo "  6. JSON, TOML, and YAML syntax"
echo ""
echo "To manually trigger all checks at any time, run:"
echo "    ./scripts/check-integrity.sh"
echo "    ./scripts/check-all.sh"
