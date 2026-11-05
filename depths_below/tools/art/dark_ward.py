#!/usr/bin/env python3
"""The Dark Ward (narrative::ward): the Silence's way of keeping quiet when the
dark comes close. Same dark, block-filling style as darkset.py, drawn alone so
the rest of the set isn't regenerated with it.

A black well in the middle that swallows light instead of giving it off,
ringed by four violet dampers aimed inward and a ring of small cold lights.
Violet is the Silence's colour; nothing here is blue (power) or orange
(thrust), because the ward doesn't make anything, it holds something out.

    cd depths_below/tools/art && python3 dark_ward.py
"""
import os
from darkset import *          # helpers, dark palette, R/U/P/save/layer/compose
from pixelate_install import pixelate, SRC, DST

FN = "dark_ward.png"


def dark_ward():
    r = R(1, 1); W = r.WU_w; cx = W / 2; cy = W / 2
    fill_block(r, 6, 6, W - 6, W - 6, "ward")

    # The well: stepped rings going down into nothing.
    for rad, col in ((40, mix(STEEL_D, DARK, 0.2)), (33, mix(STEEL_D, DARK, 0.55)), (26, (14, 12, 22)), (19, (6, 5, 10))):
        r.d.ellipse([U(r, cx - rad), U(r, cy - rad), U(r, cx + rad), U(r, cy + rad)], fill=col + (255,), outline=EDGE + (255,), width=SS * 2)
    # A faint violet rim where the light is pulled in, inside the well, not around it.
    g, gd = layer(r)
    for i in range(10):
        rad = 25 - i * 0.7
        gd.ellipse([U(r, cx - rad), U(r, cy - rad), U(r, cx + rad), U(r, cy + rad)], outline=A(VIO, 70), width=SS)
    compose(r, blur(g, 1.0))
    r.d.ellipse([U(r, cx - 12), U(r, cy - 12), U(r, cx + 12), U(r, cy + 12)], fill=(2, 2, 4, 255))

    # Four dampers, aimed into the well.
    for ang in (45, 135, 225, 315):
        a = math.radians(ang); ux, uy = math.cos(a), math.sin(a)
        x0, y0 = cx + ux * 50, cy + uy * 50
        x1, y1 = cx + ux * 34, cy + uy * 34
        pipe(r, x0, y0, x1, y1, w=9, col=mix(STEEL, STEEL_D, 0.4))
        coil_x, coil_y = cx + ux * 44, cy + uy * 44
        r.d.ellipse([U(r, coil_x - 5), U(r, coil_y - 5), U(r, coil_x + 5), U(r, coil_y + 5)], fill=mix(VIO, DARK, 0.4) + (255,), outline=EDGE + (255,), width=SS)
        small_glow(r, cx + ux * 30, cy + uy * 30, 4.5, VIO, (224, 212, 255))

    # A ring of small cold lights between the dampers, most of them out.
    for k in range(12):
        a = math.radians(k * 30 + 15)
        on = k % 4 == 0
        led(r, cx + math.cos(a) * 37, cy + math.sin(a) * 37, VIO if on else mix(VIO, DARK, 0.55), 1.4)

    save(r, FN)


if __name__ == "__main__":
    dark_ward()
    out = pixelate(Image.open(os.path.join(SRC, FN)).convert("RGBA"))
    out.save(os.path.join(DST, FN))
    print("installed", os.path.join(DST, FN), out.size)
