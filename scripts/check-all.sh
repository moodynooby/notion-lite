#!/usr/bin/env bash
set -euo pipefail

# Notion Lite Comprehensive Quality Assurance Runner
# Runs formatting, integrity checks, and Rust compilation/tests.

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

echo -e "${BOLD}${BLUE}==> Notion Lite Full Quality Suite${RESET}"

# Step 1: Integrity Checks
"${PROJECT_ROOT}/scripts/check-integrity.sh"

# Step 2: Rust Formatting Check
echo -e "\n${BOLD}${BLUE}==> Checking Rust formatting (cargo fmt)...${RESET}"
(cd "${PROJECT_ROOT}/src-tauri" && cargo fmt --all -- --check)
echo -e "  ${GREEN}✓${RESET} Rust formatting is clean"

# Step 3: Check system libclang availability
echo -e "\n${BOLD}${BLUE}==> Checking Rust compilation environment...${RESET}"
HAS_LIBCLANG=1
if ! ldconfig -p 2>/dev/null | grep -q "libclang.so"; then
    if [ ! -f "/usr/lib/libclang.so" ] && ! find /usr/lib -name "libclang.so*" 2>/dev/null | grep -q .; then
        HAS_LIBCLANG=0
    fi
fi

if [ "$HAS_LIBCLANG" -eq 1 ]; then
    if which llvm-config >/dev/null 2>&1; then
        export LIBCLANG_PATH="$(llvm-config --libdir)"
        export LD_LIBRARY_PATH="$(llvm-config --libdir):${LD_LIBRARY_PATH:-}"
    fi

    echo -e "${BOLD}Running cargo check...${RESET}"
    (cd "${PROJECT_ROOT}/src-tauri" && cargo check --all-targets)
    echo -e "  ${GREEN}✓${RESET} cargo check passed"

    echo -e "${BOLD}Running cargo clippy...${RESET}"
    (cd "${PROJECT_ROOT}/src-tauri" && cargo clippy --all-targets -- -D warnings)
    echo -e "  ${GREEN}✓${RESET} cargo clippy passed"

    echo -e "${BOLD}Running cargo test...${RESET}"
    (cd "${PROJECT_ROOT}/src-tauri" && cargo test --all-targets)
    echo -e "  ${GREEN}✓${RESET} cargo test passed"
else
    echo -e "  ${YELLOW}⚠ Note: libclang-dev is not detected on this system.${RESET}"
    echo -e "    Skipping full compilation and clippy (requires: sudo apt install -y libclang-dev clang llvm)."
    echo -e "    Formatting and integrity checks are intact."
fi

echo -e "\n${GREEN}${BOLD}✓ Finished quality checks!${RESET}"
