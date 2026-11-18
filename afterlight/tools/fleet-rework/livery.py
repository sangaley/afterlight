"""Faction liveries for the reworked fleet: more colour, laid by structure, and
small details on the shell.

The game's paint (vfx/ship_roof.rs ShipPaint) gives every ship three colours
(main, second, trim), any colour per tile (Slot::Custom) and decals of twelve
shapes in any colour on three layers. This lays all of that from a faction's
palette over a refit design:

  - the hull in the faction's main colour, its livery pattern in the second;
  - rooms tinted by what they are (guns, engines, power, the bridge, sensors)
    from the same palette, so colour follows the ship's structure;
  - the faction's motif in decals (Terran navy stripes and a roundel, the
    Choir's gold kintsugi seams, the Compact's hazard hatching...);
  - details: panel seams across big bare plating, running lights round the
    hull's edge, hatching ahead of the engines.

Everything is laid mirror-wise port and starboard, like paint_gen.rs, whose
decal conventions this follows (Stripe turn 0 runs bow to stern, Edge turn 1
is a cell's bow side, Chevron turn 1 points at the bow, four Arcs round a
corner make a roundel).
"""
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
sys.path.insert(0, os.path.join(ROOT, "tools", "fleet-concepts"))
import fleet  # noqa: E402  (module footprints, from the game's registry)


def _categories():
    """ModuleType -> ModuleCategory, read from components.rs's category()."""
    text = open(os.path.join(ROOT, "src", "components.rs")).read()
    body = text[text.index("pub fn category(&self) -> ModuleCategory"):]
    body = body[:body.index("\n    }\n")]
    out = {}
    for arm in re.finditer(r"((?:ModuleType::\w+\s*\|?\s*)+)=>\s*ModuleCategory::(\w+)", body):
        for t in re.findall(r"ModuleType::(\w+)", arm.group(1)):
            out[t] = arm.group(2)
    return out


CATEGORY = _categories()


def mix(a, b, t):
    return [round(a[i] + (b[i] - a[i]) * t) for i in range(3)]


def shade(a, k):
    return [max(0, min(255, round(c * k))) for c in a]


# A faction's paint: main hull, second (its livery pattern and panels), three
# accents (the first is the trim), a light colour for running lights, its
# livery pattern, its motif, and how strongly rooms take their tint.
THEMES = {
    # The navy: light grey plate, navy panels, red and blue stripes down the
    # spine, a yellow roundel (the developer asked for red, blue and yellow).
    "terran_hegemony": dict(main=[178, 188, 200], second=[38, 56, 104], accents=[[206, 44, 48], [44, 104, 206], [238, 192, 52]],
                            light=[250, 244, 220], pattern="Bands", motif="navy", room=0.6),
    # Crimson and gold, an empire dressed for parade: gold chevrons down the
    # nave, ivory bands.
    "eternal_hegemony": dict(main=[156, 38, 46], second=[70, 20, 28], accents=[[236, 188, 74], [238, 228, 206], [116, 64, 156]],
                             light=[255, 214, 120], pattern="Chevrons", motif="regal", room=0.62),
    # Royal slate and gold, jewel green: a gilded nose and a roundel.
    "gilded_throne": dict(main=[96, 104, 156], second=[42, 44, 76], accents=[[242, 200, 82], [240, 232, 214], [44, 176, 124]],
                          light=[255, 232, 160], pattern="Bow", motif="gilt", room=0.6),
    # Guild green and cream with burgundy and copper bands.
    "merchant_princes": dict(main=[124, 168, 108], second=[54, 76, 50], accents=[[238, 226, 186], [138, 38, 52], [200, 116, 58]],
                             light=[255, 236, 190], pattern="Bands", motif="guild", room=0.6),
    # Rust orange work paint, charcoal, hazard yellow and black on the push
    # bars, a teal patch here and there.
    "free_haulers_compact": dict(main=[196, 116, 72], second=[238, 192, 46], accents=[[242, 198, 40], [26, 26, 30], [60, 166, 164]],
                                 light=[255, 220, 120], pattern="Stripe", motif="hazard", room=0.6),
    # Violet flesh, teal veins, pink and lime spots.
    "synthesis_collective": dict(main=[152, 86, 194], second=[58, 30, 76], accents=[[72, 222, 200], [236, 128, 150], [176, 232, 84]],
                                 light=[150, 255, 230], pattern="Stripe", motif="spots", room=0.62),
    # Pale leaf-white with teal flanks, leaf green, sky blue and amber.
    "stellar_preserve": dict(main=[214, 232, 222], second=[40, 146, 128], accents=[[98, 184, 82], [82, 172, 222], [232, 172, 62]],
                             light=[230, 255, 210], pattern="Split", motif="flanks", room=0.6),
    # Pale jade, broken and mended in gold (kintsugi), a bruise violet.
    "broken_choir": dict(main=[164, 200, 174], second=[52, 70, 60], accents=[[230, 184, 78], [238, 236, 222], [96, 64, 146]],
                         light=[255, 226, 150], pattern="Checker", motif="kintsugi", room=0.62),
    # Bone, with violet ribs and the ember of a dying star.
    "corpse_stars": dict(main=[216, 206, 184], second=[66, 44, 104], accents=[[156, 100, 232], [204, 74, 44], [118, 136, 100]],
                         light=[200, 150, 255], pattern="Chevrons", motif="ribs", room=0.62),
    # White and charcoal, one pale gold pinstripe: restraint.
    "the_silence": dict(main=[232, 234, 238], second=[56, 58, 64], accents=[[214, 194, 140], [40, 42, 46], [126, 146, 176]],
                        light=[230, 236, 255], pattern="Bands", motif="quiet", room=0.3),
    # Wheat gold, crook brown, wool cream and a red mark.
    "shepherd": dict(main=[218, 184, 82], second=[74, 50, 30], accents=[[132, 86, 44], [242, 234, 212], [176, 42, 36]],
                     light=[255, 240, 190], pattern="Bow", motif="crook", room=0.6),
    # Orange and charcoal chevrons; the copies' generations in teal, cream
    # and red.
    "recursive_kingdom": dict(main=[232, 142, 62], second=[48, 44, 42], accents=[[62, 172, 182], [238, 222, 190], [204, 62, 42]],
                              light=[255, 220, 160], pattern="Chevrons", motif="generations", room=0.6),
}

# Which palette colour a room takes, by what it is.
ROOM_TINT = {
    "Weapons": 0, "Guns": 0, "Lasers": 0,      # first accent
    "Propulsion": 1,                           # second accent
    "Power": 2, "Detection": 2,                # third accent
    "Control": "second", "Utility": "second",  # panel colour
}
SKIP = {"Structural"}  # armour and frames keep the hull's paint


def accent_at(pattern, cell, lo, hi):
    """Livery::accent_at."""
    x, y = cell
    from_bow = hi[0] - x
    off = y - (lo[1] + hi[1]) * 0.5
    if pattern == "Bands":
        return x % 6 < 2
    if pattern == "Chevrons":
        return (x + int(abs(off))) % 7 < 2
    if pattern == "Bow":
        return from_bow < max((hi[0] - lo[0] + 1) // 5, 2)
    if pattern == "Split":
        return off > 0
    if pattern == "Checker":
        return (x // 2 + y // 2) % 2 == 0
    if pattern == "Stripe":
        return abs(off) < 1.5
    return False


class Painter:
    def __init__(self, design, theme):
        self.t = theme
        self.cells = {tuple(c["grid_pos"]) for c in design["hull_cells"]}
        xs = [c[0] for c in self.cells]
        ys = [c[1] for c in self.cells]
        self.lo, self.hi = (min(xs), min(ys)), (max(xs), max(ys))
        self.s = self.lo[1] + self.hi[1]          # row y mirrors to s - y
        self.len = self.hi[0] - self.lo[0] + 1
        self.wide = (self.hi[1] - self.lo[1] + 2) // 2
        # Motifs are drawn in units that grow with the ship, so a flagship's
        # stripes read as stripes and not as pinstripes.
        self.u = max(1, round(self.len / 42))
        # What stands on each cell.
        self.room = {}
        self.engines = set()
        for m in design["modules"]:
            cat = CATEGORY.get(m["module_type"])
            for c in fleet.footprint(m["module_type"], tuple(m["grid_pos"]), m.get("rotation") or "North"):
                if cat and cat not in SKIP:
                    self.room[c] = cat
                if cat == "Propulsion" and "Engine" in m["module_type"]:
                    self.engines.add(c)
        self.edge = {c for c in self.cells if any((c[0] + dx, c[1] + dy) not in self.cells for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)))}
        self.decals = []
        self.seen = set()

    def dy(self, c):
        return abs(2 * c[1] - self.s) // 2

    def upper(self):
        return sorted((c for c in self.cells if 2 * c[1] >= self.s), key=lambda c: (-c[0], c[1]))

    def put(self, c, shape, turn, colour, layer):
        """A decal and its mirror across the ship's middle (Decal::mirrored)."""
        if c not in self.cells:
            return
        for cell, tn, flip in ((c, turn, False), ((c[0], self.s - c[1]), (4 - turn % 4) % 4, True)):
            if cell not in self.cells or (cell == c and flip):
                continue
            key = (cell, shape, tn, flip, layer)
            if key in self.seen:
                continue
            self.seen.add(key)
            self.decals.append({"cell": list(cell), "shape": shape, "turn": tn, "flip": flip, "colour": list(colour), "layer": layer})

    # ------------------------------------------------------------------ slots
    def slots(self):
        t = self.t
        out = []
        for c in sorted(self.cells):
            cat = self.room.get(c)
            scaled = (c[0] // self.u, c[1] // self.u)
            lo, hi = (self.lo[0] // self.u, self.lo[1] // self.u), (self.hi[0] // self.u, self.hi[1] // self.u)
            if accent_at(t["pattern"], scaled, lo, hi):
                out.append([list(c), "Second"])
            elif cat in ROOM_TINT:
                k = ROOM_TINT[cat]
                tint = t["second"] if k == "second" else t["accents"][k]
                out.append([list(c), {"Custom": mix(t["main"], tint, t["room"])}])
            else:
                out.append([list(c), "Main"])
        return out

    # --------------------------------------------------------------- details
    def seams(self):
        """Panel seams across big bare plating: every sixth column and row."""
        dark = shade(self.t["main"], 0.62)
        for c in self.upper():
            if c in self.room or c in self.edge:
                continue
            if (self.hi[0] - c[0]) % (6 * self.u) == 3:
                self.put(c, "Edge", 1, dark, 1)
            if self.dy(c) % (6 * self.u) == 3:
                self.put(c, "Edge", 0, dark, 1)

    def lights(self):
        """Running lights round the hull's edge, about every fifth cell."""
        n = 0
        for c in self.upper():
            if c in self.edge and c not in self.room:
                if n % 5 == 0:
                    self.put(c, "Dot", 0, self.t["light"], 3)
                n += 1

    def engine_hatching(self):
        """Hazard hatching on the plating just ahead of the engines."""
        a = self.t["accents"]
        for (x, y) in self.engines:
            for k in (1, 2):
                c = (x + k, y)
                if c in self.cells and c not in self.room and 2 * c[1] >= self.s:
                    self.put(c, "Slash", 0, shade(a[1], 0.8), 2)

    # ---------------------------------------------------------------- motifs
    def spine_stripes(self, colours, start=0.1, stop=0.85):
        """Stripes bow to stern either side of the spine, one per colour."""
        x0 = self.hi[0] - int(self.len * stop)
        x1 = self.hi[0] - int(self.len * start)
        u = self.u
        for k, col in enumerate(colours):
            for c in self.upper():
                if not x0 <= c[0] <= x1:
                    continue
                d = self.dy(c) - 1 - k * u
                if u == 1 and d == 0:
                    self.put(c, "Stripe", 0, col, 2)
                elif u > 1 and 0 <= d < u:
                    self.put(c, "Full", 0, col, 2)

    def roundel(self, colour, at=0.3, ring=None):
        if self.u > 1:
            return self.disc(colour, at, ring)
        x = self.hi[0] - int(self.len * at)
        y = (self.s + 1) // 2 + (0 if self.s % 2 else max(2, self.wide // 2))
        for dx, dyy, turn in ((0, 0, 0), (-1, 0, 1), (-1, -1, 2), (0, -1, 3)):
            c = (x + dx, y + dyy)
            if 2 * c[1] >= self.s:
                self.put(c, "Arc", turn, colour, 3)

    def disc(self, colour, at, ring):
        """A painted roundel 2-3 units across on each flank (or the spine)."""
        r = 1.6 * self.u
        cx = self.hi[0] - int(self.len * at)
        cy = (self.s / 2) + (0 if self.wide < 6 * self.u else self.wide / 2)
        for c in self.upper():
            d = ((c[0] - cx) ** 2 + (c[1] - cy) ** 2) ** 0.5
            if d <= r:
                self.put(c, "Full", 0, colour, 3)
            elif ring and d <= r + 0.9:
                self.put(c, "Full", 0, ring, 3)

    def spine_chevrons(self, colour, step=3):
        for c in self.upper():
            if self.dy(c) < self.u and self.lo[0] + self.len // 5 <= c[0] <= self.hi[0] - 2 and (self.hi[0] - c[0]) % (step * self.u) < self.u:
                self.put(c, "Chevron", 1, colour, 3)

    def nose(self, colour, frac=0.18):
        n = max(2, int(self.len * frac))
        for c in self.upper():
            u = self.hi[0] - c[0] + self.dy(c)
            if u < n:
                self.put(c, "Full", 0, colour, 1)
            elif u == n:
                self.put(c, "Diagonal", 1, colour, 1)

    def flanks(self, colour, depth=2):
        for c in self.upper():
            if self.dy(c) > 0 and any((c[0], c[1] + k) not in self.cells for k in range(1, depth * self.u + 1)):
                self.put(c, "Full", 0, colour, 1)

    def bands(self, colour, line, n=3, w=2):
        w *= self.u
        gap = max(self.len // (n + 1), w + 1)
        for k in range(1, n + 1):
            x0 = self.hi[0] - k * gap
            for c in self.upper():
                if x0 - w + 1 <= c[0] <= x0:
                    self.put(c, "Full", 0, colour, 1)
                    if c[0] == x0:
                        self.put(c, "Edge", 1, line, 2)
                    if c[0] == x0 - w + 1:
                        self.put(c, "Edge", 3, line, 2)

    def seams_gold(self, colour, every=9):
        """Kintsugi: gold cracks running slantwise over the plating, mended."""
        for c in self.upper():
            if c in self.room:
                continue
            # A crack zigzags across the plating: its line wanders with y.
            zig = (self.dy(c) // (2 * self.u)) % 2
            v = (self.hi[0] - c[0]) + (self.dy(c) % (2 * self.u)) * (1 if zig else -1)
            if v % (every * self.u) < self.u:
                self.put(c, "Full", 0, colour, 2)

    def spots(self, colours, every=7):
        for c in self.upper():
            if c in self.room or c in self.edge:
                continue
            u = self.u
            h = ((c[0] // u) * 7 + (self.dy(c) // u) * 13) % every
            if h == 0:
                shape = "Circle" if u == 1 else "Full"
                self.put(c, shape, 0, colours[(c[0] // (every * u)) % len(colours)], 2)

    def hazard(self, yellow, black, frac=0.1):
        """Yellow-and-black hatching across the bow (the push bar)."""
        n = max(2, int(self.len * frac))
        for c in self.upper():
            if self.hi[0] - c[0] < n:
                self.put(c, "Full", 0, yellow, 1)
                self.put(c, "Slash", 0, black, 2)

    def generations(self, colours):
        """The Recursive Kingdom's chevrons, each copy's band in its own
        generation's colour."""
        for c in self.upper():
            if c in self.room:
                continue
            v = (c[0] + self.dy(c)) // self.u
            if v % 7 < 2:
                self.put(c, "Full", 0, colours[(v // 7) % len(colours)], 1)

    def motif(self):
        t, a = self.t, self.t["accents"]
        m = t["motif"]
        if m == "navy":
            self.spine_stripes([a[0], a[1]])
            self.roundel(a[2])
            self.spine_chevrons(a[2], step=4)
        elif m == "regal":
            self.spine_chevrons(a[0], step=3)
            self.bands(a[1], a[0], n=2, w=1)
        elif m == "gilt":
            self.nose(a[0])
            self.roundel(a[2], at=0.45)
            self.spine_stripes([a[1]], start=0.25)
        elif m == "guild":
            self.bands(a[0], a[1], n=3, w=2)
            self.spine_stripes([a[2]], start=0.2, stop=0.8)
        elif m == "hazard":
            self.hazard(a[0], a[1])
            self.flanks(a[2], depth=1)
        elif m == "spots":
            self.flanks(a[0], depth=1)
            self.spots([a[1], a[2]])
        elif m == "flanks":
            self.flanks(t["second"], depth=2)
            self.spine_stripes([a[0], a[1]], start=0.15, stop=0.7)
            self.roundel(a[2], at=0.5)
        elif m == "kintsugi":
            self.flanks(a[2], depth=1)
            self.seams_gold(a[0])
            self.spine_stripes([a[1]], start=0.3, stop=0.7)
        elif m == "ribs":
            self.bands(a[0], shade(a[0], 0.6), n=4, w=1)
            self.spots([a[1]], every=11)
        elif m == "quiet":
            self.spine_stripes([a[0]], start=0.15, stop=0.9)
        elif m == "crook":
            self.nose(a[0], frac=0.14)
            self.spine_stripes([a[1]], start=0.2)
            self.roundel(a[2], at=0.55)
        elif m == "generations":
            self.generations([a[0], a[1], a[2]])


def paint(slug, design):
    """A refit design's ShipPaint (serde form) in its faction's livery."""
    t = THEMES[slug]
    p = Painter(design, t)
    p.seams()
    p.motif()
    p.engine_hatching()
    p.lights()
    s = lambda c: [round(v / 255, 4) for v in c]  # noqa: E731
    return {"main": s(t["main"]), "second": s(t["second"]), "trim": s(t["accents"][0]), "warm": False,
            "slots": p.slots(), "decals": p.decals}
