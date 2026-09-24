# TPT CI Config Validator - task runner.
# The pipeline logic lives in build.ps1 (Windows) / build.sh (unix); these
# recipes are thin wrappers so `just` works everywhere.

set windows-shell := ["pwsh.exe", "-NoLogo", "-NoProfile", "-Command"]

default:
    @just --list

# Build the free WASM edition and report the gzip payload size.
build:
    pwsh -NoLogo -NoProfile -File build.ps1

# Build the Pro edition: batch/directory + custom rules + trunk bundle + desktop exe + CI CLI.
pro:
    pwsh -NoLogo -NoProfile -File build.ps1 -Pro

# Re-run only wasm-bindgen + copy (after an existing release build).
bindgen:
    pwsh -NoLogo -NoProfile -File build.ps1 -SkipBuild

# Engine validation suite + app type-check (free and pro) and the CLI.
test:
    cargo test -p tpt-config-engine
    cargo check --target wasm32-unknown-unknown -p tpt-app-ci-config-validator

# Type-check the wasm lib (free), pro gates, and the CLI.
check:
    cargo check --target wasm32-unknown-unknown -p tpt-app-ci-config-validator
    cargo check --target wasm32-unknown-unknown --features pro -p tpt-app-ci-config-validator
    cargo check -p tpt-cicfg-cli

# Live-reload standalone dev server (index.html mounts via #tpt-appfront-root).
dev:
    trunk serve

# Remove build outputs.
clean:
    cargo clean
    -rd /s /q dist 2>nul || rm -rf dist
