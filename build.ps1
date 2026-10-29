# Build Cerberus for the supported Windows MSVC target.
$ErrorActionPreference = 'Stop'
$target = 'x86_64-pc-windows-msvc'

$cargoCommand = Get-Command cargo -ErrorAction SilentlyContinue
if ($cargoCommand) {
    $cargo = $cargoCommand.Source
} else {
    $cargo = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'
}

if (-not (Test-Path -LiteralPath $cargo)) {
    Write-Error 'Cargo was not found. Install Rust with rustup and ensure Cargo is on PATH.'
    exit 1
}

Push-Location $PSScriptRoot
try {
    & $cargo build --release --target $target
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    $binary = Join-Path $PSScriptRoot "target\$target\release\cerberus.exe"
    if (-not (Test-Path -LiteralPath $binary)) {
        Write-Error "Cargo reported success but the executable was not found at $binary"
        exit 1
    }

    Write-Host "Release build succeeded: $binary" -ForegroundColor Green
} finally {
    Pop-Location
}
