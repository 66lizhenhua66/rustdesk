param(
    [string]$SdkNative = 'G:/Huawei/DevEco Studio/sdk/default/openharmony/native',
    [ValidateSet('arm64-v8a', 'x86_64')]
    [string[]]$Abi = @('arm64-v8a', 'x86_64')
)

$ErrorActionPreference = 'Stop'
$cache = Join-Path $PSScriptRoot '../artifacts'
$cmake = Join-Path $SdkNative 'build-tools/cmake/bin/cmake.exe'
$ninja = Join-Path $SdkNative 'build-tools/cmake/bin/ninja.exe'
$toolchain = Join-Path $SdkNative 'build/cmake/ohos.toolchain.cmake'

foreach ($tool in @($cmake, $ninja, $toolchain)) {
    if (-not (Test-Path -LiteralPath $tool)) { throw "Missing OHOS SDK tool: $tool" }
}

New-Item -ItemType Directory -Force -Path $cache | Out-Null

function Get-VerifiedArchive([string]$Name, [string]$Url, [string]$Sha256) {
    $path = Join-Path $cache $Name
    if (-not (Test-Path -LiteralPath $path)) {
        & curl.exe -L --fail --silent --show-error --output $path $Url
        if ($LASTEXITCODE -ne 0) { throw "Download failed: $Url" }
    }
    $actual = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash
    if ($actual -ne $Sha256) { throw "SHA256 mismatch for $Name`: $actual" }
    return $path
}

$crate = Get-VerifiedArchive 'libsodium-sys-0.2.7.crate' `
    'https://static.crates.io/crates/libsodium-sys/libsodium-sys-0.2.7.crate' `
    '6B779387CD56ADFBC02EA4A668E704F729BE8D6A6ABD2C27CA5EE537849A92FD'
$wrapperArchive = Get-VerifiedArchive 'libsodium-cmake-a8ac4509.tar.gz' `
    'https://api.github.com/repos/robinlinden/libsodium-cmake/tarball/a8ac4509b22b84d6c2eb7d7448f08678e4a67da6' `
    '8A722A7EF1AB8DA12068CC1FF88D7727C6510D224F3D2ADDEC4380DE6C5C1266'

$crateRoot = Join-Path $cache 'libsodium-sys-0.2.7'
$wrapper = Join-Path $cache 'robinlinden-libsodium-cmake-a8ac450'
if (-not (Test-Path -LiteralPath (Join-Path $crateRoot 'build.rs'))) {
    & tar.exe -xf $crate -C $cache
    if ($LASTEXITCODE -ne 0) { throw 'Could not extract libsodium-sys' }
}
if (-not (Test-Path -LiteralPath (Join-Path $wrapper 'CMakeLists.txt'))) {
    & tar.exe -xf $wrapperArchive -C $cache
    if ($LASTEXITCODE -ne 0) { throw 'Could not extract libsodium-cmake' }
}

$source = Join-Path $crateRoot 'libsodium'
$wrapperSource = Join-Path $wrapper 'libsodium'
if (-not (Test-Path -LiteralPath (Join-Path $wrapperSource 'src/libsodium/include/sodium/version.h.in'))) {
    Copy-Item -Path (Join-Path $source '*') -Destination $wrapperSource -Recurse -Force
}

$cmakeLists = Join-Path $wrapper 'CMakeLists.txt'
$contents = Get-Content -LiteralPath $cmakeLists -Raw
if ($contents -notmatch 'set\(VERSION 1\.0\.18\)') { throw 'CMake wrapper version differs from libsodium-sys' }
foreach ($missingHeader in @('ladder_base.h', 'ladder_base_namespace.h')) {
    $line = "    libsodium/src/libsodium/crypto_scalarmult/curve25519/sandy2x/$missingHeader`n"
    $contents = $contents.Replace($line, '')
}
[System.IO.File]::WriteAllText($cmakeLists, $contents)

foreach ($targetAbi in $Abi) {
    $build = Join-Path $cache "build-$targetAbi"
    & $cmake -S $wrapper -B $build -G Ninja `
        "-DCMAKE_MAKE_PROGRAM=$ninja" "-DCMAKE_TOOLCHAIN_FILE=$toolchain" `
        "-DOHOS_ARCH=$targetAbi" '-DBUILD_SHARED_LIBS=OFF' `
        '-DSODIUM_DISABLE_TESTS=ON' '-DSODIUM_MINIMAL=OFF' '-DCMAKE_BUILD_TYPE=Release'
    if ($LASTEXITCODE -ne 0) { throw "CMake configure failed for $targetAbi" }
    & $cmake --build $build --parallel 8
    if ($LASTEXITCODE -ne 0) { throw "libsodium build failed for $targetAbi" }
    $library = Join-Path $build 'libsodium.a'
    if (-not (Test-Path -LiteralPath $library)) { throw "Missing library: $library" }
    # libsodium-sys 0.2.7 chooses its link name using the build host's MSVC cfg.
    # Preserve the archive bytes while providing the name rustc expects for OHOS cross-builds.
    Copy-Item -LiteralPath $library -Destination (Join-Path $build 'liblibsodium.a') -Force
    Write-Output "$targetAbi`: $library"
}
