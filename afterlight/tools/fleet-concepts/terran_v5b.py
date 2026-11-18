"""Terran Hegemony, round 5b: the heavy end of the line. A cruiser, a
battlecruiser, a carrier, the fleet admiral's dreadnought, a second
experimental and a requisitioned liner. Hand-drawn, top-down only.

The approved grammar (terran.py) holds at every size:
  - an ARROWHEAD BOW with a plate on every step of both raked faces;
  - GUN SPONSONS port and starboard, raked into diamonds: guns on the outer
    faces, magazines behind them, a deck back to the keel;
  - ENGINE NACELLES as pods at the stern corners, plus a centre engine bank;
  - a KEEL corridor down the centreline with the bridge mid-ship;
  - port/starboard symmetric, Titanium.
Big Terran ships grow the way the Sovereign does: more hulls held apart by
crossbeams, with open courtyards between, never one solid slab. The note on
the approved set was "too rectangular", so every face here is raked,
stepped or chamfered.

Lore used: the present Hegemony is humanist, military and losing. A temporal
superweapon shattered the old empire across timelines; its past self (the
Eternal Hegemony) still fights as if it won. Nine billion people live under
martial law, the unification wars may not be spoken of, and the Oracle's
sensors see thirty seconds ahead, so radar (A) and fire control (T) ride on
every big hull.

Legend: see terran.py and draw.LEGEND.
"""
from collections import deque

from draw import Canvas
from terran import FACTION  # noqa: F401  (same faction, same page)

N4 = ((0, 1), (0, -1), (1, 0), (-1, 0))
WEAPONS = set("rngxzpiD")
SIZE = {  # class -> hull cells (BRIEF.md ladder; Bulwark 353, Sovereign 1043)
    "cruiser": (700, 950), "battlecruiser": (850, 1250), "carrier": (900, 1350),
    "dreadnought": (1500, 2000), "experimental": (500, 1400), "civilian": (400, 800),
}


# --------------------------------------------------------------------------- helpers
def finish(c, size):
    """Build, then check what draw.py doesn't: the hull is one piece, the
    skeleton (outline, engines, guns, drone bays) is mirror-symmetric, no
    engine fires into its own hull, the bridge is inside the hull near the
    middle, and the hull count fits the class."""
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
    for (x, y), ch in c.cells.items():
        if ch in "EN" and any((xx, y) in hull for xx in range(x - 120, x)):
            d["_checks"].append(f"engine at {(x, y)} vents into hull")
        if (ch in "EN" or ch in WEAPONS) and c.cells.get((x, -y)) != ch:
            d["_checks"].append(f"'{ch}' at {(x, y)} has no mirror twin")
    bridge = [p for p, ch in c.cells.items() if ch == "B"]
    if not bridge:
        d["_checks"].append("no bridge")
    elif any(layer.get(p) == "Outer" for p in bridge):
        d["_checks"].append("bridge on the rim")
    else:
        xs = [p[0] for p in hull]
        mid, half = (min(xs) + max(xs)) / 2, (max(xs) - min(xs)) / 2
        bx = sum(p[0] for p in bridge) / len(bridge)
        if abs(bx - mid) > 0.3 * half:
            d["_checks"].append(f"bridge at x={bx:.0f} is far from the middle ({mid:.0f})")
        if any(abs(p[1]) > 2 for p in bridge):
            d["_checks"].append("bridge off the centreline")
    lo, hi = SIZE[size]
    if not lo <= len(hull) <= hi:
        d["_checks"].append(f"{len(hull)} hull cells, outside {lo}-{hi}")
    return d


# --------------------------------------------------------------------------- ships
def marshal():
    """Cruiser. The ship that holds a world under martial law: one great pair
    of sponsons amidships, raked fore and aft into a crossguard, so from
    above it is a sword laid across the sky. The crossguard's aft faces
    step down in gatlings, its fore faces in plate, three cannons on each
    tip; railguns in the arrowhead bow, nacelle pods at the stern corners."""
    c = Canvas("th_marshal")
    c.rows(-28, 13, """
                       ##n#n#n##/
                      g.........#/
                     #.a.a.a.O.T.#/
 #####              g..Q.KK.KK.Q..#/
N.....#            #..s.O.....O.s..#/
N.R.b..#          g.GG..m.....O..a.a#/
 ###....##       #..GG.Q.KK...KK.Q...#/
    #.....#######.....................######/
    #.b.c.f.O.d.Q.KK.Q.m.O.M.O.Q.KK.Q.d.a.a.##/
    #..LL.R.b...Q..y......T......y..Q...a.A.a.##/
   E..........d.......................d.........##/
   E..LL..b.c...KK.Q.O..T.H..T..O.Q.KK..a.s.M.a.A.##/
   E.FFF.R..O....O.s.Q.m...BB...m.Q.s...rr.a.T.rr.a.##/
   E.FFF..c...d....M.......BB..T......d..a.....a......n
""")
    return finish(c.mirror(), "cruiser")


def arbiter():
    """Battlecruiser. The Hegemony's railgun doctrine carried to its end: a
    hull built round one spinal railgun. A long barrel runs out ahead of a
    plated delta, accelerator collars along it; one pair of sponsons and the
    nacelle pods sit far aft, so the whole ship reads as a spear. The pods are
    all engine (two large drives and a sustainer each, fed from their own fuel
    and reactor), so it runs as fast as it reaches. Thin plate, and the
    longest reach in the navy."""
    c = Canvas("th_arbiter")
    c.rows(-34, 14, """
             #n#n#/
N#####      g.....#/
N.Q.R.#    #.a.a.O.#/
N.f.b.O## g.Q.KK.Q..#/
N.f.c....#.s..m..O.s.#/
E######..b..KK..GG.Q..####/
       #.R.c....GG.O.m.Q..####/
       #..LL.b.f...Q.KK.Q.O.s.####/
       #.R..c.O.Q..m.y...M...T.A..####/
      E..b.c..KK.Q..KK.Q.O..m.O.Q..s..####/
      E........d............d.............####/
      E..LL.R.b..Q.KK.Q.m.O..H..T..O.a.a.A.a..###/    ###/    ###/
      E....b..c.O...O.s..Q.KK.Q.....T..s.a..M.d..#####cbc#####cbc######/
      E.FFF...m.Q.s.....O..........BB....A..a...d.rr.rr.T.rr.rr.T.rr.s.##/
      E.FFF.c..d............d......BBH..........d........................n
""")
    return finish(c.mirror(), "battlecruiser")


def garrison():
    """Carrier. The Hegemony's carrier keeps the faction's lines: a keel with an
    arrowhead bow, and two flight-deck hulls outboard of it, joined at the
    stern. Between them run two launch lanes open to the bow, drone bays down
    both walls of each lane, so the wings launch straight ahead into the
    fight. Sponsons on the outer flanks carry gatlings and point defence;
    nacelle pods on the decks' stern corners."""
    c = Canvas("th_garrison")
    c.rows(-34, 16, """
                  #g#g#/          #p#p#/
                 #.....#/        #.....#/
  #####         #.a.T.a.#/      #.a.A.a.#/
 #.b.c.#########..O...O..#######..s...s..##/
N.R.b.c..Q.KK.Q...GG..Q.O.KK.Q..M..Q.KK.y.T##/
N.R..O...m.Q.O....GG..m.Q..O..KK.Q..O..y.a.A.##/
 #######..a.a.y.....s..a.a..y.....a.a.....s..M.##/
        #......d........d.............d..........##/
        ####.y.k.k.O...##DD###DD###DD###DD###DD######
            #.LL.b.c..#
            #...d.....#
            #.R.b.KK.O#
        ####...........#####DD######DD######DD###########/
       E...............d.......................d.........##/
       E.b.c.O.Q.KK.Q.m..O.Q.T.H.T..Q.KK.O.a.a.s.M..a.A.a..##/
       E.FFF.R..m.Q..O..s......BB....s...Q..rr..a..rr..T..a..##/
       E.FFF.c...d.....M.......BB..........d..........z........n
""")
    return finish(c.mirror(), "carrier")


def unifier():
    """Dreadnought, the fleet admiral's flagship. The navy's arrowhead drawn at
    fleet scale: a keel and four gun hulls, each pair stepped back from the
    one inside it, so the bows make one great stepped wedge. Crossbeams hold
    the hulls apart round open courtyards, the way the Sovereign's do. In
    the middle of the keel sits the admiral's citadel, a plated diamond that
    is also the forward beam; the bridge is under it, on the centreline."""
    c = Canvas("th_unifier")
    c.rows(-42, 24, """
                #n#n#/      #n#n#/
               g.....#/    g.....#/
       #####  #.a.T.a.#/  #.a.A.a.#/
      #.b.c.##..O...Q..#x#..Q...O..##/
      N.R.f..a.a.a.a...d...a.a.a.s...##/
      N.....d..................d.......##/
      E.R.b..Q.KK.Q.m.O..Q.KK.Q..M..T.A..##/
       ##.....###g#######g####.....g###g#####
         #...#                #...#
         #.d.#                #.d.#
         #...#                #...#
      ###.....####n######n####.....####n####n####/
     E.R.b..Q.GG.Q..a.a.a..Q.O.GG..KK.a....a..T..##/
     E.c.f..m.GG.O..y..M..s..m.GG..Q..y..s...O..A..##/
     E.........d..................d............d.....##/
     E.R.b.O...Q.KK.a.a...Q.KK.O...a.a....a.rr....rr.a.##/
      ###.....####p######p######.............###g###g######
         #...#                  #..q.M...q..#
         #.d.#                /#.s.A....T..s.#/
         #...#               /#.O.q...m...q.O.#/
     ####.....####p######p####.d.............d.####g#####g#####g#####/
    E......d................d...................d............d.......##/
    E.LL.b.c.v.O.Q..SSS.Q.KK.O.Q..a.T.H.T..a.Q.KK.O.Q.M..a.a.s.a.A.a...##/
    E.FFF.R.b.c.m...SSS.O..Q..s..Q....BB...Q..s..Q.m....rr.T.rr.a.rr.T...##/
    E.FFF..d...M............d.........BB........d..............d........z..n
""")
    return finish(c.mirror(), "dreadnought")


def recurrence():
    """Experimental: temporal prototype. Built round a recovered fragment of the
    weapon that shattered the old empire across timelines. Three identical
    plated chevrons ride one keel, nested like the same arrowhead caught three
    times as it moved: the bow chevron carries the guns, the middle one the
    crew and the bridge, the stern one is its engine bank. The fragment rides
    in an armoured cradle in the bow chevron's mouth, held in force fields,
    nothing touching it. The three chevrons share one outline rule (a raked
    arm six cells wide, plated on its leading edge); every room in them is
    placed by hand."""
    c = Canvas("th_recurrence")
    c.rows(-25, 12, """
  ####g#/           ###rr#/           ###rr#/
   E.b..#/           #a..T#/           #a..A#/
    E.R.c#/           #Q..A#/           #T..a#/
     E..f.#/           g...Q#/           g...O#/
      E.R.b#/           #O..M#/           #a..s#/
       E..c.#/           #y..Q#/           #Q..m#/
        #O..b#/           g...s#/      ###/ g...a#/
         #Q..R#/           #O..Q#/    #.s.#/ #Q..Q#/
          #....#/           #m...#/ /#.+M+.#/ #O...#/
/#########......############......###.......##......#/
E.LL.b.c.O.Q.d..a.T..Q.O.H...M.a.d.Q..+.s.+..a.T.a..A#/
E.FFF.R.m.KK..s..Q..m......BB...Q.O...y...y..rr..rr...#/
E.FFF..d....M......d.......BB.......d...$...d...M......n
""")
    return finish(c.mirror(), "experimental")


def concord():
    """Civilian: requisitioned troop liner. A passenger liner of the old empire,
    taken for the army when martial law came. Her long hull and fine bow
    are still a liner's, and two light wells still run through her where
    the atria were, the bridge on a deck between them; but the cabins are
    barracks now, the lounge at the bow holds the troops' mess, and one
    pair of gun sponsons has been bolted on amidships. Nacelle pods flare
    at the stern corners like every Hegemony hull."""
    c = Canvas("th_concord")
    c.rows(-27, 10, """
                       #g#g#/
#######               #.....#/
N.R...b####          #.a...a.#/
N####.O.c..##########.y..T..y.############/
     #.R.b.Q.KK.Q.KK.m.Q.O.M.Q.KK.Q.KK.q..##/
     #..d...........d.......d...........d...##/
E####.LL.f.Q.KK.Q.KK.O...GG...KK.Q.KK.q.O.q.A.##/
E.R.b.c.O.m...#######..Q.GG...#######..q.Q.O.q..##/
E.b.c.Q.O.k.##       ##T.H.A##       ##.Q.O.k.k.s.##/
E.FFF.R.m.Q.#         #.BB..#         #.q.k..www.T..##/
E.FFF.d..M..#         #.BB..#         #..d...www..M...l
""")
    c.mirror()
    c.P(-10, -7, "o")       # the one asymmetric touch: a troop gangway, starboard side
    return finish(c, "civilian")


CONCEPTS = [
    ("marshal", marshal, "Marshal", "Cruiser",
     "The ship that holds a world under martial law. One great pair of sponsons raked into a crossguard, so from above it is a sword laid across the sky."),
    ("arbiter", arbiter, "Arbiter", "Battlecruiser",
     "A hull built round one spinal railgun: a long barrel ahead of a plated delta, accelerator collars along it. It has the last word in any argument it can reach."),
    ("garrison", garrison, "Garrison", "Carrier",
     "A keel and two flight decks joined at the stern, with launch lanes open to the bow and drone bays down both walls. Under martial law, the drone wings are also what watches the cities."),
    ("unifier", unifier, "Unifier", "Dreadnought (flagship)",
     "The fleet admiral's ship. A keel and four gun hulls stepped back into one great arrowhead, crossbeams round open courtyards, the admiral's plated citadel in the middle. Named for a war no one may mention."),
    ("recurrence", recurrence, "Recurrence", "Experimental: temporal prototype",
     "Built round a recovered fragment of the weapon that shattered the empire. Three plated chevrons ride one keel like the same ship caught three times; its crews say it arrives a moment before it leaves."),
    ("concord", concord, "Concord", "Civilian: requisitioned troop liner",
     "A passenger liner of the old empire, taken for the army under martial law. Two light wells still run through her where the atria were; the cabins are barracks now, and one pair of gun sponsons has been bolted on."),
]
