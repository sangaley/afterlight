#!/usr/bin/env python3
"""Pixelate the generated smooth sprites and install them into the game assets.

Run AFTER `python3 darkset.py` (which writes smooth sprites into ./smooth_mach/).
This downscales each to the logical pixel grid, quantises the palette, hard-steps
the alpha, upscales NEAREST (so file dimensions are unchanged), and copies the
result into ../../assets/sprites/modules/.
"""
import os
from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
SRC  = os.path.join(HERE, "smooth_mach")
DST  = os.path.abspath(os.path.join(HERE, "..", "..", "assets", "sprites", "modules"))

def step_alpha(a):
    # 6 hard levels: 0,51,102,153,204,255
    return a.point(lambda v: min(255, int(round(v / 51)) * 51))

def pixelate(img, factor=3, colors=32):
    w, h = img.size
    down = img.resize((w // factor, h // factor), Image.BOX)
    rgb  = down.convert("RGB").quantize(colors=colors, dither=Image.Dither.NONE).convert("RGB")
    a    = step_alpha(down.getchannel("A"))
    out  = rgb.convert("RGBA"); out.putalpha(a)
    return out.resize((w, h), Image.NEAREST)

if __name__ == "__main__":
    if not os.path.isdir(SRC):
        raise SystemExit(f"no smooth_mach/ dir at {SRC} — run `python3 darkset.py` first")
    n = 0
    for f in sorted(os.listdir(SRC)):
        if f.endswith(".png"):
            pixelate(Image.open(os.path.join(SRC, f)).convert("RGBA")).save(os.path.join(DST, f))
            n += 1
    print(f"pixelated + installed {n} module sprites -> {DST}")
