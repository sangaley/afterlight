"""Stellar Preserve: hand-drawn, top-down only.

Lore: creature-keepers who protect life at any cost, including "pruning
diseased branches". In the game they never start a fight: they flee, and they
hunt space creatures with tractor beams.

Grammar, held at every size (all of it reads from above):
  - an ARK: a soft, rounded spine with the bridge in its middle;
  - SEED PODS: octagonal containment rooms (diagonal staircase corners) that
    hang off the spine on short stalks, a gap between every pod;
  - TRACTOR BOOMS: long 3-wide arms at the bow, a tractor beam at each tip;
  - only a couple of defensive gatlings: they run, they don't fight;
  - port/starboard symmetric, Steel, plates only on the rounded prow and the
    boom tips.

Legend (draw.py): # hull  . deck  / plate  E engine  R reactor  b battery
f fuel  B bridge  H helm  M memory core  s shield  Q quarters  O oxygen
m medbay  y repair  d door  t tractor beam  g gatling  Z creature containment
(2x1)  Y specimen vault  ! research lab (2x1)  w wellness hub (3x3)  A radar
V cryo (seed stores)  c cooling  K barracks (2x1)  G galley (2x2)  L large reactor
"""
import math

from draw import Canvas

FACTION = {
    "slug": "stellar_preserve",
    "name": "Stellar Preserve",
    "tagline": "Creature-keepers who protect life at any cost, including pruning the diseased branches. They don't fight: they flee, and they hunt.",
    "grammar": [
        "An ark: a soft, rounded spine with the bridge in its middle",
        "Octagonal seed pods (containment, vaults, labs) hanging off the spine on short stalks",
        "Long 3-wide tractor booms at the bow, a tractor beam at every tip",
        "Only a couple of defensive gatlings: they run, they don't fight",
        "Port/starboard symmetric, Steel; plates only on the rounded prow and boom tips",
    ],
    "accent": "#4fd1b5",
}

# A seed pod: an octagonal room on a stalk. Stamp it with its stalk row
# sitting directly on the spine's wall (top-left corner given).
POD_PEN = """
  #####
 #.Y.Y.#
#ZZ...ZZ#
#...O...#
#ZZ...ZZ#
 #.....#
  ##d##
   #.#
"""
POD_LAB = """
  #####
 #.Y.Y.#
#!!...!!#
#...Q...#
#Y.....Y#
 #.Y.Y.#
  ##d##
   #.#
"""

POD_SEED = """
  #####
 #.V.V.#
#V.V.V.V#
#.......#
#V.V.V.V#
 #.V.V.#
  ##d##
   #.#
"""


def _stamp_out(c, template, cx, cy, side):
    """Stamp a pod template so its stalk points back at the hub: side 'E'
    puts the pod out on +x (stalk at cx), 'N' out on +y (stalk at cy)."""
    rows = template.strip("\n").split("\n")
    for r, line in enumerate(rows):
        for k, ch in enumerate(line):
            if ch == " ":
                continue
            out = len(rows) - 1 - r          # 0 at the stalk, growing outward
            if side == "E":
                c.cells[(cx + out, cy + k - 4)] = ch
            else:
                c.cells[(cx + k - 4, cy + out)] = ch


def warden_tug():
    """Scout. A seed pod with an empty one held out in front of it on a short
    boom: the front pod is a hollow octagon, open at the bow, its inner walls
    lined with tractor beams. It goes out alone, draws one creature into the
    open pod and holds it there; the containment cell waits in the boom
    behind it."""
    c = Canvas("sp_warden_tug", "Steel")
    c.rows(-11, 6, """
               #######/
              #.......#/
  ##g##      #...#t#...#/
 #Q.O.M#    #...#   #...#
E.R.b.m.#   #..#     #..#
E.f.BB.H.###..#      #t#/
E.M.BB..ZZ....t
""")
    return c.mirror().build()


def herd_keeper():
    """Frigate. A leader and its herd. A big round head pod, plated at the
    prow and fitted with tractor beams, tows two curving tether arms that
    bend further outward the further aft they trail; seed pods hang off each
    tether like a herd strung out behind its keeper, and a gatling and an
    engine sit at each tether's tip to cover the run home."""
    c = Canvas("sp_herd_keeper", "Steel")
    # The tether arm: 3-wide runs, each a row higher and shorter than the
    # last, so the arm bends outward more and more as it trails aft.
    runs = [(-3, 3), (-8, -2), (-12, -7), (-15, -11), (-18, -14), (-20, -17), (-22, -19)]
    for k, (x0, x1) in enumerate(runs):
        c.R(x0, x1, 2 + k, 4 + k)
    hull = set(c.cells)
    top, bottom = iter("QOfQcmbQO"), iter("bkfckb")
    for k, (x0, x1) in enumerate(runs):
        c.R(x0 + 1, x1, 3 + k, 3 + k, ".")
        for x in range(x0 + 1, x1 + 1):
            if x % 2 == 0 and x < -3:
                if (x, 5 + k) not in hull:
                    c.cells[(x, 4 + k)] = next(top, "#")
                if (x, 1 + k) not in hull:
                    c.cells[(x, 2 + k)] = next(bottom, "#")
    c.P(-22, 9, "E").P(-21, 10, "g")
    c.P(5, 6, "#####/")
    c.P(4, 5, "#Q.A.Q#/")
    c.P(3, 4, "#.......#/")
    c.P(2, 3, "#m.O.M.s.y#/")
    c.P(1, 2, "#.R.b.Q.O.T.t")
    c.P(1, 1, "E.LL.BB.H...#")
    c.P(1, 0, "E....BB...M.t")
    c.rows(-8, 13, POD_SEED)
    c.rows(-19, 16, POD_PEN)
    return c.mirror().build()


def seed_ark():
    """Capital. A long ark with a walled garden in its middle: four wellness
    halls ringed by planters, the bridge at the centre of the garden. Seed
    pods aft and fore, and a fan of three tractor booms at the bow."""
    c = Canvas("sp_seed_ark", "Steel")
    c.rows(-33, 12, POD_PEN)
    c.rows(-21, 12, POD_SEED)
    c.rows(11, 12, POD_LAB)
    c.rows(-38, 4, """
   #g####.###########.#######
  E..........................
 E.LL.bf..Q.Q.GG.m.O..Q.Q.M..
E...c.bfd.KK..GG.y.d..KK...d.
E............................
""")
    c.rows(-9, 9, """
     #########
    #.O.O.O.O.#
   #O.........O#
  #O..O.....O..O#
 #O...www.www...O#
#..O..www.www..O..#
..O...www.www...O..
..O.............O..
........BB.H.......
........BB.........
""")
    c.rows(10, 7, """
             #####
            #.Y.Y.#############/
           #..!!...............t
#####.#####..Y..Y..############/
..................#
.Y.Y...Y.Y..!!.M.A#
..Q.Q.d.....y..s...################/
...................................t
""")
    return c.mirror().build()


def net_weaver():
    """Experimental. A crescent hull that sweeps a net of tractor booms
    through a herd: the horns run forward into the two longest booms, and the
    tips make a curve that closes round whatever swims into it."""
    c = Canvas("sp_net_weaver", "Steel")
    c.rows(-14, 16, """
              ##############/
             #..............t
            #.....##########/
           #.M..##
         ##....#
       ##.ZZ....#########/
      #......d...........t
     #Q.O.Q##############/
    #.....g
   #Q...Q#
  E.ZZ..m.###########/
  E........d.........t
 ER.b..M#############/
 E.c.O.#
E.LL..#
E...BB.###########/
E...BB.H..........t
""")
    return c.mirror().build()


def sanctuary():
    """Station. A ring habitat round an open pen: four tractor booms reach in
    from the inner wall and hold whatever is kept in the middle. Containment
    pods hang off the outside of the ring like seeds, a stalk to each.
    One quadrant drawn, turned four ways."""
    c = Canvas("sp_sanctuary", "Steel")
    for x in range(1, 23):
        for y in range(0, 23):
            if 9.6 <= math.hypot(x, y) <= 16.6:                  # the ring
                c.cells[(x, y)] = "#"
            dx, dy = x - 16, y - 16                              # diagonal pod
            if max(abs(dx), abs(dy)) <= 4 and abs(dx) + abs(dy) <= 6:
                c.cells[(x, y)] = "#"
            if abs(x - y) <= 2 and 23 <= x + y <= 26:            # ...and its stalk
                c.cells[(x, y)] = "#"
    for k in (16, 17):                     # stalks out to the cardinal pods
        for j in (-1, 0, 1):
            c.cells[(k, j)] = c.cells[(j, k)] = "#"
    _stamp_out(c, POD_PEN, 17, 0, "E")
    _stamp_out(c, POD_PEN, 0, 17, "N")
    for k in range(4, 10):                 # a tractor boom reaching into the pen
        c.cells[(k, 0)] = "."
        c.cells[(k, 1)] = c.cells[(1, k)] = "#"
    c.P(3, 0, "t").P(3, 1, "/").P(1, 3, "/")
    # The diagonal pod: specimen vaults round two containment pens.
    for p in ((15, 19), (17, 19), (19, 15), (19, 17), (13, 17), (17, 13)):
        c.cells[p] = "Y"
    c.P(15, 17, "ZZ").P(17, 15, "Z").P(17, 16, "Z").P(16, 16, "O")
    # The ring, room by room: crew along the outer wall, plant along the inner.
    for (x, y), ch in {(11, 2): "b", (11, 3): "R", (15, 2): "Q", (15, 3): "Q", (15, 4): "m",
                       (10, 5): "O", (14, 5): "Q", (14, 6): "Q", (9, 6): "b", (8, 7): "R",
                       (13, 7): "K", (14, 7): "K", (14, 8): "g", (7, 8): "O", (6, 9): "c",
                       (12, 9): "Q", (12, 10): "M", (12, 11): "Q", (4, 10): "y", (13, 12): "A",
                       (8, 13): "Q", (7, 14): "Q", (5, 14): "m", (4, 15): "O", (3, 15): "Q"}.items():
        c.cells[(x, y)] = ch
    c.rot4()
    c.R(-1, 1, 12, 13, "B")                # the one bridge, north arc
    c.cells[(0, 14)] = "H"
    return c.build()


CONCEPTS = [
    ("warden_tug", warden_tug, "Warden Tug", "Scout",
     "A seed pod holding an empty one out in front on a short boom: a hollow pod open at the bow, its walls lined with tractor beams. It catches one creature and carries it home inside."),
    ("herd_keeper", herd_keeper, "Herd-keeper", "Frigate",
     "A leader and its herd: a round, plated head pod tows two tethers that curve outward as they trail aft, seed pods strung along them like a herd behind its keeper."),
    ("seed_ark", seed_ark, "Seed Ark", "Capital: ark",
     "A walled garden in the middle of a long ark, the bridge among the planters. Six seed pods and a fan of three tractor booms."),
    ("net_weaver", net_weaver, "Net-weaver", "Experimental: herder",
     "A crescent that sweeps seven tractor booms through a herd; the tips make a curve that closes round its catch."),
    ("sanctuary", sanctuary, "Sanctuary", "Station",
     "A ring habitat round an open pen, four tractor booms reaching in to hold what lives there; containment pods hang outside like seeds."),
]


# Sent back for rework; redrawn since.
REVISED = {
    "warden_tug": "Redrawn: a seed-pod body holding an open, tractor-lined catching pod out on a boom, with containment in between.",
    "herd_keeper": "Redrawn: a leader head with two curving tether arms, seed pods trailing off them like a herd.",
}
