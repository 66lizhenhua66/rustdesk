param(
  [string]$VcpkgRoot = $env:VCPKG_ROOT,
  [string]$LibClangPath = 'G:\Huawei\DevEco Studio\sdk\default\openharmony\native\llvm\bin',
  [switch]$CheckOnly,
  [switch]$TestSecureGate,
  [switch]$DebugBuild
)

$ErrorActionPreference = 'Stop'
$secureRepository = Split-Path $PSScriptRoot -Parent
$secureCachedVcpkg = Join-Path $secureRepository 'apps\harmony-controller\artifacts\official-build\vcpkg'
if ([string]::IsNullOrWhiteSpace($VcpkgRoot) -and (Test-Path -LiteralPath (Join-Path $secureCachedVcpkg 'vcpkg.exe'))) {
  $VcpkgRoot = $secureCachedVcpkg
}
if ([string]::IsNullOrWhiteSpace($VcpkgRoot)) {
  throw 'Official Windows build needs VCPKG_ROOT with the repository vcpkg.json dependencies installed for x64-windows-static. The standalone DEMO does not require them.'
}
$secureInstalled = if ($env:VCPKG_INSTALLED_ROOT) { $env:VCPKG_INSTALLED_ROOT } else { Join-Path $VcpkgRoot 'installed' }
foreach ($secureHeader in @('vpx\vpx_encoder.h', 'aom\aom_encoder.h', 'libyuv.h', 'opus\opus.h')) {
  $secureRequired = Join-Path $secureInstalled "x64-windows-static\include\$secureHeader"
  if (-not (Test-Path -LiteralPath $secureRequired)) { throw "Missing official build dependency: $secureRequired" }
}
if (-not (Test-Path -LiteralPath (Join-Path $LibClangPath 'libclang.dll'))) { throw 'Set -LibClangPath to the directory containing Windows libclang.dll.' }
$secureSaved = @{}
foreach ($secureName in @('VCPKG_ROOT', 'LIBCLANG_PATH', 'BINDGEN_EXTRA_CLANG_ARGS_x86_64_pc_windows_msvc', 'INCLUDE', 'LIB', 'LIBPATH', 'PATH', 'VCINSTALLDIR', 'VCToolsInstallDir', 'WindowsSdkDir', 'WindowsSDKVersion', 'UCRTVersion')) {
  $secureSaved[$secureName] = [Environment]::GetEnvironmentVariable($secureName, 'Process')
}
try {
  $secureVsWhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
  if (-not (Test-Path -LiteralPath $secureVsWhere)) { throw 'Visual Studio Build Tools with the Windows C++ workload is required.' }
  $secureVsRoot = & $secureVsWhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
  if (-not $secureVsRoot) { throw 'No Visual Studio installation with the Windows C++ workload was found.' }
  $secureDevCmd = '"' + (Join-Path $secureVsRoot 'Common7\Tools\VsDevCmd.bat') + '" -no_logo -arch=amd64 -host_arch=amd64 && set'
  $secureDevEnvironment = & $env:ComSpec /d /s /c $secureDevCmd
  if ($LASTEXITCODE -ne 0) { throw 'Visual Studio developer environment initialization failed.' }
  foreach ($secureLine in $secureDevEnvironment) {
    if ($secureLine -match '^(INCLUDE|LIB|LIBPATH|PATH|VCINSTALLDIR|VCToolsInstallDir|WindowsSdkDir|WindowsSDKVersion|UCRTVersion)=(.*)$') {
      [Environment]::SetEnvironmentVariable($Matches[1], $Matches[2], 'Process')
    }
  }
  $secureClang = Join-Path $LibClangPath 'clang.exe'
  if (-not (Test-Path -LiteralPath $secureClang)) { throw 'clang.exe must accompany libclang.dll to resolve its standard headers.' }
  $secureClangResources = & $secureClang -print-resource-dir
  if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath (Join-Path $secureClangResources 'include\stddef.h'))) { throw 'Clang standard headers are unavailable.' }
  $env:BINDGEN_EXTRA_CLANG_ARGS_x86_64_pc_windows_msvc = $secureSaved['BINDGEN_EXTRA_CLANG_ARGS_x86_64_pc_windows_msvc'] + ' --target=x86_64-pc-windows-msvc -resource-dir="' + $secureClangResources + '"'
  $env:VCPKG_ROOT = $VcpkgRoot
  $env:LIBCLANG_PATH = $LibClangPath
  if ($TestSecureGate) {
    & cargo test --manifest-path (Join-Path $secureRepository 'Cargo.toml') --locked -p rustdesk --lib --no-default-features --features ord-secure-host cm_approval_requires_this_pending_login_once -- --test-threads=1
  } elseif ($CheckOnly) {
    & cargo check --manifest-path (Join-Path $secureRepository 'Cargo.toml') --locked -p rustdesk --lib --no-default-features --features ord-secure-host
  } elseif ($DebugBuild) {
    & cargo build --manifest-path (Join-Path $secureRepository 'Cargo.toml') --locked -p rustdesk --bin rustdesk --features ord-secure-host
  } else {
    & cargo build --manifest-path (Join-Path $secureRepository 'Cargo.toml') --locked -p rustdesk --bin rustdesk --release --features ord-secure-host
  }
  if ($LASTEXITCODE -ne 0) { throw "Official secure host build failed: $LASTEXITCODE" }
} finally {
  foreach ($secureName in $secureSaved.Keys) {
    if ($null -eq $secureSaved[$secureName]) { Remove-Item -LiteralPath "Env:$secureName" -ErrorAction SilentlyContinue } else { Set-Item -LiteralPath "Env:$secureName" -Value $secureSaved[$secureName] }
  }
}
