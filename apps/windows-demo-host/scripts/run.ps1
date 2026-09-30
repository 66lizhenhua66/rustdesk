param(
    [string]$Listen = '127.0.0.1:21119',
    [string]$ProfileOut = ''
)

$ErrorActionPreference = 'Stop'
$hostRoot = Split-Path $PSScriptRoot -Parent
$binary = Join-Path $hostRoot 'target/release/windows-demo-host.exe'
if (-not (Test-Path -LiteralPath $binary)) {
    throw 'Build first with scripts/build.ps1'
}
$arguments = @('--listen', $Listen)
if ($ProfileOut) { $arguments += @('--profile-out', $ProfileOut) }
& $binary @arguments
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
