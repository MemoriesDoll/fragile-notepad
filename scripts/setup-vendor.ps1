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

$vendors = @(
    @{
        VendorDir = "vendor/iced"
        Remote = "https://github.com/iced-rs/iced.git"
        Branch = "master"
        Patch = "patches/iced/fragile-notepad-iced.patch"
        BaseRevisionFile = "patches/iced/BASE_REVISION"
        GitConfig = @("core.autocrlf=true")
    },
    @{
        VendorDir = "vendor/encoding_rs"
        Remote = "https://github.com/hsivonen/encoding_rs.git"
        Branch = "main"
        Patch = "patches/encoding_rs/oem-code-pages.patch"
        BaseRevisionFile = "patches/encoding_rs/BASE_REVISION"
        GitConfig = @()
    }
)

function Invoke-Git {
    param([Parameter(ValueFromRemainingArguments = $true)] [string[]] $Arguments)

    git @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "git $($Arguments -join ' ') failed with exit code $LASTEXITCODE"
    }
}

function Get-BaseRevision([hashtable] $Vendor) {
    if (-not (Test-Path $Vendor.BaseRevisionFile)) {
        throw "Base revision file not found: $($Vendor.BaseRevisionFile)"
    }

    return (Get-Content $Vendor.BaseRevisionFile -Raw).Trim()
}

function Test-GitCheckout([string] $VendorDir) {
    return (Test-Path (Join-Path $VendorDir ".git"))
}

function Set-VendorGitConfig([hashtable] $Vendor) {
    foreach ($entry in $Vendor.GitConfig) {
        $parts = @($entry -split "=", 2)
        if ($parts.Count -ne 2) {
            throw "Invalid Git config entry for $($Vendor.VendorDir): $entry"
        }

        Invoke-Git -C $Vendor.VendorDir config $parts[0] $parts[1]
    }
}

function Ensure-VendorCheckout([hashtable] $Vendor) {
    if (Test-Path $Vendor.VendorDir) {
        if (-not (Test-GitCheckout $Vendor.VendorDir)) {
            throw "$($Vendor.VendorDir) exists but is not a Git checkout."
        }

        return
    }

    $parent = Split-Path $Vendor.VendorDir -Parent
    if (-not [string]::IsNullOrWhiteSpace($parent) -and -not (Test-Path $parent)) {
        New-Item -ItemType Directory -Force -Path $parent | Out-Null
    }

    $baseRevision = Get-BaseRevision $Vendor
    Invoke-Git clone --no-checkout $Vendor.Remote $Vendor.VendorDir
    Set-VendorGitConfig $Vendor
    Invoke-Git -C $Vendor.VendorDir checkout $baseRevision
}

function Write-MissingVendorStatus([hashtable] $Vendor) {
    Write-Host "vendor dir:       $($Vendor.VendorDir)"
    if (Test-Path $Vendor.BaseRevisionFile) {
        Write-Host "recorded base:    $(Get-BaseRevision $Vendor)"
    } else {
        Write-Host "recorded base:    <missing>"
    }
    Write-Host "patch:            $($Vendor.Patch)"
    Write-Host "checkout status:  missing; run setup-vendor apply to bootstrap"
    Write-Host ""
}

function Invoke-VendorPatch([hashtable] $Vendor) {
    $args = @{
        Command = $Command
        VendorDir = $Vendor.VendorDir
        Patch = $Vendor.Patch
        BaseRevisionFile = $Vendor.BaseRevisionFile
        GitVendor = $true
    }

    if ($Vendor.GitConfig.Count -gt 0) {
        $args.GitConfig = $Vendor.GitConfig
    }

    if ($Command -eq "update") {
        $args.Remote = $Vendor.Remote
        $args.Branch = $Vendor.Branch
    }

    & "$PSScriptRoot\vendor-patch.ps1" @args
}

if ($SkipSubmoduleUpdate) {
    Write-Warning "-SkipSubmoduleUpdate is deprecated; vendor checkouts are managed by this script."
}

foreach ($vendor in $vendors) {
    if ($Command -eq "status" -and -not (Test-Path $vendor.VendorDir)) {
        Write-MissingVendorStatus $vendor
        continue
    }

    if ($Command -in @("apply", "update")) {
        Ensure-VendorCheckout $vendor
    }

    Invoke-VendorPatch $vendor
}
