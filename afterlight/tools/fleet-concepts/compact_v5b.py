"""Free Haulers' Compact, round 5b: nine large hulls. Hand-drawn, top-down only.

The Compact is a union: two hundred million haulers, miners and their
families, moving fuel, ore and salvage. Its ships are working ships, plain,
patched and handed down; freight that shoots back. Their allies the Terrans
guard the stations and the big convoys, so even the escort here is a hauler's
frame loaded with guns instead of cargo.

The faction grammar (compact.py) holds at this size:
  - a PUSH-TUG at the stern, a stubby chamfered crew-and-engine block;
  - an open LOAD FRAME ahead of it, three-wide keels and crossbeams with sky
    between, ending in a stepped chevron BUMPER, the only place with plates;
  - MISMATCHED LOADS on one-cell straps: wedge ore hoppers, hollow scrap
    cages, rounded fuel bladders, never two the same;
  - BOLT-ON ENGINE PODS on struts;
  - PATCHWORK Steel, mended with Titanium and Composite panels.

What the big hulls add to it:
  - by the developer's symmetry rule every skeleton is drawn on y >= 0 and
    mirrored, so the bolt-on pods now come in pairs; the old one-sidedness
    lives on only in small add-ons drawn after the mirror (a family's extra
    cabin, a crane, a spare tank, a patch);
  - the WHEELHOUSE moves off the tug and onto the frame amidships, a
    chamfered deckhouse across the keel, so the master sees both ends of the
    load and the bridge sits in the middle of the ship;
  - FAMILY DECKS: on a Compact hull the crew is a family, so the deckhouses
    carry quarters, a galley, a medbay and a workshop, strapped in the frame
    like any other load.

Legend: see terran.py and draw.LEGEND.
"""
from collections import deque

from draw import Canvas
from compact import FACTION  # noqa: F401  (FACTION is read by build.py)

N4 = ((0, 1), (0, -1), (1, 0), (-1, 0))
GUNS = set("gptX&")


# --------------------------------------------------------------------------- helpers
def keel(c, x0, x1, doors=(), y=0):
    """Top half of a three-wide keel: deck on row y, rim on row y+1 with a
    gap at every x in `doors` (where a crossbeam or a deckhouse joins)."""
    for x in range(x0, x1 + 1):
        c.cells[(x, y + 1)] = "." if x in doors else "#"
        c.cells[(x, y)] = "."


def rib(c, x, y0, y1, cap="#", door=True):
    """A three-wide crossbeam standing on row y0 (its foot joins whatever is
    below), walkway up the middle, column x..x+2, capped on row y1."""
    c.P(x, y0, "#d#" if door else "#.#")
    for y in range(y0 + 1, y1):
        c.P(x, y, "#.#")
    c.P(x, y1, "#" + cap + "#")


def rail(c, x0, x1, y, lo=(), hi=()):
    """A three-wide side keel along row y (deck), rims on y-1 and y+1, with
    a gap in the low rim at every x in `lo` and the high rim at every x in
    `hi`."""
    for x in range(x0, x1 + 1):
        c.cells[(x, y - 1)] = "." if x in lo else "#"
        c.cells[(x, y)] = "."
        c.cells[(x, y + 1)] = "." if x in hi else "#"


def bumper(c, aft, rows):
    """The push-bar bow, top half: aft[y] is its aft x on row y, rows[y] the
    cells it holds there, aft to front."""
    for y, s in rows.items():
        c.P(aft[y], y, s)


def plates(c, cells):
    """Plates on the bumper's steps, both sides (call after mirror())."""
    for x, y in cells:
        c.cells[(x, y)] = "/"
        c.cells[(x, -y)] = "/"


def drum(c, x, y, w, h, cut, fill="f", rim="#"):
    """A rounded load (fuel bladder, hab drum): a w x h box, bottom-left at
    (x, y), with its corners cut back `cut` steps so it reads as round. The
    edge is rim, everything inside is `fill`."""
    box = set()
    for i in range(w):
        for j in range(h):
            if min(i, w - 1 - i) + min(j, h - 1 - j) >= cut:
                box.add((x + i, y + j))
    for p in box:
        edge = any((p[0] + dx, p[1] + dy) not in box for dx, dy in N4)
        c.cells[p] = rim if edge else fill
    return box


def segs(c, rows):
    """Paint runs of cells row by row: {y: [(x, "cells"), ...]}."""
    for y, runs in rows.items():
        for x, s in runs:
            c.P(x, y, s)


def home(c, x, y, rows=("Q.Q.Q.k", ".......", "..GG.m.", "..GG.O.", "k.Q.Q.y"), door="w"):
    """A family home: a 9 x 7 deckhouse with its corners cut, bottom-left at
    (x, y). `rows` are its five inside rows, top to bottom; `door` is the
    side its door is on ('w' or 'e', in the middle row)."""
    c.P(x + 1, y + 6, "#######")
    for k, r in enumerate(rows):
        yy = y + 5 - k
        w = "d" if door == "w" and k == 2 else "#"
        e = "d" if door == "e" and k == 2 else "#"
        c.P(x, yy, w + r + e)
    c.P(x + 1, y, "###d###" if door == "s" else "#######")


def bladder(c, x, w, h, cut, y=3, fill="f", core=None):
    """A rounded fuel bladder slung off a keel: a drum with its bottom row
    on row y and two one-cell straps under it, a gap between them. `core`
    is an (x, y) offset for a memory core inside."""
    box = drum(c, x, y, w, h, cut, fill)
    bottom = sorted(p[0] for p in box if p[1] == y)
    c.cells[(bottom[1], y - 1)] = "#"
    c.cells[(bottom[-2], y - 1)] = "#"
    if core:
        c.cells[(x + core[0], y + core[1])] = "M"


def finish(c, patches=(), centre=0.5, asym=0):
    """Build, then check what draw.py doesn't: the hull is one piece, the
    skeleton is mirror-symmetric (no more than `asym` add-on cells off it,
    engines and guns all twinned), no engine fires into its own hull, and the
    bridge is inside the hull near the middle. Then re-skin the patches
    (x0, x1, y0, y1, material): repairs, not noise."""
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
    off = [p for p in hull if (p[0], -p[1]) not in hull]
    if len(off) > asym:
        d["_checks"].append(f"outline: {len(off)} cells off the mirror (allowed {asym}), e.g. {sorted(off)[:4]}")
    xs = [x for x, _ in hull]
    lo, hi = min(xs), max(xs)
    bridge = []
    for (x, y), ch in c.cells.items():
        if ch in "EN" and any((xx, y) in hull for xx in range(x - 150, x)):
            d["_checks"].append(f"engine at {(x, y)} vents into hull")
        if (ch in "EN" or ch in GUNS) and c.cells.get((x, -y)) != ch:
            d["_checks"].append(f"'{ch}' at {(x, y)} has no mirror twin")
        if ch == "B":
            bridge.append(x)
            if layer.get((x, y)) == "Outer":
                d["_checks"].append(f"bridge on the rim at {(x, y)}")
    if not bridge:
        d["_checks"].append("no bridge")
    else:
        t = (sum(bridge) / len(bridge) - lo) / max(1, hi - lo)
        if abs(t - centre) > 0.2:
            d["_checks"].append(f"bridge not in the middle (at {t:.2f} of the length)")
    for cell in d["hull_cells"]:
        x, y = cell["grid_pos"]
        for x0, x1, y0, y1, mat in patches:
            if x0 <= x <= x1 and y0 <= y <= y1:
                cell["material"] = mat
    return d


# --------------------------------------------------------------------------- ships
def ore_train():
    """Trader: ore train. The longest thing the Compact flies: a tug shoving
    ninety cells of keel with ore funnels slung either side, every funnel a
    different width and depth, a scrap cage and two water bladders among
    them. The family's wheelhouse rides across the keel amidships; two
    crossbeams carry gun tubs, and a pair of engine pods hangs off the tug."""
    c = Canvas("fhc_ore_train", "Steel")
    # the tug
    c.rows(-60, 7, """
    #######
   E..Q.Q.Q#
  E.R.......#
 E.c.GG.O.y..#
E....GG...b..#
E.M.....KK...#
#R.c.v.......##
E.............d
""")
    # bolt-on engine pods on struts off the tug's flanks
    c.rows(-58, 12, """
 #####
E....#
E..M.#
E....#
 ##d##
  #.#
""")
    keel(c, -45, 47, doors={-22, 21})
    # the wheelhouse across the keel amidships: the family lives here
    c.rows(-7, 8, """
    #######
   #Q.Q.Q.Q#
  #.........#
 #.GG.m.O.y.M#
#..GG.........#
#.T...q.Q..A..#
#...m..H......#
#......BB.....#
.......BB......
""")
    # two crossbeams with gun tubs
    for x in (-23, 20):
        rib(c, x, 2, 9)
        c.P(x, 9, "g.p")
        c.P(x, 10, "###")
    # aft loads: two funnels, a scrap cage, a water bladder
    c.rows(-44, 7, """
#######  ##########
#kCCkk#  #kUUkMUUk#
 #CCUU#   #kkCCkk#
  #kkk#    #kCCk#
   ####     ####
   #  #     #  #
""")
    c.rows(-19, 7, """
 ###
#kkk#
#k k#
#kkk#
 ###
 # #
""")
    drum(c, -13, 3, 5, 4, 1)
    c.P(-12, 2, "# #")
    # fore loads: a bladder and two funnels
    drum(c, 10, 3, 8, 6, 2)
    c.P(13, 6, "M")
    c.P(12, 2, "#  #")
    c.rows(25, 7, """
#########  #####
#kCCkUUk#  #UUk#
 #CCkkk#   #kk#
  #kkk#    #k#
   ###     ###
   # #     # #
""")
    # the push-bar bumper
    aft = {0: 48, 1: 48, 2: 47, 3: 47, 4: 46, 5: 46, 6: 45, 7: 44, 8: 43, 9: 42, 10: 41}
    bumper(c, aft, {0: "...o", 1: "#..#", 2: "#M.#", 3: "#..#", 4: "#a.#", 5: "#..g",
                    6: "#..#", 7: "#..#", 8: "#..#", 9: "#..#", 10: "##g#"})
    c.mirror()
    plates(c, [(51, 2), (50, 4), (49, 6), (48, 7), (47, 8), (46, 9), (45, 10)])
    # same outlines, different loads on the starboard side
    c.P(-43, -6, "kUUkk")
    c.P(-42, -5, "kkUU")
    c.P(-34, -6, "kCCkkCCk")
    c.P(-33, -5, "CCkkCC")
    c.P(-32, -4, "kUUk")
    c.P(26, -6, "UUkCCk")
    c.P(27, -5, "kkCCk")
    c.P(36, -6, "kkk")
    c.cells[(13, -6)] = "f"
    c.cells[(14, -5)] = "M"
    return finish(c, patches=[(-58, -53, 8, 12, "Titanium"), (25, 33, -7, -3, "Composite"),
                              (-19, -15, 3, 7, "Composite"), (10, 17, -8, -3, "Titanium"),
                              (-60, -56, -4, -1, "Composite")])


def colony_freighter():
    """Trader: colony freighter. A whole mining settlement moving out to a
    new claim: a tug pushing a keel with three crossbeams that shorten toward
    the bow, so the ship is a delta. The family homes ride on the beams like
    fruit on a branch, two on each aft beam, one on the wheelhouse roof; the
    aft beam's tips carry the outboard engine pods. Seed cages, tools and
    water strapped to the keel between."""
    c = Canvas("fhc_colony_freighter", "Steel")
    segs(c, {
        # the tug
        7: [(-40, "#######")],
        6: [(-41, "E.Q.Q.Q.#")],
        5: [(-42, "E.R......#")],
        4: [(-43, "E.c.GG.O..#")],
        3: [(-44, "E....GG.yb.#")],
        2: [(-44, "E.M......KK.#")],
        1: [(-44, "#R.v.........#")],
        0: [(-44, "E............d")],
        # outboard engine pods on the aft beam's tips
        22: [(-30, "#######")],
        21: [(-31, "E.......#")],
        20: [(-31, "E.M..R..#")],
        19: [(-31, "E....b..#")],
        18: [(-30, "####.###")],
    })
    keel(c, -30, 33, doors={-26, -3, 17})
    rib(c, -27, 2, 18)
    c.P(-27, 18, "#.#")
    # two family homes on each aft beam, doors onto the beam
    home(c, -24, 11)
    home(c, -24, 3, rows=("Q.Q.Q.Q", ".......", "..GG.y.", "..GG.O.", "k.m.k.k"))
    for y in (14, 6):
        c.P(-25, y, ".")
    # the wheelhouse across the keel, a short beam up to one more home
    c.rows(-10, 8, """
     #d#
  ####.####
 #Q.Q...Q.M#
#..........k#
#.GG.O.m.y..#
#.GG..A..T..#
#....H.......#
#......BB....#
.......BB.....
""")
    for y in range(9, 11):
        c.P(-5, y, "#.#")
    home(c, -8, 11, rows=("Q.Q.Q.Q", ".......", "k.m...k", "k.O.y.k", "Q.Q.Q.Q"), door="-")
    c.P(-4, 11, "d")
    # water and tools between the wheelhouse and the fore beam
    drum(c, 7, 3, 7, 5, 2)
    c.P(9, 5, "M")
    c.P(9, 2, "# #")
    rib(c, 16, 2, 8)
    c.P(16, 8, "g.p")
    c.P(16, 9, "###")
    # a grain funnel ahead of the fore beam
    c.rows(21, 6, """
#######
#kCCkk#
 #CCk#
  ###
  # #
""")
    # the push-bar
    aft = {0: 34, 1: 34, 2: 33, 3: 33, 4: 32, 5: 32, 6: 31, 7: 30, 8: 29}
    bumper(c, aft, {0: "...o", 1: "#..#", 2: "#M.#", 3: "#..g", 4: "#a.#", 5: "#..#",
                    6: "#..#", 7: "#..#", 8: "##p#"})
    c.mirror()
    plates(c, [(37, 2), (36, 4), (35, 6), (34, 7), (33, 8)])
    # one family's add-on: a lean-to cabin welded under the outer home
    c.P(-21, -18, "#Q.k#")
    c.P(-20, -19, "###")
    c.P(-19, -17, "d")
    c.P(-19, -16, ".")
    return finish(c, patches=[(-31, -23, 18, 22, "Titanium"), (-24, -16, -9, -3, "Composite"),
                              (7, 13, 3, 7, "Composite"), (-9, -1, 11, 17, "Titanium"),
                              (-21, -17, -19, -18, "Composite")], asym=8)


def convoy_tanker():
    """Trader: convoy tanker. The fuel for a whole convoy in one hull: a
    keel with a row of round bladders slung either side, small at the ends
    and big amidships, so the ship is a lens of circles. The wheelhouse sits
    in the middle of the row as one more round load, with a gun tub on its
    roof; a pair of engine pods hang off the tug."""
    c = Canvas("fhc_convoy_tanker", "Steel")
    segs(c, {
        7: [(-51, "########")],
        6: [(-51, "E.Q.Q.Q.#")],
        5: [(-52, "E.R...O..#")],
        4: [(-53, "E.c.GG.m.y#")],
        3: [(-54, "E....GG.b..#")],
        2: [(-54, "E.M....KK..#")],
        1: [(-54, "#R.c........#")],
        0: [(-54, "E...........d")],
        # engine pods on struts off the tug's flanks
        13: [(-52, "#####")],
        12: [(-53, "E....#")],
        11: [(-53, "E.M..#")],
        10: [(-53, "E....#")],
        9: [(-52, "##d##")],
        8: [(-51, "#.#")],
    })
    c.P(-50, 7, ".")
    keel(c, -41, 35)
    # bladders, small to big toward the middle
    bladder(c, -39, 7, 7, 2, core=(3, 3))
    bladder(c, -30, 9, 9, 3)
    bladder(c, -19, 10, 9, 3, core=(5, 4))
    bladder(c, 9, 10, 9, 3)
    bladder(c, 21, 8, 8, 2, core=(4, 4))
    # the wheelhouse: one more round load, a gun tub on its roof
    c.rows(-6, 10, """
     ###
     g.p
   ###.###
  #Q.....Q#
 #.Q.....Q.#
#..GG.m.y..M#
#..GG.......#
#.T...O..A..#
#.....H.....#
#.....BB....#
d.....BB....d
""")
    # the push-bar
    aft = {0: 36, 1: 36, 2: 35, 3: 35, 4: 34, 5: 34, 6: 33, 7: 32, 8: 31, 9: 30}
    bumper(c, aft, {0: "...o", 1: "#..#", 2: "#M.#", 3: "#..g", 4: "#a.#", 5: "#..#",
                    6: "#..#", 7: "#..#", 8: "#..#", 9: "##p#"})
    c.mirror()
    plates(c, [(39, 2), (38, 4), (37, 6), (36, 7), (35, 8), (34, 9)])
    return finish(c, patches=[(-53, -48, 8, 13, "Titanium"), (-19, -10, -12, -3, "Composite"),
                              (21, 28, 3, 10, "Composite"), (-39, -33, -9, -3, "Titanium")])


def union_hall():
    """Flagship: union mothership. The Compact's travelling town and its
    parliament: a big tug shoving three keels at once, the centre one and
    two side streets, tied by crossbeams with open sky between. The union
    hall sits across the middle with the bridge, the galleys and the
    clinic; family homes and a workshop line the outer side of each
    street, a docking arm for members' haulers at the bow corners, engine
    pods on the streets' aft ends and one broad push-bar across all three."""
    c = Canvas("fhc_union_hall", "Steel")
    segs(c, {
        # the tug
        9: [(-53, "########")],
        8: [(-54, "E.Q.Q.Q.#")],
        7: [(-55, "E.R.......#")],
        6: [(-56, "E.c.GG.O.y..#")],
        5: [(-57, "E.R..GG.b.m..#")],
        4: [(-57, "E.v.......KK.#")],
        3: [(-57, "E.LL.b..y....#")],
        2: [(-57, "E.M......O...#")],
        1: [(-57, "#R.c.........#")],
        0: [(-57, "E............d")],
        # engine pods on the side streets' aft ends
        15: [(-41, "####")],
        14: [(-42, "E.R.#")],
        13: [(-42, "E.M..")],
        12: [(-42, "E....")],
        11: [(-42, "E.b..")],
        10: [(-42, "E.c.#")],
        9: [(-41, "####")],
    })
    keel(c, -43, 39, doors={-34, 21})
    rail(c, -37, 31, 12, lo={-34, 0, 21}, hi={-28, -17, -6, 12})
    for x in (-35, 20):
        rib(c, x, 2, 10)
    for y in (10, 11):
        c.P(-1, y, "#.#")
    # the union hall across the middle
    segs(c, {
        9: [(-4, "####d####")],
        8: [(-5, "#Q.Q.Q.Q.Q#")],
        7: [(-7, "#......M......#")],
        6: [(-8, "#.www.GG.GG.www.#")],
        5: [(-8, "#.www.GG.GG.www.#")],
        4: [(-8, "#.www...T...www.#")],
        3: [(-8, "#.m.SSS..O.q.c..#")],
        2: [(-8, "#.y.SSS.H....A.y#")],
        1: [(-8, "#.......BB......#")],
        0: [(-8, "........BB.......")],
    })
    # the streets' outer side: two homes and a workshop, doors down
    home(c, -32, 14, door="s")
    home(c, -21, 14, rows=("Q.Q.Q.Q", ".......", "k.GG.m.", "..GG.O.", "Q.Q.k.k"), door="s")
    home(c, -10, 14, rows=("y.%%.yy", ".......", "k.....k", "..y.y..", "k.O.k.k"), door="s")
    # docking arm for members' haulers
    segs(c, {
        19: [(10, "#hhh#")],
        18: [(10, "ohhho")],
        17: [(10, "#hhh#")],
        16: [(10, "##.##")],
        15: [(11, "#.#")],
        14: [(11, "#.#")],
    })
    # loads in the bays: a funnel on the keel, a bladder hung off the
    # street, a scrap cage forward
    c.rows(-30, 7, """
#########
#kCCkCCk#
 #CCkCC#
  #kkk#
   ###
   # #
""")
    drum(c, -19, 5, 7, 5, 2)
    c.P(-17, 10, "# #")
    c.P(-16, 7, "M")
    c.rows(12, 7, """
 ###
#kkk#
#k k#
#kkk#
 ###
 # #
""")
    # the push-bar across all three keels
    aft = {0: 40, 1: 40, 2: 39, 3: 39, 4: 38, 5: 38, 6: 37, 7: 36, 8: 35, 9: 34,
           10: 33, 11: 32, 12: 31, 13: 30, 14: 29, 15: 28}
    bumper(c, aft, {0: "...o", 1: "#..#", 2: "#M.#", 3: "#..g", 4: "#a.#", 5: "#..#",
                    6: "#..#", 7: "#..p", 8: "#..#", 9: "#..#", 10: "#..#", 11: "#..#",
                    12: "...g", 13: "#..#", 14: "#..#", 15: "##p#"})
    c.mirror()
    plates(c, [(43, 2), (42, 4), (41, 6), (40, 7), (39, 8), (38, 9), (37, 10), (36, 11),
               (34, 13), (33, 14), (32, 15)])
    # one family's add-on: a lean-to cabin on the first home's roof
    c.P(-30, 21, "#Q.k#")
    c.P(-29, 22, "###")
    c.P(-28, 20, "d")
    return finish(c, patches=[(-42, -38, 9, 15, "Titanium"), (-22, -14, -20, -14, "Composite"),
                              (-10, -2, 14, 20, "Titanium"), (12, 16, -7, -3, "Composite"),
                              (-57, -54, -5, -1, "Composite"),
                              (-30, -26, 21, 22, "Composite")], asym=8)


def smelter():
    """Experimental: mobile refinery. It eats ore at the bow and leaves fuel
    and ingots aft. The push-bar carries drills and a tractor instead of
    guns; crusher funnels feed a smelter of four fusion furnaces amidships,
    and its heat goes out along two radiator spars, fins tapering toward
    the tips like a fern. Cages of ingots and a bladder of refined fuel
    ride between the smelter and the tug."""
    c = Canvas("fhc_smelter", "Steel")
    segs(c, {
        7: [(-40, "#######")],
        6: [(-41, "E.Q.Q.Q.#")],
        5: [(-42, "E.R......#")],
        4: [(-43, "E.c.GG.O..#")],
        3: [(-44, "E....GG..b.#")],
        2: [(-44, "E.M....KK..#")],
        1: [(-44, "#R.y........#")],
        0: [(-44, "E...........d")],
        # engine pods on struts off the tug's flanks
        13: [(-42, "#####")],
        12: [(-43, "E....#")],
        11: [(-43, "E.M..#")],
        10: [(-43, "E....#")],
        9: [(-42, "##d##")],
        8: [(-41, "#.#")],
    })
    c.P(-40, 7, ".")
    c.P(-41, 7, "#")
    keel(c, -31, 33)
    # aft: an ingot cage and a bladder of refined fuel
    c.rows(-28, 8, """
 #####
#kkkkk#
#k   k#
#k   k#
#kkkkk#
 #####
  # #
""")
    bladder(c, -19, 8, 7, 2, core=(3, 3))
    # the smelter: four fusion furnaces round the control room
    segs(c, {
        9: [(-5, "#####.#####")],
        8: [(-7, "#v.c.c.c.c.c.v#")],
        7: [(-8, "#.FFF.......FFF.#")],
        6: [(-9, "#..FFF...M...FFF..#")],
        5: [(-9, "#..FFF.......FFF..#")],
        4: [(-9, "#.c...b..T..b...c.#")],
        3: [(-9, "#.y.O....A....O.m.#")],
        2: [(-9, "#.Q.Q....H....Q.Q.#")],
        1: [(-9, "#........BB.......#")],
        0: [(-9, ".........BB........")],
    })
    # the radiator spar, fins tapering toward the tip
    rib(c, -1, 10, 24, cap="v")
    for y, n in ((11, 5), (13, 5), (15, 4), (17, 4), (19, 3), (21, 2), (23, 1)):
        c.P(-1 - n, y, "v" * n)
        c.P(2, y, "v" * n)
    # fore: two crusher funnels
    c.rows(12, 7, """
#########  #######
#kCCkCCk#  #kUUkk#
 #CCkCC#    #kkk#
  #kkk#      #k#
   ###       ###
   # #       # #
""")
    # the push-bar, drills and a tractor where the guns would be
    aft = {0: 34, 1: 34, 2: 33, 3: 33, 4: 32, 5: 32, 6: 31, 7: 30, 8: 29, 9: 28}
    bumper(c, aft, {0: "...t", 1: "#..&", 2: "#M.#", 3: "#..&", 4: "#a.#", 5: "#..g",
                    6: "#..#", 7: "#..#", 8: "#..#", 9: "##p#"})
    c.mirror()
    plates(c, [(37, 2), (36, 4), (35, 6), (34, 7), (33, 8), (32, 9)])
    return finish(c, patches=[(-43, -38, 8, 13, "Titanium"), (-1, 1, -24, -16, "Composite"),
                              (12, 20, -7, -3, "Composite"), (-28, -22, 3, 8, "Titanium")])


def hoop(c, hx, r, d, inner):
    """Top half of an octagonal hoop centred on (hx, 0): outer box r, outer
    corner cut d, three cells of truss round an open middle (the middle is
    `inner` = (r', d')). Interior truss cells become walkway."""
    ring = set()
    for x in range(hx - r, hx + r + 1):
        for y in range(0, r + 1):
            m, l1 = max(abs(x - hx), y), abs(x - hx) + y
            if m <= r and l1 <= d and not (m <= inner[0] and l1 <= inner[1]):
                ring.add((x, y))
    full = ring | {(x, -y) for x, y in ring}
    for p in ring:
        inside = all((p[0] + dx, p[1] + dy) in full for dx, dy in N4)
        c.cells[p] = "." if inside else "#"
    return ring


def wrangler():
    """Experimental: asteroid tow rig. Why haul ore when you can haul the
    rock? A tug and a wheelhouse pushing a great octagonal hoop of truss with
    nothing in the middle: the rock goes there. Tractors round the inside
    hold it, drills bite at the front, floodlights light the work; the
    hoop's front facets are the push-bar and carry its plates. Bladders of
    reaction mass on the keel, two big engine pods off the tug."""
    c = Canvas("fhc_wrangler", "Steel")
    segs(c, {
        7: [(-44, "#######")],
        6: [(-45, "E.Q.Q.Q.#")],
        5: [(-46, "E.R......#")],
        4: [(-47, "E.c.GG.O..#")],
        3: [(-48, "E....GG..b.#")],
        2: [(-48, "E.M....KK..#")],
        1: [(-48, "#R.y........#")],
        0: [(-48, "E...........d")],
        # big engine pods off the tug's flanks
        14: [(-46, "######")],
        13: [(-47, "E.....#")],
        12: [(-47, "E.R.M.#")],
        11: [(-47, "E.....#")],
        10: [(-47, "E.b.c.#")],
        9: [(-46, "##d###")],
        8: [(-45, "#.#")],
    })
    c.P(-45, 7, "#.")
    keel(c, -35, -1)
    # reaction mass
    bladder(c, -32, 9, 7, 3, core=(4, 3))
    # the wheelhouse
    c.rows(-16, 8, """
    #######
   #Q.Q.Q.Q#
  #.........#
 #.GG.m.O.y.M#
#..GG.........#
#.T...q.Q..A..#
#...m..H......#
#......BB.....#
.......BB......
""")
    # the hoop
    hx = 17
    keel(c, -1, hx - 15)
    hoop(c, hx, 14, 22, (11, 18))
    c.cells[(hx - 14, 0)] = "d"
    c.cells[(hx - 12, 0)] = "t"
    for p, ch in (((hx - 12, 5), "t"), ((hx - 9, 10), "l"), ((hx - 5, 12), "l"), ((hx + 5, 12), "l"),
                  ((hx + 10, 9), "t"), ((hx + 12, 6), "l"), ((hx + 12, 3), "&")):
        c.cells[p] = ch
    # a grapple reaching down into the hoop from the top side
    c.cells[(hx, 12)] = "d"
    for y in (11, 10, 9):
        c.P(hx - 1, y, "#.#")
    c.P(hx - 1, 8, "#t#")
    c.cells[(hx + 14, 0)] = "o"
    c.cells[(hx + 14, 3)] = "g"
    c.cells[(hx, 14)] = "p"
    c.mirror()
    plates(c, [(hx + 23 - y, y) for y in range(9, 15)])
    return finish(c, patches=[(-47, -42, 9, 14, "Titanium"), (hx - 3, hx + 3, 12, 14, "Composite"),
                              (hx + 9, hx + 14, -8, -2, "Titanium"), (-32, -24, -9, -3, "Composite")])


def outrigger(c, x, y0, y1, step, doors=True):
    """A four-wide outrigger on the diagonal, top half: on row y0 it spans
    x..x+3, and each row up it moves `step` (+1 swept forward, -1 swept
    aft), up to row y1. Opens the keel rim under its foot."""
    for y in range(y0, y1 + 1):
        c.P(x + step * (y - y0), y, "#..#")
    if doors:
        c.P(x + 1, y0 - 1, "..")


def outrider():
    """Escort: convoy escort. The Compact has no warships; this is a
    hauler's frame loaded with guns instead of cargo, riding shotgun on a
    convoy until the Terrans turn up. Four outriggers off the keel make an
    X: the forward pair carry gun tubs, the aft pair carry the engine pods
    and more guns. Ammunition rides in a hopper where the ore would be."""
    c = Canvas("fhc_outrider", "Steel")
    segs(c, {
        7: [(-36, "#######")],
        6: [(-37, "E.Q.Q.Q.#")],
        5: [(-38, "E.R...O..#")],
        4: [(-39, "E.c.GG.m.y#")],
        3: [(-40, "E....GG.b..#")],
        2: [(-40, "E.M....KK..#")],
        1: [(-40, "#R.c........#")],
        0: [(-40, "E...........d")],
    })
    keel(c, -27, 31)
    # outriggers: swept aft to the engine tubs, swept forward to the gun tubs
    outrigger(c, -14, 2, 12, -1)
    outrigger(c, 10, 2, 12, +1)
    segs(c, {
        16: [(-24, "#pg#"), (20, "#pg#")],
        15: [(-25, "E....#"), (19, "#....#")],
        14: [(-25, "E.a..g"), (19, "g.aa.g")],
        13: [(-25, "E....#"), (19, "#....#")],
    })
    # the wheelhouse, a gun tub on its roof
    c.rows(-7, 10, """
      ###
      g.p
    ###.###
   #Q.Q.Q.Q#
  #.........#
 #.GG.m.O.y.M#
#..GG.........#
#.T...q.a..A..#
#...a..H....a.#
#......BB.....#
.......BB......
""")
    # fuel aft, ammunition forward, strapped to the keel
    bladder(c, -25, 6, 4, 1, core=(2, 1))
    c.rows(20, 6, """
######
#aaaa#
 #aa#
 ####
 #  #
""")
    # the push-bar
    aft = {0: 32, 1: 32, 2: 31, 3: 31, 4: 30, 5: 30, 6: 29, 7: 28, 8: 27, 9: 26}
    bumper(c, aft, {0: "...o", 1: "#..g", 2: "#M.#", 3: "#..g", 4: "#a.#", 5: "#..p",
                    6: "#..#", 7: "#..g", 8: "#..#", 9: "##g#"})
    c.mirror()
    plates(c, [(35, 2), (34, 4), (33, 6), (31, 8), (30, 9)])
    return finish(c, patches=[(-25, -20, 13, 16, "Titanium"), (19, 24, -16, -13, "Composite"),
                              (-40, -37, -4, -1, "Composite"), (20, 25, 3, 6, "Titanium")])


def iceman():
    """Trader: ice hauler. Water for the mining camps, cut from comets and
    pushed home. A short tug and a small wheelhouse behind a broad, shallow
    push-bar that works as an ice plough, tractors on its face to catch the
    chunks; two hollow ice cages ride in the lee of its wings, open sky all
    round them."""
    c = Canvas("fhc_iceman", "Steel")
    segs(c, {
        5: [(-22, "#######")],
        4: [(-22, "E.Q.Q.O#")],
        3: [(-23, "E.R.....#")],
        2: [(-24, "E.c.M.y..#")],
        1: [(-24, "#R.b.......#")],
        0: [(-24, "E.........d")],
        # a pair of engine pods on struts
        10: [(-23, "#####")],
        9: [(-24, "E.M..#")],
        8: [(-24, "E....#")],
        7: [(-23, "##d##")],
        6: [(-22, "#.#")],
    })
    c.P(-21, 5, ".")
    keel(c, -13, 15)
    # the wheelhouse
    c.rows(-9, 6, """
   #######
  #Q.Q.Q.Q#
 #.GG.m.O..#
#..GG.A..y.M#
#.....H.....#
#.....BB....#
......BB.....
""")
    # ice cages in the lee of the plough
    c.rows(5, 7, """
 #####
#kkkkk#
#k   k#
#kkkkk#
 #####
 #   #
""")
    # the plough: a broad, shallow push-bar
    aft = {0: 16, 1: 16, 2: 15, 3: 15, 4: 14, 5: 14, 6: 13, 7: 13, 8: 12, 9: 12,
           10: 11, 11: 11, 12: 10, 13: 10}
    bumper(c, aft, {0: "...t", 1: "#..#", 2: "#M.#", 3: "#..t", 4: "#..#", 5: "#.a#",
                    6: "#..#", 7: "#..t", 8: "#..#", 9: "#..#", 10: "#..#", 11: "#..p",
                    12: "#..#", 13: "##g#"})
    c.mirror()
    plates(c, [(19, 2), (18, 4), (17, 6), (16, 8), (15, 10), (14, 12)])
    c.P(6, -6, "kVVkk")       # one cage keeps the camp's frozen stores
    return finish(c, patches=[(-24, -19, 6, 10, "Titanium"), (5, 11, -7, -3, "Composite"),
                              (10, 13, 9, 13, "Titanium")])


def gravel_barge():
    """Trader: aggregate hauler. Crushed rock for station hulls, by the
    thousand tonnes. Four wedge hoppers stand round a narrow deckhouse that
    crosses the keel, sloped faces outward, so the barge is a diamond with
    the tug for a tail and the push-bar for a nose. A gatling on each end
    of the deckhouse."""
    c = Canvas("fhc_gravel_barge", "Steel")
    segs(c, {
        5: [(-27, "######")],
        4: [(-28, "E.Q.Q.#")],
        3: [(-29, "E.R.O.y#")],
        2: [(-30, "E.c.M.b.#")],
        1: [(-30, "#R.y.....#")],
        0: [(-30, "E........d")],
    })
    keel(c, -20, 16)
    # the deckhouse across the keel
    segs(c, {
        11: [(-2, "##g##")],
        10: [(-3, "#Q...Q#")],
        9: [(-3, "#Q.M.Q#")],
        8: [(-3, "#.....#")],
        7: [(-3, "#GG.m.#")],
        6: [(-3, "#GG.O.#")],
        5: [(-3, "#.....#")],
        4: [(-3, "#k.y.k#")],
        3: [(-3, "#..H..#")],
        2: [(-3, "#.....#")],
        1: [(-3, "#..BB.#")],
        0: [(-3, "...BB..")],
    })
    # four wedge hoppers, sloped faces outward
    for k, y in enumerate(range(3, 12)):
        aft = "#" * (9 - k) if y in (3, 11) else "#" + "k" * (7 - k) + "#"
        c.P(-14 + k, y, aft)
        c.P(5, y, aft[::-1])
    for x0 in (-13, 6):
        c.P(x0, 2, "#")
        c.P(x0 + 6, 2, "#")
    # bulk holds in the hoppers
    c.P(-11, 5, "CC")
    c.P(-11, 4, "CC")
    c.P(-9, 5, "UU")
    c.P(-9, 7, "CC")
    c.P(-9, 6, "CC")
    c.P(-12, 4, "M")
    c.P(7, 5, "CC")
    c.P(7, 4, "CC")
    c.P(9, 4, "UU")
    c.P(6, 6, "UU")
    c.P(7, 7, "M")
    # the push-bar
    aft = {0: 17, 1: 17, 2: 16, 3: 16, 4: 15, 5: 15, 6: 14}
    bumper(c, aft, {0: "...o", 1: "#..#", 2: "#M.#", 3: "#..g", 4: "#a.#", 5: "#..#",
                    6: "##p#"})
    c.mirror()
    plates(c, [(20, 2), (19, 4), (18, 6)])
    return finish(c, patches=[(5, 13, -11, -3, "Composite"), (-30, -27, 2, 5, "Titanium"),
                              (-14, -10, 3, 6, "Titanium")])


CONCEPTS = [
    ("ore_train", ore_train, "Ore Train", "Trader: ore train",
     "The longest thing the Compact flies: a tug shoving ninety cells of keel with ore funnels slung either side, no two the same. The family's wheelhouse rides across the keel amidships."),
    ("colony_freighter", colony_freighter, "Homesteader", "Trader: colony freighter",
     "A mining settlement moving out to a new claim: three crossbeams shortening toward the bow make a delta, with family homes riding on them like fruit on a branch and the engine pods on the widest beam's tips."),
    ("convoy_tanker", convoy_tanker, "Convoy Tanker", "Trader: convoy tanker",
     "Fuel for a whole convoy in one hull: round bladders slung either side of a keel, small at the ends and big amidships, so the ship is a lens of circles. The wheelhouse rides in the row as one more round load."),
    ("union_hall", union_hall, "Union Hall", "Flagship: union mothership",
     "The Compact's travelling town and parliament: a big tug shoving three keels at once, tied by crossbeams with sky between. The union hall and bridge sit across the middle; family homes, a workshop and docking arms line the outer streets."),
    ("smelter", smelter, "Smelter", "Experimental: mobile refinery",
     "Eats ore at the bow and leaves fuel and ingots aft: drills on the push-bar, crusher funnels, four fusion furnaces amidships and two radiator spars whose fins taper like a fern."),
    ("wrangler", wrangler, "Rock Wrangler", "Experimental: asteroid tow rig",
     "Why haul ore when you can haul the rock? A tug pushing a great octagonal hoop with nothing in the middle: the asteroid rides there, held by tractors round the inside, drills biting at the front."),
    ("outrider", outrider, "Outrider", "Escort: convoy escort",
     "The Compact has no warships: this is a hauler's frame loaded with guns instead of cargo, riding shotgun until the Terrans turn up. Four outriggers make an X, gun tubs forward, engine pods aft."),
    ("iceman", iceman, "Iceman", "Trader: ice hauler",
     "Water for the mining camps, cut from comets and pushed home: a broad, shallow push-bar that works as an ice plough, tractors on its face, two hollow ice cages riding in the lee of its wings."),
    ("gravel_barge", gravel_barge, "Gravel Barge", "Trader: aggregate hauler",
     "Crushed rock for station hulls: four wedge hoppers round a narrow deckhouse across the keel, sloped faces outward, so the barge is a diamond with the tug for a tail and the push-bar for a nose."),
]
