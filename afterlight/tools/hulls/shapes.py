"""Curved claw hulls, generated rather than drawn.

An arm is a band swept along a curve: at each x it occupies a run of y centred
on the curve, `thick` cells deep. Drawing these as ASCII by hand gave arms that
pinched to two cells in the middle of a bend, and a two-cell arm has no
interior -- no decking, so no crew, so no guns that anyone can fire.
"""
import math


def ellipse(cx, cy, rx, ry):
    """Solid body blob."""
    out = set()
    for x in range(int(cx - rx), int(cx + rx) + 1):
        for y in range(int(cy - ry), int(cy + ry) + 1):
            if ((x - cx) / rx) ** 2 + ((y - cy) / ry) ** 2 <= 1.0:
                out.add((x, y))
    return out


def sweep(x_from, x_to, curve, thick):
    """A band `thick` cells deep, following `curve(x)` as its centreline."""
    out = set()
    for x in range(x_from, x_to + 1):
        c = curve(x)
        lo = int(round(c - (thick - 1) / 2.0))
        for k in range(thick):
            out.add((x, lo + k))
    return out


def claw(x0, x1, reach, rise, hook, thick, sign):
    """One arm: out from the body, up to `rise`, then hooking back by `hook`.

    `sign` is +1 for the upper arm, -1 for the lower. The centreline is a
    single smooth arc so the deck inside it never kinks.
    """
    span = max(x1 - x0, 1)

    def curve(x):
        t = (x - x0) / span                      # 0 at the shoulder, 1 at the tip
        # rises fast, then eases back in toward the centreline at the tip
        y = rise * math.sin(math.pi * min(t, 1.0) * 0.72) - hook * (t ** 3)
        return sign * (reach + y)

    return sweep(x0, x1, curve, thick)


SHAPES = {}

# --- Pincer: long slender arms, deep notch, tips hooking inward -----------
SHAPES["pincer"] = (
    ellipse(-5.0, 0.0, 4.2, 4.2)
    | claw(-3, 14, 4.6, 3.0, 2.0, 4, +1)
    | claw(-3, 14, 4.6, 3.0, 2.0, 4, -1)
)

# --- Crab: broad body, short heavy claws ---------------------------------
SHAPES["crab"] = (
    ellipse(-4.5, 0.0, 5.0, 5.0)
    | claw(-2, 11, 5.0, 2.4, 1.6, 5, +1)
    | claw(-2, 11, 5.0, 2.4, 1.6, 5, -1)
)

# --- Trident: two claws and a spine that outreaches both ------------------
SHAPES["trident"] = (
    ellipse(-5.0, 0.0, 4.2, 4.4)
    | claw(-3, 14, 4.6, 2.8, 1.8, 4, +1)
    | claw(-3, 14, 4.6, 2.8, 1.8, 4, -1)
    | sweep(-4, 15, lambda x: 0.0, 4)
)

def normalise(cells):
    xs = [c[0] for c in cells]
    ys = [c[1] for c in cells]
    ox = -9 - min(xs)
    oy = -((min(ys) + max(ys)) // 2)
    return {(x + ox, y + oy) for (x, y) in cells}


def body_of(cells):
    """The hull forward of which the ship splits into arms.

    Found rather than hand-set: walk x from the stern and stop at the first
    column that holds more than one run of cells, because that is where the
    notch opens. Everything aft of it is one solid mass -- the right and only
    place for reactors, fuel, air and berths.
    """
    xs = sorted({c[0] for c in cells})
    split = max(xs) + 1
    for x in xs:
        col = sorted(c[1] for c in cells if c[0] == x)
        runs = 1 + sum(1 for a, b in zip(col, col[1:]) if b - a > 1)
        if runs > 1:
            split = x
            break
    return {c for c in cells if c[0] < split}, split
