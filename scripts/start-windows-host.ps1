param(
  [ValidateSet('Debug', 'Release')][string]$Configuration = 'Debug',
  [string]$Listen = '127.0.0.1:21120',
  [string]$Allow = '',
  [switch]$Video
)

$ErrorActionPreference = 'Stop'
$hostRepository = Split-Path $PSScriptRoot -Parent
$hostBundle = Join-Path $hostRepository "flutter\build\windows\x64\runner\$Configuration"
$hostExe = Join-Path $hostBundle 'rustdesk.exe'
foreach ($hostFile in @('rustdesk.exe', 'librustdesk.dll', 'flutter_windows.dll')) {
  if (-not (Test-Path -LiteralPath (Join-Path $hostBundle $hostFile))) { throw "Build the Flutter host first; missing $hostFile" }
}
$hostEndpoint = [Net.IPEndPoint]::Parse($Listen)
if ($hostEndpoint.Port -eq 0 -or $hostEndpoint.Address.Equals([Net.IPAddress]::Any) -or $hostEndpoint.Address.Equals([Net.IPAddress]::IPv6Any)) {
  throw 'A concrete IP and nonzero port are required.'
}
if (-not [Net.IPAddress]::IsLoopback($hostEndpoint.Address) -and [string]::IsNullOrWhiteSpace($Allow)) {
  throw 'Non-loopback listening requires explicit -Allow source IPs.'
}
$hostArtifacts = Join-Path $hostRepository 'apps\harmony-controller\artifacts'
New-Item -ItemType Directory -Force -Path $hostArtifacts | Out-Null
$hostProfile = Join-Path $hostArtifacts 'official-current-profile.json'
$hostSaved = @{}
foreach ($hostKey in @('ORD_SECURE_LISTEN', 'ORD_SECURE_ALLOW', 'ORD_SECURE_PROFILE_OUT', 'ORD_SECURE_VIDEO')) {
  $hostSaved[$hostKey] = [Environment]::GetEnvironmentVariable($hostKey, 'Process')
}
try {
  $env:ORD_SECURE_LISTEN = $Listen
  $env:ORD_SECURE_ALLOW = $Allow
  $env:ORD_SECURE_PROFILE_OUT = $hostProfile
  $env:ORD_SECURE_VIDEO = if ($Video) { '1' } else { '0' }
  $hostServer = Start-Process -FilePath $hostExe -ArgumentList '--server' -WorkingDirectory $hostBundle -WindowStyle Hidden -PassThru
  $hostWindow = Start-Process -FilePath $hostExe -ArgumentList @('--cm', '--ord-secure-ui') -WorkingDirectory $hostBundle -WindowStyle Normal -PassThru
  Write-Output "Flutter host server PID: $($hostServer.Id); workspace PID: $($hostWindow.Id)"
  Write-Output "Public profile: $hostProfile"
  Write-Output 'Approve each request in the Flutter workspace. Video is enabled only with -Video.'
} finally {
  foreach ($hostKey in $hostSaved.Keys) {
    if ($null -eq $hostSaved[$hostKey]) { Remove-Item -LiteralPath "Env:$hostKey" -ErrorAction SilentlyContinue }
    else { Set-Item -LiteralPath "Env:$hostKey" -Value $hostSaved[$hostKey] }
  }
}
