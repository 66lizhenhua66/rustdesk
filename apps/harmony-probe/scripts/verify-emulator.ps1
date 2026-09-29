param(
  [string]$DevEcoRoot = 'G:\Huawei\DevEco Studio',
  [string]$Device = '127.0.0.1:5555'
)

$ErrorActionPreference = 'Stop'
$probeRoot = Split-Path -Parent $PSScriptRoot
$probeHdc = Join-Path $DevEcoRoot 'sdk\default\openharmony\toolchains\hdc.exe'
$probeArtifacts = Join-Path $probeRoot 'artifacts'
New-Item -ItemType Directory -Path $probeArtifacts -Force | Out-Null

function Invoke-ProbeHdc {
  param([string[]]$HdcArguments)
  $probeResult = & $probeHdc -t $Device @HdcArguments 2>&1
  if ($LASTEXITCODE -ne 0) { throw "HDC failed: $($probeResult -join ' ')" }
  $probeText = $probeResult -join "`n"
  if ($probeText -match '(?im)^\s*(error:|\[Fail\])') { throw "Device command failed: $probeText" }
  return $probeText
}

function Get-ProbeNodes($Node) {
  if ($Node.attributes.type -in @('Text', 'Button', 'Progress')) { $Node.attributes }
  foreach ($probeChild in $Node.children) { Get-ProbeNodes $probeChild }
}

function Read-ProbeLayout {
  $null = Invoke-ProbeHdc @('shell', 'uitest', 'dumpLayout', '-p', '/data/local/tmp/bridgeprobe-layout.json')
  $probeLocalLayout = Join-Path $probeArtifacts 'layout.json'
  $null = Invoke-ProbeHdc @('file', 'recv', '/data/local/tmp/bridgeprobe-layout.json', $probeLocalLayout)
  $probeTree = Get-Content -LiteralPath $probeLocalLayout -Raw -Encoding UTF8 | ConvertFrom-Json
  $probeApp = @($probeTree.children | Where-Object { $_.attributes.bundleName -eq 'com.openremotedesk.bridgeprobe' })
  if ($probeApp.Count -ne 1) { throw 'Probe application is not the visible target.' }
  return @(Get-ProbeNodes $probeApp[0])
}

function Click-ProbeButton([string]$Label) {
  $probeButton = @(Read-ProbeLayout | Where-Object { $_.type -eq 'Button' -and $_.text -eq $Label -and $_.enabled -eq 'true' })
  if ($probeButton.Count -ne 1) { throw "Expected enabled button: $Label" }
  $probeBounds = [regex]::Matches($probeButton[0].bounds, '\d+') | ForEach-Object { [int]$_.Value }
  if ($probeBounds.Count -ne 4) { throw 'Unexpected button bounds.' }
  $probeX = [int](($probeBounds[0] + $probeBounds[2]) / 2)
  $probeY = [int](($probeBounds[1] + $probeBounds[3]) / 2)
  $null = Invoke-ProbeHdc @('shell', 'uitest', 'uiInput', 'click', "$probeX", "$probeY")
}

function Wait-ProbeChecks {
  $probeDeadline = [DateTime]::UtcNow.AddSeconds(12)
  do {
    $probeNodes = Read-ProbeLayout
    if ($probeNodes.text -contains '自动验证通过 6/6') { return }
    if ($probeNodes.text -match '验证未通过|验证异常') { throw 'On-device bridge assertions failed.' }
    Start-Sleep -Milliseconds 200
  } while ([DateTime]::UtcNow -lt $probeDeadline)
  throw 'Timed out waiting for the six on-device checks.'
}

function Read-ProbeProgress {
  $probeProgress = @(Read-ProbeLayout | Where-Object { $_.type -eq 'Progress' })
  if ($probeProgress.Count -ne 1) { throw 'Expected one native task progress indicator.' }
  return [double]::Parse($probeProgress[0].text, [Globalization.CultureInfo]::InvariantCulture)
}

$probeCases = [System.Collections.Generic.List[string]]::new()
$null = Invoke-ProbeHdc @('shell', 'aa', 'start', '-a', 'EntryAbility', '-b', 'com.openremotedesk.bridgeprobe')
Wait-ProbeChecks
$probeCases.Add('startup: six ArkUI/NAPI/Rust assertions passed')

Click-ProbeButton '开始后台任务'
Start-Sleep -Milliseconds 300
$probeBefore = Read-ProbeProgress
Click-ProbeButton '取消任务'
$probeAfterCancel = Read-ProbeProgress
Start-Sleep -Milliseconds 350
$probeStable = Read-ProbeProgress
if ($probeBefore -le 0 -or $probeBefore -ge 100 -or $probeAfterCancel -ne $probeStable) {
  throw 'Manual task was not responsive to cancellation or kept progressing afterwards.'
}
$probeCases.Add("manual UI cancellation: progress stopped at $probeStable")

Click-ProbeButton '重新运行自动验证'
$null = Invoke-ProbeHdc @('shell', 'uitest', 'uiInput', 'keyEvent', 'Home')
$null = Invoke-ProbeHdc @('shell', 'aa', 'start', '-a', 'EntryAbility', '-b', 'com.openremotedesk.bridgeprobe')
Wait-ProbeChecks
Click-ProbeButton '开始后台任务'
$probeResumeBefore = Read-ProbeProgress
Start-Sleep -Milliseconds 3500
$probeResumeAfter = Read-ProbeProgress
if ($probeResumeAfter -le $probeResumeBefore -or $probeResumeAfter -ge 100) {
  throw 'Old page timeout interfered with the resumed task.'
}
Click-ProbeButton '释放资源'
$probeCases.Add('background/foreground: resumed checks passed and task survived old timeout window')

for ($probeRound = 1; $probeRound -le 3; $probeRound++) {
  Click-ProbeButton '开始后台任务'
  $null = Invoke-ProbeHdc @('shell', 'uitest', 'uiInput', 'keyEvent', 'Back')
  $null = Invoke-ProbeHdc @('shell', 'aa', 'start', '-a', 'EntryAbility', '-b', 'com.openremotedesk.bridgeprobe')
  Wait-ProbeChecks
  $probeCases.Add("page exit/re-entry round ${probeRound}: six assertions passed")
}

$probeLog = Invoke-ProbeHdc @('shell', 'hilog', '-x', '-T', 'BridgeProbe')
$probeLog | Set-Content -LiteralPath (Join-Path $probeArtifacts 'bridge-probe.log') -Encoding UTF8
$null = Invoke-ProbeHdc @('shell', 'snapshot_display', '-f', '/data/local/tmp/bridgeprobe.jpeg')
$null = Invoke-ProbeHdc @('file', 'recv', '/data/local/tmp/bridgeprobe.jpeg', (Join-Path $probeArtifacts 'bridge-probe.jpeg'))
if (-not (Test-Path -LiteralPath (Join-Path $probeArtifacts 'bridge-probe.jpeg'))) { throw 'Screenshot was not received.' }
$probeCases | Set-Content -LiteralPath (Join-Path $probeArtifacts 'emulator-results.txt') -Encoding UTF8
$probeCases
