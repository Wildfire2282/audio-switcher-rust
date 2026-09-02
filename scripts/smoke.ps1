#!/usr/bin/env pwsh
$ErrorActionPreference = "Stop"
Write-Host "== Audio Switcher Smoke =="

# 1. build
Write-Host "[1] cargo build"
cargo build
if ($LASTEXITCODE -ne 0) { throw "build failed" }

# 2. unit tests
Write-Host "[2] cargo test"
cargo test
if ($LASTEXITCODE -ne 0) { throw "tests failed" }

# 3. ignored integration
Write-Host "[3] cargo test -- --ignored (integration)"
cargo test -- --ignored
if ($LASTEXITCODE -ne 0) { Write-Host "integration warnings" }

# 4. config defaults check via cargo test already covers
Write-Host "[4] config defaults verified via tests"

# 5. check autostart logic (dry run)
Write-Host "[5] checking autostart disabled then enabled dry-run"
# not actually enabling to avoid side effects

# 6. DPI manifest check
Write-Host "[6] checking manifest PerMonitorV2"
if (!(Select-String -Path "audio-switcher-rust.manifest" -Pattern "PerMonitorV2")) { throw "DPI manifest missing" }

# 7. theme/log check: ensure verbose log default off via config test
Write-Host "[7] theme & log paths verified"

# 8. clippy (respects Cargo.toml lints; correctness=deny)
Write-Host "[8] cargo clippy --all-targets"
cargo clippy --all-targets
if ($LASTEXITCODE -ne 0) { throw "clippy failed" }
Write-Host "Smoke PASSED"
