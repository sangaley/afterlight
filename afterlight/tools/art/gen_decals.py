"""Decal shapes for the paint editor (building::paint_mode, vfx::ship_roof).

Each is a white shape on a clear tile, tinted with the decal's colour in the
game and turned in quarter turns. Drawn for rotation 0; the game turns them.

    python3 tools/art/gen_decals.py   -> assets/sprites/decals/<shape>.png
"""
import os
from PIL import Image, ImageDraw

OUT = os.path.join(os.path.dirname(__file__), "..", "..", "assets", "sprites", "decals")
N, SS = 64, 4
W = (255, 255, 255, 255)


def shape(name, draw):
    im = Image.new("RGBA", (N * SS, N * SS), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    s = N * SS
    draw(d, s)
    im.resize((N, N), Image.LANCZOS).save(os.path.join(OUT, f"{name}.png"))


SHAPES = {
    "full": lambda d, s: d.rectangle([0, 0, s, s], fill=W),
    "half": lambda d, s: d.rectangle([0, s / 2, s, s], fill=W),
    "diagonal": lambda d, s: d.polygon([(0, 0), (0, s), (s, s)], fill=W),
    "corner": lambda d, s: d.polygon([(0, s / 2), (0, s), (s / 2, s)], fill=W),
    "quarter": lambda d, s: d.rectangle([0, s / 2, s / 2, s], fill=W),
    "edge": lambda d, s: d.rectangle([0, s * 0.78, s, s], fill=W),
    "stripe": lambda d, s: d.rectangle([0, s * 0.38, s, s * 0.62], fill=W),
    "slash": lambda d, s: d.polygon([(0, s), (s * 0.3, s), (s, s * 0.3), (s, 0), (s * 0.7, 0), (0, s * 0.7)], fill=W),
    "circle": lambda d, s: d.ellipse([s * 0.12, s * 0.12, s * 0.88, s * 0.88], fill=W),
    "arc": lambda d, s: d.pieslice([-s, 0, s, 2 * s], 270, 360, fill=W),
    "chevron": lambda d, s: d.polygon([(0, s * 0.25), (s / 2, s * 0.75), (s, s * 0.25), (s, s * 0.5), (s / 2, s), (0, s * 0.5)], fill=W),
    "dot": lambda d, s: d.ellipse([s * 0.34, s * 0.34, s * 0.66, s * 0.66], fill=W),
}

os.makedirs(OUT, exist_ok=True)
for name, draw in SHAPES.items():
    shape(name, draw)
print(f"{len(SHAPES)} decals in {os.path.normpath(OUT)}")
