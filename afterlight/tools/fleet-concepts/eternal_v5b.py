"""Eternal Hegemony, the great ships and the old fleet's train (v5b): a heavy
cruiser, a battleship, a line carrier, a funeral barge, and three traders of
an empire that stopped two hundred years ago. Hand-drawn, top-down only.

Same grammar as eternal.py (FACTION["grammar"] there), held at every size:
  - a stepped CROWN BOW, every raked step plated;
  - DOUBLE-TIER ZIGGURAT SPONSONS, plated on their forward steps;
  - ENGINE PODS at the stern corners round a recessed centre engine bank;
  - a PROCESSIONAL NAVE of shield/core pillars to a throne-bridge amidships;
  - cathedral symmetry, open courtyards in the big hulls, Titanium;
  - and the TIME-ECHO: part of every hull trails a hollow copy of itself a
    beat out of time (Composite, no rooms), as the Throne of Ages' arches do.

Outlines are built from rectangles and a few geometric strokes; every room is
written by hand, row by row, onto the upper half and mirrored.

Legend: see terran.py and draw.LEGEND.
"""
import math
from collections import deque

from draw import Canvas, AIMED, ENGINES, LEGEND
from eternal import FACTION  # noqa: F401  (same faction, same page)

N4 = ((0, 1), (0, -1), (1, 0), (-1, 0))
DEBUG_MAP = False
SIZE = {  # class -> hull cells (BRIEF.md ladder; heavies below the Throne of Ages' 1965)
    "trader": (350, 900), "cruiser": (800, 1150), "battleship": (1150, 1600),
    "carrier": (950, 1500), "experimental": (800, 1500),
}


class Plan:
    """A hull outline on the upper half (y >= 0), rooms written onto it by hand,
    then mirrored. `ghost` cells are the time-echo: hull, but Composite and empty."""

    def __init__(self, name):
        self.c = Canvas(name)
        self.hull, self.ghost, self.rooms, self.spires = set(), set(), {}, set()

    # ---- outline
    def R(self, x0, x1, y0, y1):
        self.hull |= {(x, y) for x in range(x0, x1 + 1) for y in range(y0, y1 + 1)}
        return self

    def K(self, x0, x1, y0, y1):
        self.hull -= {(x, y) for x in range(x0, x1 + 1) for y in range(y0, y1 + 1)}
        return self

    def zig(self, x0, x1, y0, tiers):
        """A ziggurat sponson growing outboard from y0: tiers of
        (rows, step in at the bow, step in aft)."""
        y, a, b = y0, x0, x1
        for rows, fore, aft in tiers:
            for _ in range(rows):
                self.R(a, b, y, y)
                y += 1
            a, b = a + aft, b - fore
        return self

    def diamond(self, cx, cy, r):
        self.hull |= {(x, y) for x in range(cx - r, cx + r + 1) for y in range(cy - r, cy + r + 1)
                      if abs(x - cx) + abs(y - cy) <= r and y >= 0}
        return self

    def spire(self, x0, y0, ang, ln, w=1.5, into=None):
        """A tapering spike from (x0, y0) at `ang` degrees (0 = straight ahead)."""
        s = self.hull if into is None else into
        a = math.radians(ang)
        for x in range(int(x0) - ln - 3, int(x0) + ln + 3):
            for y in range(max(0, int(y0) - ln - 3), int(y0) + ln + 3):
                al = (x - x0) * math.cos(a) + (y - y0) * math.sin(a)
                ac = -(x - x0) * math.sin(a) + (y - y0) * math.cos(a)
                if -0.5 <= al <= ln and abs(ac) <= w * (1 - al / (ln + 1.0)) + 0.3:
                    s.add((x, y))
                    self.spires.add((x, y))
        return self

    def bar(self, x0, y0, x1, y1, w, into=None):
        """A straight beam of half-width w from (x0, y0) to (x1, y1)."""
        s = self.hull if into is None else into
        L = math.hypot(x1 - x0, y1 - y0)
        for x in range(int(min(x0, x1) - w - 1), int(max(x0, x1) + w + 2)):
            for y in range(max(0, int(min(y0, y1) - w - 1)), int(max(y0, y1) + w + 2)):
                t = max(0, min(1, ((x - x0) * (x1 - x0) + (y - y0) * (y1 - y0)) / (L * L)))
                if math.hypot(x - x0 - t * (x1 - x0), y - y0 - t * (y1 - y0)) <= w:
                    s.add((x, y))
        return self

    def coffin(self, xa, xs, xb, ya, ys, yb, into=None):
        """A coffin outline (upper half): ya high at the foot xa, widening to ys at
        the shoulder xs, narrowing to yb at the head xb."""
        s = self.hull if into is None else into
        for x in range(xa, xb + 1):
            h = ya + (ys - ya) * (x - xa) / (xs - xa) if x <= xs else ys + (yb - ys) * (x - xs) / (xb - xs)
            s |= {(x, y) for y in range(0, int(h + 0.5) + 1)}
        return s

    def echo(self, cells, dx, dy, hollow=True):
        """The time-echo: `cells` shifted by (dx, dy), only its outline if hollow,
        kept where it doesn't land on the real hull."""
        sh = {(x + dx, y + dy) for x, y in cells}
        if hollow:
            sh = {p for p in sh if any((p[0] + a, abs(p[1] + b)) not in sh
                                       for a in (-1, 0, 1) for b in (-1, 0, 1))}
        self.ghost |= {p for p in sh if p[1] >= 0} - self.hull
        return self

    # ---- rooms
    def room(self, x, y, s):
        for i, ch in enumerate(s):
            if ch != " ":
                assert (x + i, y) in self.hull, f"{self.c.name}: room cell {(x + i, y)} '{ch}' is off the hull"
                self.rooms[(x + i, y)] = ch
        return self

    def map(self):
        """The upper half as text, x labels every 5: for drawing rooms."""
        cells = self.hull | self.ghost
        xs = [p[0] for p in cells]
        x0, x1, y1 = min(xs), max(xs), max(p[1] for p in cells)
        out = ["      " + "".join(("|" if x % 5 == 0 else " ") for x in range(x0, x1 + 1)) + f"   x0={x0}"]
        for y in range(y1, -1, -1):
            row = "".join(self.rooms.get((x, y), "#" if (x, y) in self.hull else "~" if (x, y) in self.ghost else " ")
                          for x in range(x0, x1 + 1))
            out.append(f"{y:4d}  {row}")
        return "\n".join(out)

    # ---- finish
    def done(self, size, plate_zone=lambda x, y: False):
        c = self.c
        lone = {q for q in self.hull & self.spires if q[1] > 0     # trim stray cells off spire tips
                and sum((q[0] + a, abs(q[1] + b)) in self.hull for a, b in N4) <= 1}
        self.hull -= lone
        self.ghost -= self.hull
        if DEBUG_MAP:
            print(self.map())
        for p in self.hull | self.ghost:
            c.cells[p] = "#"
        for p, ch in self.rooms.items():
            c.cells[p] = ch
        c.mirror()
        ghost = self.ghost | {(x, -y) for x, y in self.ghost}
        held = set(self.hull) | {(x, -y) for x, y in self.hull}
        grow = True
        while grow:                                   # keep only echo that hangs off the hull
            grow = {q for q in ghost - held if any((q[0] + a, q[1] + b) in held for a, b in N4)}
            held |= grow
        for q in ghost - held:
            del c.cells[q]
        ghost &= held
        # Plates, selectively: an empty cell in the zone that sits in the
        # forward-facing corner of a step (hull aft of it and to one side).
        solid = {p for p, ch in c.cells.items() if ch != "/"}
        xs = [p[0] for p in solid]
        for x in range(min(xs), max(xs) + 2):
            for y in range(-60, 61):
                p = (x, y)
                if p in c.cells or not plate_zone(x, abs(y)):
                    continue
                n, s, w, e = ((x, y + 1) in solid, (x, y - 1) in solid, (x - 1, y) in solid, (x + 1, y) in solid)
                if w and n != s and not e and (x - 1, y) not in ghost and ((x, y + 1) if n else (x, y - 1)) not in ghost:
                    c.cells[p] = "/"
        for p, ch in list(c.cells.items()):          # never close a gun's last open side
            if LEGEND.get(ch) in AIMED:
                nb = [(p[0] + a, p[1] + b) for a, b in N4]
                if all(q in solid or c.cells.get(q) == "/" for q in nb):
                    for q in nb:
                        for m in (q, (q[0], -q[1])):
                            if c.cells.get(m) == "/":
                                del c.cells[m]
        d = finish(c, size)
        for h in d["hull_cells"]:
            if tuple(h["grid_pos"]) in ghost:
                h["material"] = "Composite"
        return d


def finish(c, size):
    """c.build(), plus the checks the shared checker leaves out: every gun has
    an open side, no engine fires into its own hull, the hull is one piece and
    mirror-symmetric with its engines and guns, the bridge is inside the hull
    near the middle, and the hull count fits the class."""
    d = c.build()
    hull = {tuple(h["grid_pos"]) for h in d["hull_cells"]}
    layer = {tuple(h["grid_pos"]): h["layer"] for h in d["hull_cells"]}
    for p, ch in c.cells.items():
        kind = LEGEND.get(ch)
        x, y = p
        if kind in AIMED and kind != "DockingPort":
            if all((x + a, y + b) in hull or c.cells.get((x + a, y + b)) == "/" for a, b in N4):
                d["_checks"].append(f"{kind} at {p} has no open side")
        if kind in ENGINES and any((xx, y) in hull for xx in range(x - 90, x)):
            d["_checks"].append(f"{kind} at {p} vents into the hull")
        if (kind in ENGINES or kind in AIMED) and c.cells.get((x, -y)) != ch:
            d["_checks"].append(f"'{ch}' at {p} has no mirror image")
    start = next(iter(hull))
    seen, q = {start}, deque([start])
    while q:
        x, y = q.popleft()
        for a, b in N4:
            n = (x + a, y + b)
            if n in hull and n not in seen:
                seen.add(n)
                q.append(n)
    if len(seen) != len(hull):
        d["_checks"].append(f"hull in pieces: {len(hull) - len(seen)} cells cut off from {start}")
    if {(x, -y) for x, y in hull} != hull:
        d["_checks"].append("skeleton not mirror-symmetric")
    bridge = [p for p, ch in c.cells.items() if ch == "B"]
    if not bridge:
        d["_checks"].append("no bridge")
    elif any(layer.get(p) == "Outer" for p in bridge):
        d["_checks"].append("bridge on the rim")
    else:
        xs = [p[0] for p in hull]
        mid, half = (min(xs) + max(xs)) / 2, (max(xs) - min(xs)) / 2
        bx = sum(p[0] for p in bridge) / len(bridge)
        if abs(bx - mid) > 0.45 * half or abs(sum(p[1] for p in bridge)) > 0:
            d["_checks"].append(f"bridge at x={bx:.0f} is off the middle ({mid:.0f}) or the centreline")
    lo, hi = SIZE[size]
    if not lo <= len(hull) <= hi:
        d["_checks"].append(f"{len(hull)} hull cells, outside {lo}-{hi}")
    return d


# --------------------------------------------------------------------------- ships
def exarch():
    """Heavy cruiser. A rose window: a ring on eight spokes, the nave
    running through it as the horizontal spoke and the throne at the hub, gun
    bastions on the rim's peaks. A stepped crown bow with two crown points,
    engine pods at the stern corners round a recessed centre bank. Behind the
    rose hangs its echo, the rim a moment out of time."""
    p = Plan("eh_exarch_heavy_cruiser")
    ring, rose = set(), set()
    for x in range(-17, 18):
        for y in range(0, 18):
            r, t = math.hypot(x, y), math.degrees(math.atan2(y, x))
            spoke = min(abs(t - a) for a in (0, 45, 90, 135, 180)) * math.pi / 180 * r
            if 12.0 <= r <= 15.4:
                ring.add((x, y))
            if r <= 4.6 or (r <= 12.5 and spoke <= 1.5):
                rose.add((x, y))
    p.hull |= ring | rose
    for a in (22.5, 67.5, 112.5, 157.5):                       # gun bastions on the rim
        p.diamond(round(15 * math.cos(math.radians(a))), round(15 * math.sin(math.radians(a))), 3)
    p.R(-27, 24, 0, 2).R(-31, -14, 3, 3)                       # the nave through it
    p.zig(-31, -21, 4, [(2, 1, 0), (2, 2, 1), (2, 1, 1)])      # aft sponson over the pods
    p.zig(15, 23, 3, [(2, 3, 0), (1, 2, 0)])                    # crown bow steps
    p.R(24, 30, 0, 1)
    p.spire(30, 0, 0, 5, 1.0)
    p.spire(19, 5, 24, 9, 1.4)                                 # crown points
    p.spire(12, 9, 44, 8, 1.3)
    p.echo({q for q in ring if math.hypot(*q) >= 13.6}, -7, 0, hollow=False)
    p.ghost = {q for q in p.ghost if q[0] <= -9}
    # The rose: a walk round the top of the ring, gun bastions at its peaks.
    p.room(-6, 17, "z").room(6, 17, "z").room(-8, 15, "g").room(8, 15, "g")
    p.room(-7, 15, "..a").room(5, 15, "a..")
    p.room(-7, 14, "Q.KK.M.s.M.KK.Q")
    p.room(-7, 13, "...............")
    # Its aft quarter: the gunners' side.
    p.room(-8, 12, ".a..").room(-10, 11, "n...").room(-10, 10, "...")
    p.room(-12, 9, "n.Q..").room(-12, 8, "..y.a.").room(-16, 7, "g.a...")
    p.room(-15, 6, "..Q.").room(-16, 5, "n.O..").room(-14, 4, "..").room(-15, 3, "...")
    # Its fore quarter, running into the crown.
    p.room(5, 12, "..a.").room(7, 11, "...n").room(8, 10, "...")
    p.room(8, 9, "...O..n").room(7, 8, "..Q...a.g").room(11, 7, ".Q...n")
    p.room(12, 6, "..O..").room(20, 6, "a..").room(21, 7, "n")
    p.room(12, 5, ".Q.y..").room(19, 5, "...")
    p.room(13, 4, "..........").room(13, 3, ".Q.m..O.Q.")
    # The nave: shield pillars down both sides to the throne in the hub.
    p.room(-26, 2, ".b.KK.GG.O.m.a.")
    nave = {x: "." for x in range(-26, 25)}
    nave.update({-27: "E", -25: "R", -20: "G", -19: "G", -1: "B", 0: "B", 25: "n"})
    nave.update({x: "s" for x in (-23, -17, -14, -11, -8, -5, 5, 8, 11, 14, 17)})
    p.room(-27, 1, "".join(nave[x] for x in range(-27, 26)))
    p.room(-27, 0, "E" + "." * 25 + "BBH" + "." * 30 + "rr")
    p.room(-5, 2, "...........").room(-4, 3, ".q.T.A.q.").room(-1, 4, "...")
    p.room(12, 2, ".KK.a.T.a...")
    # Stern: a fusion reactor in each pod, engines round the recessed bank.
    p.room(-29, 9, "n").room(-23, 7, "g")
    p.room(-29, 8, ".cO.f")
    p.room(-31, 7, "E..Q.Q..")
    p.room(-31, 6, "E.FFF.ba.")
    p.room(-31, 5, "E.FFF.R.M.")
    p.room(-31, 4, "E.FFF.b...")
    p.room(-27, 3, ".......")
    return p.done("cruiser", lambda x, y: x >= 13 or (x >= 4 and y >= 12) or -23 <= x <= -19)


def imperator():
    """Battleship. A cathedral nave: lancet windows (pointed-arch
    courtyards) through both flanks, a ziggurat tower standing on every rib
    between them, the throne's tower the tallest. West towers at the stern
    carry the engines; the bow is a stepped crown with two plated points.
    Each tower trails a hollow staircase of itself."""
    p = Plan("eh_imperator_battleship")
    p.R(-34, 13, 0, 10)                                        # the body
    for x0 in (-29, -20, -11, 5):                              # lancet windows
        p.K(x0, x0 + 5, 4, 7).K(x0 + 1, x0 + 4, 8, 8).K(x0 + 2, x0 + 3, 9, 9)
    towers = set(p.hull)
    for x0, t in ((-33, 2), (-24, 2), (-15, 2), (-6, 3), (3, 2)):   # ziggurat towers on the ribs
        p.zig(x0, x0 + 7, 11, [(2, 1, 1), (2, 2, 2), (1, 1, 1)][:t])
    towers = p.hull - towers
    p.zig(-41, -31, 4, [(5, 0, 0), (3, 1, 0), (2, 2, 1)])      # west towers: engine pods
    p.zig(13, 27, 0, [(2, 2, 0), (2, 3, 0), (2, 3, 0), (2, 2, 0)])   # stepped crown bow
    p.R(27, 32, 0, 1).spire(32, 0, 0, 6, 1.1)
    p.spire(12, 10, 28, 10, 1.6).spire(19, 7, 20, 8, 1.4)
    p.echo(towers, -4, 2)
    R = p.room
    # Tower tops: a battery on every ziggurat, lasers on the throne tower.
    R(-3, 15, "zz")
    for x in (-30, -28, -21, -20, -12, -11, 6, 7):
        R(x, 14, "n")
    R(-38, 13, "n").room(-35, 13, "g")
    R(-32, 13, "a.Oa.").room(-22, 13, "a..a").room(-13, 13, "a..a").room(-4, 13, "s..s").room(5, 13, "a..a")
    R(-40, 12, ".Q.Q..O...T...g").room(-23, 12, ".Q..Q.g").room(-14, 12, ".q..q.g").room(-5, 12, "..M...g")
    R(4, 12, ".Q..Q.g").room(15, 12, "..n")
    R(-40, 11, ".KK.m.GG.KK.y.").room(-23, 11, "......").room(-14, 11, "......").room(-5, 11, "......")
    R(4, 11, ".......").room(13, 11, "...g")
    R(-40, 10, "......GG.....").room(-24, 10, ".O..b.").room(-15, 10, ".b..O.").room(-6, 10, ".q.A..q.")
    R(3, 10, ".a..").room(9, 10, "..O..")
    R(-40, 9, ".c.b...Q.Q..").room(-24, 9, ".....").room(-15, 9, ".....").room(-6, 9, "." * 12).room(10, 9, "...")
    # The throne hall: surgery and galleys round the bridge.
    R(-40, 8, ".FFF.R.a...").room(-23, 8, "...").room(-14, 8, "...").room(-5, 8, ".m..SSS...")
    R(11, 8, "..").room(21, 8, "..n")
    R(-40, 7, ".FFF...b..").room(-22, 7, ".").room(-13, 7, ".").room(-4, 7, "...SSS..")
    R(12, 7, "..").room(19, 7, "..g")
    R(-41, 6, "E.FFF.f.c..p").room(-22, 6, ".p").room(-13, 6, ".p").room(-4, 6, ".GG..O..p").room(12, 6, "..a.T...")
    R(-41, 5, "E.....Q.Q..").room(-4, 5, ".GG..Q..").room(12, 5, ".KK.a.KK.n")
    R(-36, 4, "g.....").room(-4, 4, "........").room(12, 4, "..........g")
    R(-33, 3, "....").room(-5, 3, "..........").room(11, 3, ".Q.O.m..Q...n")
    # The nave: pillars to the throne, missiles and a railgun lance in the crown.
    nave = {x: "." for x in range(-33, 26)}
    nave.update({-34: "E", -32: "R", -1: "B", 0: "B"})
    nave.update({x: "s" for x in (-28, -24, -20, -16, -12, -8, -4, 4, 8, 12, 16, 20)})
    R(-34, 1, "".join(nave[x] for x in range(-34, 26))).room(28, 1, "x").room(31, 1, "x")
    aisle = {x: "." for x in range(-33, 25)}
    aisle.update({-32: "b", -30: "L", -29: "L", -27: "c", -25: "M", 6: "a", 9: "y", 12: "a", 15: "T",
                  18: "a", 21: "O", 24: "."})
    R(-34, 2, "E" + "".join(aisle[x] for x in range(-33, 25)) + "z")
    R(-34, 0, "E" + "." * 32 + "BBH" + "." * 33 + "rr")
    for y in range(6, 12):
        R(-41, y, "E")
    return p.done("battleship", lambda x, y: x >= 12 or y >= 11)


GALLERIES = ((-24, 15), (-14, 15), (-4, 14), (6, 12), (16, 9))


def sepulchre():
    """Carrier. A processional spine with five catacomb galleries off
    each side, stepping down toward the crown bow; each gallery's walls are
    lined with niches (drone bays) and it ends in a crown with its guns. The
    crowns' echoes hang a beat behind them."""
    p = Plan("eh_sepulchre_carrier")
    p.R(-32, 26, 0, 3)                                         # the spine
    caps = set()
    for cx, top in GALLERIES:                                  # hangar galleries
        p.zig(cx - 3, cx + 3, 4, [(2, 1, 1), (top - 5, 0, 0)])
        before = set(p.hull)
        p.R(cx - 3, cx + 3, top + 1, top + 1)                  # a crown on each
        p.hull |= {(cx - 3, top + 2), (cx, top + 2), (cx + 3, top + 2)}
        caps |= p.hull - before
    p.zig(-36, -30, 4, [(4, 0, 0), (2, 1, 1), (2, 1, 1)])      # engine pods
    p.zig(22, 30, 0, [(2, 0, 0), (2, 2, 0), (1, 2, 1)])         # crown
    p.R(30, 34, 0, 1).spire(34, 0, 0, 5, 1.0)
    p.spire(23, 5, 28, 7, 1.3)
    p.echo(caps, -4, 2)
    R = p.room
    # Each gallery is a catacomb: niches (drone bays) down both walls, the
    # keepers' cells between, a battery under its crown.
    wall = ["#a.a#", "D...D", "DQ.QD", "#...#", "#...#", "#y.O#", "#...#", "D...D", "DQ.QD", "#...#"]
    for cx, top in GALLERIES:
        R(cx - 1, top + 1, "n.n")
        for k, y in enumerate(range(top, 5, -1)):
            R(cx - 2, y, wall[k])
        R(cx - 2, 5, ".....").room(cx - 2, 4, ".....")
    # Stern: a fusion reactor in each pod, engines round the recessed bank.
    R(-33, 11, "n").room(-34, 4, "g")
    R(-34, 10, "Q.Q")
    R(-36, 9, "E.c.O.").room(-36, 8, "E.FFF.").room(-36, 7, "E.FFF.").room(-36, 6, "E.FFF.")
    R(-36, 5, "E..b.M").room(-32, 4, "..").room(-31, 3, "..")
    # The spine: chapels on the crossings, pillars down the nave to the throne.
    R(-27, 3, ".GG.m..").room(-17, 3, ".KK.Q..").room(-7, 3, ".q.A.q.").room(3, 3, "..Q.KK.")
    R(13, 3, ".a.T.a.").room(22, 3, ".O.Q.a.g")
    aisle = {x: "." for x in range(-31, 30)}
    aisle.update({-30: "b", -28: "L", -27: "L", -26: "G", -25: "G", -20: "M", -10: "c", -4: "T", 6: "y", 12: "M", 19: "c", 25: "a"})
    R(-31, 2, "".join(aisle[x] for x in range(-31, 30)) + "z")
    nave = {x: "." for x in range(-31, 31)}
    nave.update({-30: "R", 0: "B", 1: "B"})
    nave.update({x: "s" for x in (-24, -19, -14, -9, -4, 6, 11, 16, 21)})
    R(-32, 1, "E" + "".join(nave[x] for x in range(-31, 31)))
    R(-32, 0, "E" + "." * 31 + "BBH" + "." * 33 + "rr")
    R(23, 4, "...").room(24, 5, ".").room(24, 6, "..n").room(26, 4, "n")
    return p.done("carrier", lambda x, y: x >= 21 or -31 <= x <= -29)


CANDLES = (-22, -16, -10, -4, 4, 10, 16, 22)


def catafalque():
    """Experimental: funeral barge. The dead emperor's throne rides on
    a quatrefoil bier slung between two carrying poles, candles (floodlights)
    standing down both poles, pallbearer pods at the four ends. The bier's
    echo follows it between the poles."""
    p = Plan("eh_catafalque_funeral_barge")

    def octagon(cx, cy, r, c):
        p.hull |= {(x, y) for x in range(cx - r, cx + r + 1) for y in range(max(0, cy - r), cy + r + 1)
                   if abs(x - cx) + abs(y - cy) <= c}

    p.R(-29, 29, 11, 13)                                       # the carrying poles
    bier = set(p.hull)
    for cx, cy in ((-6, 0), (6, 0), (0, 6)):                   # the catafalque: a quatrefoil
        p.hull |= {(x, y) for x in range(cx - 6, cx + 7) for y in range(max(0, cy - 6), cy + 7)
                   if math.hypot(x - cx, y - cy) <= 5.6}
    p.R(-4, 4, 0, 4)
    bier = p.hull - bier
    octagon(-31, 12, 5, 7)                                     # pallbearer pods
    octagon(30, 12, 5, 7)
    p.spire(35, 12, 0, 5, 1.2).spire(34, 15, 35, 5, 1.2).spire(34, 9, -35, 4, 1.1)
    for x in CANDLES:                                          # funeral candles down the poles
        p.R(x, x, 14, 15)
    p.echo(bier, -15, 0)
    R = p.room
    for x in CANDLES:
        R(x, 15, "l")
    # The catafalque: the throne on the keel, the regalia vaulted before it,
    # pillars standing watch, the old court quartered in the lobes.
    R(-2, 10, ".....").room(-3, 9, ".q...q.").room(-4, 8, "a.Q...Q.a").room(-4, 7, ".GG...O..")
    R(-4, 6, ".GG.s.m..").room(-7, 5, "g" + "." * 13 + "g").room(-8, 4, "..aQ.s..$..s.Qa..")
    R(-10, 3, "n.M.KK...q.q...KK.M.n").room(-11, 2, "..LL.c.s.......s.c.LL..")
    R(-11, 1, "z.R..s..W.BB..A..s..R.z").room(-10, 0, "." * 9 + "BBH" + "." * 9)
    R(-3, 11, ".......")
    # Rear pallbearers: a fusion reactor each, engines on the aft face.
    R(-32, 17, "n").room(-31, 7, "n").room(-34, 8, "g")
    R(-33, 16, "Q.O.Q").room(-34, 15, ".FFF.c.").room(-36, 14, "E..FFF.b..").room(-36, 13, "E..FFF..Q..")
    R(-36, 12, "E" + "." * 73)
    R(-36, 11, "E.KK.R.m.O.").room(-36, 10, "E..Q.b.Q..").room(-34, 9, ".c.O.a.").room(-33, 8, ".....")
    # Front pallbearers: the guard, guns on every face, a railgun on each pole.
    R(29, 17, "n.n").room(28, 16, "a...a").room(36, 16, "z").room(36, 15, "g")
    R(27, 15, ".Q.T.Qa..").room(26, 14, ".y..M..s.").room(25, 13, ".....O.....")
    R(38, 12, "rr").room(25, 11, ".Q.a..O.Q..").room(26, 10, "..KK.a...g")
    R(27, 9, ".a....a.").room(28, 8, ".....n").room(30, 7, "n")
    return p.done("experimental", lambda x, y: x >= 33 or (8 <= x <= 12 and y <= 6))


def tribute():
    """Trader: tribute galleon. A nave with four strongbox coffers a
    side on short necks; the second coffer is an echo, a province that is no
    more. Two gatlings and a lantern on the lids, a cannon in the crown."""
    p = Plan("eh_tribute_galleon")
    p.R(-24, 16, 0, 3)                                         # the nave
    p.R(-24, 13, 4, 4)
    ghost = set()
    for x0 in (-19, -10, -1, 8):                               # the coffers, on short necks
        box = {(x, y) for x in range(x0, x0 + 7) for y in range(7, 13)} - {(x0, 12), (x0 + 6, 12)}
        neck = {(x, y) for x in range(x0 + 2, x0 + 5) for y in (5, 6)}
        if x0 == -10:
            ghost |= box | neck                                # a province that is no more
        else:
            p.hull |= box | neck
    p.zig(-28, -21, 3, [(3, 0, 0), (2, 1, 1), (1, 1, 1)])      # engine pods
    p.zig(16, 22, 0, [(2, 0, 0), (2, 2, 0)])                    # crown bow
    p.R(22, 25, 0, 1).spire(25, 0, 0, 4, 1.0).spire(15, 5, 32, 6, 1.3)
    p.echo(ghost, 0, 0)
    R = p.room
    # The coffers: bulk holds under a strongroom, a walk up the middle to the
    # gatling (or the lantern) on the lid.
    for x0, top in ((-19, "g"), (-1, "l"), (8, "g")):
        R(x0 + 3, 12, top).room(x0 + 1, 11, "CC.CC").room(x0 + 1, 10, "CC.CC").room(x0 + 1, 9, "k$.$k")
        R(x0 + 1, 8, "UU.UU").room(x0 + 2, 7, "k.k").room(x0 + 3, 6, ".").room(x0 + 3, 5, ".")
        R(x0 + 2, 4, "...")
    # Stern pods, the nave, the throne amidships, an empty hold under the
    # coffer that is no longer there.
    R(-25, 8, "g").room(-27, 7, "Q.O.Q.").room(-28, 6, "E..LL.b").room(-28, 5, "Ec.f...").room(-28, 4, "E.......")
    R(-24, 3, ".Q.Q.O....m.GG.Q.Q.q.A.q....Q.Q.y..").room(16, 3, ".")
    aisle = {x: "." for x in range(-23, 22)}
    aisle.update({-22: "L", -21: "L", -19: "b", -17: "c", -13: "M", -12: "G", -11: "G", -9: "k", -7: "k",
                  -4: "O", 4: "M", 8: "a", 12: "y", 15: "O", 19: "M"})
    R(-24, 2, "#" + "".join(aisle[x] for x in range(-23, 22)))
    nave = {x: "." for x in range(-23, 23)}
    nave.update({-22: "R", 0: "B", 1: "B"})
    nave.update({x: "s" for x in (-18, -14, -10, -6, 5, 9, 13, 17)})
    R(-24, 1, "E" + "".join(nave[x] for x in range(-23, 23)) + "n")
    R(-24, 0, "E" + "." * 23 + "BBH" + "." * 24)
    return p.done("trader", lambda x, y: x >= 14 or y >= 12 or (-22 <= x <= -20 and y >= 7))


def victualler():
    """Trader: supply train. Five chamfered wagons coupled down the
    centreline, the tender with its reactor and engines at the back, the
    throne wagon in the middle. A sixth wagon rides ahead as an echo, its
    coupling still waiting for it."""
    p = Plan("eh_victualler_train")
    wagons = (-28, -17, -6, 5, 16)
    for x0 in wagons:                                          # the wagons
        p.zig(x0, x0 + 8, 0, [(4, 0, 0), (2, 1, 1), (1, 2, 2)])
    for x0 in wagons[:-1]:                                     # couplings
        p.R(x0 + 9, x0 + 10, 0, 2)
    lead = {(x + 11, y) for x in range(16, 25) for y in range(0, 7) if (x, y) in p.hull}
    p.R(25, 26, 0, 2)                                          # a coupling, waiting
    p.zig(-32, -27, 4, [(3, 0, 0), (1, 1, 1)])                 # engine pods on the last wagon
    p.echo(lead, 0, 0)
    R = p.room
    # The tender: reactor and engines, the crew's berths.
    R(-30, 7, "g").room(-32, 6, "E..Rb.").room(-32, 5, "E.c..Q.O.Q..").room(-28, 4, "..LL....")
    R(-27, 3, ".......").room(-27, 2, ".f.c.M..")
    # Munitions for a war that ended.
    R(-13, 6, "g").room(-16, 5, "a.a.a.a").room(-16, 4, "a.a.a.a").room(-16, 3, ".......").room(-17, 2, "." * 9)
    # The throne wagon.
    R(-5, 5, "Q.O.Q.q").room(-5, 4, ".......").room(-5, 3, ".m.T.A.").room(-6, 2, "q.s...s.q")
    # Rations, and cold stores.
    R(9, 6, "g").room(6, 5, "CC...CC").room(6, 4, "CC.y.CC").room(6, 3, ".......").room(5, 2, ".k.M.k...")
    R(24, 4, "n").room(17, 5, "UU.k.UU").room(17, 4, "......").room(17, 3, "V.V.V.V").room(16, 2, "..s.O.s..")
    nave = {x: "." for x in range(-27, 26)}
    nave.update({-2: "B", -1: "B"})
    nave.update({x: "s" for x in (-24, -14, -11, 8, 11, 19, 22)})
    R(-28, 1, "E" + "".join(nave[x] for x in range(-27, 26)))
    R(-28, 0, "E" + "." * 25 + "BBH" + "." * 25)
    R(-28, 2, "E")
    return p.done("trader", lambda x, y: y >= 4)


def coronation():
    """Trader: pilgrim barge. A stepped pyramid seen from above: the
    pilgrims' terraces step down on every side, every forward step plated, two
    gardens open to the sky, the empty coronation throne at the summit under a
    crown of lasers. Ahead of the bow runs the pyramid's echo."""
    p = Plan("eh_coronation_barge")
    tiers = set()
    for x in range(-24, 25):                                   # a stepped pyramid, seen from above
        h = 13 - (abs(x) + (1 if x > 0 else 0)) // 2
        p.R(x, x, 0, h)
    for x0, x1 in ((-12, -6), (5, 11)):                        # terrace gardens, open to the sky
        p.K(x0, x1, 4, 5).K(x0 + 1, x1 - 1, 6, 6).K(x0 + 2, x1 - 2, 7, 7)
    p.R(-2, 1, 13, 14).R(-1, 0, 15, 15)                         # the crown on the summit
    p.R(24, 28, 0, 1).spire(28, 0, 0, 4, 1.0)
    outline = {(x + 6, y) for x, y in p.hull}
    p.echo({q for q in outline if q[0] >= 18}, 0, 0)
    R = p.room
    # The summit: the empty coronation throne under a crown of lasers, the
    # pilgrims' terraces stepping down on every side, two gardens open to the sky.
    R(-1, 15, "zz").room(-1, 14, "..").room(-1, 12, "..")
    R(-3, 11, "Qs..sQ").room(-5, 10, "Q.q....q.Q").room(-7, 9, "...www..www...")
    R(-7, 8, ".Q.wwwO.www.Q.").room(-6, 7, "..www..www..")
    R(-5, 6, ".GG.m..GG.").room(-4, 5, "GG..O.GG").room(-4, 4, "Q.Q..Q.Q").room(-5, 3, ".b.A..T.b.")
    R(-15, 5, "Q.").room(-18, 4, "g.Q..").room(-19, 3, ".R.Q.Q.")
    R(13, 5, ".Q").room(13, 4, "aQ..g").room(12, 3, ".Q.O.a.")
    aisle = {x: "." for x in range(-21, 21)}
    aisle.update({-20: "L", -19: "L", -17: "c", -15: "M", -11: "k", -8: "w", 7: "k", 10: "M", 14: "c", 17: "y"})
    aisle.update({-8: ".", 2: "s", -3: "s"})
    R(-23, 2, "E." + "".join(aisle[x] for x in range(-21, 21)))
    nave = {x: "." for x in range(-23, 23)}
    nave.update({-22: "R", -1: "B", 0: "B"})
    nave.update({x: "s" for x in (-16, -12, -8, 4, 8, 12, 16)})
    R(-24, 1, "E" + "".join(nave[x] for x in range(-23, 23)) + "n")
    R(-24, 0, "E" + "." * 22 + "BBH" + "." * 28)
    R(-21, 3, "E")
    return p.done("trader", lambda x, y: x >= 1 and not (4 <= x <= 12 and 3 <= y <= 7))


CONCEPTS = [
    ("exarch_heavy_cruiser", exarch, "Exarch", "Heavy cruiser",
     "A rose window under way: eight spokes round the throne at the hub, gun bastions on the rim, a crown bow ahead. Behind the wheel turns its echo, the same rim a moment late."),
    ("imperator_battleship", imperator, "Imperator", "Battleship",
     "A cathedral nave pierced by lancet windows, a ziggurat tower on every rib and the throne's tower tallest. Each tower trails a staircase of itself, a beat out of time."),
    ("sepulchre_carrier", sepulchre, "Sepulchre of the Legions", "Carrier",
     "Five crowned catacomb galleries off a processional spine, their walls lined with niches. From the niches come the ships the Hegemony has lost, called back as they were."),
    ("catafalque_funeral_barge", catafalque, "Catafalque", "Experimental: funeral barge",
     "Carries the throne of an emperor dead two hundred years on a quatrefoil bier between two poles, candles burning down both. The fleet still takes his orders, and his bier's echo follows it."),
    ("tribute_galleon", tribute, "Provincial Tribute", "Trader (tribute galleon)",
     "Four strongbox coffers on short necks, carrying the provinces' taxes to an emperor who is dead. One coffer is only an echo: its province no longer exists, and the galleon carries it anyway."),
    ("victualler_train", victualler, "Imperial Victualler", "Trader (supply train)",
     "Five wagons of rations and munitions for a war that ended two centuries ago. A sixth wagon rides ahead of it, hollow, arriving a moment before the train does."),
    ("coronation_barge", coronation, "Long Coronation", "Trader (pilgrim barge)",
     "A stepped pyramid of pilgrim terraces with two gardens open to the sky and an empty throne at the summit, bound for a coronation two hundred years late. Its bow arrives as an echo first."),
]
