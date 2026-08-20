$ErrorActionPreference = "Stop"
$ProjectDirectory = Split-Path -Parent $PSScriptRoot
$OutputDirectory = Join-Path $ProjectDirectory "out\host"
$HostDirectory = Join-Path $ProjectDirectory "host"
$HostExecutable = Join-Path $HostDirectory "target\x86_64-pc-windows-msvc\release\decky-power-host.exe"

New-Item -ItemType Directory -Force $OutputDirectory | Out-Null
Push-Location $HostDirectory
try {
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    cargo test
    cargo build --release --target x86_64-pc-windows-msvc
} finally {
    Pop-Location
}

Copy-Item $HostExecutable (Join-Path $OutputDirectory "DeckyPowerHost.exe") -Force
$InnoSetup = Join-Path ${env:ProgramFiles(x86)} "Inno Setup 7\ISCC.exe"
if (-not (Test-Path $InnoSetup)) {
    $InnoSetup = Join-Path ${env:ProgramFiles(x86)} "Inno Setup 6\ISCC.exe"
}
if (-not (Test-Path $InnoSetup)) {
    throw "Inno Setup was not found. Install it from https://jrsoftware.org/isdl.php"
}
& $InnoSetup (Join-Path $ProjectDirectory "host\installer\DeckyPowerHost.iss")
Write-Host "Windows artifacts created in $OutputDirectory"
