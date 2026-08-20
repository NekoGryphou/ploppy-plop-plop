$ErrorActionPreference = "Stop"
$SourceProjectDirectory = Split-Path -Parent $PSScriptRoot
$BuildProjectDirectory = $SourceProjectDirectory
$OutputDirectory = Join-Path $SourceProjectDirectory "out\host"
$TemporaryBuildDirectory = $null

function Import-VisualStudioEnvironment {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
    if (-not (Test-Path $vswhere)) { throw "Visual Studio Build Tools were not found. Run scripts\setup-windows-build.ps1 first." }
    $installation = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if (-not $installation) { throw "The Visual Studio C++ x64 tools were not found. Run scripts\setup-windows-build.ps1 first." }
    $developerCommand = Join-Path $installation "Common7\Tools\VsDevCmd.bat"
    $lines = & cmd.exe /s /c "`"$developerCommand`" -no_logo -arch=x64 -host_arch=x64 && set"
    foreach ($line in $lines) {
        $equals = $line.IndexOf("=")
        if ($equals -gt 0) { Set-Item -Path "env:$($line.Substring(0, $equals))" -Value $line.Substring($equals + 1) }
    }
}

$cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
$env:Path = "$cargoBin;$env:Path"
Import-VisualStudioEnvironment
if (-not (Get-Command cargo.exe -ErrorAction SilentlyContinue)) {
    throw "Cargo was not found. Run scripts\setup-windows-build.ps1 first."
}

if ($SourceProjectDirectory.StartsWith("\\")) {
    $TemporaryBuildDirectory = Join-Path $env:TEMP ("DeckyPowerHost-build-" + [Guid]::NewGuid().ToString("N"))
    New-Item -ItemType Directory -Force $TemporaryBuildDirectory | Out-Null
    Copy-Item (Join-Path $SourceProjectDirectory "host") $TemporaryBuildDirectory -Recurse
    Copy-Item (Join-Path $SourceProjectDirectory "proto") $TemporaryBuildDirectory -Recurse
    $BuildProjectDirectory = $TemporaryBuildDirectory
    Write-Host "WSL path detected; compiling in $TemporaryBuildDirectory"
}

$HostDirectory = Join-Path $BuildProjectDirectory "host"
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
& $InnoSetup (Join-Path $BuildProjectDirectory "host\installer\DeckyPowerHost.iss")
if ($LASTEXITCODE -ne 0) { throw "Inno Setup compilation failed (exit $LASTEXITCODE)." }
if ($TemporaryBuildDirectory) {
    Copy-Item (Join-Path $BuildProjectDirectory "out\host\DeckyPowerHost-Setup.exe") $OutputDirectory -Force
    Remove-Item $TemporaryBuildDirectory -Recurse -Force
}
Write-Host "Windows artifacts created in $OutputDirectory"
