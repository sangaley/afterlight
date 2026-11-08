#!/usr/bin/env python3
"""Contact sheet for effect textures -- the review gate before installing.

Three rows per candidate, because the first two rows are the ones that
flatter a texture and the third is the one that tells the truth:

  1. native size on the game's actual void background
  2. tinted the way the game will tint it (Sprite.color MULTIPLIES)
  3. AT ACTUAL GAME SIZE

Row 3 exists because the existing sheets don't have it. camera.rs defaults
`CameraState.zoom` to 1.8, so one world unit is ~0.556 screen px, and the
missile trail draws its smoke at custom_size 6..11 -- three to six pixels.
A texture can look superb at 256px and be indistinguishable from a grey dot
in play, and that is the only question worth asking here.

    python3 tools/art/effect_sheet.py [glob]   -> tools/art/preview/ALL_EFFECTS.png
"""

import glob
import os
import sys

from PIL import Image, ImageDraw

HERE = os.path.dirname(os.path.abspath(__file__))
DEFAULT_SRC = os.path.join(HERE, "preview", "effects", "puff_f*.png")
OUT = os.path.join(HERE, "preview", "ALL_EFFECTS.png")

# camera.rs:352 update_background_color bases the void on (0.01, 0.02, 0.06).
VOID = (3, 5, 15, 255)

# World units per screen pixel at the default zoom (camera.rs:37 zoom = 1.8).
UNITS_PER_PX = 1.0 / 1.8

# The sizes the game actually draws these at.
#   missiles.rs trail smoke   custom_size 6 + rand*5
#   missiles.rs cold-gas puff custom_size 5 + rand*4
#   combat/mod.rs expl. smoke radius * (0.4 + rand*0.4), radius ~50
GAME_SIZES = [
    ("trail 6u", 6.0),
    ("trail 11u", 11.0),
    ("expl 20u", 20.0),
    ("expl 40u", 40.0),
]

# Greys the spawn sites roll. missiles.rs: 0.30 + rand*0.18 at alpha 0.5.
# combat/mod.rs explosion smoke: 0.18 + rand*0.16 at alpha 0.75.
TINTS = [
    ("trail .38", (97, 93, 90), 0.50),
    ("expl .26", (66, 62, 59), 0.75),
]

NATIVE = 128
PAD = 10


def tint(im, rgb, alpha):
    """Reproduce Bevy's sprite tint: multiply texture by colour, scale alpha."""
    out = Image.new("RGBA", im.size)
    src, dst = im.load(), out.load()
    for y in range(im.height):
        for x in range(im.width):
            r, g, b, a = src[x, y]
            dst[x, y] = (r * rgb[0] // 255, g * rgb[1] // 255,
                         b * rgb[2] // 255, int(a * alpha))
    return out


def main():
    pattern = sys.argv[1] if len(sys.argv) > 1 else DEFAULT_SRC
    paths = sorted(glob.glob(pattern))
    if not paths:
        raise SystemExit("no images matched %s" % pattern)

    ims = [(os.path.basename(p).replace(".png", ""),
            Image.open(p).convert("RGBA")) for p in paths]

    # The baseline: what the game draws TODAY. An untextured Bevy Sprite is a
    # solid quad, so this is a fully opaque white square put through the same
    # tint. Without it every row above flatters the new art -- a soft puff is
    # always going to look dimmer than a hard square, and the only question
    # that matters is whether it is dimmer in a way that reads better or in a
    # way that disappears.
    ims.insert(0, ("CURRENT square", Image.new("RGBA", (256, 256), (255, 255, 255, 255))))

    col_w = NATIVE + PAD
    row1_h = NATIVE + 18
    row2_h = NATIVE + 18
    row3_h = 70

    width = PAD + col_w * len(ims) + PAD
    height = PAD + row1_h + row2_h * len(TINTS) + row3_h + 30
    sheet = Image.new("RGBA", (width, height), VOID)
    d = ImageDraw.Draw(sheet)

    y = PAD
    # ---- row 1: native, untinted
    d.text((PAD, y), "native (untinted)", fill=(150, 160, 180, 255))
    y += 14
    for i, (name, im) in enumerate(ims):
        x = PAD + i * col_w
        sheet.alpha_composite(im.resize((NATIVE, NATIVE), Image.LANCZOS), (x, y))
        d.text((x, y + NATIVE + 2), name, fill=(120, 130, 150, 255))
    y += row1_h

    # ---- row 2..n: tinted as the game tints
    for label, rgb, alpha in TINTS:
        d.text((PAD, y), "tinted %s" % label, fill=(150, 160, 180, 255))
        y += 14
        for i, (name, im) in enumerate(ims):
            x = PAD + i * col_w
            t = tint(im.resize((NATIVE, NATIVE), Image.LANCZOS), rgb, alpha)
            sheet.alpha_composite(t, (x, y))
        y += row2_h

    # ---- row 3: actual game size. The honest one.
    d.text((PAD, y), "ACTUAL GAME SIZE @ zoom 1.8 (tinted)",
           fill=(200, 190, 140, 255))
    y += 16
    for i, (name, im) in enumerate(ims):
        x = PAD + i * col_w
        cx = x
        for slabel, units in GAME_SIZES:
            px = max(1, int(round(units * UNITS_PER_PX)))
            t = tint(im.resize((px, px), Image.LANCZOS), TINTS[0][1], TINTS[0][2])
            sheet.alpha_composite(t, (cx, y + 16))
            d.text((cx, y + 40), str(px), fill=(110, 120, 140, 255))
            cx += px + 8
    y += row3_h

    sheet.convert("RGB").save(OUT)
    print("wrote", OUT, sheet.size)


if __name__ == "__main__":
    main()
