#!/usr/bin/env python3
"""Wedge art for the editor.

AngledArmorPlate and AngledHullPlate draw no sprite in the game: the module's
own image is transparent and the triangle is assembled from child quads at
spawn. So there is no file to show in a palette, and using the beam texture
the sprite map points at would draw a square where a wedge belongs.

Geometry copied from ship::spawner: the material occupies the half where
x + y <= 0 with the cell spanning -30..30, which is the lower-left triangle,
and the lit edge runs along the hypotenuse. Facing north here; the editor
turns it with the block.
"""
from PIL import Image, ImageDraw

# Both plates take the hull's own tone -- sprite_map::HULL_TONE, the median of
# the opaque pixels of sprites/hull/hull_steel.png. Keep these in step with the
# Rust constant or the yard will draw a plate the game does not.
WEDGE = {
    "AngledArmorPlate": (0.231, 0.271, 0.318),
    "AngledHullPlate":  (0.231, 0.271, 0.318),
}
S = 256   # supersampled, then reduced


def mix(c, t, k):
    return tuple(int(255 * (c[i] * (1 - k) + t[i] * k)) for i in range(3))


def draw(colour):
    face = colour
    # ship::spawner draws the body at the hull tone unchanged and lights only
    # the hypotenuse. Same numbers here.
    body = face
    body = tuple(int(255 * c) for c in body)
    lit = mix(face, (1, 1, 1), 0.30)
    im = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    # x + y <= 0 in a cell spanning -30..30, with image y running downward:
    # the lower-left triangle.
    d.polygon([(0, 0), (0, S - 1), (S - 1, S - 1)], fill=body + (255,))
    d.line([(0, 0), (S - 1, S - 1)], fill=lit + (255,), width=max(2, S // 22))
    return im


if __name__ == "__main__":
    import sys
    out = sys.argv[1] if len(sys.argv) > 1 else "."
    for name, colour in WEDGE.items():
        draw(colour).resize((64, 64), Image.LANCZOS).save(f"{out}/{name}.png", optimize=True)
        print("  wrote", name + ".png")
