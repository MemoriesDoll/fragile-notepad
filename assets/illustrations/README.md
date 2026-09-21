# Info artwork

`macaw-quill.svg` is an original flat-color illustration for the Info panel.
The curved, asymmetric vanes and swept splits were drawn using these photos as
anatomical references; no pixels or paths from them are included in the artwork:

- [Isolated blue-and-yellow macaw feather](https://commons.wikimedia.org/wiki/File:Feather_of_a_Ara_ararauna_1.jpg)
- [Scarlet macaw plumage](https://commons.wikimedia.org/wiki/File:Red_feathers1a_(8306378464).jpg)

The generated 400 × 440 straight-RGBA asset is ignored by Git. The standard
`scripts/generate_icon_assets.ps1` and `scripts/generate_icon_assets.sh` entry
points regenerate it along with the icons before CI and release builds.
To regenerate just the illustration, run:

```sh
python scripts/rasterize_illustrations.py
```

The script requires Pillow and uses the existing SVG path parser. The app embeds
the raster directly, so it needs neither an SVG renderer nor image codecs at
runtime. Both themes use the same illustration; the panel applies fade opacity.
