"""Gilded Throne, round 5b: the heavies, the Eternal CEO's flagship, two
experimentals, two traders and a defence platform. Hand-drawn, top-down only, in the approved
grammar (gilded.py): a cut-gem jewel at the heart with the bridge in it, a
crown of an odd number of spikes at the bow (centre one longest, a plate on
every tip), crescent wings curving back to aft-pointing horns, filigree
diagonal steps plated only on the forward faces. Titanium; shields, lasers,
ion and railguns.

Every skeleton is drawn on y >= 0 and mirrored (outline, engines, guns);
the platform is drawn as one eighth and turned. Every room is placed by
hand: the jewels as text blocks, the bands, rings, girders and wings as
lists of cells, section by section. Where a section is only one hall wide
its rooms are crew posts (walkable), so the hall runs through them.

Legend: see gilded.py / terran.py and draw.LEGEND.
"""
import math
from collections import deque

from draw import Canvas
from gilded import FACTION, _filigree, _horn  # same faction, same page

N4 = ((0, 1), (0, -1), (1, 0), (-1, 0))


# ---------------------------------------------------------------- helpers
def finish(c):
    """Build, then the checks draw.py doesn't do: the hull is one piece, and
    no engine fires into its own hull."""
    d = c.build()
    hull = {tuple(h["grid_pos"]) for h in d["hull_cells"]}
    start = next(iter(hull))
    seen, q = {start}, deque([start])
    while q:
        x, y = q.popleft()
        for dx, dy in N4:
            n = (x + dx, y + dy)
            if n in hull and n not in seen:
                seen.add(n)
                q.append(n)
    if len(seen) != len(hull):
        d["_checks"].append(f"hull in pieces: {len(hull) - len(seen)} cells cut off from {start}")
    for (x, y), ch in c.cells.items():
        if ch in "EN" and any((xx, y) in hull for xx in range(x - 80, x)):
            d["_checks"].append(f"engine at {(x, y)} vents into hull")
    return d


def up(c, x0, block, y0=0):
    """rows() with the bottom line on row y0 (the keel, usually)."""
    lines = block.strip("\n").split("\n")
    return c.rows(x0, y0 + len(lines) - 1, block)


def unplate_voids(c):
    """Take the plates back off the inside of enclosed voids (the meshes of a
    net, the facets of a star): plates belong on the outline's forward
    faces, not round every window."""
    hull = {p for p, ch in c.cells.items() if ch != "/"}
    xs, ys = [p[0] for p in hull], [p[1] for p in hull]
    x0, x1, y0, y1 = min(xs) - 1, max(xs) + 1, min(ys) - 1, max(ys) + 1
    out, q = {(x0, y0)}, deque([(x0, y0)])
    while q:
        x, y = q.popleft()
        for dx, dy in N4:
            n = (x + dx, y + dy)
            if x0 <= n[0] <= x1 and y0 <= n[1] <= y1 and n not in hull and n not in out:
                out.add(n)
                q.append(n)
    for p in [p for p, ch in c.cells.items() if ch == "/" and p not in out]:
        del c.cells[p]
    return c


def paint(c, runs):
    """[(y, x, "cells"), ...]; spaces skip."""
    for y, x, s in runs:
        c.P(x, y, s)


def cutgem(c, cx, cy, r, cut, ch="#", ymin=None):
    """An octagonal cut gem: max(|dx|,|dy|) <= r and |dx|+|dy| <= cut."""
    for x in range(cx - r, cx + r + 1):
        for y in range(cy - r, cy + r + 1):
            if ymin is not None and y < ymin:
                continue
            if abs(x - cx) + abs(y - cy) <= cut:
                c.cells.setdefault((x, y), ch)


def walk(pts):
    """A 4-connected run of cells along a polyline of float points."""
    out = []
    for (x0, y0), (x1, y1) in zip(pts, pts[1:]):
        n = max(2, int(max(abs(x1 - x0), abs(y1 - y0)) * 3))
        for i in range(n + 1):
            t = i / n
            p = (round(x0 + (x1 - x0) * t), round(y0 + (y1 - y0) * t))
            if out and p == out[-1]:
                continue
            if out and abs(p[0] - out[-1][0]) + abs(p[1] - out[-1][1]) == 2:
                out.append((p[0], out[-1][1]))     # step round the corner
            out.append(p)
    return out


def tube(c, path, r=1, deck="."):
    """A corridor along `path` with hull r cells either side of it."""
    for (x, y) in path:
        for dx in range(-r, r + 1):
            for dy in range(-r, r + 1):
                c.cells.setdefault((x + dx, y + dy), "#")
    for p in path:
        c.cells[p] = deck


def place(c, rooms):
    """Set rooms by hand: {(x, y): letter}."""
    for p, ch in rooms.items():
        c.cells[p] = ch


def interior_of(c):
    hull = {p for p, ch in c.cells.items() if ch != "/"}
    return {p for p in hull if all((p[0] + dx, p[1] + dy) in hull for dx, dy in N4)}


def rim_by_angle(c, test, cx=0, cy=0):
    """The rim cells passing test(x, y), sorted by angle from the bow."""
    inner = interior_of(c)
    cells = [p for p, ch in c.cells.items() if ch == "#" and p not in inner and test(*p)]
    return sorted(cells, key=lambda p: math.atan2(p[1] - cy, p[0] - cx))


def facets(c, xmin=-99, xmax=99, ymin=-99, ymax=99):
    """Plate every forward-facing step in the box, on BOTH faces (hull to its
    left and below, or to its left and above): for a part that has its own
    bow off the centreline, like a pod. Plates on the centreline side and
    the outer side alike."""
    hull = {p for p, ch in c.cells.items() if ch != "/"}
    for x in range(xmin, xmax + 1):
        for y in range(ymin, ymax + 1):
            p = (x, y)
            if p in c.cells or (x - 1, y) not in hull or (x + 1, y) in hull:
                continue
            up_, dn = (x, y + 1) in hull, (x, y - 1) in hull
            if up_ != dn:
                c.cells[p] = "/"


# ---------------------------------------------------------------- warships
def wing(c, y0, y1, root_x, tip_x, w0, w1, p=1.5):
    """A thick crescent wing (upper side): rows from y0 at the root to y1 at
    the tip, curving aft and thinning from half-width w0 to w1, like
    gilded._horn but heavy enough to walk in. Returns {y: (x0, x1)}."""
    rows = {}
    prev = None
    for y in range(y0, y1 + 1):
        u = (y - y0) / (y1 - y0)
        cx = root_x + (tip_x - root_x) * u ** p
        w = w0 + (w1 - w0) * u
        a, b = int(round(cx - w)), int(round(cx + w))
        if prev is not None and b < prev[0] + 1:
            b = prev[0] + 1
        for x in range(a, b + 1):
            c.cells.setdefault((x, y), "#")
        rows[y] = (a, b)
        prev = (a, b)
    return rows


def hostile_takeover():
    """Cruiser. A raptor brooch: a long faceted jewel of a body with two
    pairs of thick crescent wings swept back off it, open sky between them,
    lasers down every leading edge. The crown's centre spike is a spear with
    a railgun down it."""
    c = Canvas("gt_hostile_takeover")
    fore = wing(c, 6, 20, 2, -10, 3.4, 0.8, 1.4)
    aft = wing(c, 5, 25, -9, -34, 4.2, 0.8, 1.5)
    # the body: a long cut jewel, drawn by hand
    up(c, -26, """
                 #s#######s#
              ####.M.......Q####
          E.........O...q........##
         E.R.c.f..m..y..A.T..s..O..##
        E.b..LL..KK..Q..GG..q..b.....#
       E.R..d........d..GG..d.....!!...##
      E.c..R.f.b..H...BB.....T..s..A.....#
     E.........M......BB...........M.......
""")
    # the wings: a hall just behind each leading edge, crew posts beside it
    # (the wings are one hall wide, so posts only). The fore wing is the
    # fire-control wing: gunnery officers at the root, radar mid-wing, a
    # repair bay out by the tip guns. The aft wing is the engine crew's:
    # two reactors and the medbay at the root, berths down the wing, a
    # scrubber and a repair bay near the tip.
    place(c, {(-1, 8): "q", (0, 8): "q", (-1, 9): "O", (-2, 10): "A", (0, 10): "Q",
              (-2, 11): "A", (-3, 12): "y", (-3, 13): "y"})
    place(c, {(-13, 8): "R", (-12, 8): "R", (-10, 8): "m", (-14, 9): "R", (-11, 9): "O",
              (-13, 10): "Q", (-12, 10): "Q", (-14, 11): "Q", (-13, 11): "Q",
              (-15, 12): "Q", (-14, 12): "O", (-16, 13): "Q", (-15, 13): "Q",
              (-17, 14): "O", (-19, 15): "y", (-22, 17): "y"})
    inner = interior_of(c)
    for rows, top in ((fore, 15), (aft, 18)):
        for y, (a, b) in rows.items():
            if y % 3 == 0 and 7 < y <= top and (b - 1, y) in inner:
                c.cells[(b, y)] = "z"
    # the crown: a long centre spear, a shorter spike off each fore facet
    c.P(14, 2, "#z###z##/")
    c.P(14, 1, "........#z##/")
    c.P(14, 0, "............rri")
    c.P(6, 6, "##########/")
    c.P(9, 5, ".......i")
    c.P(11, 4, "#####/")
    _filigree(c, ys=range(1, 30), xmin=-30)
    return finish(c.mirror())


def diadem():
    """Battleship. A tiara seen from above: a crescent band, thick at the brow
    and thinning to its tips, arching forward with seven crown spikes fanned
    out of its front, the centre one longest. The jewel sits in the hollow,
    held by three spokes over open space; the band's tips sweep back into
    horns."""
    c = Canvas("gt_diadem")
    OC, RM = -6, 22.8                        # the band's circle
    OR = RM + 3.5
    band = []
    for x in range(-40, 30):
        for y in range(0, 40):
            r, a = math.hypot(x - OC, y), math.degrees(math.atan2(y, x - OC))
            t = 3.0 + 3.6 * max(0.0, math.cos(math.radians(a))) ** 1.3   # thick brow, thin tips
            if a <= 104 and abs(r - RM) <= t / 2:
                band.append((x, y))
                c.cells[(x, y)] = "#"
    # the crown: seven spikes, centre longest, fanned less than the band
    spikes = {21: 11, 42: 8, 62: 6}
    spine = []
    for ang, ln in spikes.items():
        a = math.radians(ang)
        bx, by = OC + (OR - 2) * math.cos(a), (OR - 2) * math.sin(a)
        d = math.radians(ang * 0.55)
        path = walk([(bx, by), (bx + (ln + 2) * math.cos(d), by + (ln + 2) * math.sin(d))])
        tube(c, path, 1)
        spine += path
        c.cells[path[-1]] = "z" if ang != 42 else "i"
    # spokes from the jewel to the band
    spokes = walk([(-5, 7), (6, 17)])
    tube(c, spokes, 1)
    _horn(c, 22, 32, -12, -34, 1.6, 0.7)
    inner = interior_of(c)
    # the band's rooms: a hall runs round the middle of the band, rooms
    # either side of it, placed by hand.
    # Outer side, at the brow: shields and state rooms; along the sweep:
    # officers and the fire-control radar; out at the tips: crew berths.
    place(c, {(18, 1): "s", (18, 2): "Q", (18, 3): "q", (18, 4): "s", (17, 5): "Q",
              (17, 6): "T", (17, 7): "s", (15, 11): "q", (15, 12): "A", (14, 13): "q",
              (13, 14): "s", (10, 18): "O", (8, 19): "m", (9, 19): "A", (3, 22): "Q",
              (-1, 23): "O"})
    # Inner side, at the brow: the power room behind the crown guns; along
    # the sweep: cooling, fuel and a reactor; at the tips: a prize hold of
    # vaults for whatever the Throne takes.
    place(c, {(16, 1): "R", (16, 2): "c", (15, 3): "R", (15, 4): "b", (15, 5): "b",
              (15, 6): "R", (14, 7): "c", (14, 8): "v", (14, 9): "f", (13, 10): "R",
              (13, 11): "b", (12, 12): "c", (11, 13): "s", (11, 14): "f", (10, 15): "$",
              (9, 16): "$", (8, 17): "b", (5, 19): "c", (3, 20): "R"})
    # lasers and shields walked round the brow's outer rim, clear of the
    # spikes; point defence round its inner rim, facing the hollow
    near = lambda p, d: any(abs(p[0] - q[0]) + abs(p[1] - q[1]) <= d for q in spine)
    brow = sorted([p for p in band if p not in inner and c.cells.get(p) == "#"
                   and math.hypot(p[0] - OC, p[1]) > RM + 1 and not near(p, 3)],
                  key=lambda p: math.atan2(p[1], p[0] - OC))
    for k, p in enumerate(brow):
        if k % 3 == 1 and math.atan2(p[1], p[0] - OC) < 1.45:
            c.cells[p] = "s" if k % 9 == 4 else "z"
    hollow = sorted([p for p in band if p not in inner and c.cells.get(p) == "#"
                     and math.hypot(p[0] - OC, p[1]) < RM - 1 and p[1] > 3],
                    key=lambda p: math.atan2(p[1], p[0] - OC))
    for k, p in enumerate(hollow):
        if k % 5 == 2 and math.atan2(p[1], p[0] - OC) < 1.4:
            c.cells[p] = "p"
    # the centre spike: the longest, a railgun down it and an ion lance
    c.P(20, 3, "#######/")
    c.P(19, 2, "#.....##z##/")
    c.P(20, 1, "..O....#####z##/")
    c.P(20, 0, ".............rri")
    # the keel across the hollow
    c.P(0, 1, "####p###p######").P(0, 0, "." * 16)
    # the jewel, in the band's hollow, with engines down its aft facets
    up(c, -24, """
            #
           #s#
          #.M.#
         #q...q#
        #.O.Q.O.#
       E...www.s.#
      E..m.www.T..#
     E.b...www..y..#
    E.RR..........A.#
   E.......H.GG....q.#
  E.R.f.c....GG.Q.....#
 E..b...d.BB...d.......#
E.........BB............
""")
    for p, ch in [((14, 12), "M"), ((-6, 22), "M")]:
        c.cells[p] = ch
    _filigree(c, ys=range(1, 50), xmin=-6)
    return finish(c.mirror())


def octo(x, y, cx=0, cy=0):
    """Octagon 'radius' (apothem): a regular octagon with flat faces at the
    bow, stern and beams is o(x, y) <= A."""
    dx, dy = abs(x - cx), abs(y - cy)
    return max(dx, dy, (dx + dy) / math.sqrt(2))


def _inside(x, y, poly):
    """Point-in-polygon (even-odd)."""
    n, inside = len(poly), False
    for i in range(n):
        (x0, y0), (x1, y1) = poly[i], poly[(i + 1) % n]
        if (y0 > y) != (y1 > y) and x < x0 + (y - y0) * (x1 - x0) / (y1 - y0):
            inside = not inside
    return inside


def eternal_boardroom():
    """Flagship. The Eternal CEO's boardroom, cut like a brilliant seen from
    above: an octagonal girdle round an eight-point star, open space in
    every facet, and at the heart the table where the ten thousand
    merged board members sit as one. A five-spike crown at the bow, the
    engines across the flat stern facet, two long crescent horns."""
    c = Canvas("gt_eternal_boardroom")
    A, W, T = 24, 4.6, 8.5               # girdle apothem, girdle width, table
    for x in range(-A, A + 1):
        for y in range(0, A + 1):
            if A - W < octo(x, y) <= A:
                c.cells[(x, y)] = "#"
    # the star: eight slender points from the table to the middle of each
    # girdle face, a hall down every point
    pts = []
    for k in range(16):
        r = (A - 2.5) if k % 2 == 0 else 11.5
        pts.append((r * math.cos(math.radians(22.5 * k)), r * math.sin(math.radians(22.5 * k))))
    for x in range(-A, A + 1):
        for y in range(0, A + 1):
            if _inside(x, y, pts) or octo(x, y) <= T:
                c.cells[(x, y)] = "#"
    halls = {}
    for k in range(5):
        u = math.cos(math.radians(45 * k)), math.sin(math.radians(45 * k))
        halls[45 * k] = [p for p in walk([(u[0] * 6, u[1] * 6), (u[0] * (A - 2), u[1] * (A - 2))])
                         if p[1] >= 0]
        tube(c, halls[45 * k], 1)
    tube(c, walk([(18, 13), (23, 16), (25, 16)]), 1)     # the crown's outer spikes
    # the table: the boardroom, drawn by hand. The Eternal CEO is the two
    # AI cores; the merged board sits round them as memory cores, the
    # board's officers in a ring of state rooms, wellness gardens either
    # side of the bridge.
    up(c, -8, """
    #########
   #qsq.s.qsq#
  #q.M.....M.q#
 #m.q.q.T.q.q.O#
#.V..@@...@@..V.#
#.R.q.q...q.q.R.#
#.M....H......M.#
#.www..BB..www..#
..www..BB..www...
""")
    # each star point is a wing of the palace: a hall down its axis, rooms
    # either side of it, each wing with its own purpose
    # bow point: power for the crown guns
    place(c, {(11, 1): "R", (12, 1): "b", (13, 1): "c", (15, 1): "R", (16, 1): "v"})
    # fore diagonal points: the officers' wing
    place(c, {(12, 13): "q", (11, 12): "q", (14, 12): "O", (10, 11): "Q", (13, 11): "s",
              (12, 10): "m", (8, 9): "q", (11, 9): "A", (7, 8): "Q", (10, 8): "O", (9, 7): "s"})
    # beam points: the shareholders' gallery, minds on ice down one side of
    # the hall, their deeds in vaults and their keepers down the other
    place(c, {(-1, 11): "V", (-1, 12): "V", (-1, 13): "$", (-1, 15): "V", (-1, 16): "V",
              (1, 11): "$", (1, 12): "q", (1, 13): "V", (1, 14): "Q", (1, 15): "$", (1, 16): "q"})
    # aft diagonal points: guest apartments
    place(c, {(-12, 13): "A", (-14, 12): "s", (-11, 12): "Q", (-13, 11): "b", (-10, 11): "Q",
              (-12, 10): "O", (-11, 9): "q", (-8, 9): "m", (-10, 8): "s", (-7, 8): "Q", (-9, 7): "b"})
    # stern point: the reactors that feed the engine bank
    place(c, {(-16, 1): "R", (-15, 1): "R", (-13, 1): "c", (-12, 1): "f", (-11, 1): "R"})
    # the girdle: a hall round its middle. Outer side, behind the guns: on
    # the beam face the board members' state rooms, down the diagonals the
    # gun crews' berths, on the bow face the gunners, repair and radar.
    place(c, {(-9, 23): "A", (-8, 23): "q", (-6, 23): "q", (-5, 23): "O", (-4, 23): "q",
              (-2, 23): "q", (-1, 23): "m", (0, 23): "q", (1, 23): "m", (2, 23): "q",
              (4, 23): "q", (5, 23): "O", (6, 23): "q", (8, 23): "q", (9, 23): "A",
              (-10, 22): "Q", (-12, 20): "O", (-13, 19): "Q", (-15, 17): "Q", (-17, 15): "A",
              (-18, 14): "Q", (-20, 12): "m",
              (10, 22): "Q", (11, 21): "y", (12, 20): "Q", (14, 18): "Q", (15, 17): "O",
              (17, 15): "A", (18, 14): "Q", (20, 12): "y", (21, 11): "Q",
              (23, 9): "Q", (23, 8): "y", (23, 7): "Q", (23, 6): "A", (23, 5): "Q",
              (23, 4): "O", (23, 3): "Q", (23, 2): "y", (23, 1): "Q"})
    # Inner side: the Board's treasury along the beam face, shields and
    # power down the diagonals, the bow guns' batteries on the bow face.
    place(c, {(-9, 21): "V", (-8, 21): "$", (-7, 21): "V", (-6, 21): "$", (-5, 21): "s",
              (-4, 21): "V", (-3, 21): "$", (-2, 21): "V", (-1, 21): "$", (1, 21): "$",
              (2, 21): "V", (3, 21): "$", (4, 21): "V", (5, 21): "s", (6, 21): "$",
              (7, 21): "V", (8, 21): "$", (9, 21): "V",
              (-10, 20): "s", (-11, 19): "b", (-12, 18): "c", (-13, 17): "s", (-14, 16): "b",
              (-16, 14): "f", (-17, 13): "s", (-18, 12): "c", (-19, 11): "b", (-20, 10): "f",
              (10, 20): "s", (11, 19): "b", (12, 18): "s", (13, 17): "c", (14, 16): "s",
              (16, 14): "b", (17, 13): "s", (18, 12): "c", (19, 11): "b", (20, 10): "s",
              (21, 9): "s", (21, 8): "b", (21, 7): "c", (21, 6): "b", (21, 5): "s",
              (21, 4): "b", (21, 3): "c", (21, 2): "b", (21, 1): "s"})
    for p in [(-21, 9), (21, 9), (-9, 21), (9, 21)]:
        c.cells[p] = "M"
    # the girdle's guns, walked round its outer rim
    outer = rim_by_angle(c, lambda x, y: A - 1 < octo(x, y) <= A and x > -A + 1 and y >= 0
                         and math.hypot(x - 20, y - 15) > 4)
    for k, p in enumerate(outer):
        if k % 4 == 2:
            c.cells[p] = "i" if k % 16 == 6 else "z"
    # the crown: five spikes, the centre one longest
    c.P(A + 1, 2, "#######/")
    c.P(A + 1, 1, "...T....#z###z#/")
    c.P(A + 1, 0, "...............rri")
    c.P(A + 1, 7, "#######/")
    c.P(A + 1, 6, ".......z")
    c.P(A + 1, 5, "#######/")
    c.cells[(26, 16)] = "z"
    # engines across the flat stern facet
    for y in range(0, 10):
        c.cells[(-A, y)] = "E"
        c.cells[(-A + 1, y)] = "."
    _horn(c, 13, 36, -19, -48, 2.6, 0.8)
    _filigree(c, ys=range(1, 40), xmin=-10)
    return finish(unplate_voids(c.mirror()))


# ---------------------------------------------------------------- experimental
def escrow():
    """Experimental: consciousness vault. A cut gem worked into a jewelled
    net: a trellis of girders inside a diamond rim, open sky in every mesh,
    and at every knot a strongroom of stored minds. The 1% fee is kept
    here. Bridge in the centre knot, engines down the aft facets, a crown
    and two short horns."""
    c = Canvas("gt_escrow")
    R, S = 25, 12                       # rim (|x|+|y|), trellis spacing
    for x in range(-R, R + 1):
        for y in range(0, R + 1):
            d = abs(x) + abs(y)
            if d > R:
                continue
            u, v = (x + y) % S, (x - y) % S
            on = min(u, S - u) <= 2 or min(v, S - v) <= 2
            if d >= R - 3 or on:
                c.cells[(x, y)] = "#"
    # the knots: every crossing swells into a small cut gem, a strongroom
    # with a mind at its heart, cryo either side and vaults fore and aft
    knots = [(kx, ky) for kx in range(-24, R + 1, 6) for ky in range(0, R + 1, 6)
             if not (kx + ky) % S and not (kx - ky) % S and abs(kx) + abs(ky) <= R - 6]
    for kx, ky in knots:
        for dx in range(-3, 4):
            for dy in range(-3, 4):
                if abs(dx) + abs(dy) <= 3:
                    c.cells[(kx + dx, ky + dy)] = "#"
    for kx, ky in knots:
        if (kx, ky) == (0, 0):
            continue
        c.cells[(kx, ky)] = "M" if ky else "y"
        c.cells[(kx - 2, ky)] = "V"
        c.cells[(kx + 2, ky)] = "V"
        if ky:
            c.cells[(kx, ky + 2)] = "$"
            c.cells[(kx, ky - 2)] = "$"
    # the heart: bridge and reactors in the centre knot
    cutgem(c, 0, 0, 5, 5)
    up(c, -4, """
   #s#
  #.O.#
 #..H..#
.R.BB.R.
""", 1)
    c.P(-4, 0, ".m.BB.T.")
    # the girders: a hall zig-zags down the middle of each, rooms along one
    # side of it; the rim: a hall just inside the facets
    # girders, top: the cold store of minds aft, a vault and its warden fore
    place(c, {(-4, 17): "V", (-3, 16): "$", (-2, 15): "V", (5, 16): "V", (4, 15): "$",
              (3, 14): "Q"})
    # girders, middle: minds aft, the custodians round the centre, crew fore
    place(c, {(-10, 11): "V", (-9, 10): "$", (-8, 9): "V", (3, 10): "q", (4, 9): "$",
              (-2, 9): "Q", (-3, 8): "k", (11, 10): "O", (10, 9): "Q", (9, 8): "k"})
    # girders, low: reactors aft by the engines, berths by the centre, the
    # receiving vault forward where the minds come aboard
    place(c, {(-16, 5): "R", (-15, 4): "R", (-14, 3): "O", (-3, 4): "Q", (4, 3): "q",
              (-8, 3): "k", (-9, 2): "Q", (9, 4): "V", (10, 3): "$", (15, 2): "V",
              (16, 3): "Q", (17, 4): "q"})
    # the rim (crew posts only: its hall steps through them down the
    # facets): the engine room's reactors and berths aft, the gunners,
    # repair and radar forward
    place(c, {(-21, 2): "R", (-19, 4): "R", (-17, 6): "R", (-15, 8): "Q", (-13, 10): "Q",
              (-11, 12): "m", (-9, 14): "Q", (-7, 16): "Q", (-5, 18): "O",
              (4, 19): "A", (6, 17): "q", (8, 15): "Q", (10, 13): "y", (12, 11): "q",
              (14, 9): "Q", (16, 7): "O", (18, 5): "y"})
    # the rim's fittings: shields and lasers on the forward facets, PD
    # drones aft, engines down the aft facets
    for y in range(0, R + 1):
        x = R - y
        if y % 4 == 2 and 2 < y < R - 2:
            c.cells[(x, y)] = "z" if y % 8 == 2 else "s"
        xa = -(R - y)
        if y <= 9:
            c.cells[(xa, y)] = "E"
        elif y % 4 == 1 and y < R - 2:
            c.cells[(xa, y)] = "p"
    # the crown: a long centre spike off the bow point, a shorter one off
    # each forward facet
    c.P(R - 3, 2, "######/")
    c.P(R - 2, 1, "########z###/")
    c.P(R - 1, 0, "...........rri")
    tube(c, walk([(16, 8), (22, 11), (26, 11)]), 1)
    c.cells[(27, 11)] = "z"
    _horn(c, 11, 24, -15, -34, 2.0, 0.7)
    _filigree(c, ys=range(1, 30), xmin=-6)
    return finish(unplate_voids(c.mirror()))


def futures_pit():
    """Experimental: mind exchange. A locket worn at the bow: a ring round an
    open pit, docking ports all round its inner face, where a client ship
    docks and sells what it is. Surgical theatres take the minds out, the
    cryo racks and cores hold them while they trade, and the bridge sits in
    the jewel behind, where the price is set. Horns from the ring's back
    sweep past the jewel."""
    c = Canvas("gt_futures_pit")
    RX, RO, RI = 21, 15.4, 9.4          # the ring
    for x in range(RX - 16, RX + 17):
        for y in range(0, 17):
            r = math.hypot(x - RX, y)
            if RI <= r <= RO:
                c.cells[(x, y)] = "#"
    inner = interior_of(c)
    rim = [p for p in c.cells if p not in inner]
    ang = lambda p: math.atan2(p[1], p[0] - RX)
    pit = sorted([p for p in rim if math.hypot(p[0] - RX, p[1]) < RI + 1], key=ang)
    out = sorted([p for p in rim if math.hypot(p[0] - RX, p[1]) > RO - 1], key=ang)
    for k, p in enumerate(pit):          # docking ports all round the pit
        if k % 3 == 1:
            c.cells[p] = "o"
    for k, p in enumerate(out):          # shields and lasers outside
        if k % 4 == 2 and 0.3 < ang(p) < 2.6:
            c.cells[p] = "z" if k % 8 == 2 else "s"
    # round the ring: the sale floor forward (vaults of minds already
    # sold, a core keeping the book), the surgeons' cabins at the top by
    # their theatres, and the intake aft, rack after rack of cryo
    place(c, {(33, 1): "$", (33, 3): "$", (32, 5): "V", (31, 7): "M", (30, 8): "V",
              (29, 9): "$", (28, 10): "V", (26, 11): "$", (24, 12): "V", (22, 12): "q",
              (18, 12): "q", (16, 11): "V", (14, 10): "V", (13, 9): "V", (12, 8): "V",
              (11, 7): "V", (10, 5): "M", (9, 3): "V", (9, 1): "V"})
    # surgical theatres at the top and bottom of the ring
    c.R(19, 21, 11, 12, "S")
    # the jewel: the bridge, where the price is set
    up(c, -12, """
          #
         #s#
        #.M.#
       E.q.q.#
      E.R...O.#
     E.b..T..m.#
    E.RR..!!...A#
   E.f....H.......
  E.....d.....d....
 E.c.R....BB.........
""", 1)
    c.P(-12, 0, "E..M.d....BB............")
    # the crown at the front of the ring: a spike off each forward shoulder
    u = math.cos(math.radians(30)), math.sin(math.radians(30))
    path = walk([(RX + 13 * u[0], 13 * u[1]), (RX + 17.5 * u[0], 13 * u[1] + 1.5),
                 (RX + 20 * u[0], 13 * u[1] + 1.5)])
    tube(c, path, 1)
    c.cells[path[-1]] = "z"
    c.P(RX + 14, 2, "####/")
    c.P(RX + 15, 1, "#z###/")
    c.P(RX + 15, 0, "....rri")
    _horn(c, 10, 26, 8, -22, 2.2, 0.8)
    _filigree(c, ys=range(1, 30), xmin=-4)
    return finish(c.mirror())


# ---------------------------------------------------------------- traders
def cufflink():
    """Trader, small. A pair of cut-gem strongboxes worn port and starboard
    like cufflinks, linked through a small jewel that holds the bridge. Each
    strongbox carries a crown spike and a short horn."""
    c = Canvas("gt_cufflink")
    paint(c, [                       # the strongbox, centred on y = 9
        (12, -3, "########"),
        (11, -5, "##kV.M.Vk###"),
        (10, -7, "E.$.$.....$.$..#"),
        (9, -8, "E................z"),
        (8, -7, "E.$.$.....$.$..#"),
        (7, -5, "##kV...Vk###"),
        (6, -3, "###.####"),
    ])
    up(c, -5, """
    #.#
    #.#
   #O.Q#
  E.m.H.#
 E.RBB...#
E.b.BB.M......rri
""")
    c.P(5, 1, "#z##/")
    _horn(c, 11, 16, -4, -12, 1.5, 0.6)
    facets(c, xmin=0, ymin=5, ymax=13)
    _filigree(c, ys=range(1, 5), xmin=0)
    return finish(c.mirror())


def solitaire():
    """Trader, large. A solitaire: one great cut gem of a cargo hold, held at
    the bow in a prong setting. The setting is a crescent cup round the
    gem's back with windows to space between them, its prongs reaching
    forward past the gem as a crown. The bridge sits where the setting meets
    the shank, the engines down the shank's aft facets."""
    c = Canvas("gt_solitaire")
    GX, G = 14, 12                       # the gem
    for x in range(GX - G, GX + G + 1):
        for y in range(0, G + 1):
            if abs(x - GX) + y <= G:
                c.cells[(x, y)] = "#"
    # the setting: a crescent cup round the gem's back, prongs forward
    for x in range(GX - 18, GX + 18):
        for y in range(0, 18):
            r, a = math.hypot(x - GX, y), math.degrees(math.atan2(y, x - GX))
            if 13.2 <= r <= 16.6 and a >= 38:
                c.cells.setdefault((x, y), "#")
    # the gem's hold, drawn by hand: bulk holds either side of a cross
    # hall, crates and long cargo above them, and at the top, under the
    # table of the gem, the vaults and the few minds it carries
    up(c, 2, """
            #
           #V#
          #$V$#
         #$.V.$#
        #.k.V.k.#
       #k.UU.UU.k#
      #..k.k.k.k..#
     #.CC.CC.CC.CC.#
    #k.CC.CC.CC.CC.k#
   #.................#
  #k.CC.CC.CC.CC.CC.k.#
 #k.kCC.CC.CC.CC.CC.k..z
.........................
""")
    # the setting's cup is one hall wide, so crew posts only: guards'
    # berths down its back by the shank, the cup's reactors at the top,
    # gunners, repair and radar out in the prongs
    place(c, {(0, 3): "Q", (0, 5): "Q", (1, 6): "q", (2, 8): "Q", (3, 9): "Q",
              (3, 10): "O", (3, 11): "m", (5, 12): "O", (6, 13): "Q", (8, 14): "y",
              (10, 14): "O", (12, 15): "R", (14, 15): "R", (16, 15): "y",
              (18, 14): "q", (21, 14): "q", (22, 13): "O", (24, 12): "A", (25, 11): "y",
              (25, 10): "A"})
    rim = rim_by_angle(c, lambda x, y: math.hypot(x - GX, y) > 15.6 and y >= 4, GX, 0)
    for k, p in enumerate(rim):
        if k % 7 == 3:
            c.cells[p] = "z" if k % 14 == 3 else "s"
    # the shank: bridge, quarters and engines
    up(c, -14, """
           ##
          #sM#
         #q..q#
        E.O..O.#
       E.R.mT..y.
      E.b..GG..s..
     E.R.f.GG.H...
    E.c...d....d...
   E.R.b...BB......
""", 1)
    c.P(-12, 0, "E..M.....BB......")
    c.P(GX + G, 1, "#/")
    c.P(GX + G + 1, 0, "i")
    _horn(c, 9, 22, -6, -28, 2.0, 0.7)
    _filigree(c, ys=range(1, 30), xmin=-8)
    return finish(c.mirror())


# ---------------------------------------------------------------- platform
def velvet_rope():
    """Defense platform. A crown seen from straight above: an octagonal
    circlet round an open centre with eight spikes, the four on the axes
    long, the four on the diagonals short, every tip plated. They ring the
    Exchange and turn away anyone whose standing isn't high enough. One
    eighth drawn, reflected across the diagonal and turned four ways."""
    c = Canvas("gt_velvet_rope")
    c.rows(0, 5, """
     /
    ##
   Rz
  #Q.s
   #..#z#/
   #M....i
""")
    for (x, y), ch in list(c.cells.items()):
        if 0 < y < x:
            c.cells[(y, x)] = ch
    c.rot4()
    return finish(c)


CONCEPTS = [
    ("hostile_takeover", hostile_takeover, "Hostile Takeover", "Cruiser",
     "A raptor brooch: a long faceted jewel of a body with two pairs of thick crescent wings swept back off it, open sky between them and lasers down every leading edge. The crown's centre spike is a spear with a railgun down it."),
    ("diadem", diadem, "Diadem", "Battleship",
     "A tiara seen from above: a broad crescent band arching forward with seven crown spikes fanned out of its front. The jewel sits in the band's hollow on three spokes over open space."),
    ("eternal_boardroom", eternal_boardroom, "Eternal Boardroom", "Flagship",
     "The Eternal CEO's boardroom, cut like a brilliant seen from above: an octagonal girdle round an eight-point star, open space in every facet, and the table of ten thousand merged board members at the heart."),
    ("escrow", escrow, "Escrow", "Experimental: consciousness vault",
     "A cut gem worked into a jewelled net: a trellis of girders inside a diamond rim, open sky in every mesh, and a strongroom of stored minds at every knot. The 1% they keep of every upload is kept here."),
    ("futures_pit", futures_pit, "Futures Pit", "Experimental: mind exchange",
     "A locket worn at the bow: a ring round an open pit, lined with docking ports where client ships dock and sell what they are. Surgical theatres take the minds out; the bridge in the jewel behind sets the price."),
    ("cufflink", cufflink, "Cufflink", "Trader: share courier",
     "Two cut-gem strongboxes worn port and starboard like cufflinks, linked through a small jewel that holds the bridge. It carries share certificates and a few stored minds between the Throne's exchanges."),
    ("solitaire", solitaire, "Solitaire", "Trader: bullion carrier (large)",
     "One great cut gem of a cargo hold, held at the bow in a prong setting: a crescent cup round the gem's back, windows to space between, prongs reaching past it as a crown. The bridge sits where the setting meets the shank."),
    ("velvet_rope", velvet_rope, "Velvet Rope", "Defense platform",
     "A crown seen from straight above: an octagonal circlet round an open centre, with eight plated spikes. They ring the Exchange and turn away anyone whose standing isn't high enough."),
]
