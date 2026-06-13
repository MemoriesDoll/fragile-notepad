param(
    [Parameter(Mandatory = $true, Position = 0)]
    [ValidateSet("status", "apply", "export", "refresh", "update")]
    [string] $Command,

    [Parameter(Mandatory = $true)]
    [string] $VendorDir,

    [Parameter(Mandatory = $true)]
    [string] $Patch,

    [Parameter(Mandatory = $true)]
    [string] $BaseRevisionFile,

    [string] $Remote,

    [string] $Revision,

    [string] $Branch,

    [string[]] $GitConfig = @(),

    [switch] $GitVendor,

    [switch] $Force
)

$ErrorActionPreference = "Stop"

function Resolve-RepoPath([string] $Path) {
    if ([System.IO.Path]::IsPathRooted($Path)) {
        return $Path
    }

    return Join-Path (Get-Location) $Path
}

function Get-RepoRoot {
    $root = (git rev-parse --show-toplevel 2>$null)
    if ($LASTEXITCODE -eq 0 -and -not [string]::IsNullOrWhiteSpace($root)) {
        return (Resolve-Path $root).Path
    }

    return (Get-Location).Path
}

function Get-RepoRelativePath([string] $Path) {
    $resolvedPath = [System.IO.Path]::GetFullPath((Resolve-Path (Resolve-RepoPath $Path)).Path)
    $repoRoot = [System.IO.Path]::GetFullPath((Get-RepoRoot).TrimEnd([System.IO.Path]::DirectorySeparatorChar, [System.IO.Path]::AltDirectorySeparatorChar))
    $repoUri = [System.Uri]::new($repoRoot + [System.IO.Path]::DirectorySeparatorChar)
    $pathUri = [System.Uri]::new($resolvedPath)
    $relativePath = [System.Uri]::UnescapeDataString($repoUri.MakeRelativeUri($pathUri).ToString())

    if ($relativePath.StartsWith("../") -or $relativePath -eq "..") {
        return $Path
    }

    return ($relativePath -replace "\\", "/")
}

function Get-ExistingPatchVendorPath {
    $patchPath = Resolve-RepoPath $Patch
    if (-not (Test-Path $patchPath)) {
        return $null
    }

    foreach ($line in Get-Content $patchPath) {
        if ($line -notmatch "^diff --git a/(\S+) b/\S+") {
            continue
        }

        $parts = @($Matches[1] -split "/" | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
        for ($index = 0; $index -lt $parts.Count; $index++) {
            $candidate = Join-Path $VendorDir ($parts[$index..($parts.Count - 1)] -join [System.IO.Path]::DirectorySeparatorChar)
            if (Test-Path $candidate) {
                if ($index -eq 0) {
                    return ""
                }

                return ($parts[0..($index - 1)] -join "/")
            }
        }
    }

    return $null
}

function Get-PatchVendorPath {
    $existingPrefix = Get-ExistingPatchVendorPath
    if ($null -ne $existingPrefix) {
        return $existingPrefix
    }

    return Get-RepoRelativePath $VendorDir
}

function Get-SuperprojectVendorPath {
    return Get-RepoRelativePath $VendorDir
}

function Assert-VendorDir {
    $path = Resolve-RepoPath $VendorDir

    if (-not (Test-Path $path)) {
        throw "Vendor directory not found: $VendorDir"
    }
}

function Get-GitConfigArgs {
    $args = @()
    foreach ($entry in $GitConfig) {
        $args += @("-c", $entry)
    }
    return $args
}

function Format-GitArgs([string[]] $Arguments) {
    return ($Arguments -join " ")
}

function Invoke-VendorGit {
    param([Parameter(ValueFromRemainingArguments = $true)] [string[]] $Arguments)

    $configArgs = Get-GitConfigArgs
    $displayArgs = @($configArgs) + @($Arguments)
    git -C $VendorDir @configArgs @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "git -C $VendorDir $(Format-GitArgs $displayArgs) failed with exit code $LASTEXITCODE"
    }
}

function Test-GitVendor {
    if ($GitVendor) {
        return $true
    }

    $path = Resolve-RepoPath $VendorDir
    return (Test-Path (Join-Path $path ".git"))
}

function Assert-GitVendor {
    if (-not (Test-GitVendor)) {
        throw "$VendorDir is not a Git checkout. Vendor directories must be Git repositories."
    }
}

function Get-BaseRevision {
    $path = Resolve-RepoPath $BaseRevisionFile

    if (-not (Test-Path $path)) {
        throw "Base revision file not found: $BaseRevisionFile"
    }

    return (Get-Content $path -Raw).Trim()
}

function Assert-CleanVendor {
    Assert-GitVendor

    $configArgs = Get-GitConfigArgs
    $status = @(git -C $VendorDir @configArgs status --short)

    if ($LASTEXITCODE -ne 0) {
        throw "Could not read vendor status"
    }

    if ($status.Count -gt 0 -and -not $Force) {
        Write-Host "$VendorDir has local changes:"
        $status | ForEach-Object { Write-Host $_ }
        throw "Use -Force to allow applying over or exporting from a dirty vendor checkout."
    }
}

function Convert-GitVendorPatch {
    param([string] $Content)

    $patchVendorPath = Get-PatchVendorPath
    $lines = $Content -split "`r?`n", -1
    $converted = $lines | ForEach-Object {
        if ($_ -like "diff --git a/* b/*") {
            $_ -replace "^diff --git a/", "diff --git a/$patchVendorPath/" -replace " b/", " b/$patchVendorPath/"
        } elseif ($_ -like "--- a/*") {
            $_ -replace "^--- a/", "--- a/$patchVendorPath/"
        } elseif ($_ -like "+++ b/*") {
            $_ -replace "^\+\+\+ b/", "+++ b/$patchVendorPath/"
        } else {
            $_
        }
    }

    return ($converted -join "`n")
}

function Get-GitVendorStripCount {
    $parts = @((Get-PatchVendorPath) -split "[/\\]+" | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
    return $parts.Count + 1
}

function Get-VendorHead {
    $configArgs = Get-GitConfigArgs
    $head = (git -C $VendorDir @configArgs rev-parse HEAD).Trim()
    if ($LASTEXITCODE -ne 0) {
        throw "Could not read $VendorDir HEAD"
    }

    return $head
}

function Get-SuperprojectGitlink {
    $path = Get-SuperprojectVendorPath
    $entry = @(git ls-files --stage -- $path)
    if ($LASTEXITCODE -ne 0) {
        throw "Could not read superproject gitlink for $path"
    }

    foreach ($line in $entry) {
        $parts = $line -split "\s+"
        if ($parts.Count -ge 4 -and $parts[0] -eq "160000") {
            return $parts[1]
        }
    }

    return $null
}

function Test-PinConsistency {
    $head = Get-VendorHead
    $base = Get-BaseRevision
    $gitlink = Get-SuperprojectGitlink

    $problems = @()
    if ($base -ne $head) {
        $problems += "recorded base $base does not match vendor HEAD $head"
    }
    if ($gitlink -and $gitlink -ne $head) {
        $problems += "staged superproject gitlink $gitlink does not match vendor HEAD $head"
    }

    return $problems
}

function Write-PinConsistencyWarning {
    $problems = @(Test-PinConsistency)
    if ($problems.Count -eq 0) {
        return
    }

    $path = Get-SuperprojectVendorPath
    Write-Warning "$path is not fully reproducible yet:"
    $problems | ForEach-Object { Write-Warning "  $_" }
    Write-Warning "Stage the matching files, for example: git add $BaseRevisionFile $Patch"
}

function Apply-Patch {
    Assert-VendorDir
    Assert-GitVendor
    Assert-CleanVendor

    $patchPath = Resolve-RepoPath $Patch

    if (-not (Test-Path $patchPath)) {
        throw "Patch file not found: $Patch"
    }

    $configArgs = Get-GitConfigArgs
    $stripArg = "-p$(Get-GitVendorStripCount)"
    git -C $VendorDir @configArgs apply $stripArg $patchPath
    if ($LASTEXITCODE -ne 0) {
        throw "git -C $VendorDir apply $stripArg $Patch failed with exit code $LASTEXITCODE"
    }
}

function Reverse-Patch {
    Assert-VendorDir
    Assert-GitVendor

    $patchPath = Resolve-RepoPath $Patch

    if (-not (Test-Path $patchPath)) {
        throw "Patch file not found: $Patch"
    }

    $configArgs = Get-GitConfigArgs
    $stripArg = "-p$(Get-GitVendorStripCount)"
    git -C $VendorDir @configArgs apply -R $stripArg $patchPath
    if ($LASTEXITCODE -ne 0) {
        throw "git -C $VendorDir apply -R $stripArg $Patch failed with exit code $LASTEXITCODE"
    }
}

function Export-Patch {
    Assert-VendorDir
    Assert-GitVendor

    $patchPath = Resolve-RepoPath $Patch
    $directory = Split-Path $patchPath -Parent

    if (-not (Test-Path $directory)) {
        New-Item -ItemType Directory -Force $directory | Out-Null
    }

    $tempPatch = Join-Path ([System.IO.Path]::GetTempPath()) ("vendor-patch-" + [System.Guid]::NewGuid() + ".patch")
    $convertedPatch = Join-Path $directory ("." + [System.IO.Path]::GetFileName($Patch) + "." + [System.Guid]::NewGuid() + ".tmp")
    $untracked = @()
    try {
        $configArgs = Get-GitConfigArgs
        $untracked = @(git -C $VendorDir @configArgs ls-files --others --exclude-standard)
        if ($LASTEXITCODE -ne 0) {
            throw "Could not list untracked files for $VendorDir"
        }

        if ($untracked.Count -gt 0) {
            git -C $VendorDir @configArgs add -N -- @untracked
            if ($LASTEXITCODE -ne 0) {
                throw "Could not mark untracked files as intent-to-add for $VendorDir"
            }
        }

        git -C $VendorDir @configArgs diff --binary --output=$tempPatch
        if ($LASTEXITCODE -ne 0) {
            throw "git diff for $VendorDir failed with exit code $LASTEXITCODE"
        }

        $content = if (Test-Path $tempPatch) {
            Get-Content $tempPatch -Raw
        } else {
            ""
        }
        Set-Content -Path $convertedPatch -Value (Convert-GitVendorPatch $content) -NoNewline
        Protect-PatchReplacement $patchPath $convertedPatch

        $configArgs = Get-GitConfigArgs
        Write-BaseRevision (Get-VendorHead)
        Write-PinConsistencyWarning
    }
    finally {
        if ($untracked.Count -gt 0) {
            $configArgs = Get-GitConfigArgs
            git -C $VendorDir @configArgs reset -q -- @untracked | Out-Null
        }

        if (Test-Path $tempPatch) {
            Remove-Item -LiteralPath $tempPatch -Force
        }

        if (Test-Path $convertedPatch) {
            Remove-Item -LiteralPath $convertedPatch -Force
        }
    }
}

function Protect-PatchReplacement([string] $PatchPath, [string] $NewPatchPath) {
    $oldLength = if (Test-Path $PatchPath) { (Get-Item $PatchPath).Length } else { 0 }
    $newLength = if (Test-Path $NewPatchPath) { (Get-Item $NewPatchPath).Length } else { 0 }

    if ($oldLength -gt 0 -and $newLength -eq 0 -and -not $Force) {
        throw "Refusing to overwrite non-empty patch with an empty export: $Patch. Use -Force if this is intentional."
    }

    if ($oldLength -gt 0) {
        $backupDir = Join-Path (Split-Path $PatchPath -Parent) ".backups"
        if (-not (Test-Path $backupDir)) {
            New-Item -ItemType Directory -Force $backupDir | Out-Null
        }

        $timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
        $backupName = "$(Split-Path $PatchPath -Leaf).$timestamp.bak"
        Copy-Item -LiteralPath $PatchPath -Destination (Join-Path $backupDir $backupName) -Force
    }

    Move-Item -LiteralPath $NewPatchPath -Destination $PatchPath -Force
}

function Write-BaseRevision([string] $RevisionValue) {
    $path = Resolve-RepoPath $BaseRevisionFile
    $directory = Split-Path $path -Parent

    if (-not (Test-Path $directory)) {
        New-Item -ItemType Directory -Force $directory | Out-Null
    }

    Set-Content -Path $path -Value $RevisionValue -NoNewline
}

function Refresh-Vendor {
    Assert-VendorDir
    Assert-GitVendor
    Assert-CleanVendor

    $targetRevision = $Revision
    if (-not $targetRevision) {
        $targetRevision = Get-BaseRevision
    }

    if (-not [string]::IsNullOrWhiteSpace($Remote)) {
        Invoke-VendorGit remote set-url origin $Remote
        Invoke-VendorGit fetch origin
    }

    Invoke-VendorGit checkout $targetRevision
    Write-BaseRevision $targetRevision
    Apply-Patch
}

function Resolve-UpdateRevision {
    $targetBranch = $Branch
    if ([string]::IsNullOrWhiteSpace($targetBranch)) {
        $targetBranch = "HEAD"
    }

    $configArgs = Get-GitConfigArgs
    if (-not [string]::IsNullOrWhiteSpace($Remote)) {
        git -C $VendorDir @configArgs fetch $Remote $targetBranch
        if ($LASTEXITCODE -ne 0) {
            throw "git -C $VendorDir fetch $Remote $targetBranch failed with exit code $LASTEXITCODE"
        }
        return (git -C $VendorDir @configArgs rev-parse FETCH_HEAD).Trim()
    }

    git -C $VendorDir @configArgs fetch origin $targetBranch
    if ($LASTEXITCODE -ne 0) {
        throw "git -C $VendorDir fetch origin $targetBranch failed with exit code $LASTEXITCODE"
    }

    if ($targetBranch -eq "HEAD") {
        return (git -C $VendorDir @configArgs rev-parse FETCH_HEAD).Trim()
    }

    return (git -C $VendorDir @configArgs rev-parse "origin/$targetBranch").Trim()
}

function Vendor-HasChanges {
    $configArgs = Get-GitConfigArgs
    $status = @(git -C $VendorDir @configArgs status --short)
    if ($LASTEXITCODE -ne 0) {
        throw "Could not read vendor status"
    }

    return $status.Count -gt 0
}

function Update-Vendor {
    Assert-VendorDir
    Assert-GitVendor

    $targetRevision = Resolve-UpdateRevision
    if ([string]::IsNullOrWhiteSpace($targetRevision)) {
        throw "Could not resolve fetched revision for $VendorDir"
    }

    if (Vendor-HasChanges) {
        Export-Patch
        Reverse-Patch
        Assert-CleanVendor
    }

    Invoke-VendorGit checkout $targetRevision
    Write-BaseRevision $targetRevision
    Apply-Patch
    Export-Patch
}

switch ($Command) {
    "status" {
        Assert-VendorDir
        Assert-GitVendor
        Write-Host "vendor dir:       $VendorDir"
        Write-Host "recorded base:    $(Get-BaseRevision)"
        Write-Host "patch:            $Patch"
        $configArgs = Get-GitConfigArgs
        Write-Host "vendor HEAD:      $((git -C $VendorDir @configArgs rev-parse HEAD).Trim())"
        $gitlink = Get-SuperprojectGitlink
        Write-Host "superproject pin: $(if ($gitlink) { $gitlink } else { "<script-managed checkout>" })"
        $pinProblems = @(Test-PinConsistency)
        if ($pinProblems.Count -gt 0) {
            Write-Host "pin status:       mismatch"
            $pinProblems | ForEach-Object { Write-Host "  - $_" }
        } else {
            Write-Host "pin status:       ok"
        }
        Write-Host ""
        git -C $VendorDir @configArgs status --short
    }
    "apply" {
        Apply-Patch
    }
    "export" {
        Export-Patch
    }
    "refresh" {
        Refresh-Vendor
    }
    "update" {
        Update-Vendor
    }
}
