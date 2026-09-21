$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
Set-Location $repoRoot

python .\scripts\generate_colored_rgba.py
if ($LASTEXITCODE -ne 0) { throw "Colored icon generation failed" }
python .\scripts\rasterize_svg_icons.py --svg-dir .\assets\icons\heroicons\svg --out-dir .\assets\icons\heroicons\rgba --size 22
if ($LASTEXITCODE -ne 0) { throw "Control icon generation failed" }
python .\scripts\rasterize_svg_icons.py --svg-dir .\assets\icons\bootstrap\svg --out-dir .\assets\icons\bootstrap\rgba --size 22
if ($LASTEXITCODE -ne 0) { throw "Shortcut icon generation failed" }
python .\scripts\rasterize_illustrations.py
if ($LASTEXITCODE -ne 0) { throw "Illustration generation failed" }
python .\scripts\generate_app_icons.py
if ($LASTEXITCODE -ne 0) { throw "Application icon generation failed" }
