"""Merchant Princes of the Void: hand-drawn, top-down only.

Lore: a new neutral trading guild that turns up in every system. They trade
across parallel universes, sell weapons to your enemies in the middle of your
battle, and (it is whispered) collapse whole universes to make scarcity.

Grammar, held at every size (all of it reads from above):
  - a SPINE: a three-wide truss with a walkway down its middle, that
    everything else clamps onto;
  - standard CONTAINER PODS, every one the same 5x5 box with its corners
    cut, hung off the spine on short struts with a gap between each;
  - exposed TRUSSWORK: thin three-wide struts with open sky between modules;
  - utilitarian ASYMMETRY: cargo racks fill one side, the crew pod, cranes and
    tanks hang off the other;
  - Steel, light guns only (gatlings, point-defence drones), docking clamps
    at the bow. It should read as freight, never as a warship.

Legend: see terran.py and draw.LEGEND.
"""
from draw import Canvas

FACTION = {
    "slug": "merchant_princes",
    "name": "Merchant Princes of the Void",
    "tagline": "A neutral guild that trades across parallel universes, sells guns to your enemies mid-battle, and is whispered to collapse universes to make scarcity.",
    "grammar": [
        "A spine: a three-wide truss with a walkway down its middle that everything clamps onto",
        "Standard container pods, every one the same 5x5 box with its corners cut, on short struts",
        "Exposed trusswork: thin three-wide struts with open sky between the modules",
        "Utilitarian asymmetry: cargo racks fill one side; crew pod, cranes and tanks hang off the other",
        "Steel, gatlings and point-defence only, docking clamps at the bow: freight, never a warship",
    ],
    "accent": "#7fb069",
}


def pedlar():
    """Small courier trader. Two pods on top of a short spine, a crew pod
    slung underneath, a crane boom aft and a docking clamp on the nose."""
    c = Canvas("mp_pedlar", "Steel")
    c.rows(-9, 6, """
     ###   ###
    #CCk# #CCk#
    #CCk# #CCk#
    #M.k# #k.M#
 ##  #.#   #.# #g#
E.R###.#####.##..#/
E.................o
E.M##.####.####..#/
 ## #.#  #.#   #g#
    #.# #BBB#
    #X# #BBB#
        #Q.H#
         ###
""")
    return c.build()


def _pod(c, x, y, rows):
    """Hang one standard container pod with its bottom-left corner cell at
    (x, y): a 5x5 box with the corners cut. `rows` are its three inside rows,
    top to bottom; `bottom` and `top` doors are the centre of the cut rows."""
    top, mid, low, *ends = rows
    bot_door, top_door = (ends + [".", "#"])[:2] if ends else (".", "#")
    c.P(x + 1, y + 4, "#" + top_door + "#")
    c.P(x, y + 3, "#" + top + "#")
    c.P(x, y + 2, "#" + mid + "#")
    c.P(x, y + 1, "#" + low + "#")
    c.P(x + 1, y, "#" + bot_door + "#")


def hauler():
    """Container hauler. A long spine with four standard pods on one side,
    clamped top and bottom by a second spine (the rack), two more pods stacked
    on the rack amidships; the crew pod, a crane and a fuel pod on the other
    side."""
    c = Canvas("mp_container_hauler", "Steel")
    # tier 2 pods, on top of the rack
    for x0, m in ((-8, "M.k"), (-2, "k.M")):      # stacked two high amidships
        _pod(c, x0, 10, ["CCk", "CCk", m])
    # the rack: a second spine clamping both tiers
    c.P(-15, 9, "#########.#####.#########")
    c.P(-15, 8, "p.......................g")
    c.P(-15, 7, "###.#####.#####.#####.###")
    # tier 1 pods, walk-through between the spine and the rack
    for x0 in (-14, -8, -2, 4):
        _pod(c, x0, 2, ["U.U", "U.U", "k.k", ".", "."])
    # the main spine, engine block aft, docking clamp at the bow
    c.P(-19, 3, "###")
    c.P(-20, 2, "N.R#")
    c.P(-20, 1, "N..." "####.#####.#####.#####.#######" "#...#")
    c.P(-20, 0, "E..." ".............................." ".....o")
    c.P(-20, -1, "N.M." "####.###########.#######.#####" "#...#")
    c.P(-20, -2, "N.R#")
    c.P(-19, -3, "###")
    c.P(15, 4, "######/")           # the clamp: two jaws round a docking port
    c.P(14, 3, "#q.....o")
    c.P(14, 2, "#.O.##g#")
    c.P(14, -2, "#.A.##g#")
    c.P(14, -3, "#Q.....o")
    c.P(15, -4, "######/")
    # under the spine: crane boom aft, crew pod amidships, fuel pod forward
    c.P(-13, -2, "#.#")
    c.P(-13, -3, "#.#")
    c.P(-17, -4, "##########")
    c.P(-18, -5, "X........#")
    c.P(-17, -6, "##p#######")
    _pod(c, -2, -6, ["Q.H", "BBB", "BBB", "#", "."])
    _pod(c, 6, -6, ["fff", "f.f", "f.f", "#", "."])
    return c.build()


def _run(x0, x1, doors=(), fill="#", marks=None):
    """One row of truss from x0 to x1: rim, with a door (deck) at each x in
    `doors` and any other cell set by `marks` {x: ch}."""
    marks = dict(marks or {})
    return "".join(marks.get(x, "." if x in doors else fill) for x in range(x0, x1 + 1))


def _bay(c, x, tiers, pods, ends="pg"):
    """A rack bay: two three-wide struts up from the spine, a rail across the
    top, and two columns of standard pods hung inside it with a gap all round,
    `tiers` deep. pods = [(tier, column, rows)] where rows is what _pod takes.
    Tier-1 pods stand on the spine; tier-2 pods hang from the rail."""
    top = 7 if tiers == 1 else 13
    for sx in (x, x + 16):
        for y in range(2, top):
            c.P(sx, y, "#.#")
    doors = {x + 1, x + 17}
    for tier, col, rows in pods:
        px = x + 4 + 6 * col
        _pod(c, px, 2 if tier == 1 else 8, rows)
        if (tier == 1 and tiers == 1) or tier == 2:
            doors.add(px + 2)
    c.P(x, top, _run(x, x + 18, doors))
    c.P(x, top + 1, ends[0] + "." * 17 + ends[1])
    c.P(x + 1, top + 2, _run(x + 1, x + 17))    # corners cut


def bulk_freighter():
    """Bulk freighter. A long spine carrying three rack bays of standard pods,
    the middle one stacked two tiers deep, each bay its own frame with open
    sky between. Underneath: fuel pods aft, the crew pod amidships, a loading
    crane by the bow clamp."""
    c = Canvas("mp_bulk_freighter", "Steel")
    walk = ["k.k", "U.U", "U.U", ".", "."]          # walk-through pods
    tier1 = ["CCk", "CCk", "k.k"]
    tier2 = ["k.k", "CCk", "CCk", "#", "."]
    _bay(c, -30, 1, [(1, 0, walk), (1, 1, walk)])
    _bay(c, -10, 2, [(1, 0, tier1), (1, 1, tier1), (2, 0, ["M.k", "CCk", "CCk", "#", "."]), (2, 1, tier2)])
    _bay(c, 10, 1, [(1, 0, walk), (1, 1, ["k.M", "U.U", "U.U", ".", "."])])
    for x, y in ((-21, 9), (-1, 15), (19, 9)):    # gatlings on the rails
        c.cells[(x, y)] = "g"
    # engine block and the spine
    c.P(-36, 4, "###")
    c.P(-37, 3, "N.R#")
    c.P(-37, 2, "N..#")
    c.P(-37, 1, "E..." + _run(-33, 28, doors={-29, -24, -18, -13, -9, -4, 2, 7, 11, 16, 22, 27}))
    c.P(-37, 0, "E..." + "." * 62)
    c.P(-37, -1, "E.M." + _run(-33, 28, doors={-26, -20, -2, 18}))
    c.P(-37, -2, "N..#")
    c.P(-37, -3, "N.R#")
    c.P(-36, -4, "###")
    # the bow clamp: two jaws round a docking port
    c.rows(29, 5, """
 #######/
#q.O....o
#.A.m...#
#...###g#
....#
.....o
....#
#...###g#
#.Q.y...#
#H.O....o
 #######/
""")
    # underneath: fuel pods aft, crew pod amidships, crane forward
    _pod(c, -28, -6, ["f.f", "f.f", "fff", "#", "."])
    _pod(c, -22, -6, ["f.f", "f.f", "fpf", "#", "."])
    _pod(c, -4, -6, ["Q.Q", "BBB", "BBB", "#", "."])
    c.P(17, -2, "#.#")
    c.P(17, -3, "#.#")
    c.P(17, -4, "##########")
    c.P(17, -5, "#........X")
    c.P(17, -6, "######p###")
    return c.build()


def _tank(c, x, y, top_door=False, bottom_door=True, crew="", length=9):
    """A long fuel tank: a capsule `length` cells long and 5 tall, two rows of
    fuel cells with a catwalk between, a door in the middle of the bottom
    (or top) to the strut. `crew` replaces catwalk cells from the left."""
    n = length - 2
    mid = x + length // 2
    c.P(x + 1, y + 4, _run(x + 1, x + length - 2, doors={mid} if top_door else ()))
    c.P(x, y + 3, "#" + "f" * n + "#")
    c.P(x, y + 2, "#" + (crew + "." * n)[:n] + "#")
    c.P(x, y + 1, "#" + "f" * n + "#")
    c.P(x + 1, y, _run(x + 1, x + length - 2, doors={mid} if bottom_door else ()))


def tanker():
    """Tanker. Long fuel tanks, rows of fuel cells either side of a catwalk,
    clamped to the spine on short struts: three above, two below with the crew
    pod slung between them."""
    c = Canvas("mp_tanker", "Steel")
    for x0, crew in ((-16, "c"), (-4, "c...M"), (8, "c.......M")):   # top row
        c.P(x0 + 4, 2, "#.#")
        _tank(c, x0, 3, crew=crew, length=11)
    for x0, crew in ((-16, "c"), (8, "c")):                            # bottom row
        c.P(x0 + 4, -2, "#.#")
        _tank(c, x0, -7, top_door=True, bottom_door=False, crew=crew, length=11)
    _pod(c, -1, -6, ["Q.H", "BBB", "BBB", "#", "."])
    # engine block, spine, bow with a docking port
    c.P(-20, 3, "###")
    c.P(-21, 2, "N.R#")
    c.P(-21, 1, "N..." + _run(-17, 19, doors={-11, 1, 13}) + "#g#/")
    c.P(-21, 0, "E..." + "." * 37 + "...o")
    c.P(-21, -1, "N.M." + _run(-17, 19, doors={-11, 1, 13}) + "#g#/")
    c.P(-21, -2, "N.R#")
    c.P(-20, -3, "###")
    return c.build()


def mining_barge():
    """Mining barge. A hammerhead cutting bar of mining drills across the bow,
    ore pods racked along the top of the spine, and underneath the crew pod,
    two smelter pods, and a folded excavator boom with its own drill and grab."""
    c = Canvas("mp_mining_barge", "Steel")
    for x0, low in ((-19, "k.k"), (-13, "k.M"), (-7, "k.k"), (-1, "k.k")):   # ore pods
        _pod(c, x0, 2, ["CCk", "CCk", low])
    c.P(-24, 3, "###")
    c.P(-25, 2, "N.R#")
    c.P(-25, 1, "N..." + _run(-21, 6, doors={-17, -11, -5, 1}))
    c.P(-25, 0, "E..." + "." * 28)
    c.P(-25, -1, "N.M." + _run(-21, 6, doors={-19, -13, -7, 0}))
    c.P(-25, -2, "N.R#")
    c.P(-24, -3, "###")
    # the cutting bar: drills across a stepped face, grabs at the corners
    c.rows(7, 7, """
 #X/
#..&
#c.#
#..&/
#R..&
#...#
....&
....&
....&
#...#
#M..&
#..&/
#c.#
#..&
 #X/
""")
    # underneath: smelter pods, crew pod amidships, folded excavator boom
    _pod(c, -21, -6, ["c.v", "R.R", "v.c", "#", "."])
    _pod(c, -15, -6, ["c.v", "R.R", "vpc", "#", "."])
    _pod(c, -9, -6, ["Q.H", "BBB", "BBB", "#", "."])
    c.P(-1, -2, "#.#")
    c.P(-1, -3, "#.#")
    c.P(-1, -4, "#.#####")
    c.P(-1, -5, "#.....&")
    c.P(-1, -6, "####X#/")
    return c.build()


def arms_bazaar():
    """Experimental: a floating arms market. A spine with a market hall in the
    middle and display aisles branching off both sides, staggered, each aisle
    a truss lined with guns on show and a customer berth at its tip. It parks
    between two fleets and sells to both."""
    c = Canvas("mp_arms_bazaar", "Steel")
    aisle = ["#.#", "g.p", "#.#", "p.g", "#.#", "g.p", "#.#"]
    for cx in (-14, 8, 20):                     # aisles to one side...
        for k, row in enumerate(aisle):
            c.P(cx - 1, 2 + k, row)
        c.P(cx, 9, "o")
    for cx in (-20, -8, 14):                    # ...and the other
        for k, row in enumerate(aisle):
            c.P(cx - 1, -2 - k, row)
        c.P(cx, -9, "o")
    c.P(-25, 3, "###")
    c.P(-26, 2, "N.R#")
    c.P(-26, 1, "N..." + _run(-22, 22, doors={-14, 8, 20}) + "..#/")
    c.P(-26, 0, "E..." + "." * 45 + "...o")
    c.P(-26, -1, "N.M." + _run(-22, 22, doors={-20, -8, 14}) + "..#/")
    c.P(-26, -2, "N.R#")
    c.P(-25, -3, "###")
    c.P(23, 2, "#g/")
    c.P(23, -2, "#g/")
    # the market hall: the bridge in the middle, a docking hub on each face
    c.rows(-5, 5, """
  ##hhh##
 #Q.hhh.Q#
#.O.hhh.O.#
#.........#
..q.BBB.q..
....BBB....
..M..H..M..
#.........#
#.O.hhh.O.#
 #Q.hhh.Q#
  ##hhh##
""")
    return c.build()


def trade_post():
    """Station. A Guild trade post: a square hub with four spines run out to
    docking clamps, standard pods racked along one side of each spine and a
    loading crane and fuel pod on the other, so the whole post turns like a
    pinwheel. One arm is drawn and turned four ways."""
    c = Canvas("mp_trade_post", "Steel")
    arm = Canvas("arm")
    # One arm, drawn the way it would sit as the +x arm (y up). The pods ride
    # on its upper side, the crane and fuel pod hang under it.
    for x0, low in ((8, "k.k"), (14, "k.M"), (20, "k.k"), (26, "kpk")):
        _pod(arm, x0, 2, ["CCk", "CCk", low])
    arm.P(7, 1, _run(7, 30, doors={10, 16, 22, 28}) + "#g#")
    arm.P(7, 0, "." * 27 + "o")
    arm.P(7, -1, _run(7, 30, doors={10, 26}) + "#g#")
    arm.P(34, 1, "/")
    arm.P(34, -1, "/")
    arm.P(9, -2, "#.#")
    arm.P(9, -3, "#.#")
    arm.P(9, -4, "#.#########")
    arm.P(9, -5, "#.........X")
    arm.P(9, -6, "###p#######")
    _pod(arm, 24, -6, ["f.f", "f.f", "fff", "#", "."])
    for (x, y), ch in arm.cells.items():
        # the underside of the +x arm is the right-hand side of the +y arm
        c.cells[(x, y) if y >= 0 else (-y, x)] = ch
    # The hub, one quarter of it: a ring corridor, rooms, guns on the corners.
    c.rows(1, 6, """
##g#
.mQ.#
....p#
.M..O#
....Q#
.Rc.Q#
......
""")
    c.rot4()
    c.rows(-1, 1, """
BBB
BBB
.H.
""")
    return c.build()


CONCEPTS = [
    ("pedlar", pedlar, "Pedlar", "Scout: courier trader",
     "Two standard pods on a short spine, a crew pod slung underneath and a docking clamp on the nose. The first Guild hull most captains meet."),
    ("hauler", hauler, "Container Hauler", "Trade",
     "Four standard pods on one side of the spine, clamped top and bottom by a rack, two more stacked on the rack amidships; crew pod, crane and fuel pod on the other side."),
    ("bulk_freighter", bulk_freighter, "Bulk Freighter", "Trade: heavy",
     "Three rack bays of standard pods on one long spine, each bay its own frame with open sky between. Fuel pods, the crew pod and a loading crane hang underneath."),
    ("tanker", tanker, "Tanker", "Trade: fuel",
     "Long fuel tanks, rows of fuel cells either side of a catwalk, clamped to the spine: three above, two below with the crew pod slung between."),
    ("mining_barge", mining_barge, "Mining Barge", "Industrial",
     "A hammerhead cutting bar of mining drills across the bow, ore pods racked on the spine, and a folded excavator boom underneath with its own drill and grab."),
    ("arms_bazaar", arms_bazaar, "Arms Bazaar", "Experimental: weapons market",
     "A market hall on a spine with display aisles of guns branching off both sides, a customer berth at every tip. It parks between two fleets mid-battle and sells to both."),
    ("trade_post", trade_post, "Guild Trade Post", "Station: neutral trade",
     "A square hub with four spines out to docking clamps, pods racked on one side of each and a crane and fuel pod on the other, so the post turns like a pinwheel."),
]
