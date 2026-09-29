param([string]$DevEcoRoot = 'G:\Huawei\DevEco Studio')

$ErrorActionPreference = 'Stop'
$controllerRoot = Split-Path -Parent $PSScriptRoot
$repositoryRoot = [IO.Path]::GetFullPath((Join-Path $controllerRoot '..\..'))
$controllerCore = Join-Path $repositoryRoot 'libs\controller-core\Cargo.toml'
$controllerNode = Join-Path $DevEcoRoot 'tools\node\node.exe'
$controllerHvigor = Join-Path $DevEcoRoot 'tools\hvigor\bin\hvigorw.js'
$controllerOhpm = Join-Path $DevEcoRoot 'tools\ohpm\bin\pm-cli.js'
$controllerSdk = Join-Path $DevEcoRoot 'sdk'
foreach ($controllerRequired in @($controllerCore, $controllerNode, $controllerHvigor, $controllerOhpm, $controllerSdk)) {
  if (-not (Test-Path -LiteralPath $controllerRequired)) { throw "Required input missing: $controllerRequired" }
}
$env:DEVECO_SDK_HOME = $controllerSdk
$env:JAVA_HOME = Join-Path $DevEcoRoot 'jbr'
$env:PATH = (Join-Path $DevEcoRoot 'tools\node') + ';' + (Join-Path $env:JAVA_HOME 'bin') + ';' + $env:PATH
foreach ($controllerTarget in @('x86_64-unknown-linux-ohos', 'aarch64-unknown-linux-ohos')) {
  & cargo build --manifest-path $controllerCore --target $controllerTarget --release --locked
  if ($LASTEXITCODE -ne 0) { throw "Shared controller build failed: $controllerTarget" }
}
Push-Location $controllerRoot
try {
  & $controllerNode $controllerOhpm install
  if ($LASTEXITCODE -ne 0) { throw 'ohpm install failed' }
  & $controllerNode $controllerHvigor --mode module -p module=entry@default -p product=default assembleHap --no-daemon
  if ($LASTEXITCODE -ne 0) { throw 'assembleHap failed' }
} finally {
  Pop-Location
}
