"""Room-sized pieces for the Cosmoteer look (vfx::ship_roof, AFTERLIGHT_COSMO).

A room of one kind of part -- a cluster of reactors, a block of quarters, a
battery rack -- is drawn as one machine the size of the room, not tiled with
cell-sized squares. Each piece has a shape of its own on a clear background,
throws a shadow on the deck round it, and carries its system's colour: the
reactor's orange core, the quarters' warm windows, the battery's yellow cells.

Drawn landscape, L cells long by S cells deep, for every 1 <= S <= L <= 6:

    sprites/part_art/<kind>_<L>x<S>.png

Light from the upper left, as on the rest of the roof.

    python3 tools/art/gen_part_pieces.py
"""
import math
import os
from PIL import Image, ImageDraw, ImageFilter

CELL = 48
SS = 3
OUT = os.path.join(os.path.dirname(__file__), "..", "..", "assets", "sprites", "part_art")

EDGE = (14, 15, 19, 255)
SHADOW = (0, 0, 0, 150)


class Piece:
    def __init__(self, L, S):
        self.L, self.S = L, S
        self.w, self.h = L * CELL * SS, S * CELL * SS
        self.im = Image.new("RGBA", (self.w, self.h), (0, 0, 0, 0))
        self.shadow = Image.new("RGBA", (self.w, self.h), (0, 0, 0, 0))
        self.glow = Image.new("RGBA", (self.w, self.h), (0, 0, 0, 0))
        self.d = ImageDraw.Draw(self.im)
        self.sd = ImageDraw.Draw(self.shadow)
        self.gd = ImageDraw.Draw(self.glow)

    def c(self, v):
        """Cells to supersampled pixels."""
        return v * CELL * SS

    def box(self, x0, y0, x1, y1):
        return [self.c(x0), self.c(y0), self.c(x1), self.c(y1)]

    def block(self, x0, y0, x1, y1, fill, radius=0.12, bevel=0.06, shadow=True, light=(255, 255, 255, 120), dark=(0, 0, 0, 120)):
        """A raised, bevelled shape with its outline and shadow."""
        r = self.c(radius)
        if shadow:
            off = self.c(0.09)
            self.sd.rounded_rectangle([self.c(x0) + off, self.c(y0) + off, self.c(x1) + off, self.c(y1) + off], r, fill=SHADOW)
        self.d.rounded_rectangle(self.box(x0, y0, x1, y1), r, fill=EDGE)
        e = self.c(0.025)
        b = self.c(bevel)
        self.d.rounded_rectangle([self.c(x0) + e, self.c(y0) + e, self.c(x1) - e, self.c(y1) - e], max(r - e, 0), fill=dark)
        self.d.rounded_rectangle([self.c(x0) + e, self.c(y0) + e, self.c(x1) - e - b, self.c(y1) - e - b], max(r - e, 0), fill=light)
        self.d.rounded_rectangle([self.c(x0) + e + b, self.c(y0) + e + b, self.c(x1) - e - b, self.c(y1) - e - b], max(r - e - b, 0), fill=fill)

    def disc(self, cx, cy, rad, fill, bevel=0.05, raised=True, outline=True):
        x, y, r, b = self.c(cx), self.c(cy), self.c(rad), self.c(bevel)
        hi, lo = ((255, 255, 255, 110), (0, 0, 0, 130)) if raised else ((0, 0, 0, 130), (255, 255, 255, 90))
        if outline:
            o = self.c(0.025)
            self.d.ellipse([x - r - o, y - r - o, x + r + o, y + r + o], fill=EDGE)
        self.d.ellipse([x - r, y - r, x + r, y + r], fill=lo)
        self.d.ellipse([x - r, y - r, x + r - b, y + r - b], fill=hi)
        self.d.ellipse([x - r + b, y - r + b, x + r - b, y + r - b], fill=fill)

    def light(self, shape, box, colour, halo=0.12, core=None):
        """A light: a soft halo on the glow layer and its bright shape."""
        getattr(self.gd, shape)(box, fill=colour[:3] + (230,))
        getattr(self.d, shape)(box, fill=core or colour)
        self._halo = getattr(self, "_halo", 0) or halo
        self._halo = max(self._halo, halo)

    def pipe(self, pts, width, fill=(150, 98, 54, 255)):
        p = [(self.c(x), self.c(y)) for x, y in pts]
        self.sd.line([(x + self.c(0.07), y + self.c(0.07)) for x, y in p], fill=SHADOW, width=int(self.c(width)))
        self.d.line(p, fill=EDGE, width=int(self.c(width)) + int(self.c(0.04)))
        self.d.line(p, fill=fill, width=int(self.c(width)))
        hl = [(x - self.c(width) * 0.18, y - self.c(width) * 0.18) for x, y in p]
        self.d.line(hl, fill=(225, 170, 110, 160), width=max(1, int(self.c(width) * 0.25)))

    def bolt(self, x, y, r=0.035):
        self.disc(x, y, r, (120, 124, 132, 255), bevel=0.015, outline=True)

    def save(self, kind):
        shadow = self.shadow.filter(ImageFilter.GaussianBlur(self.c(0.05)))
        halo = self.glow.filter(ImageFilter.GaussianBlur(self.c(getattr(self, "_halo", 0.12))))
        out = Image.new("RGBA", (self.w, self.h), (0, 0, 0, 0))
        out.alpha_composite(shadow)
        out.alpha_composite(self.im)
        out.alpha_composite(halo)
        # The bright shapes once more over their own halo.
        out = out.resize((self.L * CELL, self.S * CELL), Image.LANCZOS)
        out.save(os.path.join(OUT, f"{kind}_{self.L}x{self.S}.png"))


# --- Reactor: housings round glowing cores, coolant pipes between ---------

HOUSING = (112, 118, 130, 255)
HOUSING_L = (150, 156, 168, 255)
DARKMETAL = (52, 56, 66, 255)


def reactor(p):
    L, S = p.L, p.S
    m = 0.1
    n = max(1, round(L / S))  # cores along the length
    span = L / n
    # The base frame the cores stand on, with pipe runs.
    p.block(m, m, L - m, S - m, DARKMETAL, radius=0.2)
    if n > 1:
        p.pipe([(span * 0.5, S * 0.5), (L - span * 0.5, S * 0.5)], 0.22 * min(S, 2))
    for k in range(n):
        cx, cy = span * (k + 0.5), S * 0.5
        R = min(span, S) * 0.42
        # Corner feeds into the core.
        for sx in (-1, 1):
            for sy in (-1, 1):
                fx, fy = cx + sx * R * 0.95, cy + sy * R * 0.95
                p.pipe([(fx, fy), (cx + sx * R * 0.55, cy + sy * R * 0.55)], R * 0.16)
                p.disc(fx, fy, R * 0.14, HOUSING_L, bevel=R * 0.04)
        p.disc(cx, cy, R, HOUSING, bevel=R * 0.08)
        p.disc(cx, cy, R * 0.78, DARKMETAL, raised=False, bevel=R * 0.06)
        # Containment ring with its clamps.
        for a in range(8):
            t = math.radians(a * 45 + 22.5)
            p.bolt(cx + math.cos(t) * R * 0.88, cy + math.sin(t) * R * 0.88, R * 0.06)
        for a in range(6):
            t = math.radians(a * 60)
            x0, y0 = cx + math.cos(t) * R * 0.38, cy + math.sin(t) * R * 0.38
            x1, y1 = cx + math.cos(t) * R * 0.74, cy + math.sin(t) * R * 0.74
            p.pipe([(x0, y0), (x1, y1)], R * 0.1, fill=(96, 100, 110, 255))
        p.light("ellipse", p.box(cx - R * 0.42, cy - R * 0.42, cx + R * 0.42, cy + R * 0.42), (255, 140, 40, 255), halo=R * 0.35, core=(255, 170, 70, 255))
        p.light("ellipse", p.box(cx - R * 0.2, cy - R * 0.2, cx + R * 0.2, cy + R * 0.2), (255, 230, 170, 255), halo=R * 0.2, core=(255, 248, 220, 255))


# --- Battery: a rack of cells, each with its yellow charge strip -----------

def battery(p):
    L, S = p.L, p.S
    m = 0.08
    p.block(m, m, L - m, S - m, DARKMETAL, radius=0.1)
    cols, rows = L * 2, max(1, S)
    w, h = (L - 2 * m - 0.12) / cols, (S - 2 * m - 0.12) / rows
    for i in range(cols):
        for j in range(rows):
            x0 = m + 0.06 + i * w
            y0 = m + 0.06 + j * h
            p.block(x0 + 0.03, y0 + 0.04, x0 + w - 0.03, y0 + h - 0.04, (138, 140, 132, 255), radius=0.05, bevel=0.03, shadow=False)
            # Terminal cap, and the charge strip down the cell.
            p.disc(x0 + w * 0.5, y0 + 0.14, min(w, h) * 0.13, (176, 120, 64, 255), bevel=0.012)
            p.light("rectangle", p.box(x0 + w * 0.3, y0 + 0.28, x0 + w * 0.7, y0 + h - 0.14), (255, 212, 40, 255), halo=0.06, core=(255, 226, 90, 255))
    # The bus bar along the top.
    p.pipe([(m + 0.15, m + 0.07), (L - m - 0.15, m + 0.07)], 0.07, fill=(176, 120, 64, 255))


# --- Quarters: a hab block, a grid of warm lit windows ---------------------

def quarters(p):
    L, S = p.L, p.S
    m = 0.09
    p.block(m, m, L - m, S - m, (128, 132, 142, 255), radius=0.22)
    # A darker spine along it where it's deep enough for a hall.
    if S >= 2:
        p.block(m + 0.16, S / 2 - 0.1, L - m - 0.16, S / 2 + 0.1, (70, 74, 84, 255), radius=0.05, bevel=0.02, shadow=False,
                light=(0, 0, 0, 90), dark=(255, 255, 255, 60))
    for i in range(L):
        for j in range(S):
            # A cabin a cell: its window on the outer side, warm light inside.
            if S >= 2:
                cy = j + (0.36 if j < S / 2 else 0.64)
            else:
                cy = 0.5
            x0, x1 = i + 0.16, i + 0.84
            p.block(x0, cy - 0.17, x1, cy + 0.17, EDGE[:3] + (255,), radius=0.05, bevel=0.0, shadow=False)
            p.light("rectangle", p.box(x0 + 0.05, cy - 0.12, x1 - 0.05, cy + 0.12), (255, 160, 70, 255), halo=0.1, core=(255, 200, 120, 255))
            # The mullion splitting it in two.
            p.d.rectangle(p.box(i + 0.48, cy - 0.12, i + 0.52, cy + 0.12), fill=EDGE)
    # Roof vents at the ends.
    for x in (m + 0.1, L - m - 0.3):
        p.block(x, m + 0.06, x + 0.2, m + 0.2, (100, 104, 114, 255), radius=0.03, bevel=0.02, shadow=False)


KINDS = {"reactor": reactor, "battery": battery, "quarters": quarters}


def main():
    os.makedirs(OUT, exist_ok=True)
    n = 0
    for kind, draw in KINDS.items():
        for L in range(1, 7):
            for S in range(1, L + 1):
                p = Piece(L, S)
                draw(p)
                p.save(kind)
                n += 1
    print(f"{n} pieces in {os.path.normpath(OUT)}")


if __name__ == "__main__":
    main()
