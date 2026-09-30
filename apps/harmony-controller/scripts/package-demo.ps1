param([switch]$Build, [string]$DevEcoRoot = 'G:\Huawei\DevEco Studio')

$ErrorActionPreference = 'Stop'
$demoRepository = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..\..'))
$demoHostRoot = Join-Path $demoRepository 'apps\windows-demo-host'
$demoControllerRoot = Join-Path $demoRepository 'apps\harmony-controller'
if ($Build) {
  & (Join-Path $demoHostRoot 'scripts\build.ps1')
  & (Join-Path $PSScriptRoot 'build.ps1') -DevEcoRoot $DevEcoRoot
}
$demoExe = Join-Path $demoHostRoot 'target\release\windows-demo-host.exe'
$demoHap = Join-Path $demoControllerRoot 'entry\build\default\outputs\default\entry-default-unsigned.hap'
foreach ($demoInput in @($demoExe, $demoHap)) {
  if (-not (Test-Path -LiteralPath $demoInput)) { throw "Missing DEMO build: $demoInput. Run with -Build." }
}
$demoOutput = Join-Path $demoControllerRoot 'artifacts\interactive-demo'
New-Item -ItemType Directory -Path $demoOutput -Force | Out-Null
Copy-Item -LiteralPath $demoExe -Destination (Join-Path $demoOutput 'windows-demo-host.exe') -Force
Copy-Item -LiteralPath $demoHap -Destination (Join-Path $demoOutput 'harmony-controller-unsigned.hap') -Force
Copy-Item -LiteralPath (Join-Path $demoRepository 'docs\project\DEMO.md') -Destination (Join-Path $demoOutput 'README.md') -Force
Copy-Item -LiteralPath (Join-Path $demoRepository 'LICENCE') -Destination (Join-Path $demoOutput 'LICENCE') -Force
$demoHashes = @('windows-demo-host.exe', 'harmony-controller-unsigned.hap') |
  ForEach-Object { Get-FileHash -LiteralPath (Join-Path $demoOutput $_) -Algorithm SHA256 }
$demoHashes | ForEach-Object { "$($_.Hash)  $([IO.Path]::GetFileName($_.Path))" } |
  Set-Content -LiteralPath (Join-Path $demoOutput 'SHA256SUMS.txt') -Encoding utf8
Write-Output "Local DEMO: $demoOutput"
