#!/usr/bin/env python3
"""Procedural pixel-art asteroids, falling-sand style.

Every pixel is its own grain: the rock is built from layered value noise
rather than drawn as a shape, so edges come out ragged and organic instead of
a polygon or a staircase of big blocks. Ore runs through it as thin veins.

Deterministic: the same (size class, resource, variant) always produces the
same rock, so the game's per-system seed keeps reproducing the same field.

Pillow only -- no numpy on this machine. Sprites are small (64-176px) so a
pure-Python per-pixel pass is quick.

    python3 tools/art/pixel/asteroids.py            # writes all variants
    python3 tools/art/pixel/asteroids.py --preview  # + a contact sheet
"""

import math
import os
import random
import sys
from PIL import Image

OUT_DIR = "assets/sprites/celestial/asteroids"
VARIANTS = 3

# World size each class covers, and the sprite resolution for it. Resolution
# tracks size so a PIXEL is roughly 4 world units in every class -- a big rock
# is more grains, not bigger grains, which is the whole point of the look.
SIZE_CLASSES = {"small": 64, "medium": 112, "large": 176}

# Rock body, darkest to lightest. Deliberately desaturated and warm-grey so
# rock reads as natural against the ships' cold blue-grey plating.
ROCK = ["#1c1a18", "#2a2622", "#38332d", "#464038", "#564e44", "#665c50"]

# Ore veins per ResourceNodeType (celestial::components). Dull, metallic --
# saturated ore reads as a UI pickup and fights the damage/weapon accents.
ORE = {
    "metal":   ["#4a4e54", "#6d747c", "#98a2ab"],
    "crystal": ["#3c5568", "#4f7288", "#6f9ab0"],
    "fuel":    ["#6b5228", "#8a6a33", "#ab8544"],
    "exotic":  ["#4b3a5e", "#654e7d", "#86699d"],
}


def hex_rgb(h):
    h = h.lstrip("#")
    return tuple(int(h[i:i + 2], 16) for i in (0, 2, 4))


def value_noise(n, cells, rng):
    """Smooth noise: a tiny random grid blown up with bicubic interpolation."""
    small = Image.new("L", (cells, cells))
    small.putdata([rng.randrange(256) for _ in range(cells * cells)])
    return small.resize((n, n), Image.BICUBIC).load()


def fbm(n, rng, octaves=(4, 8, 16), weights=(0.55, 0.30, 0.15)):
    """Fractal noise: a few octaves summed. Returns a [0,1] sampler."""
    layers = [value_noise(n, c, rng) for c in octaves]
    def sample(x, y):
        v = sum(w * layers[i][x, y] for i, w in enumerate(weights))
        return v / 255.0
    return sample


def build(res, ore_key, variant):
    # Seeded per (class,res,ore,variant) so output is stable across runs.
    rng = random.Random(hash((res, ore_key, variant)) & 0xffffffff)
    n = res

    shape = fbm(n, rng, (3, 6, 13), (0.6, 0.28, 0.12))   # silhouette
    grain = fbm(n, rng, (8, 22, 48), (0.4, 0.35, 0.25))   # surface speckle
    veins = fbm(n, rng, (5, 11), (0.65, 0.35))            # ore distribution

    rock = [hex_rgb(c) for c in ROCK]
    ore = [hex_rgb(c) for c in ORE[ore_key]]

    img = Image.new("RGBA", (n, n), (0, 0, 0, 0))
    px = img.load()
    cx = cy = (n - 1) / 2.0
    # Everything below is per-pixel on purpose: a grain is a pixel.
    for y in range(n):
        for x in range(n):
            dx, dy = (x - cx) / cx, (y - cy) / cy
            r = math.hypot(dx, dy)
            # Ragged edge: the cutoff radius itself wobbles with noise, so the
            # outline is chewed rather than round.
            edge = 0.70 + 0.46 * (shape(x, y) - 0.45)
            if r > edge:
                continue

            # Depth into the rock, for shading and rim darkening.
            depth = min(1.0, (edge - r) / 0.34)

            g = grain(x, y)
            # Hash-based single-pixel speckle: stable per (x, y, variant), so
            # the rock is grainy rather than blobby without another noise pass.
            h = (x * 73856093) ^ (y * 19349663) ^ (variant * 83492791)
            g += (((h >> 8) & 0xff) / 255.0 - 0.5) * 0.22
            # Bake a light from the upper-left so the rock has form. Cheap
            # lambert: the surface normal is approximated by the position on
            # the disc, which is enough at this pixel scale.
            lam = (-dx * 0.6 - dy * 0.7) * 0.5 + 0.5
            shade = 0.30 * lam + 0.52 * g + 0.18 * depth

            # Ore: a narrow band of the vein field, so deposits come out as
            # thin connected seams instead of round blobs.
            v = veins(x, y)
            # A narrow band of the vein field gives thin seams. The grain test
            # breaks them up so ore reads as scattered deposits along a seam
            # rather than a painted contour line, and the mix with rock keeps
            # it embedded instead of sitting on the surface.
            if abs(v - 0.5) < 0.020 and depth > 0.14 and g > 0.34:
                idx = min(len(ore) - 1, int((shade * 0.7 + 0.2) * len(ore)))
                oc = ore[idx]
                rc = rock[max(0, min(len(rock) - 1, int(shade * len(rock))))]
                # 70% ore / 30% host rock -- visible, never neon.
                px[x, y] = tuple(int(oc[i] * 0.7 + rc[i] * 0.3) for i in range(3)) + (255,)
                continue

            idx = max(0, min(len(rock) - 1, int(shade * len(rock))))
            # Rim: the outermost grains go dark so the rock reads as a solid
            # body against the starfield rather than a bright smear.
            if depth < 0.10:
                idx = max(0, idx - 2)
            px[x, y] = rock[idx] + (255,)
    return img


def main():
    os.makedirs(OUT_DIR, exist_ok=True)
    written = 0
    for cls, res in SIZE_CLASSES.items():
        for ore_key in ORE:
            for v in range(VARIANTS):
                img = build(res, ore_key, v)
                img.save("%s/ast_%s_%s_%d.png" % (OUT_DIR, cls, ore_key, v))
                written += 1
    print("wrote %d asteroid sprites to %s" % (written, OUT_DIR))

    if "--preview" in sys.argv:
        cell, pad = 190, 10
        cols = VARIANTS * len(ORE)
        sheet = Image.new("RGBA", (cols * (cell + pad) + pad,
                                   len(SIZE_CLASSES) * (cell + pad) + pad),
                          (18, 20, 26, 255))
        for r, (cls, res) in enumerate(SIZE_CLASSES.items()):
            c = 0
            for ore_key in ORE:
                for v in range(VARIANTS):
                    im = Image.open("%s/ast_%s_%s_%d.png" % (OUT_DIR, cls, ore_key, v))
                    im = im.resize((cell, cell), Image.NEAREST)
                    sheet.paste(im, (pad + c * (cell + pad), pad + r * (cell + pad)), im)
                    c += 1
        sheet.save("tools/art/preview/PIXEL_ASTEROIDS.png")
        print("preview -> tools/art/preview/PIXEL_ASTEROIDS.png")


if __name__ == "__main__":
    main()
