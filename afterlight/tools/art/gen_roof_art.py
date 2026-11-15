"""Roof art for the zoomed-out ship: one piece per kind of room.

Each kind gets a 2x2-cell piece and a 1x1 piece, drawn here so they can be
tweaked and drawn again. Two layers per piece:

  roof_art/<kind>_<n>.png        the roof, in greys -- the game tints it with
                                 the room's paint; its average is normalised
                                 to MEAN so one tint rule fits every piece
  roof_art/<kind>_<n>_decal.png  what keeps its own colour on top: lights,
                                 glowing cores, hazard stripes

Light comes from the upper left, as on the rest of the roof. Shapes are
bold: from the zoom you fly at, a cell is about eleven pixels on screen.

    python3 tools/art/gen_roof_art.py
"""
import math
import os
from PIL import Image, ImageDraw, ImageFilter, ImageStat

CELL = 48          # pixels per cell in the output
SS = 4             # supersampling
OUT = os.path.join(os.path.dirname(__file__), "..", "..", "assets", "sprites", "roof_art")
MEAN = 0.62        # the roof layer's average brightness after normalising

# Greys of the roof layer.
FACE = (196, 196, 196, 255)
RAISED = (222, 222, 222, 255)
SUNK = (120, 120, 122, 255)
DEEP = (70, 70, 74, 255)
LIT = (246, 246, 246, 255)
SHADE = (52, 52, 56, 255)


class Piece:
    def __init__(self, n):
        self.n = n
        self.size = CELL * n * SS
        self.roof = Image.new("RGBA", (self.size, self.size), (0, 0, 0, 0))
        self.decal = Image.new("RGBA", (self.size, self.size), (0, 0, 0, 0))
        self.r = ImageDraw.Draw(self.roof)
        self.d = ImageDraw.Draw(self.decal)

    def u(self, v):
        """A length in 1/100ths of the piece, to supersampled pixels."""
        return v * self.size / 100.0

    def box(self, x0, y0, x1, y1):
        return [self.u(x0), self.u(y0), self.u(x1), self.u(y1)]

    # --- Bevelled shapes: lit on the upper left, shaded on the lower right.
    def plate(self, x0, y0, x1, y1, fill=FACE, raised=True, bevel=2.2, radius=3):
        b = self.u(bevel)
        top, bot = (LIT, SHADE) if raised else (SHADE, LIT)
        r = self.u(radius)
        self.r.rounded_rectangle(self.box(x0, y0, x1, y1), r, fill=bot)
        self.r.rounded_rectangle([self.u(x0), self.u(y0), self.u(x1) - b, self.u(y1) - b], r, fill=top)
        self.r.rounded_rectangle([self.u(x0) + b, self.u(y0) + b, self.u(x1) - b, self.u(y1) - b], max(r - b, 0), fill=fill)

    def disc(self, cx, cy, rad, fill=FACE, raised=True, bevel=2.2):
        b = self.u(bevel)
        top, bot = (LIT, SHADE) if raised else (SHADE, LIT)
        x, y, r = self.u(cx), self.u(cy), self.u(rad)
        self.r.ellipse([x - r, y - r, x + r, y + r], fill=bot)
        self.r.ellipse([x - r, y - r, x + r - b, y + r - b], fill=top)
        self.r.ellipse([x - r + b, y - r + b, x + r - b, y + r - b], fill=fill)

    def rivets(self, x0, y0, x1, y1, step=12):
        for x in frange(x0, x1, step):
            for y in (y0, y1):
                self.dot(x, y)
        for y in frange(y0 + step, y1 - step / 2, step):
            for x in (x0, x1):
                self.dot(x, y)

    def dot(self, x, y, rad=0.9, fill=SHADE):
        cx, cy, r = self.u(x), self.u(y), self.u(rad)
        self.r.ellipse([cx - r, cy - r, cx + r, cy + r], fill=fill)
        self.r.ellipse([cx - r, cy - r, cx + r * 0.2, cy + r * 0.2], fill=LIT)

    def line(self, pts, w, fill):
        self.r.line([(self.u(x), self.u(y)) for x, y in pts], fill=fill, width=int(self.u(w)))

    def glow(self, shape, box, colour, blur=2.5, core=None):
        """A light on the decal layer, with a soft halo."""
        halo = Image.new("RGBA", self.decal.size, (0, 0, 0, 0))
        hd = ImageDraw.Draw(halo)
        getattr(hd, shape)(box, fill=colour[:3] + (150,))
        halo = halo.filter(ImageFilter.GaussianBlur(self.u(blur)))
        self.decal.alpha_composite(halo)
        getattr(self.d, shape)(box, fill=core or colour)

    def stripes(self, x0, y0, x1, y1, width=6):
        """Hazard stripes on the decal layer."""
        tile = Image.new("RGBA", self.decal.size, (0, 0, 0, 0))
        td = ImageDraw.Draw(tile)
        td.rectangle(self.box(x0, y0, x1, y1), fill=(30, 28, 26, 235))
        w = self.u(width)
        for k in range(-40, 40):
            o = self.u(x0) + k * w * 2
            td.polygon([(o, self.u(y1)), (o + w, self.u(y1)), (o + w + (self.u(y1) - self.u(y0)), self.u(y0)), (o + (self.u(y1) - self.u(y0)), self.u(y0))], fill=(232, 180, 40, 235))
        mask = Image.new("L", self.decal.size, 0)
        ImageDraw.Draw(mask).rectangle(self.box(x0, y0, x1, y1), fill=255)
        self.decal.paste(tile, (0, 0), mask)

    def save(self, kind):
        size = CELL * self.n
        roof = self.roof.resize((size, size), Image.LANCZOS)
        decal = self.decal.resize((size, size), Image.LANCZOS)
        # Normalise the roof's average brightness so one tint rule fits all.
        alpha = roof.getchannel("A")
        stat = ImageStat.Stat(roof.convert("L"), mask=alpha)
        k = MEAN * 255 / max(stat.mean[0], 1)
        rgb = roof.convert("RGB").point(lambda v: min(255, int(v * k)))
        roof = Image.merge("RGBA", (*rgb.split(), alpha))
        roof.save(os.path.join(OUT, f"{kind}_{self.n}.png"))
        decal.save(os.path.join(OUT, f"{kind}_{self.n}_decal.png"))


def frange(a, b, step):
    x = a
    while x <= b + 1e-6:
        yield x
        x += step


def housing(p):
    """The roof plate every piece stands on."""
    p.plate(1, 1, 99, 99, FACE, bevel=2.0, radius=2)


# --- The pieces ---------------------------------------------------------

def reactor(p):
    housing(p)
    # Coolant pipes from the corners into the ring.
    for cx, cy in ((12, 12), (88, 12), (12, 88), (88, 88)):
        p.line([(cx, cy), (50 + (cx - 50) * 0.5, 50 + (cy - 50) * 0.5)], 7, SUNK)
        p.disc(cx, cy, 6, RAISED)
    p.disc(50, 50, 38, SUNK, raised=False)
    p.disc(50, 50, 33, RAISED)
    # Spokes.
    for a in range(0, 360, 45):
        r = math.radians(a + 22.5)
        p.line([(50 + math.cos(r) * 15, 50 + math.sin(r) * 15), (50 + math.cos(r) * 31, 50 + math.sin(r) * 31)], 3, SUNK)
    p.disc(50, 50, 15, DEEP, raised=False)
    p.glow("ellipse", p.box(38, 38, 62, 62), (255, 170, 60, 255), blur=4, core=(255, 214, 120, 255))
    for a in range(0, 360, 90):
        r = math.radians(a)
        x, y = 50 + math.cos(r) * 26, 50 + math.sin(r) * 26
        p.glow("ellipse", p.box(x - 2.5, y - 2.5, x + 2.5, y + 2.5), (255, 150, 50, 255), blur=1.5)


def battery(p):
    housing(p)
    rows, cols = (2, 3) if p.n == 2 else (1, 2)
    w, h = 84 / cols, 84 / rows
    for i in range(cols):
        for j in range(rows):
            x0, y0 = 8 + i * w, 8 + j * h
            p.plate(x0, y0, x0 + w - 4, y0 + h - 4, RAISED, radius=4)
            p.plate(x0 + w * 0.32, y0 + 2, x0 + w * 0.62, y0 + 9, SUNK, raised=False, bevel=1.2, radius=1)
            # Charge bar.
            p.glow("rectangle", p.box(x0 + 5, y0 + h - 14, x0 + w - 9, y0 + h - 10), (80, 230, 210, 255), blur=1.5)


def cargo(p):
    housing(p)
    p.plate(6, 6, 94, 94, SUNK, raised=False)
    p.stripes(8, 8, 92, 16, width=5)
    p.stripes(8, 84, 92, 92, width=5)
    p.plate(12, 20, 88, 80, RAISED, radius=2)
    # Hatch seam and cross braces.
    p.line([(50, 22), (50, 78)], 2.2, SHADE)
    p.line([(16, 26), (84, 74)], 3.5, SUNK)
    p.line([(16, 74), (84, 26)], 3.5, SUNK)
    p.rivets(16, 24, 84, 76, step=17)


def fuel(p):
    housing(p)
    tanks = 2 if p.n == 2 else 1
    h = 84 / tanks
    for j in range(tanks):
        y0 = 8 + j * h
        p.plate(8, y0, 92, y0 + h - 6, RAISED, radius=(h - 6) / 2, bevel=3)
        for x in (28, 50, 72):
            p.line([(x, y0 + 2), (x, y0 + h - 8)], 2.2, SUNK)
        p.glow("ellipse", p.box(84, y0 + h / 2 - 7, 90, y0 + h / 2 - 1), (230, 70, 50, 255), blur=1.2)


def scrubber(p):
    housing(p)
    fans = [(50, 50, 40)] if p.n == 1 else [(27, 27, 21), (73, 27, 21), (27, 73, 21), (73, 73, 21)]
    for cx, cy, r in fans:
        p.disc(cx, cy, r, SUNK, raised=False)
        for a in range(0, 360, 60):
            t = math.radians(a)
            p.line([(cx, cy), (cx + math.cos(t) * r * 0.85, cy + math.sin(t) * r * 0.85)], r * 0.22, RAISED)
        p.disc(cx, cy, r * 0.25, RAISED)
    p.glow("ellipse", p.box(90, 4, 96, 10), (90, 230, 120, 255), blur=1.2)


def quarters(p):
    housing(p)
    p.plate(8, 8, 92, 92, RAISED, radius=4)
    rows = 2 if p.n == 2 else 1
    for j in range(rows):
        y = 24 + j * 44 if rows == 2 else 40
        for i in range(4):
            x = 14 + i * 19
            p.plate(x, y, x + 14, y + 16, DEEP, raised=False, bevel=1.4, radius=2)
            p.glow("rectangle", p.box(x + 2, y + 2, x + 12, y + 14), (255, 205, 120, 255), blur=1.8, core=(255, 222, 160, 230))
    p.rivets(12, 12, 88, 88, step=19)


def galley(p):
    housing(p)
    p.plate(8, 8, 92, 92, RAISED, radius=4)
    # A skylight over the mess hall, in panes.
    p.plate(18, 18, 82, 62, DEEP, raised=False, radius=3)
    for i in range(4):
        for j in range(2):
            x, y = 21 + i * 15.5, 21 + j * 20.5
            p.glow("rectangle", p.box(x, y, x + 12.5, y + 17.5), (255, 196, 110, 255), blur=1.6, core=(255, 214, 150, 220))
    # Extractor vents.
    for x in (22, 42, 62):
        p.plate(x, 70, x + 16, 84, SUNK, raised=False, bevel=1.2, radius=1)
        for k in range(3):
            p.line([(x + 2, 73 + k * 4), (x + 14, 73 + k * 4)], 1.4, SHADE)


def surgical(p):
    housing(p)
    p.plate(8, 8, 92, 92, RAISED, radius=4)
    p.plate(30, 30, 70, 70, SUNK, raised=False, radius=20)
    p.glow("rectangle", p.box(45, 35, 55, 65), (235, 80, 70, 255), blur=2.0, core=(255, 240, 236, 255))
    p.glow("rectangle", p.box(35, 45, 65, 55), (235, 80, 70, 255), blur=2.0, core=(255, 240, 236, 255))
    p.rivets(12, 12, 88, 88, step=19)


def repair(p):
    housing(p)
    p.plate(6, 6, 94, 94, SUNK, raised=False)
    p.stripes(6, 6, 94, 13, width=4)
    p.stripes(6, 87, 94, 94, width=4)
    # The crane: two rails and a gantry with its hook.
    p.plate(10, 18, 18, 82, RAISED, radius=1)
    p.plate(82, 18, 90, 82, RAISED, radius=1)
    p.plate(10, 44, 90, 56, RAISED, radius=1)
    p.disc(50, 50, 9, FACE)
    p.glow("ellipse", p.box(47, 47, 53, 53), (255, 180, 60, 255), blur=1.4)


def cooling(p):
    housing(p)
    p.plate(8, 8, 92, 92, SUNK, raised=False)
    # Radiator fins.
    for k in range(9):
        x = 13 + k * 9
        p.plate(x, 12, x + 5, 88, RAISED, bevel=1.2, radius=1)
    p.line([(8, 50), (92, 50)], 5, DEEP)
    p.glow("rectangle", p.box(10, 48.5, 90, 51.5), (90, 190, 255, 255), blur=1.8)


def vent(p):
    housing(p)
    p.plate(8, 8, 92, 92, DEEP, raised=False)
    # Louvres with heat showing between them.
    for k in range(7):
        y = 12 + k * 11.5
        p.glow("rectangle", p.box(12, y + 7, 88, y + 10), (255, 110, 40, 255), blur=1.6)
        p.plate(10, y, 90, y + 7, RAISED, bevel=1.4, radius=1)


def memory(p):
    housing(p)
    p.plate(8, 8, 92, 92, SUNK, raised=False)
    n = 4 if p.n == 2 else 2
    w = 80 / n
    for i in range(n):
        for j in range(n):
            x, y = 10 + i * w, 10 + j * w
            p.plate(x, y, x + w - 3, y + w - 3, RAISED, bevel=1.4, radius=1.5)
            p.glow("rectangle", p.box(x + 3, y + w - 9, x + w * 0.45, y + w - 6), (90, 230, 255, 255), blur=1.2)


PIECES = {
    "reactor": reactor,
    "battery": battery,
    "cargo": cargo,
    "fuel": fuel,
    "scrubber": scrubber,
    "quarters": quarters,
    "galley": galley,
    "surgical": surgical,
    "repair": repair,
    "cooling": cooling,
    "vent": vent,
    "memory": memory,
}


def main():
    os.makedirs(OUT, exist_ok=True)
    for kind, draw in PIECES.items():
        for n in (1, 2):
            p = Piece(n)
            draw(p)
            p.save(kind)
    print(f"{len(PIECES) * 2} pieces in {os.path.normpath(OUT)}")


if __name__ == "__main__":
    main()
