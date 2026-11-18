"""The Silence, round 5b: the great ships, the mother house, two experimentals
and three traders. Hand-drawn, top-down only.

Lore: they found perfect peace by removing ambition; they lobotomised their
entire species. They cannot start a fight, and what hits them is reflected.
They carry no weapons at all: they shadow you at a distance, flee when shot,
and are the fastest thing in the sky. Their ships listen, record and hide.

Every hull here speaks the approved grammar (silence.py FACTION["grammar"]):
needles 3-5 cells wide with a plated point at the bow, single-cell outrigger
rails on one-cell struts carrying the passive sensors, feathered sterns, one
keel corridor with a few quiet rooms and stealth coating in the walls at an
even beat, perfect port/starboard symmetry, Composite, no weapons. The big
ships are several needles held together by yokes, struts and rails, with open
space between them; never a solid block.

Legend (draw.py): # hull  . deck  / plate  E engine  R reactor  L large reactor
b battery  f fuel  c cooling  B bridge  H helm  M memory core  s shield
A radar  W advanced radar  ? passive radar  I thermal imager  J signal buoy
= signal jammer  ~ stealth coating  Q quarters  q officer quarters  K barracks
G galley  S surgery  m medbay  O oxygen  V cryo  $ vault  Y specimen vault
! research lab  y repair  + force field  C bulk cargo  U large cargo
k small cargo  d door
"""
import math
from collections import deque

from draw import Canvas
from silence import FACTION  # same faction, same page

N4 = ((0, 1), (0, -1), (1, 0), (-1, 0))
NO_WEAPONS = set("ngrxzpiej*^P&tTa")      # guns, launchers, tractor, targeting, ammo
_last_cells = {}                           # key -> drawn cells (for previews)


# --------------------------------------------------------------------------- helpers
def finish(c):
    """Build, then check what draw.py doesn't: the hull is one piece, its
    skeleton is mirror-symmetric, no engine fires into its own hull, nothing
    is armed, and the bridge sits inside the hull, never on its rim."""
    d = c.build()
    _last_cells[c.name[3:]] = dict(c.cells)
    hull = {tuple(h["grid_pos"]) for h in d["hull_cells"]}
    layer = {tuple(h["grid_pos"]): h["layer"] for h in d["hull_cells"]}
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
    if {(x, -y) for x, y in hull} != hull:
        d["_checks"].append("skeleton not mirror-symmetric")
    for (x, y), ch in c.cells.items():
        if ch in "EN" and any((xx, y) in hull for xx in range(x - 80, x)):
            d["_checks"].append(f"engine at {(x, y)} vents into hull")
        if ch in NO_WEAPONS:
            d["_checks"].append(f"armed: '{ch}' at {(x, y)}")
        if ch == "B" and layer.get((x, y)) == "Outer":
            d["_checks"].append(f"bridge on the rim at {(x, y)}")
    return d


def wall(c, x0, x1, y, beat=4, phase=0):
    """A wall run of hull with stealth coating set in at an even beat."""
    for x in range(x0, x1 + 1):
        c.cells[(x, y)] = "~" if (x - phase) % beat == 0 else "#"


def deck(c, x0, x1, y):
    for x in range(x0, x1 + 1):
        c.cells[(x, y)] = "."


def bead(c, xc, top, a, y0=1):
    """An octagonal bead swelling out of a keel: rows y0..top, half-length
    a + (top - y), 45-degree shoulders. Rim '#', inside '.'."""
    for y in range(y0, top + 1):
        h = a + top - y
        for x in range(xc - h, xc + h + 1):
            rim = y == top or abs(x - xc) == h
            c.cells[(x, y)] = "#" if rim else "."


def ring(c, cx, cy, r0, r1, ch="#", keep=lambda x, y: True):
    """Every cell whose centre lies between radius r0 and r1 of (cx, cy)."""
    for x in range(int(cx - r1) - 1, int(cx + r1) + 2):
        for y in range(int(cy - r1) - 1, int(cy + r1) + 2):
            if r0 <= math.hypot(x - cx, y - cy) <= r1 and keep(x, y):
                c.cells[(x, y)] = ch


def halo_cell(r0, r1, ang, side, cx=0, cy=0):
    """The cell of a ring's walk nearest a bearing (degrees, 0 = bow), on its
    inner side ('i'), its outer side ('o') or the walk itself ('c'), so rooms
    can be set round a ring by hand without counting cells."""
    best = None
    for x in range(int(cx - r1) - 1, int(cx + r1) + 2):
        for y in range(int(cy - r1) - 1, int(cy + r1) + 2):
            r = math.hypot(x - cx, y - cy)
            if not r0 <= r <= r1 or not all(r0 <= math.hypot(x + dx - cx, y + dy - cy) <= r1
                                            for dx, dy in N4):
                continue
            band = "i" if r < r0 + 1.8 else "o" if r > r1 - 1.8 else "c"
            if band != side:
                continue
            d = abs((math.degrees(math.atan2(y - cy, x - cx)) - ang + 180) % 360 - 180)
            if best is None or d < best[0]:
                best = (d, (x, y))
    return best[1]


# --------------------------------------------------------------------------- ships
def psaltery():
    """Great listening array. A keel threaded through a lens of strings:
    four single-cell rails a side, strung on three masts, longest next to the
    hull and shortest outside, so the whole array makes an ellipse. Every
    rail ends in a passive ear and carries advanced radar along its length.
    The bridge is in a bead amidships, under the tallest mast; a thin radar
    spar runs on to the point. Lore: the rails ring in sympathy with every
    drive in the system, like the strings of an instrument nobody plays."""
    c = Canvas("si_psaltery", "Composite")
    # the keel, 5 wide, feathered stern
    deck(c, -48, 50, 0)
    c.P(-48, 0, "E")
    c.P(-47, 1, "E" + "." * 81)
    wall(c, -45, 34, 2, beat=4, phase=-1)
    c.P(-45, 2, "E")
    c.P(-44, 1, "R.b.R..O.Q..m..Q.O..y")
    c.P(10, 1, "Q.O..Q..y..R.b.R")
    for x in (-32, -16, 16):
        c.cells[(x, 2)] = "M"
    # the bead amidships, with the bridge
    bead(c, 0, 4, 2)
    c.P(-2, 4, "#~#~#")
    c.P(-3, 3, "#.s.s.#")
    c.P(-4, 2, "#.......#")
    c.P(-5, 1, "..m.BB.m...")
    c.P(-1, 0, "BB.H")
    # the radar spar, 3 wide, to a plated point
    wall(c, 35, 49, 1, beat=4, phase=3)
    for x in (37, 41, 45):
        c.cells[(x, 1)] = "A"
    c.cells[(50, 1)] = "/"
    c.P(50, 0, "A?")
    # the strings: (row, half-length); ears at both ends, radar along them
    for y, half in ((6, 32), (9, 29), (12, 25), (15, 17)):
        for x in range(-half, half + 1):
            c.cells[(x, y)] = "W" if x % 8 == 4 else "#"
        c.cells[(-half, y)] = c.cells[(half, y)] = "?"
    # the masts: the tallest stands on the bead on a one-cell strut
    for x, y0, top in ((0, 5, 17), (-16, 3, 15), (16, 3, 15)):
        for y in range(y0, top + 1):
            if (x, y) not in c.cells:
                c.cells[(x, y)] = "#"
    c.cells[(0, 17)] = "?"
    c.cells[(0, 16)] = "J"
    return finish(c.mirror())


def motherhouse():
    """Flagship: the order's mother house, and the monastery set moving. The
    same still halo, a thin ring with nothing in it but a bead hung on the
    keel, and the same eight needles: the keel runs through it as the bow
    lance and the stern, two long needles stand out to port and starboard
    with radar at their tips, two short ones lean forward with passive ears,
    and the two aft diagonals are wide fletching, stepped with engines all
    the way out. The abbess's bridge is in the bead at the dead centre."""
    c = Canvas("si_motherhouse", "Composite")
    # the halo: rim, then the walk inside it
    ring(c, 0, 0, 15.6, 20.6, keep=lambda x, y: y >= 0)
    # the keel: stern, through the halo, out to the bow lance
    deck(c, -52, 72, 0)
    deck(c, -51, 44, 1)
    wall(c, -49, 44, 2, beat=4, phase=-1)
    # stern, feathered
    c.P(-52, 0, "E")
    c.P(-51, 1, "E..R.LL.b.R..O.f.c.O..Q.m.Q..")
    c.P(-49, 2, "E")
    c.cells[(-37, 2)] = c.cells[(-29, 2)] = "M"
    # the bead at the dead centre: the abbess's cell
    bead(c, 0, 6, 3, y0=3)
    c.P(-3, 6, "#~#J#~#")
    c.P(-4, 5, "#.M...M.#")
    c.P(-5, 4, "#.q.....q.#")
    c.P(-6, 3, "#..s.....s..#")
    c.P(-6, 2, ".!!......!!.")
    c.P(-8, 1, ".Q..m..BB..m..Q.")
    c.P(-1, 0, "BB.H")
    # bow lance: 5 wide to a shoulder, then a radar spar with rails
    c.P(22, 1, "Q.O..y..Q..s..Q.O..R.b")
    c.cells[(28, 2)] = c.cells[(40, 2)] = "M"
    wall(c, 45, 70, 1, beat=4, phase=1)
    for x in range(47, 70, 4):
        c.cells[(x, 1)] = "A"
    c.cells[(71, 1)] = "/"
    c.P(71, 0, "A?")
    c.P(46, 3, "?####I####I####I####?")
    c.cells[(47, 2)] = c.cells[(65, 2)] = "#"
    # side needles, 3 wide, radar at the tips
    for y in range(18, 37):
        c.cells[(-1, y)] = c.cells[(1, y)] = "~" if y % 4 == 0 else "#"
        c.cells[(0, y)] = "."
    c.P(0, 37, "A").P(0, 38, "?").P(-1, 37, "/").P(1, 37, "/")
    # fore diagonals: passive ears; aft diagonals: fletching stepped with engines
    for x in range(0, 40):
        for y in range(0, 40):
            if abs(x - y) <= 1 and x + y >= 30 and max(x, y) <= 23:
                c.cells.setdefault((x, y), "#")
                if x == y and x % 3 == 0:
                    c.cells[(x, y)] = "~"
            if abs(x - y) <= 2 and 28 <= x + y <= 60:
                c.cells.setdefault((-x, y), "." if abs(x - y) <= 1 and x + y <= 58 else "#")
    c.cells[(23, 23)] = "?"
    c.cells[(22, 22)] = "I"
    for y in range(20, 32):            # the aft-most cell of every row of the fletching
        x = min(x for (x, yy) in c.cells if yy == y)
        c.cells[(x, y)] = "E"
    # stern outriggers to answer the bow's
    c.P(-47, 5, "?####I####I####?")
    c.cells[(-46, 3)] = c.cells[(-46, 4)] = c.cells[(-33, 3)] = c.cells[(-33, 4)] = "#"
    # rooms round the halo walk, set by bearing on its inner or outer side;
    # one quadrant drawn, fore and aft the same
    rooms = [(12, "o", "Q"), (12, "i", "b"), (20, "i", "R"), (26, "o", "Q"), (32, "i", "O"),
             (38, "o", "m"), (52, "o", "Q"), (58, "i", "s"), (64, "o", "R"), (70, "i", "Q"),
             (76, "o", "O"), (82, "i", "M"), (17, "o", "M")]
    for ang, side, ch in rooms:
        for a in (ang, 180 - ang):
            c.cells[halo_cell(15.6, 20.6, a, side)] = ch
    # stealth coating on the halo's outer rim at an even beat
    for (x, y), ch in list(c.cells.items()):
        r, a = math.hypot(x, y), math.degrees(math.atan2(y, x))
        if ch == "#" and 19.8 <= r <= 20.6 and y >= 4 and round(a) % 15 == 0:
            c.cells[(x, y)] = "~"
    return finish(c.mirror())


def still_water():
    """Experimental: the mirror. Lore: what hits the Silence is reflected;
    this is that idea built whole. A dish concave to the bow, its inner face
    set with shield emitters at an even beat, the needle run through its
    centre and a receiver bead hung at its focus. The dish's back is the
    feathered stern: engines step forward along the curve."""
    c = Canvas("si_still_water", "Composite")
    # the crescent: inside the back circle, outside the face circle
    R, xi, ri = 20.4, 10.0, 18.0          # back circle, face circle (centre x, radius)
    for x in range(-22, 16):
        for y in range(0, 22):
            ro, rf = math.hypot(x, y), math.hypot(x - xi, y)
            if ro <= R and rf >= ri and (ro >= R - 5 or rf <= ri + 3):
                c.cells[(x, y)] = "#"
            if ro <= R and rf >= ri and rf <= ri + 0.9 and (x * 7 + y * 3) % 4 == 0:
                c.cells[(x, y)] = "s"          # the mirror face
    # the keel across the void, the bridge in it, a stalk to the receiver
    deck(c, -19, -7, 0)
    c.P(-19, 1, "...........")
    c.P(-17, 2, "##~#####")
    c.P(-14, 1, "BB.")
    c.P(-14, 0, "BB.H")
    deck(c, -6, 5, 0)
    c.P(-7, 1, "#~###.W.W.###/")      # the stalk, swelling into the receiver
    c.P(-2, 2, "#.I.#")
    c.P(-1, 3, "#?#")
    c.P(6, 0, "A?")
    # rooms round the back arc, by bearing
    for ang, side, ch in ((168, "i", "R"), (160, "o", "Q"), (150, "i", "O"), (142, "o", "M"),
                          (132, "i", "Q"), (122, "o", "b"), (112, "i", "m"), (102, "o", "Q"),
                          (94, "i", "M"), (84, "o", "R")):
        c.cells[halo_cell(R - 5, R, ang, side)] = ch
    # engines stepped forward along the back curve
    for y in range(0, 13, 2):
        x = min(x for (x, yy) in c.cells if yy == y)
        c.cells[(x, y)] = "E"
    return finish(c.mirror())


def abbey():
    """Abbey ship. The plan of an abbey seen from above: a long nave, a
    transept crossing it near the bow with a plated point and radar at each
    end, and aft of the transept two cloister courts, open to the sky, framed
    by the dormitory range and the west range. The monks live round the
    courts; the bridge is in the nave between them."""
    c = Canvas("si_abbey", "Composite")
    # nave
    deck(c, -50, 45, 0)
    deck(c, -49, 44, 1)
    wall(c, -47, 43, 2, beat=4, phase=-1)
    c.P(-50, 0, "E").P(-49, 1, "E").P(-47, 2, "E")
    c.cells[(44, 2)] = "/"
    c.cells[(45, 1)] = "/"
    c.P(45, 0, "A?")
    # transept, 5 wide, plated points with radar
    for y in range(3, 25):
        c.P(12, y, "~" if y % 4 == 1 else "#")
        c.P(13, y, "...")
        c.P(16, y, "~" if y % 4 == 1 else "#")
    c.P(12, 24, "/").P(16, 24, "/").P(13, 25, "/A/").P(14, 26, "?")
    c.P(13, 24, "#.#")
    # the cloister walks: thin rails round each court, corners cut at 45
    # degrees, passive ears where they turn
    for x in range(-14, 12):
        c.cells[(x, 17)] = "#"
    for y in range(3, 12):
        c.cells[(-20, y)] = "#"
    for k in range(6):                      # the cut corner, a two-cell stair
        c.cells[(-20 + k, 12 + k)] = "#"
        c.cells[(-19 + k, 12 + k)] = "#"
    c.cells[(-15, 17)] = "#"
    c.cells[(-20, 11)] = "?"
    c.cells[(-14, 17)] = "?"
    # the west front: two short towers flanking the stern on yokes, their
    # engines stepped forward of the nave's
    c.rows(-49, 10, """
   E~###~##
 E..R.Q..O#
E..........?
 E..b.M..y#
   E~###~##
""")
    for y in (3, 4, 5):
        c.P(-44, y, "#.#")
    # rooms: the nave
    c.P(-48, 1, ".R.b.LL.R..O..f.c..O.m..Q.Q.")
    c.P(-18, 1, "Q..q..GG..m..BB..m..GG..q..Q.")
    c.P(-5, 0, "BB.H")
    c.P(-12, 2, "GG").P(2, 2, "GG")
    c.P(17, 1, "..s..!!..Y..!!..s..........")
    for x in (-36, -28, 24, 34):
        c.cells[(x, 2)] = "M"
    for x in (32, 36, 40):
        c.cells[(x, 2)] = "A"
    # rooms: the transept arms, the monks' cells
    for y, a, b in ((4, "Q", "Q"), (6, "O", "m"), (8, "Q", "Q"), (10, "R", "b"), (12, "Q", "Q"),
                    (14, "y", "O"), (16, "Q", "Q"), (18, "M", "s"), (20, "Q", "Q"), (22, "I", "I")):
        c.cells[(13, y)] = a
        c.cells[(15, y)] = b
    return finish(c.mirror())


def lancet():
    """Experimental: the quieting ship. A lancet window seen from above: two
    jambs rising into a pointed arch at the bow, a mullion down the middle
    (the keel) and a thin sill at the stern, so the ship is two pointed
    lights of open space framed in hull. Lore: the Silence removed ambition
    from their own species, and this is the ship that did it; it still
    offers the same peace to anyone it finds alone. A lancet is also the
    surgeon's knife."""
    c = Canvas("si_lancet", "Composite")
    x0, s, cc = -26, 11, 60                    # springing line, jamb row, arch centre below
    R = s + cc
    for x in range(-48, 20):
        for y in range(0, 14):
            d = math.hypot(x - x0, y + cc)
            if x >= x0 and abs(d - R) <= 1.5:
                k = (x + y) % 10                                       # the arch
                c.cells[(x, y)] = "=" if k == 0 else "~" if k == 5 else "#"
    apex = max(x for (x, y) in c.cells if y == 0)
    # the mullion: 5 wide, from a feathered stern into the apex
    deck(c, -48, apex - 2, 0)
    deck(c, -47, apex - 6, 1)
    wall(c, -45, apex - 7, 2, beat=4, phase=-1)
    c.P(-48, 0, "E").P(-47, 1, "E").P(-45, 2, "E")
    c.P(-46, 1, ".R.b...R.O.Q.m....SSS...BB...SSS..m.Q.V.V.V.V.V.O")
    c.P(-28, 2, "SSS").P(-17, 2, "SSS")
    c.P(-22, 0, "BB.H")
    for x in (-35, -10, 3):
        c.cells[(x, 2)] = "M"
    # the sill: a short yoke from the mullion to each jamb at the stern
    for y in range(3, 10):
        c.P(-41, y, "#.#")
    # the jambs: 3 wide, feathered, a cell for each sister
    c.P(-43, 12, "E~#M#~###~###M#~###")
    c.P(-44, 11, "E..R.Q..Q..O..Q..Q.")
    c.P(-43, 10, "E##########~###=###")
    # the point: plates on each step, radar and an ear at the tip
    for y in range(1, 6):
        xe = max(x for (x, yy) in c.cells if yy == y)
        c.cells[(xe + 1, y)] = "/"
    c.cells[(apex - 1, 0)] = "A"
    c.cells[(apex, 0)] = "?"
    return finish(c.mirror())


def mendicant():
    """Trader, small hauler. A needle with a pannier slung each side on a
    short yoke, like a begging friar's bags: two short sealed pods of holds
    with their own plated points and their own engines, stepped forward of
    the needle's so the stern still feathers."""
    c = Canvas("si_mendicant", "Composite")
    # the needle
    deck(c, -13, 11, 0)
    deck(c, -12, 11, 1)
    wall(c, -10, 10, 2, beat=4, phase=-2)
    c.P(-13, 0, "E").P(-12, 1, "E").P(-10, 2, "E")
    c.P(-11, 1, ".R.b.O.QBB..Q.m.s.k.I.")
    c.P(-3, 0, "BB.H")
    c.cells[(-7, 2)] = c.cells[(6, 2)] = "M"
    c.cells[(11, 2)] = "/"
    c.cells[(12, 1)] = "/"
    c.P(12, 0, "A?")
    # the yokes
    for y in (3, 4):
        c.P(-2, y, "#.#")
    # the panniers: rows 5..8, a corridor in the middle row, holds outboard
    c.P(-5, 9, "#~CC#CC~#/")
    c.P(-7, 8, "E.k.CC.CC.k#/")
    c.P(-8, 7, "E" + "." * 12 + "I?")
    c.P(-7, 6, "E.k.UU..UU.#/")
    c.P(-5, 5, "#~##.##~#/")
    return finish(c.mirror())


def quill():
    """Archive ship. A scribe's quill seen from above: the keel is the
    shaft, and thin vanes swept toward the point at 45 degrees are the
    barbs, longest in the middle of the feather, bare at the root where the
    engines feather the stern. Every vane is a shelf of sealed memory with a
    passive ear at its tip. The Silence keep everything they have heard;
    this is where it is written down."""
    c = Canvas("si_quill", "Composite")
    # the shaft: 5 wide, feathered stern, plated point
    deck(c, -50, 47, 0)
    deck(c, -49, 45, 1)
    wall(c, -47, 44, 2, beat=4, phase=-1)
    c.P(-50, 0, "E").P(-49, 1, "E").P(-47, 2, "E")
    c.cells[(45, 2)] = "/"
    c.cells[(46, 1)] = "/"
    c.P(46, 0, "A?")
    # rooms: the engine room in the bare root, the scriptorium under the
    # vanes, the bridge mid-shaft, radar near the point
    c.P(-48, 1, ".R.b.LL.R..O.f.c..")
    for x, ch in ((-28, "Q"), (-25, "!!"), (-21, "$"), (-18, "Q"), (-15, "O"), (-12, "!!"), (-8, "$"),
                  (-5, "q"), (-3, "BB"), (1, "q"), (4, "$"), (7, "!!"), (11, "O"), (14, "Q"), (17, "$"),
                  (20, "!!"), (24, "m"), (27, "Q"), (33, "R"), (35, "b"), (38, "s"), (42, "I")):
        c.P(x, 1, ch)
    c.P(-3, 0, "BB.H")
    for x in (-44, -36, 30):
        c.cells[(x, 2)] = "M"
    for x in (33, 37, 41):
        c.cells[(x, 2)] = "A"
    # the vanes: (root x on the shaft, length); 3 wide, swept toward the bow
    for x0, n in ((-30, 7), (-21, 10), (-12, 12), (-3, 13), (6, 12), (15, 10), (24, 7)):
        for k in range(1, n + 1):
            for d in (-1, 0, 1):
                c.cells[(x0 + k + d, 2 + k)] = "#"
        tip = (x0 + n, 2 + n)
        c.cells[tip] = "?"
        c.cells[(x0 + n + 1, 2 + n)] = "#"
        for k in range(3, n - 1, 3):
            c.cells[(x0 + k, 2 + k)] = "M" if k % 2 else "~"
    return finish(c.mirror())


def almoner():
    """Trader, medium hauler: the almoner keeps the order's stores and gives
    them away, so it is built as a monk's alms bowl seen from above. A round
    bowl with a broad rim holding the bulk holds and a crown of cryo pods,
    a terrace a step below it where the crew lives, and the open hollow at
    the bottom. Ribs cross the hollow: the keel rib, three wide, runs fore and
    aft with the bridge on it at the dead centre; a single-cell rib reaches
    to each beam. The engines cluster on the aft lip of the rim, the ears and
    radar on the bow lip. As wide as it is long, and perfectly still."""
    c = Canvas("si_almoner", "Composite")
    for x in range(-16, 17):
        for y in range(0, 17):
            r = math.hypot(x, y)
            if 6.0 <= r <= 15.3 and abs(r - 9.5) > 0.55:
                c.cells[(x, y)] = "#"              # the rim, a step, the terrace
            if math.hypot(x * 0.8, y) <= 3.0:
                c.cells[(x, y)] = "#"              # the hub at the bottom of the bowl
    for k in range(3, 15):                         # the keel rib, fore and aft
        for d in (-1, 0, 1):
            c.cells.setdefault((k, d), "#")
            c.cells.setdefault((-k, d), "#")
    for k in range(3, 14):                         # a single-cell rib to each beam
        c.cells.setdefault((0, k), "#")
    # stealth coating on the outer lip at an even beat
    for (x, y), ch in list(c.cells.items()):
        r, a = math.hypot(x, y), math.degrees(math.atan2(y, x))
        if ch == "#" and r > 14.4 and y >= 2 and round(a) % 18 < 5:
            c.cells[(x, y)] = "~"
    # the bridge on the keel rib at the bottom of the bowl
    c.R(-1, 0, 0, 1, "B")
    c.P(1, 0, "H")
    c.P(-9, 0, "d").P(-5, 0, "d").P(5, 0, "d").P(9, 0, "d")
    # the stern of the rim: engines down the aft lip, the engine room
    for y in range(0, 4):
        c.cells[(-15, y)] = "E"
    c.cells[(-14, 4)] = "E"
    c.P(-13, 1, "Rb").P(-13, 3, "OR")
    # the stores, round the outer side of the rim; the walk keeps the inner side
    for x, y in ((-13, 5), (-12, 7), (-10, 9), (-8, 11), (-5, 12),
                 (12, 5), (11, 7), (9, 9), (7, 11), (4, 12), (13, 1)):
        c.R(x, x + 1, y, y + 1, "C")
    c.P(-3, 14, "VVVVVVV")                         # cryo along the crown of the rim
    c.P(0, 13, "s")
    c.cells[(-11, 9)] = c.cells[(11, 9)] = "M"
    # the bow of the rim: radar and ears
    c.P(13, 0, ".A?").P(12, 4, "W").P(15, 3, "I")
    c.P(-5, 14, "I").P(5, 14, "I").P(0, 15, "?").P(-9, 12, "?").P(9, 12, "?")
    # the terrace: the crew lives a step down from the stores
    c.P(-7, 3, "Q").P(7, 3, "Q").P(-6, 5, "O").P(6, 5, "O").P(-3, 7, "m").P(3, 7, "m")
    c.P(0, 7, "M")
    return finish(c.mirror())


def grange():
    """Trader, bulk hauler. The grange was the abbey's farm and barn; this
    is a sheaf of five needles bound at the waist. Each straw is straight
    and crosses the others at the band, so the ears fan out long ahead and
    the stalks splay short behind like fletching. The crew lives in the
    centre needle and the band; the four leaning straws are sealed holds,
    each with an ear at its tip."""
    c = Canvas("si_grange", "Composite")
    xb = -8                                    # the band
    for m, xa, xf in ((0.3, -27, 33), (0.6, -24, 22)):      # slope, stalk end, ear tip
        for x in range(xa, xf + 1):
            yc = round(m * (x - xb))
            k = x - xa
            mid = "k" if k % 2 else "#"
            for y, ch in ((yc + 1, "~" if k % 4 == 0 else "#"), (yc, mid), (yc - 1, "#")):
                c.cells[(x, y)] = c.cells[(x, -y)] = ch
        ye = round(m * (xf - xb))
        c.cells[(xf + 1, ye)] = c.cells[(xf + 1, -ye)] = "?"
        c.cells[(xf, ye)] = c.cells[(xf, -ye)] = "#"
        c.cells[(xf + 1, ye + 1)] = c.cells[(xf + 1, -ye - 1)] = "/"
        c.cells[(xf + 1, ye - 1)] = c.cells[(xf + 1, -ye + 1)] = "/"
    # the band: a bead tied round the waist, holding the bridge
    bead(c, xb, 6, 2, y0=3)
    # the centre needle, 5 wide: crew and bulk holds
    deck(c, -34, 40, 0)
    deck(c, -33, 39, 1)
    wall(c, -31, 38, 2, beat=4, phase=-1)
    c.P(-34, 0, "E").P(-33, 1, "E").P(-31, 2, "E")
    c.cells[(39, 2)] = "/"
    c.cells[(40, 1)] = "/"
    c.P(40, 0, "A?")
    c.P(-32, 1, ".R.b.R.O..Q.m.")
    c.P(xb - 2, 6, "#~J~#")
    c.P(xb - 3, 5, "#..M..#")
    c.P(xb - 4, 4, "#..q.q..#")
    c.P(xb - 4, 3, ".s.....s.")
    c.P(xb - 4, 2, ".........")
    c.P(xb - 4, 1, "...BB....").P(xb - 1, 0, "BB.H")
    for x in (2, 5, 8, 11, 14, 17, 20, 23):
        c.P(x, 2, "CC").P(x, 1, "CC")
    c.P(27, 1, "Q.O.s..I..")
    c.cells[(-26, 2)] = c.cells[(30, 2)] = "M"
    c.cells[(-20, 2)] = "M"
    return finish(c.mirror())


CONCEPTS = [
    ("grange", grange, "Grange", "Trader: bulk hauler",
     "A sheaf of five needles bound at the waist: the straws cross at the band, so the ears fan long ahead and "
     "the stalks splay short behind. The crew lives in the centre needle; the leaning straws are sealed holds."),
    ("almoner", almoner, "Almoner", "Trader: medium hauler",
     "The almoner keeps the order's stores and gives them away: a monk's alms bowl from above. A broad rim of "
     "bulk holds and cryo, a terrace a step below where the crew lives, ribs across the open hollow and the "
     "bridge on the keel rib at the centre. Engines on the aft lip."),
    ("mendicant", mendicant, "Mendicant", "Trader: small hauler",
     "A needle with a pannier slung each side on a short yoke, like a begging friar's bags: two sealed pods "
     "of holds with their own points and engines, so the stern still feathers."),
    ("lancet", lancet, "Lancet", "Experimental: quieting ship",
     "A lancet window from above: jambs rising to a pointed arch, a mullion down the middle, two pointed lights "
     "of open space. Lore: this is the ship that removed ambition from their whole species."),
    ("abbey", abbey, "Abbey", "Cruiser: abbey ship",
     "The plan of an abbey from above: a long nave, a transept near the bow, and two open cloister courts "
     "framed by the dormitory and west ranges. The monks live round the courts."),
    ("still_water", still_water, "Still Water", "Experimental: mirror",
     "A dish concave to the bow, its inner face set with shield emitters, the needle through its centre and "
     "a receiver at its focus. Lore: what hits the Silence is reflected; this is that idea built whole."),
    ("motherhouse", motherhouse, "Motherhouse", "Flagship: mother house",
     "The monastery set moving: the same still halo and eight needles, the keel run through it, the aft "
     "diagonals widened into fletching stepped with engines. The bridge hangs in a bead at the dead centre."),
    ("psaltery", psaltery, "Psaltery", "Cruiser: great listening array",
     "A keel threaded through a lens of strings: four single-cell rails a side on three masts, every rail "
     "ending in a passive ear. It listens to a whole system at once."),
    ("quill", quill, "Quill", "Cruiser: archive ship",
     "A scribe's quill from above: the keel for a shaft and thin vanes of sealed memory swept toward the point, "
     "longest mid-feather, each with an ear at its tip. Everything the Silence hear is written down here."),
]
