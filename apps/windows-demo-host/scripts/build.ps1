$ErrorActionPreference = 'Stop'
$manifest = Join-Path (Split-Path $PSScriptRoot -Parent) 'Cargo.toml'
cargo build --manifest-path $manifest --release --locked
if ($LASTEXITCODE -ne 0) { throw "Demo host build failed with exit code $LASTEXITCODE" }
