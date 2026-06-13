param(
    [Parameter(Position = 0)]
    [ValidateSet("apply", "status", "update")]
    [string] $Command = "apply",

    [switch] $SkipSubmoduleUpdate
)

$ErrorActionPreference = "Stop"

$repoRoot = (git rev-parse --show-toplevel 2>$null)
if (-not $repoRoot) {
    $repoRoot = Split-Path -Parent $PSScriptRoot
}

Set-Location $repoRoot

function Initialize-Submodules([string[]] $Paths) {
    if ($Paths.Count -eq 0) {
        return
    }

    git submodule update --init @Paths
    if ($LASTEXITCODE -ne 0) {
        throw "git submodule update failed with exit code $LASTEXITCODE"
    }
}

if (-not $SkipSubmoduleUpdate) {
    if ($Command -eq "apply") {
        Initialize-Submodules @("vendor/iced", "vendor/encoding_rs")
    } elseif ($Command -eq "update") {
        $missing = @("vendor/iced", "vendor/encoding_rs") | Where-Object { -not (Test-Path $_) }
        Initialize-Submodules $missing
    }
}

$icedArgs = @{
    Command = $Command
    VendorDir = "vendor/iced"
    Patch = "patches/iced/fragile-notepad-iced.patch"
    BaseRevisionFile = "patches/iced/BASE_REVISION"
    GitVendor = $true
    GitConfig = @("core.autocrlf=true")
}
$encodingArgs = @{
    Command = $Command
    VendorDir = "vendor/encoding_rs"
    Patch = "patches/encoding_rs/oem-code-pages.patch"
    BaseRevisionFile = "patches/encoding_rs/BASE_REVISION"
    GitVendor = $true
}

if ($Command -eq "update") {
    $icedArgs.Remote = "https://github.com/iced-rs/iced.git"
    $icedArgs.Branch = "master"
    $encodingArgs.Remote = "https://github.com/hsivonen/encoding_rs.git"
    $encodingArgs.Branch = "main"
}

& "$PSScriptRoot\vendor-patch.ps1" @icedArgs

& "$PSScriptRoot\vendor-patch.ps1" @encodingArgs
