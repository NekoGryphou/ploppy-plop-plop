$ErrorActionPreference = "Stop"
$SourceProjectDirectory = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$BuildProjectDirectory = $SourceProjectDirectory
$SourceOutputDirectory = Join-Path $SourceProjectDirectory "out\host"
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
    $TemporaryToolsDirectory = Join-Path $TemporaryBuildDirectory "tools"
    New-Item -ItemType Directory -Force $TemporaryToolsDirectory | Out-Null
    Copy-Item (Join-Path $SourceProjectDirectory "tools\decky-power-test") $TemporaryToolsDirectory -Recurse
    $BuildProjectDirectory = $TemporaryBuildDirectory
    Write-Host "WSL path detected; compiling in $TemporaryBuildDirectory"
}

$HostDirectory = Join-Path $BuildProjectDirectory "host"
$HostExecutable = Join-Path $HostDirectory "target\x86_64-pc-windows-msvc\release\decky-power-host.exe"
$ControlOutputDirectory = Join-Path $BuildProjectDirectory "out\control"
$BuildOutputDirectory = Join-Path $BuildProjectDirectory "out\host"

New-Item -ItemType Directory -Force $BuildOutputDirectory | Out-Null
Push-Location $HostDirectory
try {
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    cargo test
    cargo build --release --target x86_64-pc-windows-msvc
} finally {
    Pop-Location
}

Copy-Item $HostExecutable (Join-Path $BuildOutputDirectory "DeckyPowerHost.exe") -Force
Push-Location (Join-Path $BuildProjectDirectory "tools\decky-power-test")
try {
    cargo build --release
} finally { Pop-Location }
$dotnet8Sdks = if (Get-Command dotnet.exe -ErrorAction SilentlyContinue) { @(dotnet.exe --list-sdks | Where-Object { $_ -match '^8\.' }) } else { @() }
if ($dotnet8Sdks.Count -eq 0) { throw ".NET 8 SDK was not found. Run scripts\setup-windows-build.ps1 first." }
dotnet.exe test (Join-Path $BuildProjectDirectory "host\control\DeckyPowerHostControl.Core.Tests\DeckyPowerHostControl.Core.Tests.csproj") -c Release
if ($LASTEXITCODE -ne 0) { throw "WinUI model tests failed (exit $LASTEXITCODE)." }
$SigningThumbprintProperty = if ([string]::IsNullOrWhiteSpace($env:SIGNING_CERTIFICATE_THUMBPRINT)) { "UNCONFIGURED" } else { $env:SIGNING_CERTIFICATE_THUMBPRINT }
dotnet.exe publish (Join-Path $BuildProjectDirectory "host\control\DeckyPowerHostControl\DeckyPowerHostControl.csproj") -c Release -r win-x64 --self-contained true -o $ControlOutputDirectory -p:DeckySigningCertificateThumbprint=$SigningThumbprintProperty
if ($LASTEXITCODE -ne 0) { throw "DeckyPowerHostControl publish failed (exit $LASTEXITCODE)." }
$SigningScript = Join-Path $SourceProjectDirectory "scripts\windows\sign-artifacts.ps1"
& $SigningScript -Paths @(
    (Join-Path $BuildOutputDirectory "DeckyPowerHost.exe"),
    (Join-Path $ControlOutputDirectory "DeckyPowerHostControl.exe")
)
$InnoSetupCandidates = @(
    (Join-Path ${env:ProgramFiles(x86)} "Inno Setup 7\ISCC.exe"),
    (Join-Path ${env:ProgramFiles(x86)} "Inno Setup 6\ISCC.exe"),
    (Join-Path $env:LOCALAPPDATA "Programs\Inno Setup 7\ISCC.exe"),
    (Join-Path $env:LOCALAPPDATA "Programs\Inno Setup 6\ISCC.exe")
)
$InnoSetup = $InnoSetupCandidates | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $InnoSetup) {
    throw "Inno Setup was not found. Install it from https://jrsoftware.org/isdl.php"
}
& $InnoSetup (Join-Path $BuildProjectDirectory "host\installer\DeckyPowerHost.iss")
if ($LASTEXITCODE -ne 0) { throw "Inno Setup compilation failed (exit $LASTEXITCODE)." }
& $SigningScript -Paths @((Join-Path $BuildOutputDirectory "DeckyPowerHost-Setup.exe"))
if ($TemporaryBuildDirectory) {
    $SourceControlOutput = Join-Path $SourceProjectDirectory "out\control"
    New-Item -ItemType Directory -Force $SourceOutputDirectory | Out-Null
    New-Item -ItemType Directory -Force $SourceControlOutput | Out-Null
    Copy-Item (Join-Path $ControlOutputDirectory "*") $SourceControlOutput -Recurse -Force
    Copy-Item (Join-Path $BuildOutputDirectory "DeckyPowerHost.exe") $SourceOutputDirectory -Force
    Copy-Item (Join-Path $BuildOutputDirectory "DeckyPowerHost-Setup.exe") $SourceOutputDirectory -Force
    Remove-Item $TemporaryBuildDirectory -Recurse -Force
}
Write-Host "Windows host, protocol client, WinUI control app, and installer artifacts created. WINDOWS CI/LOCAL WINDOWS VERIFIED only if this script completed on Windows."
