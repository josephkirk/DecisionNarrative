#!/usr/bin/env bash
set -e

# Support Windows scoop / WSL paths
export PATH="$PATH:/mnt/c/Users/nguye/scoop/persist/bun/bin:/mnt/c/Users/nguye/scoop/shims:/c/Users/nguye/scoop/persist/bun/bin:/c/Users/nguye/scoop/shims:$HOME/.bun/bin"

BUN_CMD="bun"
if ! command -v bun >/dev/null 2>&1; then
    if command -v bun.exe >/dev/null 2>&1; then
        BUN_CMD="bun.exe"
    fi
fi

# Build bundle
$BUN_CMD run build >/dev/null 2>&1

# Execute deterministic benchmark workload
$BUN_CMD run bench
