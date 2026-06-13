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
Invoke-CiCommand cargo check
Invoke-CiCommand cargo test
Invoke-CiCommand cargo check --examples
