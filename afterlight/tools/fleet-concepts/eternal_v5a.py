"""Eternal Hegemony, round 5a: eight light and medium warships, scout to light
cruiser, next to the approved Echo Escort and Herald. Hand-drawn, top-down
only.

The approved grammar (eternal.py) holds at every size: a stepped CROWN BOW
(a central spire between crown points, the raked steps plated), DOUBLE-TIER
ZIGGURAT SPONSONS, ENGINE PODS at the stern corners round a recessed centre
bank, a PROCESSIONAL NAVE of shield pillars down the keel to the
throne-bridge amidships, cathedral symmetry, Titanium. It is the Terran
grammar of terran.py two hundred years earlier, dressed for parade: where a
Terran hull has one raked face, these have a crown; where it has a square
sponson, these have a stepped tower.

The time-echo: some hulls trail a faint double of one of their parts, a
beat out of time, drawn ':' here and built in Composite so it reads as a
ghost (as the Throne of Ages trails its arches).

Legend: see terran.py and draw.LEGEND; ':' is ghost hull (Composite).
"""
from collections import deque

from draw import Canvas, AIMED, ENGINES, LEGEND
from eternal import FACTION  # noqa: F401  (read by build.py)

N4 = ((0, 1), (0, -1), (1, 0), (-1, 0))
WEAPONS = set("rngxzpiD")
SIZE = {  # BRIEF.md size ladder
    "scout": (80, 150), "corvette": (150, 300), "frigate": (350, 500), "destroyer": (500, 760),
}


# --------------------------------------------------------------------------- helpers
def paint(c, x0, ytop, block):
    """c.rows(), but ':' paints ghost hull: plain hull now, Composite at the end."""
    ghost = c.__dict__.setdefault("ghost", set())
    for k, line in enumerate(block.strip("\n").split("\n")):
        for i, ch in enumerate(line):
            if ch == " ":
                continue
            p = (x0 + i, ytop - k)
            if ch == ":":
                ghost.add(p)
                ch = "#"
            c.cells[p] = ch
    return c


def finish(c, size):
    """Build, then check what draw.py leaves out: every gun has an open side,
    no engine fires into its own hull, the hull is one piece, the skeleton
    (hull, engines, guns) is mirror-symmetric, the bridge is inside the hull
    near the middle, and the hull count fits the class. Ghost cells become
    Composite."""
    d = c.build()
    cells = c.cells
    hull = {tuple(h["grid_pos"]) for h in d["hull_cells"]}
    layer = {tuple(h["grid_pos"]): h["layer"] for h in d["hull_cells"]}
    chk = d["_checks"]
    for p, ch in cells.items():
        kind = LEGEND.get(ch)
        x, y = p
        block = [p] + [(x + dx, y + dy) for dx, dy in N4 if cells.get((x + dx, y + dy)) == ch]
        if kind in AIMED and all((bx + dx, by + dy) in hull or cells.get((bx + dx, by + dy)) == "/"
                                 for bx, by in block for dx, dy in N4):
            chk.append(f"{kind} at {p} has no open side")
        if kind in ENGINES and any((xx, y) in hull for xx in range(x - 60, x)):
            chk.append(f"{kind} at {p} vents into hull")
        if (ch in "EN" or ch in WEAPONS) and cells.get((x, -y)) != ch:
            chk.append(f"'{ch}' at {p} has no mirror image")
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
        chk.append(f"hull in pieces: {len(hull) - len(seen)} cells cut off")
    if {(x, -y) for x, y in hull} != hull:
        chk.append("skeleton not mirror-symmetric")
    bridge = [p for p, ch in cells.items() if ch == "B"]
    if not bridge:
        chk.append("no bridge")
    elif any(layer.get(p) == "Outer" for p in bridge):
        chk.append("bridge on the rim")
    else:
        xs = [p[0] for p in hull]
        mid, half = (min(xs) + max(xs)) / 2, (max(xs) - min(xs)) / 2
        bx = sum(p[0] for p in bridge) / len(bridge)
        if abs(bx - mid) > 0.4 * half:
            chk.append(f"bridge at x={bx:.0f} is far from the middle ({mid:.0f})")
        if any(p[1] != 0 for p in bridge) and not any(p[1] == 0 for p in bridge):
            chk.append("bridge off the centreline")
    lo, hi = SIZE[size]
    if not lo <= len(hull) <= hi:
        chk.append(f"{len(hull)} hull cells, outside {lo}-{hi} for a {size}")
    ghost = getattr(c, "ghost", set())
    ghost = ghost | {(x, -y) for x, y in ghost}
    for h in d["hull_cells"]:
        if tuple(h["grid_pos"]) in ghost:
            h["material"] = "Composite"
    return d


# --------------------------------------------------------------------------- ships
def pursuivant():
    """Scout: outrider. A herald's junior, sent ahead to cry the old fleet's
    coming. The whole grammar at its smallest: a three-point crown, one
    ziggurat sponson a side, pods at the stern corners round a recessed
    centre bank, the notches behind the crown points cut deep."""
    c = Canvas("eh_pursuivant")
    paint(c, -10, 5, """
       #g#/
 ##   #a.a#/   /##l
E..###Q...Q#/ #####
 #.R.d.sM.sM##A#
  E.Q.d.BB.H.T..##g##/
  E.M...BB...O.......n
""")
    return finish(c.mirror(), "scout")


def sealbearer():
    """Scout: courier of the decrees. The decrees of emperors two centuries
    dead still go out by hand, sealed. The hull is the scroll case: a
    roller at each end and the writ between, the decree in three vaults
    behind the throne. The bow roller is a crown, its notches plated; the
    stern roller is all engine, pods on its corners round a recessed bank."""
    c = Canvas("eh_sealbearer")
    paint(c, -9, 5, """
 ##         g##l
E.#        #.Q##
E.R###g####..#/
#..Q.d..sM.AO#/
 E.b..$BB.H...#z#/
 E.M..$BB...M....n
""")
    return finish(c.mirror(), "scout")


def lictor():
    """Corvette: gunboat. A lictor walked before a magistrate carrying the
    fasces, rods bound round an axe. Here the rods are three hulls bound by
    two bands: the centre rod is the nave, with the throne, and runs out to
    the crown's spire; the outer rods are the crown points forward and the
    engine pods aft. An axe blade flares from each outer rod, its edge lined
    with guns, its forward tip plated."""
    c = Canvas("eh_lictor")
    paint(c, -14, 9, """
          ##g#g#g###
           #.....##/
     /#/    #M.a#
E############...####/
E.R.Q...O.Q.........n
E#####.#########.###/
     #d#       #d#
   E##.#########.######/
   E.R.KKsO.BBH.s.T..a.#z#/
   E.M..m...BB............n
""")
    return finish(c.mirror(), "corvette")


def remembrancer():
    """Corvette: escort. The Remembrancer keeps the old fleet's memory of
    itself, and it shows: behind its one great ziggurat tower hang two
    echoes of the same tower, hollow and a beat late, as if the ship were
    still arriving from two hundred years ago. A crown at the bow, a nave of
    pillars to the throne, pods at the stern corners."""
    c = Canvas("eh_remembrancer")
    paint(c, -13, 7, """
        ::: ::: #g#/
       :: ::: :#a.a#/
 #g#  ::  ::: #Q...Q#/
E...  ########s.....s####l
E.R.##.QO.M.a.....a.T..A##
 #.f.KK.s..s..s..s..s..#
  E...c......BBH.....T..#z#g#/
  E.M..m.....BB..M...........n
""")
    return finish(c.mirror(), "corvette")


def sceptre():
    """Frigate: lance. The emperor's sceptre carried into battle: a pommel of
    engines, a rod, an orb with the throne inside and a gun tower stepped on
    top of it, a second rod, and a crowned head. Each point of the crown is a
    railgun and so is the spire; the notches between them are plated."""
    c = Canvas("eh_sceptre")
    paint(c, -20, 8, """
                 #g#/
                #a.a#/
               #Q...Q#/         ##g##/
E###          #.O...m.#/        #a...rr
E..# /#g#/   #.Q.s..s.Q#/ /#g#/ #..##/
E...###.#####.b.c.M..a..####.###..#/
 #...Q..O.d..s.O.s...s.Q.d.T.a.M..#/
  E.R.LL.f....KK..BBH.T...A..KK..a.##z###/
  E..M............BB.....................rr
""")
    return finish(c.mirror(), "frigate")


def thurible():
    """Frigate: missile. A thurible, the censer swung before the procession:
    a stepped dome pierced with a ring of open windows round the throne,
    the smoke going up from every face of it. Here the smoke is missiles:
    the dome's top tier and every step below it is a launcher rail, and a
    long missile rides the spire. Pods at the stern corners, a recessed
    bank between them, a small crown at the bow."""
    c = Canvas("eh_thurible")
    paint(c, -14, 10, """
           #x#x#x#
         #x.......x#/
 ##g###x#..M.###.M..#x/
E.R.....Q.O.#   #.QaT.#/
E..b.d.m.s..#   #..s.A.#/
 ###.Q.#  #.a###a.#  #.O####/
    #..#  #..KK...#  #.a.####
    #.c.##.s....s..##..T#/
   E.R.LL.Q..s..s..Q.a..#/
   E.b.KK.....BBH.W..A...#z####/
   E.M........BB...............x
""")
    return finish(c.mirror(), "frigate")


def oriflamme():
    """Destroyer. The oriflamme was the sacred war banner: once it was
    raised, no quarter. The Eternal raised theirs two hundred years ago and
    have never lowered it. The hull is the banner: the pole across the bow,
    crowned at both ends with the spire between them; the cloth behind it,
    a tall ziggurat tower on each edge; the fly cut into a swallowtail whose
    two tails are the engine pods, each with a small tower of its own, and
    the centre engine bank recessed deep in the fork."""
    c = Canvas("eh_oriflamme")
    paint(c, -22, 12, """
                     #g#/
                    #a.a#/
   #g#g#/          #Q...Q#/         ##g##/
  #a.a.a#/        #O.s.s.m#/        #..###
 #.Q...Q.#     ###.Q.....Q.####/    #.#/
E.R.b.O...#g#g#.a..O...s...s...M..a...z
E.LL.M.Q....Q.Q..T.KK...KK..y...A.....#
E.........d...........................g
E###.R.b...c.f......GG..........W....a#
    ######.Q.OM.Q...GG..m..Q.a..T.....z
          E..s..s..s..s..s..s..s......#/
          E.LL.KK.y...BBH..KK.A.T......#z#g#/
          E...........BB....................rr
""")
    return finish(c.mirror(), "destroyer")


def basilica():
    """Light cruiser. The basilica an emperor's governor would have raised in
    a conquered province, laid out as a hull and seen from above: two
    crowned towers and a spire at the west front (the bow), a long nave with buttress piers along both aisles, a transept whose arms
    end in four-tier ziggurat towers, the throne under the crossing, and a
    chevet at the stern: a rounded apse with chapels radiating from it, every
    chapel an engine pod, the centre bank recessed in the apse."""
    c = Canvas("eh_basilica")
    paint(c, -26, 13, """
                        #g#/
                       #a.a#/
                      #Q...Q#/
                     n..a.a..n
                      #Q.O.Q#/
                      #..d..#               #gg#/
E####                 #a.y.a#              #....n
E.R.#  ####  g#/ g#/  #.s.s.#  g#/ g#/ g#/ #.O..n
E....##.Q..##..##..###.......##..##..##..##..s..#/
E.....O..b..Q.KK..Q....GG......a..Q...Q..a...M####
 ###.M.f.c...O..y..m...GG..T....O..W..O......#/
   #...LL..s..s..s..s.........s..s..s..s..s..#/
  E.R..KK..A..KK..T..s..BBH.s..KK..T..A..KK...#z#g##/
  E.....................BB.........................rr
""")
    return finish(c.mirror(), "destroyer")


CONCEPTS = [
    ("pursuivant", pursuivant, "Pursuivant", "Scout: outrider",
     "A herald's junior, sent ahead to cry the coming of a fleet two hundred years gone: the whole crown-and-tower grammar at its smallest."),
    ("sealbearer", sealbearer, "Sealbearer", "Scout: decree courier",
     "The decrees of emperors two centuries dead still go out under seal. The hull is the seal, a stepped disc ringed with shield pillars, the decree locked in vaults behind the throne."),
    ("lictor", lictor, "Lictor", "Corvette: gunboat",
     "The fasces made a warship: three rod hulls bound by bands, the crown's spire and points at their heads, and an axe blade of guns flaring from each side. It walks before the old fleet's magistrates."),
    ("remembrancer", remembrancer, "Remembrancer", "Corvette: escort",
     "It keeps the old fleet's memory of itself: behind its one ziggurat tower hang two hollow echoes of the same tower, a beat late, as if the ship were still arriving from two hundred years ago."),
    ("sceptre", sceptre, "Sceptre", "Frigate: railgun lance",
     "The emperor's sceptre carried into battle: an engine pommel, a rod, an orb holding the throne under a stepped gun tower, and a crowned head whose every point is a railgun."),
    ("thurible", thurible, "Thurible", "Frigate: missile",
     "The censer swung before the procession: a stepped dome pierced with a ring of open windows round the throne. The smoke that rises from it is missiles, off every step of the dome."),
    ("oriflamme", oriflamme, "Oriflamme", "Destroyer",
     "The sacred war banner: once raised, no quarter. The Eternal raised theirs two hundred years ago and never lowered it. A crowned pole across the bow, a towered banner behind, a swallowtail fly whose tails are the engine pods."),
    ("basilica", basilica, "Basilica", "Light cruiser",
     "The cathedral an emperor's governor raised in a conquered province, seen from above: crowned towers and a spire at the bow, a buttressed nave, transept arms ending in four-tier gun towers, the throne under the crossing, and a chevet of engine chapels round the apse."),
]
