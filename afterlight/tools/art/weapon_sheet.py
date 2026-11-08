#!/usr/bin/env python3
"""Contact sheet for the candidate weapon sprites.

Turret weapons are shown assembled (base + rotating barrel) the way the engine
draws them: the barrel sprite renders at 132 world units against a 66-unit
cell, so it is composited at twice the plate's size.
"""

import os
from PIL import Image, ImageDraw, ImageFont

SRC = "tools/art/preview/weapons"
OUT = "tools/art/preview/ALL_WEAPONS.png"

TURRETS = ["railgun", "cannon", "coilgun", "gatling", "mining_drill"]
FIXED = ["laser", "plasma_caster", "ion_disruptor", "emp_pulse", "tractor_beam",
         "heavy_missile", "guided_missile", "cluster_rocket", "ammo_autoloader"]

f = ImageFont.truetype("/System/Library/Fonts/Supplemental/Arial.ttf", 16)
fb = ImageFont.truetype("/System/Library/Fonts/Supplemental/Arial Bold.ttf", 18)


def assembled(name):
    plate = Image.open("%s/%s.png" % (SRC, name)).convert("RGBA")
    c = Image.new("RGBA", (756, 756), (0, 0, 0, 0))
    c.paste(plate.resize((378, 378), Image.LANCZOS), (189, 189))
    bp = "%s/turret_%s_barrel.png" % (SRC, name)
    if os.path.exists(bp):
        c.alpha_composite(Image.open(bp).convert("RGBA").resize((756, 756), Image.LANCZOS))
    return c


CELL, PAD, LBL, COLS = 168, 16, 26, 5
rows = 1 + (len(FIXED) + COLS - 1) // COLS
W = COLS * (CELL + PAD) + PAD
H = 44 + rows * (CELL + PAD + LBL) + 40
sheet = Image.new("RGBA", (W, H), (18, 20, 26, 255))
d = ImageDraw.Draw(sheet)
d.text((PAD, 12), "Candidate weapon sprites  -  14 weapons, 19 files", font=fb,
       fill=(190, 200, 215))

y0 = 48
d.text((PAD, y0), "TURRETS  (base + rotating barrel, shown assembled)", font=f,
       fill=(150, 190, 160))
for i, n in enumerate(TURRETS):
    cx, cy = PAD + i * (CELL + PAD), y0 + 24
    im = assembled(n); im.thumbnail((CELL, CELL), Image.LANCZOS)
    d.rectangle([cx-1, cy-1, cx+CELL, cy+CELL], outline=(44, 50, 60))
    sheet.paste(im, (cx + (CELL-im.width)//2, cy + (CELL-im.height)//2), im)
    d.text((cx, cy + CELL + 4), n, font=f, fill=(165, 175, 190))

y1 = y0 + 24 + CELL + LBL + 24
d.text((PAD, y1), "FIXED MOUNTS  (no barrel - fire from the module)", font=f,
       fill=(150, 190, 160))
for i, n in enumerate(FIXED):
    cx = PAD + (i % COLS) * (CELL + PAD)
    cy = y1 + 24 + (i // COLS) * (CELL + PAD + LBL)
    im = Image.open("%s/%s.png" % (SRC, n)).convert("RGBA")
    im.thumbnail((CELL, CELL), Image.LANCZOS)
    d.rectangle([cx-1, cy-1, cx+CELL, cy+CELL], outline=(44, 50, 60))
    sheet.paste(im, (cx + (CELL-im.width)//2, cy + (CELL-im.height)//2), im)
    d.text((cx, cy + CELL + 4), n, font=f, fill=(165, 175, 190))

sheet.save(OUT)
print("wrote", OUT)
