"""Free Haulers' Compact: hand-drawn, top-down only.

Lore: a union of independent haulers, miners and salvagers who refuse the
Merchant Princes' monopoly. They move bulk goods, fuel, scrap and ammunition,
cheap and plain. They have no real warfleet (their allies defend them); every
hull is a civilian workhorse, patched, practical, and it shoots back.

Grammar, held at every size (all of it reads from above):
  - a PUSH-TUG at the stern: a stubby, chamfered crew-and-engine block that
    shoves its load ahead of it like a river push-boat (never a spine);
  - an open LOAD FRAME ahead of the tug: a three-wide keel beam and
    crossbeams with open sky between, ending in a stepped BUMPER bow, the
    only place that carries plates;
  - MISMATCHED LOADS strapped into the frame on one-cell straps, a gap all
    round: wedge-shaped ore hoppers, scrap cages with a hole in the middle,
    rounded fuel bladders, never two the same;
  - BOLT-ON ENGINES: extra engine pods on struts, on one side only, because
    that's where there was room;
  - PATCHWORK hull: Steel, with salvaged panels of Titanium and Composite
    where it has been holed and mended, so the repairs read without colour.

Legend: see terran.py and draw.LEGEND.
"""
from draw import Canvas

FACTION = {
    "slug": "free_haulers_compact",
    "name": "Free Haulers' Compact",
    "tagline": "A union of independent haulers, miners and salvagers who refuse the Merchant Princes' monopoly. Cheap, plain, patched, and they shoot back.",
    "grammar": [
        "A push-tug at the stern: a stubby, chamfered crew-and-engine block that shoves its load ahead",
        "An open load frame: three-wide keel and crossbeams, open sky between, a stepped bumper bow (the only plates)",
        "Mismatched loads on one-cell straps: wedge ore hoppers, hollow scrap cages, rounded fuel bladders",
        "Bolt-on engine pods on struts, one side only, wherever there was room",
        "Patchwork hull: Steel mended with salvaged Titanium and Composite panels",
    ],
    "accent": "#b4623a",
}


def _patched(c, patches):
    """Build, then re-skin hand-chosen rectangles of hull in salvaged
    material: (x0, x1, y0, y1, material). Repairs, not noise."""
    d = c.build()
    for cell in d["hull_cells"]:
        x, y = cell["grid_pos"]
        for x0, x1, y0, y1, mat in patches:
            if x0 <= x <= x1 and y0 <= y <= y1:
                cell["material"] = mat
    return d


def tug():
    """Workboat. A one-room push-tug shoving a short keel with an ore hopper
    strapped above and a fuel bladder below, a stepped bumper on the nose and
    one bolt-on engine pod up on a strut."""
    c = Canvas("fhc_tug", "Steel")
    # tug, keel and the push-bar bumper, drawn top half and mirrored
    c.rows(-10, 5, """
               ##/
               #.#/
 ######        #..g
E.R.Q.#        #..#
E.....##########..#
 ..H..d...........t
""")
    c.mirror()
    # the tug's rooms, one side and the other
    c.P(-10, 1, "E.O.M.#")
    c.P(-10, -1, "E.c...#")
    c.P(-10, -2, "E.R.k.#")
    c.P(7, 2, "M")
    # bolt-on engine pod on a strut, top side only
    c.rows(-10, 7, """
  ###
 E..#
 EM.#
  #.#
   d
""")
    # ore hopper strapped above the keel, a wedge narrowing to the straps
    c.P(-2, 5, "#####")
    c.P(-2, 4, "#UUk#")
    c.P(-1, 3, "###")
    c.P(-1, 2, "# #")
    # fuel bladder strapped below, further forward, rounded
    c.P(0, -2, "# #")
    c.P(0, -3, "###")
    c.P(-1, -4, "#fff#")
    c.P(0, -5, "###")
    return _patched(c, [(-9, -6, 4, 7, "Titanium"), (1, 3, -5, -3, "Composite"), (-10, -8, -3, -2, "Composite")])


def _bumper(c, aft, rows):
    """The push-bar bow, top half: a four-thick bar bent back into a shallow
    chevron. aft[y] is its aft x on row y; rows[y] is what it holds, aft to
    front. Plates go on the bow steps after mirror()."""
    for y, s in rows.items():
        c.P(aft[y], y, s)


def ore_hauler():
    """Bulk hauler. A tug pushing an open frame: a keel, one rib and a
    chevron push-bar, four mismatched loads strapped into the bays between
    them (a wedge ore hopper, a scrap cage, two silos and a deep hopper), and
    one engine pod bolted on underneath on a strut."""
    c = Canvas("fhc_ore_hauler", "Steel")
    # the tug (top half)
    c.rows(-16, 5, """
  #######
 E.Q.m.Q.#
E.R.GG.O.#
E...GG...#
E.c..BB..#
#.M..BBH.d
""")
    # the keel, and one rib across it
    c.P(-6, 1, "#" * 22)
    c.P(-6, 0, "." * 22)
    c.P(4, 1, ".")
    c.P(3, 2, "#d#")
    for y in range(3, 7):
        c.P(3, y, "#.#")
    c.P(3, 7, "#p#")
    # the push-bar
    aft = {0: 16, 1: 16, 2: 15, 3: 15, 4: 14, 5: 14, 6: 13, 7: 12}
    _bumper(c, aft, {0: "...o", 1: "#..#", 2: "#M.#", 3: "#..g", 4: "#a.#", 5: "#..#",
                     6: "#..#", 7: "##g#"})
    c.mirror()
    for x, y in ((19, 2), (18, 4), (17, 6), (16, 7)):
        c.P(x, y, "/")
        c.P(x, -y, "/")
    c.P(3, 5, "M")              # a core in each rib segment
    c.P(5, -5, "M")
    # the tug's other side
    c.P(-16, -1, "E.v..BB..#")
    c.P(-16, -2, "E...ff...#")
    c.P(-16, -3, "E.R.KK.y.#")
    c.P(-15, -4, "E.Q.m.q.#")
    c.P(-12, -5, "d")
    # bolt-on engine pod, underneath on a strut
    c.rows(-15, -6, """
  #.#
E.....#
E.M...#
E.....#
 #####
""")
    # bay A, top: a wedge ore hopper, sloped on its aft side
    c.P(-5, 7, "#######")
    c.P(-5, 6, "#kCCkM#")
    c.P(-4, 5, "#CCUU#")
    c.P(-3, 4, "#kkk#")
    c.P(-2, 3, "###")
    c.P(-2, 2, "# #")
    # bay A, bottom: two silos of different depths
    c.P(-4, -2, "#   #")
    c.P(-5, -3, "### ###")
    for y in (-4, -5):
        c.P(-5, y, "#k#")
    c.P(-1, -4, "#k#")
    c.P(-1, -5, "###")
    c.P(-5, -6, "#k#")
    c.P(-5, -7, "###")
    # bay B, top: a scrap cage, open in the middle
    c.P(8, 7, "###")
    c.P(7, 6, "#kkk#")
    c.P(7, 5, "#k k#")
    c.P(7, 4, "#kkk#")
    c.P(8, 3, "###")
    c.P(8, 2, "# #")
    # bay B, bottom: a deep hopper, widening outboard
    c.P(8, -2, "# #")
    c.P(8, -3, "####")
    c.P(7, -4, "#kUU#")
    c.P(7, -5, "#CCk#")
    c.P(7, -6, "#CCk#")
    c.P(7, -7, "#####")
    return _patched(c, [(-16, -10, -10, -6, "Titanium"), (-16, -14, 2, 5, "Composite"),
                        (7, 11, 3, 7, "Composite"), (3, 5, -7, -5, "Composite"),
                        (-5, -3, -7, -6, "Titanium")])


def fuel_runner():
    """Tanker. A taller tug pushing an open ladder truss: two rails joined by
    two rungs, fuel bladders of three sizes strapped in the gaps, two more
    slung outboard underneath. Gun tubs on the rung tips and the push-bar,
    because the Compact shoots back. One engine pod bolted on top."""
    c = Canvas("fhc_fuel_runner", "Steel")
    # the tug (top half)
    c.rows(-16, 6, """
   ######
  E.Q.Q.d
 E.R.....#
E.c.GG.O#
E...GG..#
E.M..BB.#
#....BBH#
""")
    # the ladder: two rails, two rungs
    c.P(-7, 6, "#" * 27)
    c.P(-7, 5, "." * 27)
    c.P(-7, 4, "#" * 27)
    for x in (2, 12):
        c.P(x + 1, 4, ".")
        c.P(x, 3, "#d#")
        for y in range(0, 3):
            c.P(x, y, "#.#")
    # bladders between the rails: big, middling, small
    c.P(-5, 3, " #   # ")
    c.P(-5, 2, "#####")
    c.P(-6, 1, "#fffff#")
    c.P(-6, 0, "#fffff#")
    c.P(7, 3, "# #")
    c.P(7, 2, "###")
    c.P(6, 1, "#fff#")
    c.P(6, 0, "#fff#")
    c.P(18, 3, "#")
    c.P(17, 2, "##")
    c.P(16, 1, "#ff#")
    c.P(16, 0, "#ff#")
    # the push-bar
    aft = {0: 22, 1: 22, 2: 21, 3: 21, 4: 20, 5: 20, 6: 19, 7: 18}
    _bumper(c, aft, {0: "#..o", 1: "#..#", 2: "#M.#", 3: "#..g", 4: "#a.#", 5: "...#",
                     6: "#..#", 7: "##g#"})
    c.mirror()
    for x, y in ((25, 2), (24, 4), (23, 6), (22, 7)):
        c.P(x, y, "/")
        c.P(x, -y, "/")
    c.P(2, 0, "M")              # a core in each rung's middle
    c.P(14, 0, "M")
    # the tug's other side
    c.P(-16, -1, "E.c..BB.#")
    c.P(-16, -2, "E.b.ff..#")
    c.P(-16, -3, "E.y.KK.m#")
    # bolt-on engine pod on rung one, top side
    c.rows(0, 12, """
 #####
E....#
E.M..#
E....#
 ##d##
  #.#
""")
    # gun tubs on the other rung tips
    for x, y, s in ((12, 7, 1), (2, -7, -1), (12, -7, -1)):
        c.P(x, y, "#.#")
        c.P(x, y + s, "p.g")
        c.P(x, y + 2 * s, "###")
    # two bladders slung outboard underneath, mismatched
    c.P(-4, -7, "#  #")
    c.P(-5, -8, "#####")
    c.P(-6, -9, "#fffff#")
    c.P(-6, -10, "#ffMff#")
    c.P(-5, -11, "#####")
    c.P(7, -7, "# #")
    c.P(7, -8, "###")
    for y in (-9, -10):
        c.P(6, y, "#fff#")
    c.P(7, -11, "###")
    return _patched(c, [(-1, 6, 8, 13, "Titanium"), (-16, -14, -6, -3, "Composite"),
                        (16, 19, -3, 3, "Composite"), (6, 10, -11, -9, "Composite"),
                        (12, 14, 7, 9, "Titanium")])


def _arm(c, k, block):
    """Paint an arm drawn as the +x arm (rows top line at local y = len//2,
    column 0 at local x = 10), turned k quarter-turns anticlockwise."""
    lines = block.strip("\n").split("\n")
    top = len(lines) // 2
    for r, line in enumerate(lines):
        for i, ch in enumerate(line):
            if ch == " ":
                continue
            x, y = 10 + i, top - r
            for _ in range(k):
                x, y = -y, x
            c.cells[(x, y)] = ch


def depot():
    """Station. A Compact trade depot: an octagonal ring of truss round an
    open repair yard, floodlit from the inside. Four arms of different
    lengths: a pier with a docking head, a short berth, a salvage crane and
    a fuel pier lined with tanks. Between them, strapped to the ring like a
    tug's load, whatever the Compact had: ore hoppers, scrap cages, fuel
    bladders, a power block, and an old tug's crew hull welded on as the hab."""
    c = Canvas("fhc_depot", "Steel")
    # the ring: an octagon of truss three thick, the yard open inside it
    for x in range(-10, 11):
        for y in range(-10, 11):
            m, d = max(abs(x), abs(y)), abs(x) + abs(y)
            if m <= 10 and d <= 17 and (m >= 8 or d >= 12):
                c.cells[(x, y)] = "#"
    ring = set(c.cells)
    for (x, y) in ring:
        if all((x + dx, y + dy) in ring for dx, dy in ((0, 1), (0, -1), (1, 0), (-1, 0))):
            c.cells[(x, y)] = "."
    # the pier (east): a long arm to a docking head
    _arm(c, 0, """
               #o#
               #.#
               #.o
##############M#.#
d.................o
##############a#.#
               #.o
               #.#
               #o#
""")
    # the berth (north): short, two ports and a floodlight
    _arm(c, 1, """
    #p#
##M##..o
d.......o
#####..o
    #l#
""")
    # the salvage crane (west)
    _arm(c, 2, """
       #X
####M####.#
d.........t
#########.#
       #X
""")
    # the fuel pier (south): tanks either side of a catwalk
    _arm(c, 3, """
  #####M#####g
#fffffffffff#.#
d.............o
#fffffffffff#.#
  ###########g
""")
    # floodlights round the yard, a tractor to hold work in it
    for p in ((8, 4), (8, -4), (-8, 4), (-8, -4), (4, -8), (-4, -8), (-4, 8)):
        c.cells[p] = "l"
    c.cells[(4, 8)] = "t"
    c.cells[(0, 8)] = "M"
    c.cells[(0, -8)] = "M"
    # NE: two ore hoppers on the ring, funnels pointing in
    c.rows(11, 8, """
   ####
  #UUk#
 #kCCk#
 #kCCk#
 #kkkk#
  #####
""")
    c.P(11, 6, "#")
    c.P(11, 4, "#")
    c.rows(4, 20, """
 ###
#UUk#
 ###
 # #
 ###
#kUU#
#kkk#
 ###
#   #
""")
    c.rows(19, 10, """
 ##
#kk#
#UU#
#kk#
 ##
""")
    c.P(18, 8, "#")
    c.P(18, 7, "#")
    # NW: the hab, an old tug's crew hull with its engines pulled, the
    # empty mounts capped with heat vents
    c.rows(-22, 12, """
 #########
#Q.Q.Q.Q.#
v........#
#GG.BBB.m#
#GG.BBB.O#
v...H....#
#R.c...b...d
#y...M..q#
 #########
""")
    c.P(-12, 7, "##")
    c.P(-12, 5, "##")
    # NW, on the top face: a scrap cage
    c.rows(-8, 16, """
 ###
#kkk#
#k k#
#kkk#
 ###
 # #
""")
    # SW: fuel bladders
    c.rows(-18, -4, """
 #####
#fffff#
#ffMff#
#fffff#
 #####
""")
    c.P(-11, -5, "#")
    c.P(-11, -7, "#")
    c.rows(-25, -4, """
 ####
#ffff#
#ffff#
#ffff#
#ffff#
 ####
""")
    c.P(-19, -5, "#")
    c.P(-19, -7, "#")
    c.rows(-8, -11, """
 # #
 ###
#fff#
#fff#
#fMf#
#fff#
 ###
""")
    # SE: the power block, another old tug hull, the depot's second reactor
    c.rows(11, -3, """
  ######
 #R.b.c.v
d.......#
 #a.M...#
 #aa..R.v
 #......#
 #kk.y.O#
  ######
""")
    c.P(11, -4, "#")
    c.P(11, -6, "#")
    # SE, on the bottom face: a long scrap cage
    c.rows(4, -11, """
 #  #
 ####
#kkkk#
#k  k#
#kkkk#
 ####
""")
    # gun tubs squared off onto three corners of the ring
    for sx, sy in ((1, 1), (1, -1), (-1, -1)):
        for x, y in ((8, 10), (9, 10), (9, 9), (10, 9), (10, 8)):
            c.cells[(sx * x, sy * y)] = "#"
        c.cells[(sx * 9, sy * 10)] = "g"
        c.cells[(sx * 10, sy * 9)] = "p"
    # a work jetty into the yard, clamps either side
    c.P(-7, 1, "###X#")
    c.P(-7, 0, "....t")
    c.P(-7, -1, "###X#")
    return _patched(c, [(-22, -13, 4, 12, "Titanium"), (12, 19, -10, -3, "Titanium"), (12, 17, 3, 8, "Composite"),
                        (-8, -4, 12, 16, "Composite"), (4, 9, -16, -12, "Composite"),
                        (-10, -8, -4, 4, "Composite"), (6, 9, 8, 10, "Titanium")])


# A whole new faction: every design is new, and the page says so.
REVISED_LABEL = "New faction"
REVISED = {
    "tug": "First draw of the Compact. Push-tug grammar at its smallest.",
    "ore_hauler": "First draw of the Compact. The open load frame with four mismatched loads.",
    "fuel_runner": "First draw of the Compact. Ladder truss variant, with gun tubs.",
    "depot": "First draw of the Compact's station, for the trade-depot slot.",
}

CONCEPTS = [
    ("tug", tug, "Push-Tug", "Civilian: workboat",
     "A one-room tug shoving a short keel with an ore hopper strapped above and a fuel bladder below. One engine pod bolted on top, because that's where there was room."),
    ("ore_hauler", ore_hauler, "Ore Hauler", "Civilian: bulk hauler",
     "A tug pushing an open frame of keel, rib and chevron push-bar, four mismatched loads strapped into its bays: a wedge ore hopper, a scrap cage, two silos and a deep hopper. One engine pod bolted on underneath."),
    ("fuel_runner", fuel_runner, "Fuel Runner", "Civilian: tanker",
     "A taller tug pushing an open ladder truss, fuel bladders of three sizes strapped in its gaps and two more slung outboard. Gun tubs on the rung tips and the push-bar: the Compact shoots back."),
    ("depot", depot, "Haulers' Depot", "Station: trade depot",
     "An octagonal truss ring round an open, floodlit repair yard; four arms of different lengths (pier, berth, salvage crane, fuel pier) and whatever the Compact had strapped to the ring between them, an old tug's crew hull welded on as the hab."),
]
