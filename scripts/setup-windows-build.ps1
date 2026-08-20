$ErrorActionPreference = "Stop"

function Install-WingetPackage {
    param(
        [Parameter(Mandatory = $true)][string]$Id,
        [string]$Override = ""
    )

    $arguments = @("install", "--id", $Id, "--exact", "--source", "winget", "--accept-package-agreements", "--accept-source-agreements")
    if ($Override) { $arguments += @("--override", $Override) }
    & winget.exe @arguments
    if ($LASTEXITCODE -ne 0) { throw "winget failed to install $Id (exit $LASTEXITCODE)." }
}

if (-not (Get-Command winget.exe -ErrorAction SilentlyContinue)) {
    throw "Windows Package Manager is required: https://learn.microsoft.com/windows/package-manager/winget/"
}

if (-not (Test-Path (Join-Path $env:USERPROFILE ".cargo\bin\rustup.exe"))) {
    Install-WingetPackage -Id "Rustlang.Rustup"
}

$vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
$visualStudio = if (Test-Path $vswhere) {
    & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
} else { "" }
if (-not $visualStudio) {
    Install-WingetPackage -Id "Microsoft.VisualStudio.2022.BuildTools" -Override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
}

$inno6 = Join-Path ${env:ProgramFiles(x86)} "Inno Setup 6\ISCC.exe"
$inno7 = Join-Path ${env:ProgramFiles(x86)} "Inno Setup 7\ISCC.exe"
if (-not (Test-Path $inno6) -and -not (Test-Path $inno7)) {
    Install-WingetPackage -Id "JRSoftware.InnoSetup"
}

$cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
$env:Path = "$cargoBin;$env:Path"
& rustup.exe toolchain install stable --profile default --component rustfmt clippy
if ($LASTEXITCODE -ne 0) { throw "Rust stable toolchain installation failed." }
& rustup.exe target add x86_64-pc-windows-msvc
if ($LASTEXITCODE -ne 0) { throw "The Windows MSVC Rust target installation failed." }

Write-Host "Windows build tools are ready. DeckyPowerHost was not installed, and no service or firewall rule was changed."
