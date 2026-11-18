"""Stellar Preserve, the great ships (v5b): two line ships, the flagship, two
experimentals and three traders. Hand-drawn, top-down only.

Same grammar as preserve.py (FACTION["grammar"] there), held at every size:
  - an ARK: a soft, rounded spine with the bridge in its middle;
  - SEED PODS: octagonal rooms on short stalks off the spine, a gap round each;
  - TRACTOR BOOMS: long 3-wide arms at the bow, a tractor beam at every tip;
  - a couple of defensive gatlings, no more: they run, they don't fight;
  - port/starboard symmetric, Steel, plates only on the rounded prow and the
    boom tips.

Lore used here: the Preserve herd Void Drifters, hunt Leviathans and cull
Parasite Swarms; they have watched the Cascade longer than anyone, and keep a
sample of it that grows near minds and starves alone (Summit Record 4).

Legend (draw.py): # hull  . deck  / plate  d door  E engine  R reactor
L large reactor  b battery  f fuel  c cooling  B bridge  H helm  M memory core
s shield  A radar  Q quarters  q officer quarters  K barracks  G galley
m medbay  O oxygen  y repair  t tractor beam  g gatling  Z creature containment
Y specimen vault  ! research lab  V cryo (seed stores)  w wellness hub (garden)
C bulk cargo  U large cargo  k small cargo  + force field  = signal jammer
"""
from collections import deque

from draw import Canvas
from preserve import FACTION  # noqa: F401  (same faction, same page)

N4 = ((0, 1), (0, -1), (1, 0), (-1, 0))
WEAPONS = set("gt")
SIZE = {  # class -> hull cells (BRIEF.md size ladder and the job's ranges)
    "small": (150, 300), "medium": (400, 600), "large": (750, 1000),
    "line": (750, 1300), "flagship": (1500, 2000), "any": (80, 2500),
}


# --------------------------------------------------------------------------- helpers
def finish(c, size):
    """Build, then check what draw.py doesn't: the hull is one piece, the
    skeleton (hull, engines, guns, tractors) is mirror-symmetric, no engine
    fires into its own hull, the bridge sits inside the hull near the middle,
    and the hull count fits the class."""
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
        d["_checks"].append("skeleton not mirror-symmetric")
    for (x, y), ch in c.cells.items():
        if ch in "EN" and any((xx, y) in hull for xx in range(x - 90, x)):
            d["_checks"].append(f"engine at {(x, y)} vents into hull")
        if (ch in "EN" or ch in WEAPONS) and c.cells.get((x, -y)) != ch:
            d["_checks"].append(f"'{ch}' at {(x, y)} has no mirror image")
    bridge = [p for p, ch in c.cells.items() if ch == "B"]
    if not bridge:
        d["_checks"].append("no bridge")
    elif any(layer.get(p) == "Outer" for p in bridge):
        d["_checks"].append("bridge on the rim")
    else:
        xs = [p[0] for p in hull]
        mid, half = (min(xs) + max(xs)) / 2, (max(xs) - min(xs)) / 2
        bx = sum(p[0] for p in bridge) / len(bridge)
        if abs(bx - mid) > 0.45 * half:
            d["_checks"].append(f"bridge at x={bx:.0f} is far from the middle ({mid:.0f})")
    lo, hi = SIZE[size]
    if not lo <= len(hull) <= hi:
        d["_checks"].append(f"{len(hull)} hull cells, outside {lo}-{hi}")
    return d


# --------------------------------------------------------------------------- ships
def seedling():
    """Trader (small hauler). A seed just sprouting: a small soft ark with its
    two seed leaves held out in front, each a cargo pod on a short slanting
    stalk, and the shoot between them a tractor boom that lifts the crates
    aboard. Two gatlings for the road."""
    c = Canvas("sp_seedling", "Steel")
    c.rows(-12, 12, """
                  ###
                 #k.k#
                #CC.kk#
                #CC.CC#
                #k..CC#
               #.d..k#
              #...###
             #...#
     ###g####...#
  ###Qcb.O.Mk..k#
 E....d...Q.m...#/
E.R.b..BB.H...k.k#######/
E.M.UU.BB..UU...........t
""")
    return finish(c.mirror(), "small")


# Small pods for the traders: 7 wide, 6 tall, on a one-cell stalk. Stamp with
# c.rows(stalk_x - 3, top_y, POD), the stalk row sitting on the spine's rim.
PEA_CARGO = """
  ###
 #k.k#
#CC.kk#
#CC.kk#
 #k.k#
  #d#
  #.#
"""
PEA_SEED = """
  ###
 #V.V#
#VV.VV#
#V...V#
 #...#
  #d#
  #.#
"""
PEA_LAB = """
  ###
 #Y.Y#
#!!.!!#
#..O..#
 #...#
  #d#
  #.#
"""
PEA_LIVE = """
  ###
 #Y.Y#
#ZZ.ZZ#
#..O..#
 #Y.Y#
  #d#
  #.#
"""


def seed_wain():
    """Trader (medium hauler). A pea pod: a slim ark with four small pods a
    side in an even row, peas in a pod. Live specimens ride in the aft
    pair, then seed stock in cryo, then two pairs of crates; a loading boom
    at the bow, a gatling a side."""
    c = Canvas("sp_seed_wain", "Steel")
    # the middle two pods ride a cell higher, on the spine's swell: the row
    # of peas bows like a real pod
    for sx, top, pod in ((-15, 10, PEA_LIVE), (-6, 11, PEA_SEED), (3, 11, PEA_CARGO), (12, 10, PEA_CARGO)):
        c.rows(sx - 3, top, pod)
    c.rows(-21, 4, """
            ###.####g###.###
   ###.#####.QQ.m.O..Y.k.k..#####.####/
 E.........d.....................d......#/
E.R.b.f.Q.O.KK.m...BB...UU.kk.Y.V.k.s.A..#########/
E.LL.c.M.UU..Q.....BB.H....M...k...M..............t
""")
    return finish(c.mirror(), "medium")


# An open pen: a seed pod with its heart left out. A creature floats free in
# the hole, held in a tractor field; containment cells and the keepers' rooms
# ring it. 13 wide; stamp with c.rows(stalk_x - 6, top_y, PEN_RING).
PEN_RING = """
    #####
  ##Y.Q.Y##
 #.........#
#..ZZ###ZZ..#
#...#   #...#
#Z.#     #.Z#
#Z.#     #.Z#
#Y.#     #.Y#
#...#   #...#
#..ZZ###ZZ..#
 #.........#
  ##.O.m.##
    ##d##
     #.#
"""


def menagerie():
    """Trader (large hauler). A herd-barge: four open pens on short stalks,
    rings round an empty heart where a live creature floats in a tractor
    field, containment cells and keepers' rooms in the ring. The ark between
    them carries the feed, the keepers and the bridge, in the gap between
    the pens."""
    c = Canvas("sp_menagerie", "Steel")
    for sx in (-14, 3):
        c.rows(sx - 6, 18, PEN_RING)
    c.rows(-27, 4, """
    #########.#######g########.##########/
  E#..............d................d.....##/
 E.LL.fM.c.O.QGG.m.Q....CC.CC.kk.UU.Y.Y.s.A.#/
E.R.b.f.Q.KK..GG.y.Q.BB.CC.CC.k...V.V.b.O....#############/
E..R..c..............BBH.........M.......M................t
""")
    return finish(c.mirror(), "large")


def lure():
    """Experimental: stalker lure. Stalkers ambush lone Void Drifters, and
    drifters are round and glow. So the Lure spreads four round bait pods on
    short slanting stalks, a floodlight on each, and from a distance it looks
    like a little herd strung out on its own. Whatever strikes at the glow,
    the long boom at its head takes, and the bait pods are pens to keep it.
    Thorax amidships with the bridge, a slim tail of engines."""
    c = Canvas("sp_lure", "Steel")
    c.rows(-37, 17, """
                       ##l##                 ##l##
                      #Y.O.Y#               #Y.O.Y#
                     #Y.....Y#             #Y.....Y#
                     #ZZ...ZZ#             #ZZ...ZZ#
                     #.......#             #.......#
                     #ZZ...ZZ#             #ZZ...ZZ#
                     #Y.....Y#             #Y.....Y#
                      #.Y.Y.d.#           #.d.Y.Y.#
                       #####...#         #...#####
                            #...#       #...#
                             #...###g###...#
                              #.Y.!!.!!.Y.#
                             #.q.O.....O.q.#
                            #.Q.s.y...y.s.Q.#       ####/
   E########################.O.KK.....KK.O..#      #I.A.##/
 E#.f.b.R.c.GG.Q.O.m.Q.y.M...b.Y.........Y.b.######.s.M.s.#/
E.LL.R.b.c..GG.KK...KK.Q.O...LL.O..BB.H..O.LL.......A...A..##########/
E..R...............................BB.................I..............t
""")
    return finish(c.mirror(), "any")


def hermitage():
    """Experimental: isolation ark. The Preserve's sample of the Cascade grows
    near minds and starves alone, so it rides in a sealed vault at the end of
    a long stalk, as far behind the crew as the hull allows, signal jammers
    in the vault walls and all down the stalk so nothing can talk near it.
    No one walks the stalk. If the sample ever grows, they cut it and leave
    the vault to starve in the dark. Only four engines, high on the stern,
    clear of the stalk: it is slow, and it is meant to be."""
    c = Canvas("sp_hermitage", "Steel")
    c.rows(-3, 13, PEA_LAB)          # the crew study it from here, at a distance
    c.rows(-44, 6, """
                                      E##g###.######
                                   E##.Q.q.O.O.q.Q.###/
  #=#=#                           #.KK.m.y..M..y.A.s..##/
 #Y.+.Y#                         #..d...............d...#/
=Y.Y.Y.Y=                        #.LL.b.GG.Y.Y.!!.Y.M.s..#/
=.+...+..##=#####=#####=#####=###.R.b.c.GG.BB.H.O...A.I...###########/
=.Y.$.Y..d......................d.R........BB........................t
""")
    return finish(c.mirror(), "any")


def herd_ranger():
    """Cruiser. The rangers ride with the herds and cull what preys on them:
    stalkers, and the poachers who thin the herd-ships. An arrow: a long soft
    shaft, a plated arrowhead with one long boom, and four pods swept back
    off the shaft on slanting stalks like fletching, the aft pair trailing
    past the stern. The pods are kennels: tame hunters ride in them, let
    out on what the boom holds. Four gatlings, the most a Preserve hull
    below the flagship carries."""
    c = Canvas("sp_herd_ranger", "Steel")
    c.rows(-35, 16, """
  #####               #####
 #Y.O.Y#             #Y.O.Y#
#Y.....Y#           #Y.....Y#
#ZZ...ZZ#           #ZZ...ZZ#
#.......#           #.......#
#ZZ...ZZ#           #ZZ...ZZ#
#Y.....Y#           #Y.....Y#
 #.Y.Y.d.#           #.Y.Y.d.#
  #####...#           #####...#
       #...#               #...#                #g#/
        #...#######         #...###g###         #..##/
         #..Q.O.Q..#         #..q.A.q..#        #.s..##/
         E.b.KK.GG..#########.m.Q.M..Q..########..I.A.M##/
       E#.......GG...d.....................d.............##/
      E.R.b.f.c.O.Q.Q.Y.Y.Y.!!.y.....s.KK.M.Y.Y.O.b.c.f.s..##/
     E.LL.R.b..f.Q.Q.m.ZZ.ZZ.O......BB.H..LL.R.b.Y.O.A..Y......#########/
     E..R.......................M...BB..................................t
""")
    return finish(c.mirror(), "line")


def windbreak():
    """Battleship. The herd's shelter, and the Preserve's cold side. A great
    curved wall stands across its bow, shield emitters all along it, a seed
    pod of watchers at each end and booms reaching through it; the herd
    rides in its lee. What the herd can't outrun, it stops at the wall. Two
    pairs of quarantine pods on the keel hold the sick of the herd, force
    fields between the pens, until the wardens decide which branches go."""
    c = Canvas("sp_windbreak", "Steel")
    c.rows(-29, 31, """
                              #####
                             #Y.A.Y#
                            #!!...!!#
                            #.......#
                            #Y.q.q.Y#
                            #.......#
                            #!!...!!#
                             #Y...Y#
                              #.....##
                              #.......###
                              #..........#
                               #####M.....##
                                    #s......#
                                      ##.....#
                                        #s....#
                                         #.....g
     #####       #####                    #....#
    #Y.O.Y#     #Y.O.Y#                    #s...#
   #ZZ...ZZ#   #ZZ...ZZ#                    #....#
   #+.....+#   #+.....+#                     #....########/
   #ZZ...ZZ#   #ZZ...ZZ#                     #............t
   #Y.....Y#   #Y.....Y#                      #....#######/
   #ZZ...ZZ#   #ZZ...ZZ#                      #s..#
    #Y...Y#     #Y...Y#                        #...#
     #.d.#       #.d.#                         #...#
      #.#         #.#                          #...g
    ##...#########...###################        #..#
  E#.Q...Q.m.O.y.Q...Q.M.KK.q.q.KK.Y.Y..##      s...#
 E...........d.....................d......#     #...#
E.LL.b.f.c.GG.Q.O.!!.Y.Y.s.......s.V.V.M.A..####....#
E.R.b.R.f..GG.KK.m..O.Q.LL..HBB..LL.b.b.O............#########/
E....c...R...................BB...............................t
""")
    return finish(c.mirror(), "line")


def great_cradle():
    """Flagship, the Warden's own. An ark held in two great arms: the arms
    curve out from a rib at the heart of the ship and round toward bow and
    stern, open space between them and the spine, where the young of the
    herds are carried in tractor fields. Their nurseries hang off the arms
    on stalks, the Preserve's seed bank between them, and the arms end in
    booms forward and engines aft. In the heart, the Warden's hall: four
    gardens round the bridge. Kind, until it decides a branch must go."""
    c = Canvas("sp_great_cradle", "Steel")
    c.rows(-34, 31, """
                  #####         #####         #####
                 #Y.O.Y#       #V.V.V#       #Y.O.Y#
                #ZZ...ZZ#     #V.V.V.V#     #ZZ...ZZ#
                #.......#     #.......#     #.......#
                #ZZ.m.ZZ#     #V.V.V.V#     #ZZ.m.ZZ#
                #.......#     #.......#     #.......#
                #ZZ...ZZ#     #V.V.V.V#     #ZZ...ZZ#
                 #Y...Y#       #V...V#       #Y...Y#
                  #.d.#         #.d.#         #.d.#
                   #.#           #.#           #.#
                   #.#           #.#           #.#
                   #.#  ####g####...####g####  #.#
                   #..##Q.O..Y.q.....q.Y..O.Q##..#
                 ##...............................##
              ###..s....########.....########....s..###
            ##...Q...###        #...#        ###...Q...##
           #....Y.###           #...#           ###.Y....#
         ##..O.###              #M..#              ###.O..##
        #....##                 #...#                 ##....#
  ######....#                   #...#                   #....############/
 E.R......##                    #...#                    ##..............t
  ########                      #...#                      ##############/
                             ###.....###
                            #q.Y.....Y.q#
                           #..!!.....!!..#
                          #.O...........O.#
                         #.www.......www...#
   E#####################..www.......www....#################/
 E............d............www.......www.......d......d.......#/
E.LL.b.f.c.GG.KK.Q.M.O.m..O.......d......O...Y.!!.M.Y.V.V.A.s.s../
E.R.b.f.c..GG...Q.O.Q.y........BB.H..........Y.O.m..Y.V.V.A.......############/
E..R.c.........................BB.............................................t
""")
    return finish(c.mirror(), "flagship")


CONCEPTS = [
    ("seedling", seedling, "Seedling", "Trader (small hauler)",
     "A seed just sprouting: two cargo pods held out in front like seed leaves on slanting stalks, a loading boom between them for the shoot."),
    ("seed_wain", seed_wain, "Seed-wain", "Trader (medium hauler)",
     "A pea pod: a slim ark with four small pods a side in an even row, live specimens aft, seed stock and crates forward."),
    ("menagerie", menagerie, "Menagerie", "Trader (large hauler)",
     "A herd-barge: four open pens on stalks, each a ring round an empty heart where a live creature floats in a tractor field."),
    ("lure", lure, "Lure", "Experimental: stalker lure",
     "Four round bait pods on slanting stalks, a floodlight on each: from afar, a little herd of drifters on its own. What strikes at the glow, the long boom takes."),
    ("hermitage", hermitage, "Hermitage", "Experimental: isolation ark",
     "Keeps the Preserve's sample of the Cascade alone: a sealed vault far behind the crew on a long stalk walled with jammers. If it ever grows, they cut the stalk and leave it to starve."),
    ("herd_ranger", herd_ranger, "Herd Ranger", "Cruiser",
     "An arrow: a soft shaft, a plated arrowhead with one long boom, and four kennel pods swept back like fletching. It rides with the herds and culls what preys on them."),
    ("windbreak", windbreak, "Windbreak", "Battleship",
     "A great curved wall across the bow, shields all along it and a pod at each end; the herd rides in its lee. Quarantine pods on the keel hold the sick until the wardens decide which branches go."),
    ("great_cradle", great_cradle, "Great Cradle", "Flagship",
     "The Warden's ark, held in two great arms that curve round open space where the herds' young ride in tractor fields; nurseries and the seed bank hang off the arms, four gardens round the bridge at its heart."),
]
