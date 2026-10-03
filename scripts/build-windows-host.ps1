param(
  [ValidateSet('Debug', 'Release')][string]$Configuration = 'Debug',
  [string]$FlutterRoot = '',
  [string]$PubCache = '',
  [string]$BridgeTools = '',
  [string]$VcpkgRoot = $env:VCPKG_ROOT,
  [string]$LibClangPath = 'G:\Huawei\DevEco Studio\sdk\default\openharmony\native\llvm\bin',
  [switch]$Offline,
  [switch]$GenerateBridge,
  [switch]$Test
)

$ErrorActionPreference = 'Stop'
$hostRepository = Split-Path $PSScriptRoot -Parent
$hostCache = Join-Path $hostRepository 'apps\harmony-controller\artifacts\flutter-build'
if (-not $FlutterRoot) {
  $FlutterRoot = Join-Path (Split-Path $hostRepository -Parent) '.tools\flutter-3.24.5\flutter'
}
if (-not $PubCache) { $PubCache = Join-Path $hostCache 'pub-cache' }
if (-not $BridgeTools) { $BridgeTools = Join-Path $hostCache 'tools\bin' }
if (-not $VcpkgRoot) { $VcpkgRoot = Join-Path $hostRepository 'apps\harmony-controller\artifacts\official-build\vcpkg' }
$hostFlutter = Join-Path $FlutterRoot 'bin\flutter.bat'
$hostCodegen = Join-Path $BridgeTools 'flutter_rust_bridge_codegen.exe'
foreach ($hostRequired in @($hostFlutter, (Join-Path $LibClangPath 'libclang.dll'), (Join-Path $VcpkgRoot 'installed\x64-windows-static\include\vpx\vpx_encoder.h'))) {
  if (-not (Test-Path -LiteralPath $hostRequired)) { throw "Missing build input: $hostRequired" }
}
$hostVersion = Get-Content -LiteralPath (Join-Path $FlutterRoot 'bin\cache\flutter.version.json') -Raw | ConvertFrom-Json
if ($hostVersion.frameworkVersion -ne '3.24.5') { throw 'Use Flutter 3.24.5 for this Windows x64 build.' }
$hostSaved = @{}
foreach ($hostKey in @('VCPKG_ROOT', 'LIBCLANG_PATH', 'BINDGEN_EXTRA_CLANG_ARGS_x86_64_pc_windows_msvc', 'INCLUDE', 'LIB', 'LIBPATH', 'PATH', 'VCINSTALLDIR', 'VCToolsInstallDir', 'WindowsSdkDir', 'WindowsSDKVersion', 'UCRTVersion', 'PUB_CACHE', 'PUB_HOSTED_URL', 'CARGO_NET_OFFLINE')) {
  $hostSaved[$hostKey] = [Environment]::GetEnvironmentVariable($hostKey, 'Process')
}
try {
  $hostVsWhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
  $hostVsRoot = & $hostVsWhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
  if (-not $hostVsRoot) { throw 'Visual Studio C++ build tools are required.' }
  $hostDevCmd = '"' + (Join-Path $hostVsRoot 'Common7\Tools\VsDevCmd.bat') + '" -no_logo -arch=amd64 -host_arch=amd64 && set'
  $hostDevEnvironment = & $env:ComSpec /d /s /c $hostDevCmd
  if ($LASTEXITCODE -ne 0) { throw 'MSVC environment initialization failed.' }
  foreach ($hostLine in $hostDevEnvironment) {
    if ($hostLine -match '^(INCLUDE|LIB|LIBPATH|PATH|VCINSTALLDIR|VCToolsInstallDir|WindowsSdkDir|WindowsSDKVersion|UCRTVersion)=(.*)$') {
      [Environment]::SetEnvironmentVariable($Matches[1], $Matches[2], 'Process')
    }
  }
  $env:PATH = $BridgeTools + ';' + (Join-Path $FlutterRoot 'bin') + ';' + $env:PATH
  $env:PUB_CACHE = $PubCache
  $env:PUB_HOSTED_URL = 'https://pub.dev'
  if ($Offline) { $env:CARGO_NET_OFFLINE = 'true' }
  $env:VCPKG_ROOT = $VcpkgRoot
  $env:LIBCLANG_PATH = $LibClangPath
  $hostClangResources = & (Join-Path $LibClangPath 'clang.exe') -print-resource-dir
  if ($LASTEXITCODE -ne 0) { throw 'Could not locate Clang resources.' }
  $env:BINDGEN_EXTRA_CLANG_ARGS_x86_64_pc_windows_msvc = $hostSaved['BINDGEN_EXTRA_CLANG_ARGS_x86_64_pc_windows_msvc'] + ' --target=x86_64-pc-windows-msvc -resource-dir="' + $hostClangResources + '"'
  Push-Location (Join-Path $hostRepository 'flutter')
  try {
    $hostPubArgs = @('pub', 'get', '--enforce-lockfile')
    if ($Offline) { $hostPubArgs += '--offline' }
    & $hostFlutter @hostPubArgs
    if ($LASTEXITCODE -ne 0) { throw 'Locked Flutter dependency resolution failed.' }
  } finally { Pop-Location }

  Push-Location $hostRepository
  try {
    if ($GenerateBridge -or -not (Test-Path 'src\bridge_generated.rs') -or -not (Test-Path 'flutter\lib\generated_bridge.freezed.dart')) {
      if (-not (Test-Path -LiteralPath $hostCodegen)) { throw 'Install the pinned flutter_rust_bridge_codegen 1.80.1 and cargo-expand 1.0.95 first.' }
      $hostFfigenOptions = '--target=x86_64-pc-windows-msvc -resource-dir "' + $hostClangResources + '"'
      $hostBridgeOutput = & $hostCodegen --rust-input ./src/flutter_ffi.rs --dart-output ./flutter/lib/generated_bridge.dart --c-output ./flutter/macos/Runner/bridge_generated.h --llvm-path (Split-Path $LibClangPath -Parent) "--llvm-compiler-opts=$hostFfigenOptions" --skip-add-mod-to-lib 2>&1
      $hostBridgeExit = $LASTEXITCODE
      $hostBridgeOutput | Write-Output
      if ($hostBridgeExit -ne 0 -or ($hostBridgeOutput -match '\[SEVERE\]|fatal error:')) { throw 'Flutter Rust Bridge generation failed.' }
      Copy-Item -LiteralPath flutter/macos/Runner/bridge_generated.h -Destination flutter/ios/Runner/bridge_generated.h
    }
    if (Select-String -LiteralPath flutter/lib/generated_bridge.dart -Pattern '^typedef bool\s*=' -Quiet) {
      throw 'Invalid FFI bool binding; regenerate the bridge with the Clang standard headers available.'
    }
    if ($Test) {
      foreach ($hostFilter in @('secure_host_ui::tests', 'cm_approval_requires_this_pending_login_once', 'secure_video::tests', 'secure_input::tests')) {
        & cargo test --locked -p rustdesk --lib --features flutter,ord-secure-host $hostFilter -- --test-threads=1
        if ($LASTEXITCODE -ne 0) { throw "Rust test failed: $hostFilter" }
      }
    } else {
      $hostCargoArgs = @('build', '--locked', '-p', 'rustdesk', '--lib', '--features', 'flutter,ord-secure-host')
      if ($Configuration -eq 'Release') { $hostCargoArgs += '--release' }
      & cargo @hostCargoArgs
      if ($LASTEXITCODE -ne 0) { throw 'The strict Flutter Rust library failed to build.' }
    }
  } finally { Pop-Location }

  Push-Location (Join-Path $hostRepository 'flutter')
  try {
    if ($Test) {
      & $hostFlutter test --no-pub test/desktop/secure_host_workspace_test.dart
      if ($LASTEXITCODE -ne 0) { throw 'Flutter workspace tests failed.' }
      & $hostFlutter analyze --no-pub lib/desktop/secure_host_mode.dart lib/desktop/pages/secure_host_page.dart lib/desktop/widgets/secure_host_workspace.dart
      if ($LASTEXITCODE -ne 0) { throw 'Flutter host analysis failed.' }
    } else {
      & $hostFlutter build windows --no-pub "--$($Configuration.ToLowerInvariant())"
      if ($LASTEXITCODE -ne 0) { throw 'Flutter Windows runner build failed.' }
      $hostOutput = Join-Path $hostRepository "flutter\build\windows\x64\runner\$Configuration"
      foreach ($hostFile in @('rustdesk.exe', 'librustdesk.dll', 'flutter_windows.dll')) {
        if (-not (Test-Path -LiteralPath (Join-Path $hostOutput $hostFile))) { throw "Missing bundle file: $hostFile" }
      }
      Write-Output "Flutter Windows host: $hostOutput"
    }
  } finally { Pop-Location }
} finally {
  foreach ($hostKey in $hostSaved.Keys) {
    if ($null -eq $hostSaved[$hostKey]) { Remove-Item -LiteralPath "Env:$hostKey" -ErrorAction SilentlyContinue }
    else { Set-Item -LiteralPath "Env:$hostKey" -Value $hostSaved[$hostKey] }
  }
}
