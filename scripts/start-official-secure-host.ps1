param([string]$Listen = '127.0.0.1:21120', [string]$Allow = '')

$ErrorActionPreference = 'Stop'
$cmRepository = Split-Path $PSScriptRoot -Parent
$cmExe = Join-Path $cmRepository 'target\debug\rustdesk.exe'
$cmRuntime = Join-Path $cmRepository 'target\debug\sciter.dll'
$cmArtifacts = Join-Path $cmRepository 'apps\harmony-controller\artifacts'
foreach ($cmRequired in @($cmExe, $cmRuntime)) {
  if (-not (Test-Path -LiteralPath $cmRequired)) { throw "Missing $cmRequired; build the official Debug host and provide Sciter first." }
}
$cmEndpoint = [Net.IPEndPoint]::Parse($Listen)
if ($cmEndpoint.Port -eq 0 -or $cmEndpoint.Address.Equals([Net.IPAddress]::Any) -or $cmEndpoint.Address.Equals([Net.IPAddress]::IPv6Any)) { throw 'A concrete IP and nonzero port are required.' }
if (-not [Net.IPAddress]::IsLoopback($cmEndpoint.Address) -and [string]::IsNullOrWhiteSpace($Allow)) { throw 'LAN listening requires explicit -Allow source IPs.' }
New-Item -ItemType Directory -Force -Path $cmArtifacts | Out-Null
$cmProfile = Join-Path $cmArtifacts 'official-current-profile.json'
$cmEnv = @{}
foreach ($cmKey in @('ORD_SECURE_LISTEN', 'ORD_SECURE_ALLOW', 'ORD_SECURE_PROFILE_OUT')) { $cmEnv[$cmKey] = [Environment]::GetEnvironmentVariable($cmKey, 'Process') }
try {
  $env:ORD_SECURE_LISTEN = $Listen
  $env:ORD_SECURE_ALLOW = $Allow
  $env:ORD_SECURE_PROFILE_OUT = $cmProfile
  $cmServer = Start-Process -FilePath $cmExe -ArgumentList '--server' -WorkingDirectory $cmRepository -WindowStyle Hidden -PassThru
  $cmWindow = Start-Process -FilePath $cmExe -ArgumentList '--cm' -WorkingDirectory $cmRepository -WindowStyle Normal -PassThru
  Write-Output "Server PID: $($cmServer.Id); CM PID: $($cmWindow.Id)"
  Write-Output "Public profile will be exported to: $cmProfile"
  Write-Output 'Use the controller to connect; approve locally in CM. This script does not approve connections.'
} finally {
  foreach ($cmKey in $cmEnv.Keys) {
    if ($null -eq $cmEnv[$cmKey]) { Remove-Item -LiteralPath "Env:$cmKey" -ErrorAction SilentlyContinue } else { Set-Item -LiteralPath "Env:$cmKey" -Value $cmEnv[$cmKey] }
  }
}
