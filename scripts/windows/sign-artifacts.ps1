param(
    [Parameter(Mandatory = $true)] [string[]] $Paths
)

$ErrorActionPreference = "Stop"
$thumbprint = $env:SIGNING_CERTIFICATE_THUMBPRINT
if ([string]::IsNullOrWhiteSpace($thumbprint)) {
    if ($env:REQUIRE_CODE_SIGNING -eq "1") {
        throw "SIGNING_CERTIFICATE_THUMBPRINT is required for a production release."
    }
    Write-Host "Code signing skipped: SIGNING_CERTIFICATE_THUMBPRINT is not configured."
    return
}

$signTool = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin" -Filter signtool.exe -Recurse |
    Where-Object { $_.FullName -match '\\x64\\signtool\.exe$' } |
    Sort-Object FullName -Descending |
    Select-Object -First 1
if (-not $signTool) { throw "The Windows SDK signing tool was not found." }

foreach ($path in $Paths) {
    if (-not (Test-Path $path)) { throw "Signing input does not exist: $path" }
    & $signTool.FullName sign /sha1 $thumbprint /fd SHA256 /tr "http://timestamp.digicert.com" /td SHA256 $path
    if ($LASTEXITCODE -ne 0) { throw "Authenticode signing failed for $path." }
    & $signTool.FullName verify /pa /all $path
    if ($LASTEXITCODE -ne 0) { throw "Authenticode verification failed for $path." }
}
