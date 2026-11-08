"""Placeholder art for the filler blocks: Light Frame, Supply Crates, Coolant
Run, Ballast Block. Drawn in the house module style -- a dark steel tile with
a rounded frame, corner bolts, soft bevels and orange accents -- at the
modules' 378 px. Writes assets/sprites/modules/{light_frame, supply_crates,
coolant_run, ballast_block}.png.

Run from afterlight/: python3 tools/filler-art/filler_art.py
"""
import os
from PIL import Image, ImageDraw

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
OUT = os.path.join(ROOT, "assets/sprites/modules")
N = 378
BASE = (60, 66, 77)
DEEP = (40, 44, 52)
PANEL = (50, 56, 65)
LIT = (78, 85, 98)
SHADE = (34, 38, 45)
ORANGE = (186, 138, 64)
BOLT = (88, 95, 108)


def tile():
    im = Image.new("RGBA", (N, N), BASE + (255,))
    d = ImageDraw.Draw(im)
    d.rounded_rectangle([0, 0, N - 1, N - 1], radius=14, outline=DEEP, width=8)
    for x, y in [(26, 26), (N - 27, 26), (26, N - 27), (N - 27, N - 27)]:
        d.ellipse([x - 9, y - 9, x + 9, y + 9], fill=SHADE)
        d.ellipse([x - 6, y - 6, x + 6, y + 6], fill=BOLT)
    return im, d


def bevel(d, box, fill, lit=LIT, dark=SHADE, w=6, r=10):
    x0, y0, x1, y1 = box
    d.rounded_rectangle(box, radius=r, fill=fill)
    d.line([(x0 + r, y0 + w // 2), (x1 - r, y0 + w // 2)], fill=lit, width=w)
    d.line([(x0 + w // 2, y0 + r), (x0 + w // 2, y1 - r)], fill=lit, width=w)
    d.line([(x0 + r, y1 - w // 2), (x1 - r, y1 - w // 2)], fill=dark, width=w)
    d.line([(x1 - w // 2, y0 + r), (x1 - w // 2, y1 - r)], fill=dark, width=w)


def light_frame():
    """Open girders: a square frame, cross-braced, the floor dark between."""
    im, d = tile()
    lo, hi = 48, N - 49
    d.rectangle([lo, lo, hi, hi], fill=DEEP)
    w = 30
    for box in ([lo, lo, hi, lo + w], [lo, hi - w, hi, hi], [lo, lo, lo + w, hi], [hi - w, lo, hi, hi]):
        bevel(d, box, PANEL, w=5, r=4)
    d.line([(lo + w, lo + w), (hi - w, hi - w)], fill=PANEL, width=26)
    d.line([(lo + w, hi - w), (hi - w, lo + w)], fill=PANEL, width=26)
    d.line([(lo + w, lo + w - 8), (hi - w + 8, hi - w)], fill=LIT, width=5)
    c = N // 2
    d.rectangle([c - 22, c - 22, c + 22, c + 22], fill=BASE)
    d.ellipse([c - 9, c - 9, c + 9, c + 9], fill=BOLT)
    for x in (lo + 15, hi - 15):
        for y in (lo + 15, hi - 15):
            d.ellipse([x - 5, y - 5, x + 5, y + 5], fill=BOLT)
    return im


def supply_crates():
    """A stack of four crates, one knocked askew, strapped in orange."""
    im, d = tile()
    crate = (112, 96, 70)
    for (x, y, s) in [(52, 52, 130), (196, 60, 122), (56, 196, 124), (192, 192, 134)]:
        bevel(d, [x, y, x + s, y + s], crate, lit=(140, 122, 92), dark=(70, 58, 42), w=6, r=6)
        d.line([(x + 14, y + 14), (x + s - 14, y + s - 14)], fill=(88, 74, 52), width=8)
        d.line([(x + 14, y + s - 14), (x + s - 14, y + 14)], fill=(88, 74, 52), width=8)
        d.rectangle([x + s // 2 - 6, y + 4, x + s // 2 + 6, y + s - 4], fill=ORANGE)
    return im


def coolant_run():
    """Two pipes across the block with flanges, a valve wheel between them."""
    im, d = tile()
    pipe = (66, 92, 108)
    for y in (122, 238):
        d.rectangle([0, y - 26, N, y + 26], fill=SHADE)
        d.rectangle([0, y - 22, N, y + 22], fill=pipe)
        d.line([(0, y - 12), (N, y - 12)], fill=(118, 160, 178), width=7)
        d.line([(0, y + 16), (N, y + 16)], fill=(40, 58, 70), width=6)
        for x in (70, N - 70):
            bevel(d, [x - 14, y - 34, x + 14, y + 34], (80, 88, 100), w=4, r=4)
    c = N // 2
    d.rectangle([c - 10, 140, c + 10, 222], fill=pipe)
    d.ellipse([c - 44, 180 - 44, c + 44, 180 + 44], outline=ORANGE, width=12)
    d.line([(c - 40, 180), (c + 40, 180)], fill=ORANGE, width=8)
    d.line([(c, 140), (c, 220)], fill=ORANGE, width=8)
    d.ellipse([c - 12, 168, c + 12, 192], fill=BOLT)
    return im


def ballast_block():
    """A heavy slab: thick bevel, hazard edge, big bolts."""
    im, d = tile()
    bevel(d, [40, 40, N - 41, N - 41], (54, 58, 66), lit=(84, 90, 102), dark=(28, 31, 37), w=14, r=12)
    bevel(d, [92, 92, N - 93, N - 93], (46, 50, 58), lit=(30, 33, 39), dark=(72, 78, 90), w=8, r=8)
    for i in range(0, 200, 28):
        d.polygon([(110 + i, 300), (126 + i, 300), (110 + i + 16 - 32, 318), (110 + i - 32, 318)], fill=ORANGE)
    d.rectangle([92, 300, N - 93, 318], outline=None)
    for x, y in [(70, 70), (N - 71, 70), (70, N - 71), (N - 71, N - 71)]:
        d.ellipse([x - 15, y - 15, x + 15, y + 15], fill=SHADE)
        d.ellipse([x - 11, y - 11, x + 11, y + 11], fill=BOLT)
    return im


for name, draw in [("light_frame", light_frame), ("supply_crates", supply_crates), ("coolant_run", coolant_run), ("ballast_block", ballast_block)]:
    draw().save(os.path.join(OUT, name + ".png"))
    print("wrote", name)
