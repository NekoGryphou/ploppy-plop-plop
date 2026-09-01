$ErrorActionPreference = "Stop"
$ProjectDirectory = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)

function Invoke-TestWithSummary {
    param(
        [Parameter(Mandatory = $true)][string]$Title,
        [Parameter(Mandatory = $true)][scriptblock]$Command
    )

    $commandOutput = @()
    $failure = $null
    try {
        & $Command 2>&1 | Tee-Object -Variable commandOutput | Write-Host
        $exitCode = $LASTEXITCODE
        if ($null -ne $exitCode -and $exitCode -ne 0) { throw "$Title failed (exit $exitCode)." }
    } catch {
        $failure = $_
    } finally {
        if (-not [string]::IsNullOrWhiteSpace($env:GITHUB_STEP_SUMMARY)) {
            $status = if ($null -eq $failure) { "✅ Passed" } else { "❌ Failed" }
            Add-Content -Path $env:GITHUB_STEP_SUMMARY -Encoding utf8 -Value "## $Title`n`n$status`n`n<details><summary>Test output</summary>`n`n``````text"
            $commandOutput | Select-Object -Last 500 | ForEach-Object {
                $line = ($_ | Out-String).TrimEnd()
                Add-Content -Path $env:GITHUB_STEP_SUMMARY -Encoding utf8 -Value $line
            }
            if ($null -ne $failure) {
                Add-Content -Path $env:GITHUB_STEP_SUMMARY -Encoding utf8 -Value $failure.Exception.Message
            }
            Add-Content -Path $env:GITHUB_STEP_SUMMARY -Encoding utf8 -Value "```````n`n</details>"
        }
    }

    if ($null -ne $failure) { throw $failure }
}

$dotnet8Sdks = if (Get-Command dotnet.exe -ErrorAction SilentlyContinue) { @(dotnet.exe --list-sdks | Where-Object { $_ -match '^8\.' }) } else { @() }
if ($dotnet8Sdks.Count -eq 0) { throw ".NET 8 SDK was not found." }

Push-Location (Join-Path $ProjectDirectory "host")
try {
    cargo fmt --check
    if ($LASTEXITCODE -ne 0) { throw "cargo fmt failed." }
    cargo clippy --all-targets -- -D warnings
    if ($LASTEXITCODE -ne 0) { throw "cargo clippy failed." }
    Invoke-TestWithSummary "Windows host unit tests" { cargo test --all-targets }
    cargo clean --target x86_64-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) { throw "cargo target cleanup failed." }
    cargo check --target x86_64-pc-windows-msvc --all-targets
    if ($LASTEXITCODE -ne 0) { throw "cargo Windows target check failed." }
} finally { Pop-Location }

Invoke-TestWithSummary "Windows control-model tests" {
    dotnet test (Join-Path $ProjectDirectory "host\control\DeckyMyRigHostControl.Core.Tests\DeckyMyRigHostControl.Core.Tests.csproj") -c Release
}
Invoke-TestWithSummary "Windows pairing integration" {
    & (Join-Path $PSScriptRoot "test-decky-my-rig-host-e2e.ps1")
}
Write-Host "Safe Windows automated tests: PASS (no service installation, firewall modification, or shutdown was performed)."
