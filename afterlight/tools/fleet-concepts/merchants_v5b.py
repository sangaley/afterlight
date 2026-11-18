"""Merchant Princes of the Void, round 5b: seven large hulls. Hand-drawn,
top-down only.

The faction's approved grammar (merchants.py) holds at this size too: a
three-wide spine with a walkway down its middle that everything clamps onto,
standard 5x5 container pods with their corners cut, every one the same box,
open trusswork with sky between the modules, Steel, gatlings and point-defence
drones only, docking clamps at the bow. By the developer's symmetry rule every
skeleton (outline, engines, guns) is drawn on y >= 0 and mirrored; the Guild's
utilitarian asymmetry lives inside identical pods (cargo on one side, crew,
fuel and tanks on the other) and in a few small fittings added after the
mirror: a crane, a hatch, a cabin laid out differently.

Legend: see terran.py and draw.LEGEND.
"""
from collections import deque

from draw import Canvas
from merchants import FACTION, _run  # noqa: F401  (FACTION is read by build.py)

N4 = ((0, 1), (0, -1), (1, 0), (-1, 0))
GUNS = set("gp")


# --------------------------------------------------------------------------- helpers
def pod(c, x, y, rows, n="#", s=".", w="#", e="#"):
    """One standard container pod, bottom-left corner cell at (x, y): a 5x5 box
    with the corners cut. `rows` are its three inside rows, top to bottom;
    n, s, w, e are the middle cell of each face ('.' a door, '#' wall, or a
    gun)."""
    a, b, d = rows
    c.P(x + 1, y + 4, "#" + n + "#")
    c.P(x, y + 3, "#" + a + "#")
    c.P(x, y + 2, w + b + e)
    c.P(x, y + 1, "#" + d + "#")
    c.P(x + 1, y, "#" + s + "#")


def stack(c, x, y, pods, top="#", gap=1):
    """Pods stood one on another, the first with its foot at row y, each hung
    off the one below on a short three-wide strut `gap` rows long (0: neck to
    neck). The last pod's top face is `top`."""
    for k, rows in enumerate(pods):
        y0 = y + (5 + gap) * k
        pod(c, x, y0, rows, n="." if k < len(pods) - 1 else top)
        if k < len(pods) - 1:
            for g in range(gap):
                c.P(x + 1, y0 + 5 + g, "#.#")


def spine(c, x0, x1, doors=(), marks=None):
    """The upper half of a spine from x0 to x1: rim on row 1 (a door at every
    x in `doors`), walkway on row 0. Mirror it for the lower rim."""
    c.P(x0, 1, _run(x0, x1, doors, marks=marks))
    c.P(x0, 0, "." * (x1 - x0 + 1))


def finish(c, centre=0.5):
    """Build, then check what draw.py doesn't: the hull is one piece, the
    skeleton is mirror-symmetric (outline, engines, guns), no engine fires
    into its own hull, and the bridge is inside the hull, near the middle."""
    d = c.build()
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
        d["_checks"].append("outline not mirror-symmetric")
    xs = [x for x, _ in hull]
    lo, hi = min(xs), max(xs)
    bridge = []
    for (x, y), ch in c.cells.items():
        if ch in "EN" and any((xx, y) in hull for xx in range(x - 120, x)):
            d["_checks"].append(f"engine at {(x, y)} vents into hull")
        if (ch in "EN" or ch in GUNS) and c.cells.get((x, -y)) != ch:
            d["_checks"].append(f"'{ch}' at {(x, y)} has no mirror twin")
        if ch == "B":
            bridge.append(x)
            if layer.get((x, y)) == "Outer":
                d["_checks"].append(f"bridge on the rim at {(x, y)}")
    if bridge:
        t = (sum(bridge) / len(bridge) - lo) / max(1, hi - lo)
        if abs(t - centre) > 0.2:
            d["_checks"].append(f"bridge not in the middle (at {t:.2f} of the length)")
    return d


# --------------------------------------------------------------------------- ships
def argosy():
    """Trader: container giant. The biggest box carrier on the Guild's books.
    Its pods stand in stacks off both sides of one spine, each pod on a short
    strut from the one below: one high at bow and stern, three high
    amidships, so the cargo makes a lozenge. The deckhouse sits across the
    spine in the middle with two more pods stacked on its roof; a clamp at
    the bow, a wide engine block aft."""
    c = Canvas("mp_argosy", "Steel")
    walk = ["U.U", "U.U", "k.k"]
    walk2 = ["k.k", "U.U", "U.U"]
    top_a = ["CCk", "CCk", "k.k"]
    top_b = ["kCC", "kCC", "k.k"]
    armed = ["k.k", "U.U", "U.U"]                  # a gun on its roof needs the aisle
    stacks = [
        (-34, [armed], "g"),
        (-27, [walk, armed], "g"),
        (-20, [walk, walk2, top_a], "#"),
        (-13, [walk, walk2, top_b], "#"),
        (9, [walk, walk2, top_a], "#"),
        (16, [walk, armed], "g"),
        (23, [armed], "g"),
    ]
    for x, pods, top in stacks:
        stack(c, x, 2, pods, top=top)
    spine(c, -36, 29, doors={x + 2 for x, _, _ in stacks})
    # the deckhouse across the spine, two pods stacked on its roof
    c.rows(-6, 6, """
  ####.####
 #q.O...O.q#
#M.........A#
#.GG.Q.Q.m..#
#.GG.......c#
.....bBB.H...
......BB.....
""")
    c.P(-1, 7, "#.#")
    stack(c, -2, 8, [walk2, top_b])
    # engine block aft
    c.rows(-40, 6, """
 ###
N.R#
N..#
E.c#
N.M#
N...
E...
""")
    # the bow clamp: two jaws round a docking port
    c.rows(30, 5, """
 #######/
#q.O....o
#.A.m...#
#...###g#
....#
.....o
""")
    c.mirror()
    # small, deliberate asymmetry: the aft starboard pod carries fuel
    pod(c, -34, -6, ["f.f", "f.f", "f.f"], n=".", s="g")
    return finish(c)


def _hamlet(c, xc, foot=True, crown="g", ends=("p", "p")):
    """One hamlet of the Company Town: five standard pods in a cross round a
    crossroads pod, each on a one-row strut. (xc, 8) is the crossroads pod's
    corner; with `foot` a stalk pod stands under it on the spine."""
    if foot:
        pod(c, xc, 2, ["Q.Q", "V.V", "V.V"], n=".")          # cryo dorm, walk-through
    c.P(xc + 1, 7, "#.#")
    pod(c, xc, 8, ["O.R", "...", "c.b"], n=".", w=".", e=".")  # the well: air and power
    c.P(xc - 1, 9, "#").P(xc - 1, 10, ".").P(xc - 1, 11, "#")
    pod(c, xc - 6, 8, ["VVQ", "...", "VVQ"], s="#", w=ends[0], e=".")
    c.P(xc + 5, 9, "#").P(xc + 5, 10, ".").P(xc + 5, 11, "#")
    pod(c, xc + 6, 8, ["QVV", "...", "QVV"], s="#", w=".", e=ends[1])
    c.P(xc + 1, 13, "#.#")
    pod(c, xc, 14, ["V.V", "V.V", "Q.Q"], n=crown)


def company_town():
    """Trader: colony transport. A company town in transit: three hamlets a
    side, each five standard pods in a cross round a crossroads pod that
    carries its own air and power, the settlers asleep in cryo. The company
    store sits across the spine in the middle with the bridge, the middle
    hamlets standing on its roof. On arrival the pods unclamp and are the
    town; the settlers start out owing for every one."""
    c = Canvas("mp_company_town", "Steel")
    _hamlet(c, -26, ends=("p", "#"))
    _hamlet(c, -2, foot=False, ends=("#", "#"))
    _hamlet(c, 22, ends=("#", "p"))
    spine(c, -36, 33, doors={-24, 24})
    # the company store: the factor's office, the strongbox, the bridge
    c.rows(-5, 5, """
 ####.####
#k.GG.$.mk#
#..GG.....#
#M.q.Q.q.A#
...O.BB.H..
.....BB....
""")
    c.P(-1, 6, "#.#")
    # engine block aft
    c.rows(-40, 5, """
 ###
N.R#
N..#
E.M#
E...
E...
""")
    # the bow clamp, a passenger gangway either side
    c.rows(34, 5, """
 #######/
#Q.m....o
#.O.Q...#
#...###p#
....#
.....o
""")
    c.mirror()
    # small, deliberate asymmetry: the starboard hamlets carry the livestock
    # and the seed bank
    pod(c, 28, -12, ["YZZ", "...", "YZZ"], n="#", s="#", w=".", e="p")
    pod(c, 16, -12, ["ZZY", "...", "ZZY"], n="#", s="#", w="#", e=".")
    pod(c, -26, -18, ["Q.Q", "Y.Y", "Y.Y"], n=".", s="g")
    return finish(c)


def _barge(c, b, pods, caps=("#", "#")):
    """One grain barge, x = b..b+18: a three-wide rail across each end (its
    bumpers, standing a row proud of the cargo) and two standard pods stood
    on the spine between them. Returns the spine doors it needs."""
    for i, rx in enumerate((b, b + 16)):
        for y in range(2, 7):
            c.P(rx, y, "#.#")
        c.P(rx, 7, "#" + caps[i] + "#")
    pod(c, b + 4, 2, pods[0])
    pod(c, b + 10, 2, pods[1])
    return {b + 1, b + 6, b + 12, b + 17}


def grain_tow():
    """Trader: barge train. A pusher tug shoving four grain barges coupled
    nose to tail along one spine, the factor's car in the middle of the train
    with the bridge. Each barge is two standard pods between a pair of bumper
    rails, so the train reads as cars with couplings between. It turns up
    wherever the harvest failed; the price is whatever the Guild says."""
    c = Canvas("mp_grain_tow", "Steel")
    grain = (["CCk", "CCk", "k.k"], ["kCC", "kCC", "k.k"])
    doors = set()
    doors |= _barge(c, -49, grain, caps=("g", "#"))
    doors |= _barge(c, -27, grain)
    doors |= _barge(c, 9, grain)
    doors |= _barge(c, 31, grain, caps=("#", "g"))
    # the factor's car, in the middle of the train
    c.rows(-5, 6, """
  #######
 #Q.q.q.Q#
#M.......A#
#.GG.O.m..#
#.GG......#
....bBB.H..
.....BB....
""")
    # the pusher tug: an engine block, a machinery pod each side and a push
    # bar across its nose, bearing on the first barge's bumper
    c.rows(-66, 7, """
 ###
N.R#
N..#
E.c#
N.M#
N...
E...
E...
""")
    pod(c, -61, 2, ["R.c", "b.b", "Q.O"])
    for y in range(2, 7):
        c.P(-55, y, "#.#")
    c.P(-55, 7, "###")
    doors |= {-59, -54}
    spine(c, -63, 51, doors=doors)
    # the bow clamp on the lead barge
    c.rows(52, 5, """
 #######/
#q.O....o
#.A.m...#
#...###g#
....#
.....o
""")
    c.mirror()
    # small, deliberate asymmetry: fuel in the aft starboard barge, a crane
    # on the lead barge's starboard bumper
    pod(c, -45, -6, ["f.f", "f.f", "fff"], n=".", s="#")
    pod(c, -39, -6, ["f.f", "f.f", "fff"], n=".", s="#")
    c.P(25, -7, "#X#")
    return finish(c)


def palazzo():
    """Flagship: a merchant prince's travelling palace and counting house.
    A square cloister of three-wide galleries round four open courts, the
    spine running through it, and in the middle the palace itself: the
    counting house's vaults and ledgers round the bridge, the prince's baths,
    banqueting galley and physician. The court lives in standard pods hung
    round the outside of the cloister, guards in the corner towers; a water
    gate of two clamps at the bow, a wall of engines aft."""
    c = Canvas("mp_palazzo", "Steel")
    # the palace, in the middle of the cloister
    c.rows(-10, 8, """
   #######.#######
  #q.$.J.....J.$.q#
 #.................#
#Mwww.GG.$.$SSS.q.M#
#.www.GG....SSS.q.A#
#.www..............#
#KK.m.$$$..$$$.O.KK#
..R.J.$$.qBB.H.$$.J..
..........BB.........
""")
    # the cloister: gallery across the top, a wall at each end, a crossbeam
    # from the palace roof up to the gallery
    for y in range(2, 16):
        c.P(-24, y, "#.#")
        c.P(22, y, "#.#")
    for y in range(9, 16):
        c.P(-1, y, "#.#")
    c.P(-23, 16, _run(-23, 23, doors={-18 + 2, -10 + 2, 0, 6 + 2, 14 + 2, -23, 23}))
    c.P(-24, 17, "." * 49)
    c.P(-23, 18, _run(-23, 23, doors={-16, -8, 0, 8, 16}))
    c.P(-24, 16, "#").P(24, 16, "#").P(-24, 18, " ").P(24, 18, " ")
    # the court, in pods round the outside of the gallery
    pod(c, -18, 19, ["Q.Q", "Q.Q", "..."], n="g")         # guards
    pod(c, -10, 19, ["CCk", "CCk", "k.k"])                # the prince's stores
    pod(c, -2, 19, ["q.q", "...", "$.$"], n="o")          # audience: callers dock on its roof
    pod(c, 6, 19, ["kCC", "kCC", "k.k"])
    pod(c, 14, 19, ["Q.Q", "Q.Q", "..."], n="g")
    # corner towers on the end walls
    pod(c, -30, 9, ["Q.O", "...", "a.Q"], s="#", w="g", e=".")
    c.P(-25, 10, "#").P(-25, 11, ".").P(-25, 12, "#")
    pod(c, 26, 9, ["O.Q", "...", "Q.a"], s="#", w=".", e="g")
    c.P(25, 10, "#").P(25, 11, ".").P(25, 12, "#")
    # the cloister's corners are standard pods too, knuckles where gallery
    # meets wall, so the frame never shows a square corner
    pod(c, -25, 15, ["Q.Q", "...", "O.."], n="g", s=".", w="g", e=".")
    pod(c, 21, 15, ["Q.Q", "...", "..O"], n="g", s=".", w=".", e="g")
    # the spine, end to end
    spine(c, -40, 24, doors={-36, -23, 23})
    # power pods and a wall of engines aft
    pod(c, -38, 2, ["FFF", "FFF", "FFF"])
    c.rows(-44, 9, """
 ###
N.R#
N..#
E.c#
N.M#
N..#
E.c#
N.R#
N...
E...
""")
    # the water gate: two clamps, one above the other, round a docking port
    c.rows(25, 7, """
  ####/
 #q.O.o
#..A.##
#.m...#/
#...#.g.o
#...###/
....#
.....o
""")
    c.mirror()
    # small, deliberate asymmetry: the starboard stores hold the prince's
    # luxuries from a dozen universes, in specimen vaults and strongboxes
    pod(c, -10, -23, ["k.k", "Y$Y", "Y$Y"], n=".", s="#")
    pod(c, 6, -23, ["k.k", "$Y$", "$Y$"], n=".", s="#")
    return finish(c)


def sale_ring(R=17, D=24):
    """Experimental: a travelling auction hall. An octagonal ring of truss
    round an empty arena, the bidders' boxes hung round its outside, each
    with a berth for the bidder's ship. The auctioneer's rostrum stands in
    the middle on a pier from the stern, tractor beams and floodlights on the
    ring's inner face. The lots come through the middle of the ring from
    other universes; nobody asks what became of the universe afterwards."""
    c = Canvas("mp_sale_ring", "Steel")
    m = lambda x, y: max(abs(x), abs(y))
    d = lambda x, y: abs(x) + abs(y)
    ring = {(x, y) for x in range(-R, R + 1) for y in range(0, R + 1)
            if m(x, y) <= R and d(x, y) <= D and not (m(x, y) <= R - 3 and d(x, y) <= D - 4)}
    for p in ring:
        c.cells[p] = "#"
    for (x, y) in ring:                 # the walkway down the middle of the ring
        if all((x + dx, abs(y + dy)) in ring for dx, dy in N4):
            c.cells[(x, y)] = "."
    # lights and tractors on the inner face, looking into the arena
    for x, y, ch in ((-8, 13, "l"), (0, 14, "t"), (8, 13, "l"), (-15, 6, "t"), (15, 6, "t")):
        c.cells[(x, y)] = ch
    # the bidders' boxes: pods round the outside, a berth on each
    for x0 in (-8, -2, 4):
        pod(c, x0, R + 1, ["q.Q", "...", "Q.q"], n="o")
        c.cells[(x0 + 2, R)] = "."
    # two more on the shoulders, set into the ring's corners like knuckles
    pod(c, 11, 11, ["q.Q", "...", "Q.."], n="o", s=".", w=".", e="#")
    pod(c, -15, 11, ["Q.q", "...", "..Q"], n="o", s=".", w="#", e=".")
    # the rostrum on its pier in the middle, the bridge on it
    c.rows(-5, 4, """
 #######
#M.$.$.A#
#.......t
..q.BB.H#
....BB..#
""")
    spine(c, -36, -6)
    # the stern: lot warehouses on the spine and an engine block
    pod(c, -30, 2, ["CCk", "CCk", "k.k"])
    pod(c, -24, 2, ["kCC", "kCC", "k.k"])
    c.rows(-40, 6, """
 ###
N.R#
N..#
E.c#
N.M#
N...
E...
""")
    c.P(-R, 1, "."), c.P(-R + 1, 1, "."), c.P(-R + 2, 1, ".")
    # bow: a short spine from the ring to the clamp
    spine(c, R - 2, R + 3)
    c.rows(R + 4, 5, """
 #######/
#q.O....o
#.A.M...#
#...###g#
....#
.....o
""")
    c.mirror()
    return finish(c)


def slipway():
    """Experimental: a mobile shipyard. The yard's own spine down the middle,
    and out on each side, held off it on one gantry, a new Guild hull on
    the slip: a bare spine with every other pod hung and the struts for the
    rest standing empty, a clamp nose, no engines fitted yet. Cranes on the
    gantry and tractors on the yard office work across the open slip. It builds on contract and launches sideways, two at a time."""
    c = Canvas("mp_slipway", "Steel")
    # the new hull on the slip (its spine on rows 14..16): a Guild hull with
    # a pedlar's nose, its stern still an open truss waiting for engines
    c.P(-25, 16, _run(-25, 16, doors={-19, -13, -7, -1, 5, 11}) + "#/")
    c.P(-25, 15, "." * 43 + "o")
    c.P(-25, 14, _run(-25, 16, doors={0}) + "#/")
    for x0 in (-21, -9, 3):                         # pods hung so far
        pod(c, x0, 17, ["CCk", "CCk", "k.k"] if x0 < 0 else ["kCC", "kCC", "k.k"])
    for x0 in (-15, -3, 9):                         # struts waiting for theirs
        c.P(x0 + 1, 17, "#.#").P(x0 + 1, 18, "#.#").P(x0 + 1, 19, "###")
    c.P(-26, 17, "#.#").P(-26, 18, "###")           # the empty engine bed
    c.P(-26, 13, "###").P(-26, 12, "###")
    c.P(-26, 16, "#").P(-26, 15, "#").P(-26, 14, "#")
    # one gantry from the yard office out to the slip, cranes along it
    # working fore and aft over the new hull
    for y in range(7, 14):
        c.P(-1, y, "#.#")
    c.P(-1, 9, "X").P(1, 11, "X").P(-1, 12, "l").P(1, 8, "l")
    # the yard office across the spine, tractors on its roof
    c.rows(-5, 6, """
  t##.##t
 #y.....y#
#M.......A#
#.%%.q.%%.#
#.........#
...O.BB.H..
.....BB....
""")
    # the yard's spine, parts and repair pods, engines aft, clamp at the bow
    spine(c, -40, 27, doors={-35, -28, 16, 23})
    pod(c, -37, 2, ["U.U", "U.U", "k.k"])
    pod(c, -30, 2, ["DDy", "...", "y.k"])
    pod(c, 14, 2, ["DDy", "...", "y.k"])
    pod(c, 21, 2, ["k.k", "U.U", "U.U"])
    c.rows(-44, 6, """
 ###
N.R#
N..#
E.c#
N.M#
N...
E...
""")
    c.rows(28, 5, """
 #######/
#q.O....o
#.A.m...#
#...###g#
....#
.....o
""")
    c.mirror()
    # small, deliberate asymmetry: the starboard hull is further along, its
    # fuel pod already in
    pod(c, 3, -21, ["f.f", "f.f", "f.f"], n=".", s="#")
    return finish(c)


def marque_carrier():
    """Escort: carrier. The Guild's own guns, for when the Throne's escorts
    aren't there. A crossbow: a long stock of spine with gun pods hung on it
    and a launch bar across it amidships. The bar carries the berths of a
    flight of privateers sailing under Guild letters of marque, docked nose
    out along its fore face, their crews quartered behind, and a gun tower
    at each tip. Same parts as every Guild hull; every pod a gun pod."""
    c = Canvas("mp_marque_carrier", "Steel")
    gun = ["a.a", "...", "a.a"]
    # the launch bar: a stem up from the command house, berth pods forward,
    # quarters aft, a gun tower at the tip
    for y in range(6, 19):
        c.P(0, y, "#.#")
    for y0 in (7, 13):
        pod(c, 4, y0, ["Q..", "...", "y.."], s="#", w=".", e="o")      # a privateer's berth
        c.P(3, y0 + 1, "#").P(3, y0 + 2, ".").P(3, y0 + 3, "#")
        pod(c, -6, y0, ["Q.Q", "...", "Q.O"], s="#", w="p", e=".")     # her crew
        c.P(-1, y0 + 1, "#").P(-1, y0 + 2, ".").P(-1, y0 + 3, "#")
    pod(c, -1, 19, ["a.a", "...", "a.a"], n="g", w="g", e="g")
    # the command house where bar meets spine
    c.rows(-4, 5, """
 ####.####
#M.a...a.A#
#.KK.q.KK.#
#.O.....T.#
....bBB.H..
.....BB....
""")
    # the stock: gun pods hung on the spine fore and aft of the bar
    for x0 in (-32, -25, -18, 11, 18):
        pod(c, x0, 2, gun, n="g", w="p" if x0 in (-32, 11) else "#",
            e="p" if x0 in (-18, 18) else "#")
    spine(c, -36, 24, doors={-30, -23, -16, 13, 20})
    c.rows(-40, 6, """
 ###
N.R#
N..#
E.c#
N.M#
N...
E...
""")
    c.rows(25, 5, """
 #######/
#q.O....g
#.A.a...#
#...###g#
....#
.....o
""")
    c.mirror()
    # small, deliberate asymmetry: the starboard aft crew pod is the sick bay
    pod(c, -6, -11, ["Q.m", "...", "m.Q"], n="#", s="#", w="p", e=".")
    return finish(c)


CONCEPTS = [
    ("argosy", argosy, "Argosy", "Trader: container giant",
     "The biggest box carrier on the Guild's books: pods stacked neck to neck off both sides of one spine, one high at the ends and three amidships, so the cargo makes a lozenge round the deckhouse."),
    ("company_town", company_town, "Company Town", "Trader: colony transport",
     "A colony in transit: three hamlets a side, five pods in a cross round a crossroads pod, settlers asleep in cryo, the company store and bridge across the spine. The pods unclamp on arrival and are the town; the settlers owe for every one."),
    ("grain_tow", grain_tow, "Grain Tow", "Trader: barge train",
     "A pusher tug shoving four grain barges coupled nose to tail, the factor's car and bridge in the middle of the train. It turns up wherever the harvest failed; the price is whatever the Guild says."),
    ("palazzo", palazzo, "Palazzo", "Flagship: prince's palace",
     "A merchant prince's travelling palace and counting house: a square cloister round four open courts, the palace and its vaults in the middle on the spine, the court in pods round the outside, a water gate at the bow."),
    ("sale_ring", sale_ring, "Sale Ring", "Experimental: travelling auction hall",
     "An octagonal ring round an empty arena, bidders' boxes and berths round its outside, the auctioneer's rostrum in the middle. The lots come through the arena from other universes; nobody asks what became of those universes."),
    ("slipway", slipway, "Slipway", "Experimental: mobile shipyard",
     "A shipyard that travels: the yard's spine down the middle and a half-built Guild hull on a slip each side, held off it on a gantry with cranes, every other pod hung and the struts for the rest standing empty. It builds on contract and launches sideways."),
    ("marque_carrier", marque_carrier, "Marque Carrier", "Escort: carrier",
     "The Guild's own guns for when the Throne isn't there: a crossbow of a hull, gun pods hung all along the stock, and across it a launch bar where a flight of privateers under Guild letters of marque dock nose out, a gun tower at each tip."),
]
