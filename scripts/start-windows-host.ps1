param(
  [ValidateSet('Debug', 'Release')][string]$Configuration = 'Debug',
  [string]$Listen = '127.0.0.1:21120',
  [string]$Allow = '',
  [string]$Rendezvous = '',
  [string]$ServerKey = '',
  [string]$Relay = '',
  [switch]$Unattended, [switch]$Video,
  [Alias('Input')][switch]$EnableInput
)

$ErrorActionPreference = 'Stop'
if ($Rendezvous -or $ServerKey -or $Relay) {
  if (-not $Rendezvous -or -not $ServerKey -or -not $Relay -or -not $Allow) { throw 'ID access requires -Rendezvous host:port -ServerKey base64 -Relay host:port and explicit -Allow controller source IPs.' }
  if ([Convert]::FromBase64String($ServerKey).Length -ne 32) { throw 'ServerKey must encode the pinned 32-byte hbbs public key.' }
}
if ($EnableInput -and -not $Video) { throw 'Keyboard and mouse require the video session; specify -Video with -Input.' }
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
foreach ($hostKey in @('ORD_SECURE_UNATTENDED', 'ORD_SECURE_RENDEZVOUS', 'ORD_SECURE_SERVER_KEY', 'ORD_SECURE_RELAY', 'ORD_SECURE_ID_PROFILE_OUT', 'ORD_SECURE_LISTEN', 'ORD_SECURE_ALLOW', 'ORD_SECURE_PROFILE_OUT', 'ORD_SECURE_VIDEO', 'ORD_SECURE_INPUT')) {
  $hostSaved[$hostKey] = [Environment]::GetEnvironmentVariable($hostKey, 'Process')
}
try {
  $env:ORD_SECURE_UNATTENDED = if ($Unattended) { '1' } else { '0' }
  $env:ORD_SECURE_RENDEZVOUS = $Rendezvous
  $env:ORD_SECURE_SERVER_KEY = $ServerKey
  $env:ORD_SECURE_RELAY = $Relay
  $env:ORD_SECURE_LISTEN = $Listen
  $env:ORD_SECURE_ALLOW = $Allow
  $env:ORD_SECURE_PROFILE_OUT = $hostProfile
  $env:ORD_SECURE_ID_PROFILE_OUT = Join-Path (Split-Path $hostProfile -Parent) 'official-current-id-profile.json'
  if ($Rendezvous) { Write-Output "ID public profile (after registration): $env:ORD_SECURE_ID_PROFILE_OUT" }
  $env:ORD_SECURE_VIDEO = if ($Video) { '1' } else { '0' }
  $env:ORD_SECURE_INPUT = if ($EnableInput) { '1' } else { '0' }
  $hostServer = Start-Process -FilePath $hostExe -ArgumentList '--server' -WorkingDirectory $hostBundle -WindowStyle Hidden -PassThru
  $hostWindow = Start-Process -FilePath $hostExe -ArgumentList @('--cm', '--ord-secure-ui') -WorkingDirectory $hostBundle -WindowStyle Normal -PassThru
  Write-Output "Flutter host server PID: $($hostServer.Id); workspace PID: $($hostWindow.Id)"
  Write-Output "Public profile: $hostProfile"
  Write-Output 'Approve each request in the Flutter workspace. Video is enabled only with -Video.'
  Write-Output 'After local connection approval, the controller chooses keyboard/mouse on or off; -Input enables that capability.'
} finally {
  foreach ($hostKey in $hostSaved.Keys) {
    if ($null -eq $hostSaved[$hostKey]) { Remove-Item -LiteralPath "Env:$hostKey" -ErrorAction SilentlyContinue }
    else { Set-Item -LiteralPath "Env:$hostKey" -Value $hostSaved[$hostKey] }
  }
}
