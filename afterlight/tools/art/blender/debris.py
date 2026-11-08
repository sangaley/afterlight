"""Scrap chunks thrown off a destroyed block.

vfx/debris.rs currently spawns a plain rectangle per chunk, so a shattered
module sheds tidy little bricks. ART_BRIEF asks for "4-6 small irregular
scrap shapes ... (engine spawns and tints these)" -- irregular is the whole
point, because at these sizes the SILHOUETTE is the only thing carrying the
read.

    Blender -b -P debris.py -- all tools/art/preview/debris/chunk 256

Unlike the smoke puffs these keep their luminance. Smoke is shapeless, so
its texture is pure white and the shape lives in alpha; a scrap chunk is a
solid object and its shading is what stops it reading as a flat sticker.

They must stay LIGHT, though. spawn_chunks tints by the block's own colour
already darkened to 0.55 ("charred shade of the block's own color"), and
Sprite.color multiplies -- so mid-grey art times a 0.55 tint lands on
near-black. ART_BRIEF says it outright: "design them tint-friendly
(light/neutral base)".
"""

import math
import os
import random
import sys

import bmesh
import bpy

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import (  # noqa: E402
    new_scene, metal, light_rig, ortho_camera, configure, render_to,
    argv_after_ddash,
)

# Deliberately far lighter than PALETTE's hull tones. See the module docstring:
# this is multiplied by an already-darkened tint, so it is authored bright and
# lets the engine take it down.
SCRAP = "#b9bec6"

# One chunk spans roughly one cell; the camera frames a little wider so a
# jagged corner never clips the canvas.
CAMERA_SPAN = 1.35


def chunk_mesh(seed, name):
    """An irregular flattened shard.

    Built by pushing an icosphere's vertices around and then squashing Z: a
    chunk seen from directly overhead is a plate, not a potato, and the
    engine draws these flat anyway. Randomising per-vertex rather than
    scaling the whole shape is what makes the six read as six different
    pieces instead of one piece at six sizes.
    """
    rng = random.Random(seed)

    bpy.ops.mesh.primitive_ico_sphere_add(radius=0.5, subdivisions=1)
    obj = bpy.context.object
    obj.name = name

    me = obj.data
    bm = bmesh.new()
    bm.from_mesh(me)

    for v in bm.verts:
        # Big per-vertex jitter. Small values just give a lumpy ball; the
        # facets have to actually cut into each other to read as broken metal.
        v.co.x *= rng.uniform(0.45, 1.55)
        v.co.y *= rng.uniform(0.45, 1.55)
        v.co.z *= rng.uniform(0.30, 0.70)
        v.co.x += rng.uniform(-0.16, 0.16)
        v.co.y += rng.uniform(-0.16, 0.16)

    # Flatten. Some thickness stays so the top-light still finds facets to
    # catch; a truly flat plane renders as one dead constant tone.
    for v in bm.verts:
        v.co.z *= 0.34

    bm.to_mesh(me)
    bm.free()

    # Flat shading: torn plate has hard facet breaks, not a smooth gradient.
    for poly in me.polygons:
        poly.use_smooth = False

    bev = obj.modifiers.new("bevel", type="BEVEL")
    bev.width = 0.012
    bev.segments = 2

    obj.rotation_euler = (0.0, 0.0, rng.uniform(0.0, math.tau))
    return obj


def build(which, seed):
    scene = new_scene()
    mat = metal("scrap", SCRAP, metallic=0.35, roughness=0.62)
    obj = chunk_mesh(seed, which)
    obj.data.materials.append(mat)
    return scene


BUILDERS = {"chunk_%d" % i: i for i in range(1, 7)}


def main():
    args = argv_after_ddash()
    flags = [a for a in args if a.startswith("--")]
    pos = [a for a in args if not a.startswith("--")]

    which = pos[0] if pos else "all"
    out = pos[1] if len(pos) > 1 else "/tmp/chunk"
    res = int(pos[2]) if len(pos) > 2 else 256

    span = CAMERA_SPAN
    for a in flags:
        if a.startswith("--span="):
            span = float(a.split("=", 1)[1])

    os.makedirs(os.path.dirname(os.path.abspath(out)) or ".", exist_ok=True)

    names = sorted(BUILDERS) if which == "all" else [which]
    for name in names:
        scene = build(name, BUILDERS[name])
        light_rig(scene)
        ortho_camera(scene, span)
        configure(scene, res, samples=128)
        render_to(scene, "%s_%s.png" % (out, name))

    print("DONE", len(names), "chunks ->", out)


if __name__ == "__main__":
    main()
