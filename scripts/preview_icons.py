"""Build a standalone, offline review of every icon's SVG and embedded pixels."""

import argparse
import base64
import html
from io import BytesIO
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
LABELS = {
    "accessories-character-map": "Show all characters",
    "document-close": "Close document",
    "document-close-all": "Close all documents",
    "document-save-all": "Save all documents",
    "edit-find-replace": "Replace",
    "format-justify-fill": "Word wrap",
    "format-indent-more": "Indent guides",
    "text-x-script": "Function list",
}


def png_uri(image):
    buffer = BytesIO()
    image.save(buffer, format="PNG")
    return "data:image/png;base64," + base64.b64encode(buffer.getvalue()).decode()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, default=ROOT / "target/icon-review/index.html")
    args = parser.parse_args()
    sections = []
    for family, title in [("colored", "Commands and document states"),
                          ("heroicons", "Controls and editor markers"),
                          ("bootstrap", "Shortcuts and pins")]:
        cards = []
        for source in sorted((ROOT / "assets/icons" / family / "svg").glob("*.svg")):
            raw = source.parent.parent / "rgba" / (source.stem + ".rgba")
            bitmap = Image.frombytes("RGBA", (22, 22), raw.read_bytes())
            uri = png_uri(bitmap)
            label = LABELS.get(source.stem, source.stem.replace("-", " ").capitalize())
            if family == "colored":
                sample = f'<img class="raster" src="{uri}" alt="{html.escape(label)}">'
            else:
                sample = f'<span class="mask raster" style="mask-image:url({uri})" role="img" aria-label="{html.escape(label)}"></span>'
            svg = source.read_text(encoding="utf-8").replace('<svg ', '<svg aria-hidden="true" ')
            cards.append(f'<article><div class="samples"><div>{sample}<small>RGBA</small></div>'
                         f'<div class="vector">{svg}<small>Vector</small></div></div>'
                         f'<h3>{html.escape(label)}</h3><code>{source.stem}</code></article>')
        sections.append(f'<section><h2>{title}</h2><div class="grid">{"".join(cards)}</div></section>')
    document = '''<!doctype html>
<html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Fragile Notepad — icon review</title>
<style>
:root{--bg:#f4f6f9;--panel:white;--ink:#27394c;--muted:#596c80;--line:#d8e0e9;--size:22px;color-scheme:light}
body.dark{--bg:#202733;--panel:#293341;--ink:#e7eef8;--muted:#b0bfd2;--line:#435269;color-scheme:dark}
*{box-sizing:border-box}body{margin:0;padding:32px;font:15px/1.5 system-ui;background:var(--bg);color:var(--ink)}
header,main{max-width:1200px;margin:auto}h1{font-size:28px;margin:0 0 6px}p{color:var(--muted);margin:0 0 20px}
.controls{display:flex;gap:24px;align-items:center;flex-wrap:wrap}label{display:flex;gap:10px;align-items:center}
select{padding:6px 10px;font:inherit}h2{font-size:18px;margin:32px 0 14px}
.grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(175px,1fr));gap:12px}
article{background:var(--panel);border:1px solid var(--line);border-radius:10px;padding:16px;min-width:0}
.samples{display:flex;justify-content:space-around;align-items:end;min-height:84px;gap:16px}
.samples>div{text-align:center}.raster,.vector svg{display:block;width:var(--size);height:var(--size);margin:0 auto 10px}
.mask{background:currentColor;mask-size:contain;mask-repeat:no-repeat}small{color:var(--muted);font-size:11px}
h3{font-size:13px;margin:16px 0 4px;font-weight:600}code{font-size:10px;overflow-wrap:anywhere;color:var(--muted)}
</style>
<header><h1>Fragile Notepad icons</h1><p>Every maintained icon, shown as the embedded 22px bitmap and its editable vector. Check small sizes and both themes.</p>
<p>Colored artwork: Copyright (c) 2026, Fragile Notepad authors. All rights reserved. The control and shortcut families retain their MIT licenses.</p>
<div class="controls"><label>Size <select id="size"><option value="12">12px · small controls</option><option value="14">14px · tabs</option><option value="18">18px · toolbar</option><option value="22" selected>22px · source bitmap</option><option value="44">44px · enlarged</option></select></label>
<label><input type="checkbox" id="theme"> Dark background</label></div></header>
<main>SECTIONS</main>
<script>
document.querySelector('#theme').addEventListener('change',e=>document.body.classList.toggle('dark',e.target.checked));
document.querySelector('#size').addEventListener('change',e=>document.documentElement.style.setProperty('--size',e.target.value+'px'));
</script></html>'''.replace("SECTIONS", "".join(sections))
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(document, encoding="utf-8")
    print(args.out)


if __name__ == "__main__":
    main()
