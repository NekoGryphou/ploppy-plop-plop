$ErrorActionPreference = "Stop"
$ProjectDirectory = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$HostExecutable = Join-Path $ProjectDirectory "out\host\DeckyPowerHost.exe"
$ClientExecutable = Join-Path $ProjectDirectory "tools\decky-power-test\target\release\decky-power-test.exe"
if (-not (Test-Path $HostExecutable)) { throw "Run scripts\build-windows.ps1 first (host missing)." }
if (-not (Test-Path $ClientExecutable)) { throw "Run scripts\build-windows.ps1 first (client missing)." }

$listener = [System.Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
$listener.Start()
$port = ([Net.IPEndPoint]$listener.LocalEndpoint).Port
$listener.Stop()
$directory = Join-Path ([IO.Path]::GetTempPath()) ("decky-power-windows-e2e-" + [Guid]::NewGuid().ToString("N"))
$config = Join-Path $directory "DeckyPowerHost.toml"
$credential = Join-Path $directory "credential.json"
New-Item -ItemType Directory $directory | Out-Null
Set-Content -Path $config -Value "port = $port" -Encoding ascii
$hostProcess = Start-Process $HostExecutable -ArgumentList "--dev", "--mock-shutdown", "--config", $config, "--pairing-code-value", "483921" -PassThru
try {
    $ready = $false
    foreach ($attempt in 1..40) {
        if (Test-NetConnection 127.0.0.1 -Port $port -InformationLevel Quiet -WarningAction SilentlyContinue) { $ready = $true; break }
        Start-Sleep -Milliseconds 250
    }
    if (-not $ready) { throw "Safe Windows host did not bind port $port." }
    & $ClientExecutable pair --host 127.0.0.1 --port $port --code 483921 --credential-file $credential
    if ($LASTEXITCODE -ne 0) { throw "Pair failed." }
    & $ClientExecutable status --host 127.0.0.1 --port $port --credential-file $credential
    if ($LASTEXITCODE -ne 0) { throw "Status failed." }
    & $ClientExecutable shutdown --host 127.0.0.1 --port $port --credential-file $credential
    if ($LASTEXITCODE -ne 0) { throw "Mock shutdown failed." }
    if ($hostProcess.HasExited) { throw "Safe host exited after mock shutdown." }
    Write-Host "Safe Windows pair/status/mock-shutdown E2E: PASS"
} finally {
    Stop-Process -Id $hostProcess.Id -Force -ErrorAction SilentlyContinue
    Remove-Item $directory -Recurse -Force -ErrorAction SilentlyContinue
}
