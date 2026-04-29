#!/usr/bin/env python3
"""Convert UI SVG sources to PNG assets for the Bevy game."""
from pathlib import Path
import cairosvg

ROOT = Path(__file__).resolve().parent
OUT = ROOT.parent.parent / "assets" / "ui"
OUT.mkdir(parents=True, exist_ok=True)

# (svg_name, output_name, scale)
JOBS = [
    ("panel_bg.svg", "panel_bg.png", 2.0),
    ("halo.svg",     "halo.png",     2.0),
    ("flourish.svg", "flourish.png", 2.0),
    ("tab_bg.svg",   "tab_bg.png",   2.0),
    ("slot_frame.svg","slot_frame.png", 2.0),
    ("divider.svg",  "divider.png",  2.0),
]

for src, dst, scale in JOBS:
    src_path = ROOT / src
    dst_path = OUT / dst
    cairosvg.svg2png(
        url=str(src_path),
        write_to=str(dst_path),
        scale=scale,
    )
    print(f"  {src}  ->  {dst_path.name}")

print(f"Done. Assets in {OUT}")
