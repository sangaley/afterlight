#!/usr/bin/env python3
"""Effect textures that are gradients, not fluids.

smoke.py and fire.py exist because smoke and fire have turbulent internal
structure worth simulating. A spark streak and a shock ring do not -- they are
an analytic falloff, and simulating them would be slower, less controllable
and no better looking.

Both are pure white with the shape in alpha, because Sprite.color multiplies
and every call site already picks a meaningful colour.

    python3 tools/art/gen_procedural_fx.py assets/sprites/effects
"""

import math
import os
import sys

from PIL import Image


def _px(v):
    return max(0, min(255, int(round(v * 255))))


def spark_streak(w=96, h=24):
    """A spark: hot head, tapering tail.

    Authored pointing along +X, because spawn_impact_sparks rotates the sprite
    to the particle's own heading so the spray reads as motion. The head sits
    at the leading edge and the tail thins behind it -- which is the whole
    reason this cannot be the radial smoke puff stretched thin, as that gives
    a symmetric smear with no sense of direction.
    """
    im = Image.new("RGBA", (w, h))
    px = im.load()
    cy = (h - 1) / 2.0
    for x in range(w):
        u = x / (w - 1.0)                     # 0 tail .. 1 head
        # Along the streak: a long ramp into a bright, short head.
        body = u ** 2.2
        head = math.exp(-((u - 0.80) ** 2) / (2 * 0.065 ** 2))
        along = min(1.0, body * 0.7 + head * 0.95)
        # Round the head off before the canvas edge. Without this the streak
        # is a bright bar chopped flat, which reads as a rectangle -- the
        # exact thing these textures exist to stop.
        t = max(0.0, min(1.0, (1.0 - u) / 0.17))
        along *= t * t * (3.0 - 2.0 * t)
        # Across it: the tail is thin, the head is round.
        half = (0.16 + 0.80 * (min(u, 0.84) / 0.84) ** 1.6) * (h / 2.0)
        for y in range(h):
            d = abs(y - cy) / max(half, 0.5)
            across = math.exp(-(d * d) * 2.1)
            px[x, y] = (255, 255, 255, _px(along * across))
    return im


def shock_ring(size=128, peak=0.74, inner=0.22, outer=0.11):
    """An expanding blast ring: an annulus, soft on both edges.

    The fireball texture cannot do this job. It is a filled ball, so stretching
    it over the shock-ring layer draws a second, fainter fireball inside the
    first instead of a ring around it -- which is why that layer was left
    untextured when the fireball got its art.

    Asymmetric falloff on purpose: a real blast front is steeper on the inside
    (the gas has already passed) than on the outside (it is still pushing).
    """
    im = Image.new("RGBA", (size, size))
    px = im.load()
    c = (size - 1) / 2.0
    for y in range(size):
        for x in range(size):
            r = math.hypot(x - c, y - c) / c
            if r <= peak:
                a = math.exp(-((peak - r) ** 2) / (2 * inner ** 2))
            else:
                a = math.exp(-((r - peak) ** 2) / (2 * outer ** 2))
            # Fade the very edge so the ring never meets the canvas, which
            # would show as a straight cut once the sprite rotates.
            a *= max(0.0, min(1.0, (1.0 - r) / 0.12))
            px[x, y] = (255, 255, 255, _px(a))
    return im


if __name__ == "__main__":
    out = sys.argv[1] if len(sys.argv) > 1 else "/tmp"
    os.makedirs(out, exist_ok=True)
    spark_streak().save(os.path.join(out, "spark_streak.png"))
    shock_ring().save(os.path.join(out, "shock_ring.png"))
    print("wrote spark_streak.png, shock_ring.png ->", out)
