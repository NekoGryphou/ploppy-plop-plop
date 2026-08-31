param(
    [Parameter(Mandatory = $true)] [string] $Version,
    [Parameter(Mandatory = $true)] [string] $ArtifactDirectory,
    [Parameter(Mandatory = $true)] [string] $Repository
)

$ErrorActionPreference = "Stop"
if ($Version -notmatch '^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$') {
    throw "Release version must use strict X.Y.Z format."
}
$installer = Join-Path $ArtifactDirectory "DeckyPowerHost-Setup.exe"
$plugin = Join-Path $ArtifactDirectory "RemotePCPower.zip"
foreach ($path in @($installer, $plugin)) {
    if (-not (Test-Path $path)) { throw "Release artifact is missing: $path" }
}
$base = "https://github.com/$Repository/releases/download/v$Version"
$manifest = [ordered]@{
    schemaVersion = 1
    version = $Version
    publishedAt = [DateTimeOffset]::UtcNow.ToString("O")
    host = [ordered]@{
        url = "$base/DeckyPowerHost-Setup.exe"
        sha256 = (Get-FileHash $installer -Algorithm SHA256).Hash.ToLowerInvariant()
    }
    plugin = [ordered]@{
        url = "$base/RemotePCPower.zip"
        sha256 = (Get-FileHash $plugin -Algorithm SHA256).Hash.ToLowerInvariant()
    }
}
$manifest | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $ArtifactDirectory "release-manifest.json") -Encoding utf8NoBOM
Get-ChildItem $ArtifactDirectory -File | Where-Object Name -ne "SHA256SUMS.txt" | Sort-Object Name | ForEach-Object {
    "{0}  {1}" -f (Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant(), $_.Name
} | Set-Content (Join-Path $ArtifactDirectory "SHA256SUMS.txt") -Encoding ascii
