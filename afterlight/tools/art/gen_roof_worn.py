"""Weathered roof for the Cosmoteer look (vfx::ship_roof, AFTERLIGHT_COSMO).

From each quiet roof plate (gen_roof_panels.py) makes roof_<mat>_worn.png:
broad grime that pools darker in patches, fine scratches, paint chipped
light along the plate seams, soot in the corners. Its average is put back to
the quiet plate's, so the game's paint maths (plate_mean) still holds.

Also writes sprites/roof_marks/light.png: a soft light for indicator lamps.

    python3 tools/art/gen_roof_worn.py
"""
import math, os, random
from PIL import Image, ImageDraw, ImageFilter, ImageStat, ImageChops

HERE = os.path.dirname(__file__)
HULL = os.path.join(HERE, "..", "..", "assets", "sprites", "hull")
MARKS = os.path.join(HERE, "..", "..", "assets", "sprites", "roof_marks")
MATERIALS = ["steel", "titanium", "composite", "abyssal"]


def noise(size, scale, rng):
    """Smooth value noise in 0..255, tiling."""
    small = max(2, size // scale)
    im = Image.new("L", (small, small))
    im.putdata([rng.randint(0, 255) for _ in range(small * small)])
    big = im.resize((size, size), Image.BICUBIC)
    return big.filter(ImageFilter.GaussianBlur(scale / 4))


def grime(size, rng):
    """0..255 how dirty each pixel is: broad patches plus fine speckle."""
    a = noise(size, 64, rng)
    b = noise(size, 16, rng)
    out = ImageChops.multiply(a, a)
    out = ImageChops.add(out, b.point(lambda v: v // 3))
    return out


def worn(material, rng):
    src = Image.open(os.path.join(HULL, f"roof_{material}_quiet.png")).convert("RGBA")
    size = src.width
    mean_before = ImageStat.Stat(src.convert("RGB")).mean
    px = src.convert("RGB")
    # Grime: darken by up to 32% where dirty.
    g = grime(size, rng)
    dark = Image.new("RGB", (size, size), (18, 16, 14))
    mask = g.point(lambda v: int(min(255, max(0, (v - 70) * 0.9))))
    px = Image.composite(dark, px, mask.point(lambda v: int(v * 0.36)))
    d = ImageDraw.Draw(px, "RGBA")
    # Scratches: short thin strokes, light over dark.
    for _ in range(46):
        x, y = rng.uniform(0, size), rng.uniform(0, size)
        a = rng.uniform(0, math.pi)
        L = rng.uniform(6, 26)
        x2, y2 = x + math.cos(a) * L, y + math.sin(a) * L
        d.line([(x + 1, y + 1), (x2 + 1, y2 + 1)], fill=(0, 0, 0, 70), width=1)
        d.line([(x, y), (x2, y2)], fill=(235, 235, 235, 60), width=1)
    # Chipped paint along the seams (the plate's edges and its cross seam).
    seams = [0, size // 2 - 1, size // 2, size - 1]
    for _ in range(260):
        if rng.random() < 0.5:
            x = rng.choice(seams) + rng.randint(-5, 5)
            y = rng.uniform(0, size)
        else:
            y = rng.choice(seams) + rng.randint(-5, 5)
            x = rng.uniform(0, size)
        r = rng.uniform(0.6, 1.8)
        d.ellipse([x - r, y - r, x + r, y + r], fill=(220, 214, 200, 90))
    # Soot in each plate's corners.
    for cx in (0, size // 2, size):
        for cy in (0, size // 2, size):
            soot = Image.new("L", (size, size), 0)
            ImageDraw.Draw(soot).ellipse([cx - 40, cy - 40, cx + 40, cy + 40], fill=110)
            soot = soot.filter(ImageFilter.GaussianBlur(18))
            px = Image.composite(dark, px, soot)
    # Back to the quiet plate's average, channel by channel.
    mean_after = ImageStat.Stat(px).mean
    k = [mean_before[i] / max(mean_after[i], 1) for i in range(3)]
    px = Image.merge("RGB", [ch.point(lambda v, k=k[i]: min(255, int(v * k))) for i, ch in enumerate(px.split())])
    out = px.convert("RGBA")
    out.putalpha(src.getchannel("A"))
    out.save(os.path.join(HULL, f"roof_{material}_worn.png"))


def light():
    n = 64
    im = Image.new("RGBA", (n, n), (0, 0, 0, 0))
    for y in range(n):
        for x in range(n):
            r = math.hypot(x - n / 2 + 0.5, y - n / 2 + 0.5) / (n / 2)
            core = max(0.0, 1 - r / 0.28)
            halo = max(0.0, 1 - r) ** 2.2
            a = min(1.0, core + halo * 0.55)
            w = int(255 * min(1.0, core * 1.2))
            im.putpixel((x, y), (255, 255 - w // 4, 255 - w // 4, int(255 * a)))
    os.makedirs(MARKS, exist_ok=True)
    im.save(os.path.join(MARKS, "light.png"))


rng = random.Random(7)
for m in MATERIALS:
    worn(m, rng)
light()
print("worn roof plates and light.png written")
