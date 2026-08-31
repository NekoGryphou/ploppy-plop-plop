$ErrorActionPreference = "Stop"
$ProjectDirectory = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$Timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
$OutputDirectory = Join-Path $ProjectDirectory "out\diagnostics\DeckyMyRigHost-$Timestamp"
New-Item -ItemType Directory -Force $OutputDirectory | Out-Null

& (Join-Path $PSScriptRoot "validate-windows.ps1") | Set-Content (Join-Path $OutputDirectory "validation.json")
Get-Service DeckyMyRigHost -ErrorAction SilentlyContinue | Format-List * | Out-String | Set-Content (Join-Path $OutputDirectory "service.txt")
Get-NetTCPConnection -State Listen -ErrorAction SilentlyContinue | Where-Object OwningProcess -ne 0 | Select-Object LocalAddress,LocalPort,OwningProcess | ConvertTo-Json | Set-Content (Join-Path $OutputDirectory "listening-ports.json")
Get-NetFirewallRule -DisplayName DeckyMyRigHost -ErrorAction SilentlyContinue | Select-Object DisplayName,Enabled,Profile,Direction,Action | ConvertTo-Json | Set-Content (Join-Path $OutputDirectory "firewall.json")
$LogPath = Join-Path $env:ProgramData "DeckyMyRigHost\DeckyMyRigHost.log"
if (Test-Path $LogPath) { Get-Content $LogPath -Tail 500 | Set-Content (Join-Path $OutputDirectory "host-log-tail.txt") }
$ConfigPath = Join-Path $env:ProgramFiles "DeckyMyRigHost\DeckyMyRigHost.toml"
if (Test-Path $ConfigPath) { Select-String -Path $ConfigPath -Pattern '^\s*port\s*=\s*\d+\s*$' | ForEach-Object Line | Set-Content (Join-Path $OutputDirectory "DeckyMyRigHost.sanitized.toml") }
$Archive = "$OutputDirectory.zip"
Compress-Archive -Path "$OutputDirectory\*" -DestinationPath $Archive
Write-Host "Non-sensitive diagnostic bundle: $Archive"
Write-Host "Pairing credentials and pairing codes were not collected."
