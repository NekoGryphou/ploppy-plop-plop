param([switch]$AllowShutdownTest)
$ErrorActionPreference = "Stop"
$InstallDirectory = Join-Path $env:ProgramFiles "DeckyMyRigHost"
$ConfigPath = Join-Path $InstallDirectory "DeckyMyRigHost.toml"
$HostPath = Join-Path $InstallDirectory "DeckyMyRigHost.exe"
$ControlPath = Join-Path $InstallDirectory "DeckyMyRigHostControl.exe"
$results = [ordered]@{
    Timestamp = (Get-Date).ToString("o")
    Os = [Environment]::OSVersion.VersionString
    HostExecutableExists = Test-Path $HostPath
    HostVersion = $null
    ControlExecutableExists = Test-Path $ControlPath
    ControlVersion = $null
    ConfigExists = Test-Path $ConfigPath
    ConfigValid = $false
    ServiceInstalled = $false
    ServiceStatus = "Not installed"
    ConfiguredPort = $null
    PortListening = $false
    LocalStatusHttpStatus = $null
    LocalStatusEndpointReachable = $false
    ManagementPipeExists = $false
    FirewallRuleExists = $false
    FirewallProfiles = @()
    ShutdownTest = "NOT EXECUTED"
}
if ($results.HostExecutableExists) { $results.HostVersion = (Get-Item $HostPath).VersionInfo.FileVersion }
if ($results.ControlExecutableExists) { $results.ControlVersion = (Get-Item $ControlPath).VersionInfo.FileVersion }
if ($results.ConfigExists) {
    $match = Select-String -Path $ConfigPath -Pattern '^\s*port\s*=\s*(\d+)\s*$' | Select-Object -First 1
    if ($match) {
        $candidatePort = [int]$match.Matches[0].Groups[1].Value
        if ($candidatePort -ge 1 -and $candidatePort -le 65535) {
            $results.ConfiguredPort = $candidatePort
            $results.ConfigValid = $true
        }
    }
}
$service = Get-Service DeckyMyRigHost -ErrorAction SilentlyContinue
if ($service) { $results.ServiceInstalled = $true; $results.ServiceStatus = $service.Status.ToString() }
if ($results.ConfiguredPort) {
    $results.PortListening = [bool](Get-NetTCPConnection -State Listen -LocalPort $results.ConfiguredPort -ErrorAction SilentlyContinue)
    try {
        Invoke-WebRequest -UseBasicParsing -Method Post -Uri "http://127.0.0.1:$($results.ConfiguredPort)/v1/status" -ContentType "application/x-protobuf" -Body ([byte[]](0x08, 0x01)) -TimeoutSec 3 | Out-Null
        $results.LocalStatusHttpStatus = 200
    } catch {
        if ($_.Exception.Response) { $results.LocalStatusHttpStatus = [int]$_.Exception.Response.StatusCode }
    }
    $results.LocalStatusEndpointReachable = $results.LocalStatusHttpStatus -in @(200, 401)
}
$results.ManagementPipeExists = [bool](Get-ChildItem "\\.\pipe\" -ErrorAction SilentlyContinue | Where-Object Name -eq "DeckyMyRigHostControl")
$firewallRules = @(Get-NetFirewallRule -DisplayName DeckyMyRigHost -ErrorAction SilentlyContinue)
$results.FirewallRuleExists = $firewallRules.Count -gt 0
$results.FirewallProfiles = @($firewallRules | ForEach-Object Profile | ForEach-Object { $_.ToString() })
if ($AllowShutdownTest) {
    Write-Warning "A real shutdown validation is destructive and is intentionally not automated by this safe script. Follow docs/WINDOWS_VALIDATION.md on a disposable test PC."
    $results.ShutdownTest = "REQUIRES EXPLICIT MANUAL WINDOWS VALIDATION"
}
$results | ConvertTo-Json -Depth 3
