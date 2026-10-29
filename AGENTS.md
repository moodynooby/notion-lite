# AGENTS.md — Notion Lite Agent Guidelines

This document provides operational rules, architectural context, and verification standards for autonomous coding agents (and human contributors) working on **Notion Lite**.

---

## 1. Project Overview & Architecture

Notion Lite is a secure, lightweight native desktop client for Notion, built using **Tauri v2** and **Rust**.

### Directory Structure
```
notion-lite/
├── src-tauri/                  # Rust / Tauri backend application
│   ├── src/
│   │   ├── main.rs             # Tauri app entry point and initialization
│   │   ├── config.rs           # TOML configuration loading and persistence
│   │   ├── window_controller.rs # Native window management
│   │   ├── tab_controller.rs   # Multi-tab logic and webview lifecycle
│   │   ├── tab_manager.rs      # Active webviews, tab hibernation & memory trim
│   │   ├── policy.rs           # Zero-Trust network policy enforcement
│   │   ├── security.rs         # CSP, sandbox, and origin validation
│   │   ├── crypto.rs           # AES-GCM / PBKDF2 state encryption
│   │   └── theming.rs          # Native CSS theme injection
│   ├── assets/                 # Icons, index.html, notion-lite.desktop, banner
│   ├── Cargo.toml              # Rust crate manifest & dependencies
│   └── tauri.conf.json         # Tauri v2 application configuration
├── scripts/                    # Automation, packaging, and QA scripts
│   ├── check-integrity.sh      # Project hygiene & integrity checker
│   ├── check-all.sh            # Full QA runner (fmt, integrity, clippy, test)
│   ├── setup-hooks.sh          # Git pre-commit hooks setup script
│   └── build-deb.sh            # Debian package builder
├── .githooks/                  # Native git hooks (zero-dependency)
├── .github/workflows/          # CI/CD pipelines (ci.yml, integrity.yml, release.yml)
├── docs/                       # Architecture decisions and security manifesto
│   ├── DECISIONS.md            # Architectural Decision Records (ADRs)
│   ├── MANIFESTO.md            # Zero-Trust Engineering Manifesto
│   └── ROADMAP_RUST.md         # Rust migration roadmap
├── .gitattributes              # Line ending (LF) & binary attributes
├── .pre-commit-config.yaml     # Standard pre-commit framework config
└── README.md                   # User and developer documentation
```

---

## 2. Non-Negotiable Operational Rules

Every agent operating on this repository **must** follow these non-negotiable rules:

### A. Pre-Commit Verification is Mandatory
Before concluding any task or pushing changes, you must verify that:
1. `scripts/check-integrity.sh` passes without errors.
2. Rust code is formatted via `cargo fmt --all -- --check` (or run `cargo fmt --all` in `src-tauri`).
3. Shell scripts have valid syntax (`bash -n`).
4. All JSON, TOML, and YAML files parse correctly.

### B. Strict Version Parity
The version number in `src-tauri/Cargo.toml` (`version = "x.y.z"`) **MUST match exactly** the version in `src-tauri/tauri.conf.json` (`"version": "x.y.z"`). Any mismatch will fail CI and pre-commit checks.

### C. Zero-Trust Security Compliance
- Respect the Zero-Trust Engineering Manifesto (`docs/MANIFESTO.md`).
- Never bypass the `PolicyManager` in `src-tauri/src/policy.rs`. Only Notion-authorized origins (`notion.so`, `app.notion.com`, and configured custom hosts) are permitted.
- Never add arbitrary remote network calls, tracking, or telemetry.

### D. File Hygiene & Encoding
- All text files must use **LF** (`\n`) line endings. Do not introduce CRLF line endings.
- Do not leave trailing whitespace or merge conflict markers.
- Follow `.gitattributes` conventions.

### E. Linux Dependencies
Building and checking the full Rust codebase requires system libraries:
`libwebkit2gtk-4.1-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`, `libhunspell-dev`, and `libclang-dev clang llvm`.

---

## 3. Agent Command Cheat Sheet

### Fast Pre-commit / Hygiene Check
Run this frequently during edits to ensure immediate compliance:
```bash
./scripts/check-integrity.sh
```

### Rust Code Formatting
Format Rust code:
```bash
cd src-tauri && cargo fmt
```
Check Rust formatting without modifying:
```bash
cd src-tauri && cargo fmt --all -- --check
```

### Run Full Quality Suite
Executes integrity checks, formatting, compilation, and tests:
```bash
./scripts/check-all.sh
```

### Run Pre-commit Framework (if uv or pre-commit is installed)
```bash
uvx pre-commit run --all-files
# or
pre-commit run --all-files
```

### Setup Local Git Hooks
Activate native git hooks (zero external dependencies):
```bash
./scripts/setup-hooks.sh native
```
Or activate the Python pre-commit framework:
```bash
./scripts/setup-hooks.sh pre-commit
```

### Development & Build
```bash
# Check Rust backend
cd src-tauri && cargo check

# Run in development mode
cd src-tauri && cargo tauri dev

# Build release bundle
cd src-tauri && cargo tauri build

# Package Debian deb
./scripts/build-deb.sh
```

---

## 4. Key Learnings & Micro-Optimizations

Refer to `.jules/bolt.md` for historical learnings:
- **Serialization**: Prefer `serde_json::to_vec` over `serde_json::to_string` when writing to encrypted storage to minimize intermediate string allocations.
- **Domain Matching**: Avoid heap-allocated strings inside domain checking loops; check slices directly.
- **Memory Compaction**: Inactive webviews are capped and background tabs are hibernated. When closing or switching tabs, call `malloc_trim(0)` on Linux to reclaim memory back to the OS.
