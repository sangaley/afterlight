"""Crew sprite sheets - pure overhead, animated.

Crew are seen from straight down, like everything else in this game. That has
two consequences the art has to respect:

  * You need ONE sprite set, not four or eight. From directly above a person
    turning genuinely rotates, so the engine rotates the sprite to face travel
    and the frames only ever carry animation. (See crew::walking.)

  * The classic walk-cycle vertical bob is INVISIBLE. Looking straight down,
    up-and-down motion does nothing. So the walk reads through arm swing,
    shoulder counter-rotation and a lateral sway instead - the arms have to
    break the body's outline or the whole thing is a sliding dot.

Six frames for the walk, which is the small-sprite sweet spot: enough to read,
cheap to make. Sheets are single-row horizontal strips, matching the creature
convention (stalker.png is 384x64 = 6 frames of 64).

    Blender -b -P crew.py -- walk out.png     # writes a 6x64 strip
    Blender -b -P crew.py -- idle out.png
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import (  # noqa: E402
    new_scene, metal, light_rig, ortho_camera, configure, render_to,
    box, cyl, argv_after_ddash,
)

FRAME = 64

# Suit tones. DARK on purpose: hallway decking is the brightest surface on the
# hull, so a pale crew member vanishes into the floor they spend all their time
# on. At ~24 world units a crew member is only a couple of dozen screen pixels,
# and the thing that reads at that size is a dark mass carrying ONE bright
# accent - the visor - not the sum of its details.
SUIT = "#33343a"
SUIT_LIT = "#43454d"
HELMET = "#8f9aa6"
VISOR = "#7fd4e8"
VISOR_DEAD = "#2c3238"   # unlit: life support is off
BOOT = "#232428"
TRIM = "#8a6a33"

# Frames per state. `dead` is a single static pose.
STATES = {"walk": 6, "idle": 3, "work": 3, "dead": 1}


def suit_materials():
    return {
        "suit": metal("suit", SUIT, metallic=0.10, roughness=0.72),
        "suit_lit": metal("suit_lit", SUIT_LIT, metallic=0.10, roughness=0.66),
        "helmet": metal("helmet", HELMET, metallic=0.35, roughness=0.30),
        "visor": metal("visor", VISOR, metallic=0.55, roughness=0.20),
        "visor_dead": metal("visor_dead", VISOR_DEAD, metallic=0.55, roughness=0.30),
        "boot": metal("boot", BOOT, metallic=0.10, roughness=0.80),
        "trim": metal("trim", TRIM, metallic=0.40, roughness=0.45),
    }


def figure(m, arm_swing, sway, twist, lean=0.0, splay=0.0, dead=False):
    """One crew member, posed.

    `arm_swing` is fore/aft arm offset (the main read), `sway` lateral body
    shift, `twist` shoulder counter-rotation in radians, `lean` a forward
    push for the working pose, `splay` pushes the limbs out for the dead pose.
    Forward is +Y, matching every other sprite in the game.
    """
    # Torso, seen from above as a rounded slab across the shoulders. Wider
    # than deep, which is what makes a human shape read from this angle.
    torso = box("torso", (0.30, 0.21, 0.10), (sway, lean, 0.05),
                m["suit"], bevel=0.045, segments=3)
    torso.rotation_euler = (0.0, 0.0, twist)

    # Backpack/life support, offset aft. Breaks the silhouette's symmetry so
    # the figure has an obvious front and back at a glance.
    pack = box("pack", (0.19, 0.09, 0.08), (sway, lean - 0.115, 0.09),
               m["suit_lit"], bevel=0.025)
    pack.rotation_euler = (0.0, 0.0, twist)

    # Arms. These are the animation: they swing fore and aft in opposite
    # phase and stick out past the torso, so the outline changes every frame.
    for side, phase in ((-1, 1.0), (1, -1.0)):
        ax = sway + side * (0.165 + splay)
        ay = lean + phase * arm_swing
        arm = box("arm", (0.075, 0.155, 0.075), (ax, ay, 0.055),
                  m["suit"], bevel=0.030)
        arm.rotation_euler = (0.0, 0.0, twist + phase * 0.22)
        # Glove at the leading end of each arm - a small tone break that makes
        # the swing legible even when the sprite is only ~24 units across.
        box("glove", (0.070, 0.055, 0.070), (ax, ay + phase * 0.075, 0.058),
            m["boot"], bevel=0.028)

    # Boots peek out fore and aft of the torso on alternate strides. From
    # overhead this is most of what says "walking" rather than "sliding".
    for side, phase in ((-1, -1.0), (1, 1.0)):
        bx = sway + side * 0.075
        by = lean + phase * arm_swing * 0.75
        box("boot", (0.085, 0.105, 0.05), (bx, by, 0.03), m["boot"], bevel=0.025)

    # Helmet dome, the brightest thing on the figure.
    cyl("helmet", 0.115, 0.12, (sway, lean + 0.015, 0.135), m["helmet"],
        bevel=0.045, verts=28)
    # Visor, forward of centre: the one cue for which way they are facing.
    # The lit visor is the "this one is alive" signal, and at ~24 world units
    # it is the only detail that reliably reads. Killing the light is a
    # stronger death cue than any change of pose.
    visor = box("visor", (0.155, 0.080, 0.05), (sway, lean + 0.072, 0.168),
                m["visor_dead"] if dead else m["visor"], bevel=0.028)
    visor.rotation_euler = (0.0, 0.0, twist)
    # Shoulder trim, so crew carry a touch of the palette's warm accent.
    for side in (-1, 1):
        box("trim", (0.045, 0.10, 0.03), (sway + side * 0.135, lean, 0.104),
            m["trim"])


def pose_for(state, i, n, m):
    """Build the figure for frame `i` of `n` in `state`."""
    if state == "walk":
        # Full cycle over n frames: two strides. Sway runs at double rate so
        # the body shifts onto each foot as it lands.
        t = (i + 0.5) / n * math.tau
        figure(m, arm_swing=0.095 * math.sin(t), sway=0.014 * math.sin(2.0 * t),
               twist=0.16 * math.sin(t))
    elif state == "idle":
        # Breathing. Tiny, but a completely static crew member reads as a
        # placed prop rather than a person. The engine holds the extreme
        # frames longer than the middle one (see CrewAnimation timing).
        t = i / n * math.tau
        figure(m, arm_swing=0.012 * math.sin(t), sway=0.0,
               twist=0.04 * math.sin(t))
    elif state == "work":
        # Leaning into a console, one arm reaching and returning.
        t = i / n * math.tau
        figure(m, arm_swing=0.055 + 0.035 * math.sin(t), sway=0.0,
               twist=0.10, lean=0.045)
    else:  # dead
        # Collapsed: dark visor, hard twist off-axis and limbs splayed
        # unevenly. Everything about it should look wrong next to the tidy
        # upright poses either side of it.
        figure(m, arm_swing=-0.055, sway=0.02, twist=0.62, splay=0.075, dead=True)


def main():
    args = argv_after_ddash()
    state = args[0] if args else "walk"
    out = args[1] if len(args) > 1 else "/tmp/crew_%s.png" % state
    n = STATES[state]

    # Render each frame to its own file; the caller packs them into a strip.
    base, ext = os.path.splitext(out)
    for i in range(n):
        scene = new_scene()
        m = suit_materials()
        pose_for(state, i, n, m)
        light_rig(scene)
        configure(scene, FRAME * 4, samples=180)   # 4x supersample, downscaled later
        # Tight crop: the figure spans ~0.62 units, so framing at 1.0 wasted
        # nearly half the sprite on empty space.
        ortho_camera(scene, 0.72)
        render_to(scene, "%s_%02d%s" % (base, i, ext))
    print("FRAMES %d" % n)


main()
