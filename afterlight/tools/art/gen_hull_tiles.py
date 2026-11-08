#!/usr/bin/env python3
"""Seam variants for the hull tiles.

Every hull texture carries its own frame, so two neighbours drew two frames
and a hull read as a grid of loose plates rather than one surface. Each tile
now picks one of sixteen versions of itself from which of its four neighbours
use the same texture, with the frame lifted off the sides that touch a match.

The art is not redrawn. Each variant is the original with the frame strip on a
connected side replaced by a mirror of the pixels just inside it, so the two
tiles either side of a seam are symmetric about it and read as continuous.
Corner fastenings are deliberately KEPT -- a deck bolted at its panel joints is
the look that was chosen; `stud_reach` exists for the other option and is
currently unused.

Variant 00 is byte-identical in appearance to the source, so a tile spawned
with the plain texture and corrected a frame later does not flicker.

    python3 tools/art/gen_hull_tiles.py

Mask bits: 1 north, 2 south, 4 west, 8 east. North is -y in image space, which
is +y in the game's grid; the mapping lives in the Rust side, not here.
"""

import os

import numpy as np
from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
HULL = os.path.join(HERE, "..", "..", "assets", "sprites", "hull")

# Every hull texture the game loads. Hallway is shared across all materials.
SOURCES = [
    "hull_steel",
    "hull_titanium",
    "hull_composite",
    "hull_abyssal",
    "hull_hallway",
]

N, S, W, E = 1, 2, 4, 8


def frame_depth(a):
    """How deep the frame goes, measured off the art rather than hardcoded.

    Walks in from the edge along the middle row and takes the last pixel that
    is markedly darker than the plate face. The four materials disagree (steel
    insets a groove, hallway has a hard border), so guessing one number for all
    of them would have cut into the face of some and left a line on others.
    """
    size = a.shape[0]
    mid = size // 2
    row = a[mid, :, :3].mean(axis=1)
    face = row[mid]
    look = max(4, int(size * 0.12))
    dark = [i for i in range(look) if row[i] < face * 0.75]
    return (max(dark) + 2) if dark else 3


def variant(img, mask, depth):
    a = np.asarray(img).copy()
    if mask & N:
        a[0:depth, :, :] = a[2 * depth - 1:depth - 1:-1, :, :]
    if mask & S:
        a[-depth:, :, :] = a[-depth - 1:-2 * depth - 1:-1, :, :]
    if mask & W:
        a[:, 0:depth, :] = a[:, 2 * depth - 1:depth - 1:-1, :]
    if mask & E:
        a[:, -depth:, :] = a[:, -depth - 1:-2 * depth - 1:-1, :]
    return Image.fromarray(a)


def main():
    total = 0
    for name in SOURCES:
        path = os.path.join(HULL, name + ".png")
        img = Image.open(path).convert("RGBA")
        depth = frame_depth(np.asarray(img).astype(int))
        for mask in range(16):
            out = os.path.join(HULL, f"{name}_{mask:02d}.png")
            variant(img, mask, depth).save(out, optimize=True)
            total += os.path.getsize(out)
        print(f"  {name:16s} {img.size[0]:>3}px  frame {depth}px  x16")
    print(f"wrote {len(SOURCES) * 16} tiles, {total / 1024:.0f} KB total")


if __name__ == "__main__":
    main()
