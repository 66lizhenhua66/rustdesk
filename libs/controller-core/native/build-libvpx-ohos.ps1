param(
    [string]$SdkNative = 'G:/Huawei/DevEco Studio/sdk/default/openharmony/native',
    [ValidateSet('arm64-v8a', 'x86_64')]
    [string[]]$Abi = @('arm64-v8a', 'x86_64'),
    [string]$MsysBin
)

$ErrorActionPreference = 'Stop'
$repositoryRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..\..'))
$archive = Join-Path $repositoryRoot 'apps\harmony-controller\artifacts\official-build\vcpkg\downloads\webmproject-libvpx-v1.15.2.tar.gz'
$expectedSha512 = '824fe8719e4115ec359ae0642f5e1cea051d458f09eb8c24d60858cf082f66e411215e23228173ab154044bafbdfbb2d93b589bb726f55b233939b91f928aae0'
$outputRoot = Join-Path $repositoryRoot 'libs\controller-core\artifacts\libvpx-ohos'
$sourceCache = Join-Path $outputRoot 'source'
$source = Join-Path $sourceCache 'libvpx-1.15.2'
$msysRoot = Join-Path $repositoryRoot 'apps\harmony-controller\artifacts\official-build\vcpkg\downloads\tools\msys2'
if (-not $MsysBin) {
    if (Test-Path -LiteralPath $msysRoot) {
        foreach ($candidateRoot in (Get-ChildItem -LiteralPath $msysRoot -Directory | Sort-Object Name)) {
            $candidateBin = Join-Path $candidateRoot.FullName 'usr\bin'
            $complete = $true
            foreach ($tool in @('bash.exe', 'make.exe', 'cygpath.exe', 'perl.exe')) {
                if (-not (Test-Path -LiteralPath (Join-Path $candidateBin $tool))) {
                    $complete = $false
                    break
                }
            }
            if ($complete) {
                $MsysBin = $candidateBin
                break
            }
        }
    }
    if (-not $MsysBin) { throw "MSYS2 bash/make/cygpath/perl not found under $msysRoot; pass -MsysBin" }
}
$msys = [IO.Path]::GetFullPath($MsysBin)
$llvm = Join-Path $SdkNative 'llvm\bin'
$bash = Join-Path $msys 'bash.exe'
$cygpath = Join-Path $msys 'cygpath.exe'
foreach ($required in @($bash, (Join-Path $msys 'make.exe'), $cygpath, (Join-Path $msys 'perl.exe'), $llvm)) {
    if (-not (Test-Path -LiteralPath $required)) { throw "Missing libvpx build input: $required" }
}
if (-not (Test-Path -LiteralPath $archive)) {
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $archive) | Out-Null
    & curl.exe -L --fail --silent --show-error --output $archive `
        'https://github.com/webmproject/libvpx/archive/refs/tags/v1.15.2.tar.gz'
    if ($LASTEXITCODE -ne 0) { throw "libvpx archive download failed: $archive" }
}
if ((Get-FileHash -LiteralPath $archive -Algorithm SHA512).Hash -ne $expectedSha512) {
    throw "libvpx archive SHA512 mismatch: $archive"
}
if ([IO.Path]::GetFullPath($source) -ne [IO.Path]::GetFullPath((Join-Path $outputRoot 'source\libvpx-1.15.2'))) {
    throw "Unexpected libvpx source cache path: $source"
}
if (Test-Path -LiteralPath $source) { Remove-Item -LiteralPath $source -Recurse -Force }
New-Item -ItemType Directory -Force -Path $sourceCache | Out-Null
& tar.exe -xf $archive -C $sourceCache
if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath (Join-Path $source 'configure'))) {
    throw "Could not extract verified libvpx source: $archive"
}
$sourceUnix = (& $cygpath -u $source).Trim()
$msysUnix = (& $cygpath -u $msys).Trim()
$llvmUnix = (& $cygpath -u $llvm).Trim()
$runWithTools = 'export PATH="$1:$2:$PATH"; shift 2; exec "$@"'
$savedEnvironment = @{}
foreach ($name in @('PATH', 'CC', 'CXX', 'AR', 'LD', 'RANLIB', 'LDFLAGS')) {
    $savedEnvironment[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
}
try {
    $env:PATH = "$msys;$llvm;$($savedEnvironment['PATH'])"
    $env:AR = 'llvm-ar'
    $env:RANLIB = 'llvm-ranlib'
    foreach ($targetAbi in $Abi) {
        $triple = if ($targetAbi -eq 'x86_64') { 'x86_64-linux-ohos' } else { 'aarch64-linux-ohos' }
        $env:CC = 'clang.exe'
        $env:CXX = 'clang++.exe'
        $env:LD = $env:CC
        $env:LDFLAGS = "--target=$triple"
        $build = Join-Path $outputRoot "build-$targetAbi"
        $install = Join-Path $outputRoot $targetAbi
        if ([IO.Path]::GetFullPath($build) -ne [IO.Path]::GetFullPath((Join-Path $outputRoot "build-$targetAbi"))) {
            throw "Unexpected libvpx build path: $build"
        }
        if (Test-Path -LiteralPath $build) { Remove-Item -LiteralPath $build -Recurse -Force }
        New-Item -ItemType Directory -Force -Path $build | Out-Null
        $installUnix = (& $cygpath -u $install).Trim()
        Push-Location $build
        try {
            & $bash --noprofile --norc -c $runWithTools -- $msysUnix $llvmUnix "$sourceUnix/configure" '--target=generic-gnu' `
                '--disable-runtime-cpu-detect' '--disable-vp9' '--disable-vp8-encoder' '--size-limit=1280x720' `
                '--disable-examples' '--disable-tools' '--disable-docs' '--disable-unit-tests' `
                '--enable-static' '--disable-shared' '--enable-pic' "--prefix=$installUnix" `
                "--extra-cflags=--target=$triple" "--extra-cxxflags=--target=$triple"
            if ($LASTEXITCODE -ne 0) { throw "libvpx configure failed: $targetAbi" }
            & $bash --noprofile --norc -c $runWithTools -- $msysUnix $llvmUnix "$msysUnix/make" -j8
            if ($LASTEXITCODE -ne 0) { throw "libvpx build failed: $targetAbi" }
            & $bash --noprofile --norc -c $runWithTools -- $msysUnix $llvmUnix "$msysUnix/make" install
            if ($LASTEXITCODE -ne 0) { throw "libvpx install failed: $targetAbi" }
            if (-not (Test-Path -LiteralPath (Join-Path $install 'lib\libvpx.a'))) {
                throw "libvpx output missing: $targetAbi"
            }
            Write-Output "$targetAbi`: $(Join-Path $install 'lib\libvpx.a')"
        } finally {
            Pop-Location
        }
    }
} finally {
    foreach ($name in $savedEnvironment.Keys) {
        if ($null -eq $savedEnvironment[$name]) {
            Remove-Item -LiteralPath "Env:$name" -ErrorAction SilentlyContinue
        } else {
            Set-Item -LiteralPath "Env:$name" -Value $savedEnvironment[$name]
        }
    }
}
