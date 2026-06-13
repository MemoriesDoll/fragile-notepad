$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
Set-Location $repoRoot

python .\scripts\generate_tango_rgba.py
python .\scripts\rasterize_svg_icons.py --svg-dir .\assets\icons\heroicons\svg --out-dir .\assets\icons\heroicons\rgba --size 22
python .\scripts\rasterize_svg_icons.py --svg-dir .\assets\icons\bootstrap\svg --out-dir .\assets\icons\bootstrap\rgba --size 22
