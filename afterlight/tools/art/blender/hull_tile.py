"""Hull tile, rendered in two treatments for an A/B.

Tiles are rendered as the centre of a 3x3 block so ambient occlusion and
light spill are continuous across the cell border. Rendering a lone tile
darkens its outer edge and produces a visible grid of seams once the engine
tiles it across a hull.

    Blender -b -P hull_tile.py -- restrained out.png
    Blender -b -P hull_tile.py -- full       out.png
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import (  # noqa: E402
    new_scene, materials, light_rig, ortho_camera, configure, render_to,
    box, cyl, argv_after_ddash,
)

CELL = 1.0
NEIGHBOURS = (-1, 0, 1)


def build_cell_restrained(cx, cy, mats):
    """Current silhouette, but with real bevels and contact AO."""
    # Raised armour plate, inset from the cell edge so neighbours read as
    # separate plates rather than one continuous sheet.
    box("plate", (0.94, 0.94, 0.08), (cx, cy, 0.08), mats["body"], bevel=0.014)
    # Corner rivets, matching the four in the existing sprite.
    for sx in (-1, 1):
        for sy in (-1, 1):
            cyl("rivet", 0.028, 0.03,
                (cx + sx * 0.36, cy + sy * 0.36, 0.125),
                mats["light"], bevel=0.008)


def build_cell_full(cx, cy, mats):
    """Heavier industrial read, but the plate stays ONE plate.

    An earlier pass quartered it; that made every tile read as four smaller
    tiles and fought the real grid. Detail here is added inside the plate
    instead of subdividing it.
    """
    # Outer plate with a recessed inner panel -- the step between them is
    # what gives the tile depth without breaking its silhouette.
    box("plate", (0.94, 0.94, 0.08), (cx, cy, 0.08), mats["body"], bevel=0.016)
    box("inner", (0.72, 0.72, 0.09), (cx, cy, 0.065), mats["dark"], bevel=0.010)

    # Heavy corner rivets and mid-edge fixings.
    for sx in (-1, 1):
        for sy in (-1, 1):
            cyl("rivet", 0.034, 0.036,
                (cx + sx * 0.395, cy + sy * 0.395, 0.128),
                mats["light"], bevel=0.010)
    for s in (-1, 1):
        cyl("bolt", 0.024, 0.030, (cx + s * 0.395, cy, 0.126), mats["highlight"])
        cyl("bolt", 0.024, 0.030, (cx, cy + s * 0.395, 0.126), mats["highlight"])

    # Raised cooling fins. Sinking them into the plate needed a boolean cut
    # and read as nothing once downscaled; standing proud of the surface they
    # catch the key light and hold their shape at 128px.
    for i in range(4):
        box("fin", (0.34, 0.028, 0.030),
            (cx - 0.13, cy - 0.20 + i * 0.058, 0.135), mats["light"])

    # Raised reinforcement rib across one corner: breaks the symmetry so a
    # tiled field does not read as wallpaper.
    rib = box("rib", (0.30, 0.045, 0.03), (cx + 0.20, cy + 0.20, 0.115),
              mats["light"], bevel=0.008)
    rib.rotation_euler = (0.0, 0.0, -0.7854)


def build_cell_hallway(cx, cy, mats):
    """Walkable decking.

    Hallways were the same armour plate darkened to ~45%, which read as a
    burnt or shadowed block rather than a floor -- and sat within a few
    percent of the Void tint, so a corridor was hard to tell from empty space.
    That matters more than looks: hallway is the ONLY surface crew can cross
    (crew/navigation.rs), so a player has to be able to trace a route across
    the ship at a glance.

    So this is deliberately the BRIGHTEST thing on the hull, and textured
    across its whole face rather than framed like a plate: continuous decking
    reads as somewhere you walk, a bordered plate reads as armour.
    """
    # Deck panel, nearly the full cell so runs read as continuous decking
    # with a shallow joint between panels rather than as separate tiles.
    box("deck", (0.98, 0.98, 0.07), (cx, cy, 0.075), mats["light"], bevel=0.008)

    # Raised tread. Diamonds on a 4x4 lattice, period 0.25, so the pattern
    # continues unbroken across the joint into the next cell.
    for gx in range(4):
        for gy in range(4):
            d = box("tread", (0.085, 0.085, 0.022),
                    (cx - 0.375 + gx * 0.25, cy - 0.375 + gy * 0.25, 0.118),
                    mats["highlight"])
            d.rotation_euler = (0.0, 0.0, 0.7854)

    # No edge striping. A stripe down two sides is directional, and hallways
    # run both ways -- on a horizontal corridor the same tile would draw its
    # markings straight across the walking direction. Brightness and tread
    # carry the read instead, and they work at any orientation.
    #
    # Corner studs instead: rotationally symmetric, and they pick out the
    # panel joints so a long run still has rhythm.
    for sx in (-1, 1):
        for sy in (-1, 1):
            cyl("stud", 0.030, 0.024, (cx + sx * 0.425, cy + sy * 0.425, 0.116),
                mats["gold"], bevel=0.006)


def main():
    args = argv_after_ddash()
    variant = args[0] if args else "restrained"
    out = args[1] if len(args) > 1 else "/tmp/hull_%s.png" % variant
    res = int(args[2]) if len(args) > 2 else 512

    scene = new_scene()
    mats = materials()

    # Backing slab spans past the framed area so every rendered pixel is
    # opaque -- no semi-transparent edge pixels to seam against.
    box("backing", (3.6, 3.6, 0.08), (0.0, 0.0, 0.0), mats["recess"])

    builder = {
        "full": build_cell_full,
        "hallway": build_cell_hallway,
    }.get(variant, build_cell_restrained)
    for ix in NEIGHBOURS:
        for iy in NEIGHBOURS:
            builder(ix * CELL, iy * CELL, mats)

    light_rig(scene)
    ortho_camera(scene, CELL)          # frame exactly the centre cell
    configure(scene, res)
    render_to(scene, out)


main()
