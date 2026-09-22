$ErrorActionPreference = "Stop"

# GitHub Windows runners have Chrome but no physical Vulkan GPU. Use Chrome's
# real SwiftShader ICD and loader for CI, without downloading an unofficial DLL.
$browserRoots = @(
    "$env:ProgramFiles\Google\Chrome\Application",
    "${env:ProgramFiles(x86)}\Google\Chrome\Application"
)
$manifests = foreach ($root in $browserRoots) {
    if (Test-Path -LiteralPath $root) {
        Get-ChildItem -LiteralPath $root -Filter vk_swiftshader_icd.json -Recurse
    }
}
$manifest = $manifests | Sort-Object { [version] $_.Directory.Name } -Descending | Select-Object -First 1
if (-not $manifest) {
    throw "Chrome's SwiftShader ICD is required on the Windows CI runner."
}
$runtime = $manifest.Directory.FullName
foreach ($library in @("vulkan-1.dll", "vk_swiftshader.dll")) {
    if (-not (Test-Path -LiteralPath (Join-Path $runtime $library))) {
        throw "Missing Vulkan CI library: $runtime\$library"
    }
}
$env:VK_DRIVER_FILES = $manifest.FullName
$env:VK_ICD_FILENAMES = $manifest.FullName
$env:WGPU_BACKEND = "vulkan"
$env:PATH = "$runtime;$env:PATH"
if ($env:GITHUB_ENV) {
    @(
        "VK_DRIVER_FILES=$env:VK_DRIVER_FILES",
        "VK_ICD_FILENAMES=$env:VK_ICD_FILENAMES",
        "WGPU_BACKEND=vulkan"
    ) | Out-File -LiteralPath $env:GITHUB_ENV -Append -Encoding utf8
    $runtime | Out-File -LiteralPath $env:GITHUB_PATH -Append -Encoding utf8
}
Write-Host "Windows CI Vulkan runtime: $runtime (SwiftShader, software adapter)"
