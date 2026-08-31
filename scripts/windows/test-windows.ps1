$ErrorActionPreference = "Stop"
$ProjectDirectory = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$dotnet8Sdks = if (Get-Command dotnet.exe -ErrorAction SilentlyContinue) { @(dotnet.exe --list-sdks | Where-Object { $_ -match '^8\.' }) } else { @() }
if ($dotnet8Sdks.Count -eq 0) { throw ".NET 8 SDK was not found." }

Push-Location (Join-Path $ProjectDirectory "host")
try {
    cargo fmt --check
    if ($LASTEXITCODE -ne 0) { throw "cargo fmt failed." }
    cargo clippy --all-targets -- -D warnings
    if ($LASTEXITCODE -ne 0) { throw "cargo clippy failed." }
    cargo test --all-targets
    if ($LASTEXITCODE -ne 0) { throw "cargo test failed." }
    cargo clean --target x86_64-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) { throw "cargo target cleanup failed." }
    cargo check --target x86_64-pc-windows-msvc --all-targets
    if ($LASTEXITCODE -ne 0) { throw "cargo Windows target check failed." }
} finally { Pop-Location }

dotnet test (Join-Path $ProjectDirectory "host\control\DeckyMyRigHostControl.Core.Tests\DeckyMyRigHostControl.Core.Tests.csproj") -c Release
if ($LASTEXITCODE -ne 0) { throw "Windows control-model tests failed." }
& (Join-Path $PSScriptRoot "test-decky-my-rig-host-e2e.ps1")
Write-Host "Safe Windows automated tests: PASS (no service installation, firewall modification, or shutdown was performed)."
