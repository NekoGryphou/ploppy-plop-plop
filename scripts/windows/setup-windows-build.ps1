$ErrorActionPreference = "Stop"

function Install-WingetPackage {
    param(
        [Parameter(Mandatory = $true)][string]$Id,
        [string]$Override = ""
    )

    & $script:WingetExecutable list --id $Id --exact --source winget --accept-source-agreements --disable-interactivity | Out-Null
    if ($LASTEXITCODE -eq 0) {
        Write-Host "$Id is already installed."
        return
    }

    $arguments = @("install", "--id", $Id, "--exact", "--source", "winget", "--accept-package-agreements", "--accept-source-agreements", "--disable-interactivity")
    if ($Override) { $arguments += @("--override", $Override) }
    & $script:WingetExecutable @arguments
    if ($LASTEXITCODE -ne 0) { throw "winget failed to install $Id (exit $LASTEXITCODE)." }
}

if (-not (Get-Command winget.exe -ErrorAction SilentlyContinue)) {
    Install-PackageProvider -Name NuGet -Force | Out-Null
    Install-Module -Name Microsoft.WinGet.Client -Force -Repository PSGallery -Scope CurrentUser | Out-Null
    Import-Module Microsoft.WinGet.Client
    Repair-WinGetPackageManager -Force -Latest | Out-Null
}
$script:WingetExecutable = (Get-Command winget.exe -ErrorAction SilentlyContinue).Source
if (-not $script:WingetExecutable) {
    $appInstaller = Get-AppxPackage -Name Microsoft.DesktopAppInstaller | Select-Object -First 1
    if ($appInstaller) { $script:WingetExecutable = Join-Path $appInstaller.InstallLocation "winget.exe" }
}
if (-not $script:WingetExecutable -or -not (Test-Path $script:WingetExecutable)) {
    throw "Windows Package Manager bootstrap failed: https://learn.microsoft.com/windows/package-manager/winget/troubleshooting"
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
$innoUser6 = Join-Path $env:LOCALAPPDATA "Programs\Inno Setup 6\ISCC.exe"
$innoUser7 = Join-Path $env:LOCALAPPDATA "Programs\Inno Setup 7\ISCC.exe"
if (-not (@($inno6, $inno7, $innoUser6, $innoUser7) | Where-Object { Test-Path $_ })) {
    Install-WingetPackage -Id "JRSoftware.InnoSetup"
}
$dotnet8Sdks = if (Get-Command dotnet.exe -ErrorAction SilentlyContinue) { @(dotnet.exe --list-sdks | Where-Object { $_ -match '^8\.' }) } else { @() }
if ($dotnet8Sdks.Count -eq 0) {
    Install-WingetPackage -Id "Microsoft.DotNet.SDK.8"
}

$nugetSourceName = "nuget.org"
$nugetSourceUrl = "https://api.nuget.org/v3/index.json"
$configuredNugetSources = @(dotnet.exe nuget list source --format short)
if (-not ($configuredNugetSources | Where-Object { $_ -match [regex]::Escape($nugetSourceUrl) })) {
    dotnet.exe nuget add source $nugetSourceUrl --name $nugetSourceName
    if ($LASTEXITCODE -ne 0) { throw "Failed to configure the official NuGet package source." }
}

$cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
$env:Path = "$cargoBin;$env:Path"
& rustup.exe toolchain install stable --profile default --component rustfmt --component clippy
if ($LASTEXITCODE -ne 0) { throw "Rust stable toolchain installation failed." }
& rustup.exe target add x86_64-pc-windows-msvc
if ($LASTEXITCODE -ne 0) { throw "The Windows MSVC Rust target installation failed." }

Write-Host "Windows build tools are ready. DeckyMyRigHost was not installed, and no service or firewall rule was changed."
