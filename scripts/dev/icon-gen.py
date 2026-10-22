#!/usr/bin/env python3
"""The page's icon (docs/WEB.md 3.8): what a browser shows on the tab and on the game
installed as an app, at the two sizes a web app's manifest names (192 and 512 texels).
The skin's frame (scripts/dev/skin-gen.py) round the title face's "G". Writes
`icon-192.png` and `icon-512.png`; a person who draws a better one replaces the files.

    scripts/dev/icon-gen.py [web]
"""
import importlib.util
import os
import sys

from PIL import ImageFont

HERE = os.path.dirname(os.path.abspath(__file__))
# The skin's drawing, without leaving a __pycache__ beside it.
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("skin_gen", os.path.join(HERE, "skin-gen.py"))
skin = importlib.util.module_from_spec(spec)
spec.loader.exec_module(skin)

FACE = os.path.join(HERE, "..", "..", "assets", "content", "ui", "medievalsharp.ttf")
# The picture in dots: 192 texels at 3 a dot, 512 at 8.
DOTS = 64
SIZES = ((192, 3), (512, 8))


def icon(k, letter="G"):
    s = DOTS
    p = skin.Pic(s, s, k)
    skin.framed(p, 6, 6, (46, 42, 52), (30, 27, 34))
    # Corner studs, as the portrait frame has.
    for x, y in [(3.2, 3.2), (s - 3.2, 3.2), (3.2, s - 3.2), (s - 3.2, s - 3.2)]:
        p.ellipse(x - 1.6, y - 1.6, x + 1.6, y + 1.6, skin.BRONZE_DARK)
        p.ellipse(x - 1.1, y - 1.35, x + 1.1, y + 0.8, skin.BRONZE_LIGHT)
    # The letter's ink centred in the inside, gold with a dark edge.
    font = ImageFont.truetype(FACE, round(54 * p.u))
    edge = round(1.0 * p.u)
    left, top, right, bottom = font.getbbox(letter, stroke_width=edge)
    cx, cy = s / 2 * p.u, s / 2 * p.u
    p.d.text((cx - (left + right) / 2, cy - (top + bottom) / 2), letter, font=font,
             fill=skin.GOLD, stroke_width=edge, stroke_fill=skin.OUTLINE)
    return p.done()


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, "..", "..", "web")
    for size, k in SIZES:
        icon(k).save(os.path.join(out, f"icon-{size}.png"), optimize=True)
    print(f"icon: {', '.join(str(size) for size, _ in SIZES)} written to {out}")


if __name__ == "__main__":
    main()
