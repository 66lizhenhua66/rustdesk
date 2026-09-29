param(
  [string]$DevEcoRoot = 'G:\Huawei\DevEco Studio',
  [ValidateSet('all', 'x86_64', 'arm64-v8a')][string]$Abi = 'all',
  [switch]$SkipHap
)

$ErrorActionPreference = 'Stop'
$probeRoot = Split-Path -Parent $PSScriptRoot
$probeSdk = Join-Path $DevEcoRoot 'sdk'
$probeNative = Join-Path $probeSdk 'default\openharmony\native'
$probeNode = Join-Path $DevEcoRoot 'tools\node\node.exe'
$probeHvigor = Join-Path $DevEcoRoot 'tools\hvigor\bin\hvigorw.js'
$probeOhpm = Join-Path $DevEcoRoot 'tools\ohpm\bin\pm-cli.js'
foreach ($probeRequired in @($probeNative, $probeNode, $probeHvigor, $probeOhpm)) {
  if (-not (Test-Path -LiteralPath $probeRequired)) { throw "Required tool missing: $probeRequired" }
}
$env:DEVECO_SDK_HOME = $probeSdk
$env:JAVA_HOME = Join-Path $DevEcoRoot 'jbr'
$env:PATH = (Join-Path $DevEcoRoot 'tools\node') + ';' + (Join-Path $env:JAVA_HOME 'bin') + ';' + $env:PATH
$probeTargets = if ($Abi -eq 'all') {
  @('x86_64-unknown-linux-ohos', 'aarch64-unknown-linux-ohos')
} elseif ($Abi -eq 'x86_64') { @('x86_64-unknown-linux-ohos') } else { @('aarch64-unknown-linux-ohos') }

Push-Location $probeRoot
try {
  foreach ($probeTarget in $probeTargets) {
    & cargo build --manifest-path native/rust/Cargo.toml --target $probeTarget --release --locked
    if ($LASTEXITCODE -ne 0) { throw "Rust build failed: $probeTarget" }
  }
  if ($SkipHap) { return }
  if ($Abi -ne 'all') { throw 'The HAP includes both ABIs. Use -Abi all to assemble, or -SkipHap for one native target.' }
  & $probeNode $probeOhpm install
  if ($LASTEXITCODE -ne 0) { throw 'ohpm install failed' }
  & $probeNode $probeHvigor --mode module -p module=entry@default -p product=default assembleHap --no-daemon
  if ($LASTEXITCODE -ne 0) { throw 'assembleHap failed' }
} finally {
  Pop-Location
}
