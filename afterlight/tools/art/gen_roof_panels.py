"""Roof plates for the zoomed-out view of the player's ship (vfx/ship_roof.rs).

One 256x256 image per hull material, covering a 2x2 block of cells: a flat
plate in the material texture's average colour, a faint light falloff across
the whole plate (never per cell -- shading every tile is what made the old
roof read as corrugated sheet), a fine grain, a seam around the plate's edge,
and the hull texture's own rivets at its corners and mid-edges.

    python3 tools/art/gen_roof_panels.py

Writes assets/sprites/hull/roof_<material>.png.
"""

import random
from pathlib import Path

from PIL import Image

HULL = Path(__file__).resolve().parents[2] / "assets" / "sprites" / "hull"
MATERIALS = ["steel", "titanium", "composite", "abyssal"]
SIZE = 256  # two cells of 128 px each way
FALLOFF = 0.05  # +-5% light across the whole plate
GRAIN = 0.025  # +-2.5% per pixel
RIVET = 12  # the rivet square in the source art's corner, in px
SEAM = 8  # px of seam on each plate edge


def average(img):
    px = [p for p in img.getdata() if p[3] > 128]
    return [sum(p[i] for p in px) / len(px) for i in range(3)]


def make(material, rng):
    src = Image.open(HULL / f"hull_{material}_15.png").convert("RGBA")
    base = average(Image.open(HULL / f"hull_{material}.png").convert("RGBA"))
    out = Image.new("RGBA", (SIZE, SIZE))
    px = out.load()
    for y in range(SIZE):
        for x in range(SIZE):
            # Light from the top left, across the plate as one surface.
            light = 1.0 + FALLOFF * (1.0 - (x + y) / SIZE)
            grain = 1.0 + rng.uniform(-GRAIN, GRAIN)
            px[x, y] = tuple(max(0, min(255, round(c * light * grain))) for c in base) + (255,)
    # The seam: a dark band on the plate's edge, easing into the plate, and a
    # faint highlight just inside it. Wide, because the roof is only seen from
    # far out, where a cell is ten-odd pixels and a thin line vanishes.
    def mix(c, k):
        return tuple(max(0, min(255, round(v * k))) for v in c[:3]) + (255,)
    for y in range(SIZE):
        for x in range(SIZE):
            d = min(x, y, SIZE - 1 - x, SIZE - 1 - y)
            if d < SEAM:
                px[x, y] = mix(px[x, y], 0.6 + 0.4 * (d / SEAM) ** 2)
            elif d < SEAM + 3:
                px[x, y] = mix(px[x, y], 1.12)
    # Rivets, lifted from the source tile's corner and set at the plate's
    # corners and the middle of each edge.
    rivet = src.crop((0, 0, RIVET, RIVET))
    m = SIZE // 2 - RIVET // 2
    e = SIZE - RIVET
    for (x, y) in [(0, 0), (e, 0), (0, e), (e, e), (m, 0), (m, e), (0, m), (e, m)]:
        out.alpha_composite(rivet, (x + SEAM if x == 0 else x - SEAM if x == e else x, y + SEAM if y == 0 else y - SEAM if y == e else y))
    return out


def main():
    rng = random.Random(7)
    for material in MATERIALS:
        path = HULL / f"roof_{material}.png"
        make(material, rng).save(path)
        print("wrote", path)


if __name__ == "__main__":
    main()
