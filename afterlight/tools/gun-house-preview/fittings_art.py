"""Placeholder art for the gun house roof (style D): the roof plate and wedge,
baked lighter from the turret deck's armour, and the four roof fittings drawn
on that plate. Writes assets/sprites/gun_parts/{roof_plate, roof_wedge,
rangefinder, roof_vent, periscope, armoured_hatch}.png.

Run from afterlight/: python3 tools/gun-house-preview/fittings_art.py
"""
import os
from PIL import Image, ImageDraw

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
GP = os.path.join(ROOT, "assets/sprites/gun_parts")
LIGHT = (1.6, 1.66, 1.8)


def lighter(name):
    im = Image.open(os.path.join(GP, name)).convert("RGBA")
    r, g, b, a = im.split()
    f = lambda k: (lambda v: min(255, int(v * k)))
    return Image.merge("RGBA", (r.point(f(LIGHT[0])), g.point(f(LIGHT[1])), b.point(f(LIGHT[2])), a))


def box(d, x0, y0, x1, y1, fill, lit=(150, 165, 185), dark=(30, 34, 42), w=3):
    d.rectangle([x0, y0, x1, y1], fill=fill)
    d.line([(x0, y1), (x0, y0), (x1, y0)], fill=lit, width=w)
    d.line([(x1, y0), (x1, y1), (x0, y1)], fill=dark, width=w)


plate = lighter("turret_armour.png")
plate.save(os.path.join(GP, "roof_plate.png"))
lighter("turret_armour_wedge.png").save(os.path.join(GP, "roof_wedge.png"))
STEEL = (78, 88, 104)

# Rangefinder: a long housing across the plate, a lens at each end.
im = plate.copy(); d = ImageDraw.Draw(im)
box(d, 14, 50, 114, 78, STEEL)
for x0 in (14, 100):
    d.rectangle([x0 + 2, 54, x0 + 12, 74], fill=(26, 30, 36))
    d.rectangle([x0 + 4, 57, x0 + 10, 63], fill=(120, 185, 215))
d.line([(30, 64), (98, 64)], fill=(55, 62, 74), width=2)
im.save(os.path.join(GP, "rangefinder.png"))

# Roof vent: a framed grille of slats.
im = plate.copy(); d = ImageDraw.Draw(im)
box(d, 24, 24, 104, 104, (52, 58, 68))
for y in range(34, 98, 10):
    d.rectangle([32, y, 96, y + 5], fill=(24, 27, 32))
    d.line([(32, y), (96, y)], fill=(110, 122, 140), width=1)
im.save(os.path.join(GP, "roof_vent.png"))

# Periscope: a raised armoured hood with a glass slit facing forward (east).
im = plate.copy(); d = ImageDraw.Draw(im)
box(d, 38, 36, 92, 92, STEEL)
d.polygon([(92, 44), (106, 52), (106, 76), (92, 84)], fill=(60, 68, 80))
d.rectangle([94, 54, 104, 74], fill=(120, 185, 215))
d.rectangle([48, 46, 82, 52], fill=(58, 66, 78))
im.save(os.path.join(GP, "periscope.png"))

# Armoured hatch: a square lid, inner bevel, hinge and handle.
im = plate.copy(); d = ImageDraw.Draw(im)
box(d, 26, 26, 102, 102, (70, 79, 94), w=4)
d.rectangle([36, 36, 92, 92], outline=(140, 155, 175), width=2)
d.rectangle([20, 52, 28, 76], fill=(40, 44, 52))
d.rectangle([70, 58, 84, 70], fill=(40, 44, 52))
d.rectangle([73, 61, 81, 67], fill=(150, 160, 175))
im.save(os.path.join(GP, "armoured_hatch.png"))
print("roof art written")


# ---- angle pieces: a plate cut by one straight line (building::module_shape) ----
import math

SHAPES = {
    # name: (outward normal angle at North, cut)
    "bevelled_plate": (math.pi / 4, 0.6 / math.sqrt(2)),
    "slope_low": (math.atan2(1, -0.5), -0.25 / math.sqrt(1.25)),
    "slope_high": (math.atan2(1, -0.5), 0.25 / math.sqrt(1.25)),
    "slope_low_mirror": (math.atan2(1, 0.5), -0.25 / math.sqrt(1.25)),
    "slope_high_mirror": (math.atan2(1, 0.5), 0.25 / math.sqrt(1.25)),
    "half_plate": (math.pi / 2, 0.0),
}


def cut_piece(base, angle, cut):
    """The base plate, kept where p . n <= cut (p in cells from the centre,
    y up), with a lit edge along the cut face."""
    im = base.copy().convert("RGBA")
    w, h = im.size
    n = (math.cos(angle), math.sin(angle))
    mask = Image.new("L", im.size, 0)
    px = mask.load()
    for y in range(h):
        for x in range(w):
            p = ((x + 0.5) / w - 0.5, 0.5 - (y + 0.5) / h)
            if p[0] * n[0] + p[1] * n[1] <= cut:
                px[x, y] = 255
    a = im.getchannel("A")
    from PIL import ImageChops
    im.putalpha(ImageChops.multiply(a, mask))
    # The cut face catches the light: a bright line along it.
    d = ImageDraw.Draw(im)
    t = (-n[1], n[0])
    pts = []
    for s in (-1.5, 1.5):
        cx, cy = n[0] * cut + t[0] * s, n[1] * cut + t[1] * s
        pts.append(((cx + 0.5) * w, (0.5 - cy) * h))
    edge = Image.new("RGBA", im.size)
    ImageDraw.Draw(edge).line(pts, fill=(205, 215, 230, 255), width=max(3, w // 40))
    edge.putalpha(ImageChops.multiply(edge.getchannel("A"), mask))
    im.alpha_composite(edge)
    return im


hull_base = Image.open(os.path.join(ROOT, "assets/sprites/modules/hull_beam.png")).convert("RGBA")
for name, (angle, cut) in SHAPES.items():
    cut_piece(hull_base, angle, cut).save(os.path.join(ROOT, "assets/sprites/modules", f"{name}.png"))
    cut_piece(plate, angle, cut).save(os.path.join(GP, f"roof_{name}.png"))
print("angle pieces written")
