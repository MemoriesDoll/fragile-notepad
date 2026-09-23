$ErrorActionPreference = "Stop"

$repoRoot = (git rev-parse --show-toplevel 2>$null)
if (-not $repoRoot) {
    $repoRoot = Split-Path -Parent $PSScriptRoot
}

Set-Location $repoRoot

function Invoke-CiCommand {
    param([Parameter(ValueFromRemainingArguments = $true)] [string[]] $Command)

    & $Command[0] $Command[1..($Command.Length - 1)]
    if ($LASTEXITCODE -ne 0) {
        throw "$($Command -join ' ') failed with exit code $LASTEXITCODE"
    }
}

Invoke-CiCommand cargo fmt --package fragile-notepad --check
& .\scripts\generate_icon_assets.ps1
Invoke-CiCommand python scripts/test_icon_assets.py
# cargo test also compiles the application and examples.
Invoke-CiCommand cargo test
Invoke-CiCommand cargo test --locked --package iced_wgpu --lib
Invoke-CiCommand cargo test --locked --package cryoglyph --lib
Invoke-CiCommand cargo check --no-default-features
