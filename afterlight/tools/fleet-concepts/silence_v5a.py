"""The Silence, round 5a: the small and medium ships that fill out the ladder.

Same grammar as silence.py (the approved Listener, Hush, Vigil and Monastery):
needles 3-5 cells wide with a plate on each step of the point, single-cell
rails and masts carrying the passive ears, a feathered stern, one keel
corridor with a few rooms and stealth coating at an even beat in the walls,
calm port/starboard symmetry, Composite, and no weapons anywhere.

Legend (draw.py): # hull  . deck  / plate  E engine  R reactor  b battery
B bridge  H helm  M memory core  s shield  Q quarters  O oxygen  m medbay
A radar  W advanced radar  ? passive radar  I thermal imager  ~ stealth
J signal buoy  = signal jammer  ! research lab  V cryo  y repair  d door
"""
import math

from draw import Canvas
from silence import FACTION


def vow():
    """Scout. A novice's skiff: a thin needle crossed amidships by one short
    arm each side, the bridge at the crossing. A novice keeps the first vow
    alone at the edge of a system for a year, listening."""
    c = Canvas("si_vow", "Composite")
    c.rows(-13, 7, """
            /?/
            #A#
            #.#
            ~.~
            #.#
  E#~      M#.#M
 E.RO###~##sBB.Q##~###~#/
E...........BB.H........A?
""")
    return c.mirror().build()


def murmur():
    """Scout. The courier. The Silence never broadcast, so what the pickets
    hear is carried home by hand, sealed in memory cores. A thin shaft, the
    bridge in a bead behind it, and a swallow's forked tail, each fork with
    its own feathered engines."""
    c = Canvas("si_murmur", "Composite")
    c.rows(-16, 4, """
 E##~#M#~###
E.R..Q..O...#
 E##~###~....###M##
         #.O.sBB.Q.#~###~###~#/
         E....BB.H............A?
""")
    return c.mirror().build()


def hark():
    """Corvette. The eavesdropper, built like a Yagi aerial: a needle with
    five pairs of masts, the longest aft, each shorter toward the bow, an
    ear at every tip. It parks off a trade lane and hears every hail."""
    c = Canvas("si_hark", "Composite")
    c.rows(-14, 2, """
  E###M~#####~M###
 E.R.Q.O.b.s..BB.Q#~#####~#####~#/
E.............BB.H...............A?
""")
    for x, top in ((-10, 10), (-4, 9), (2, 8), (8, 7), (14, 6)):
        for y in range(2, top):
            c.cells.setdefault((x, y), "#")
        c.cells[(x, top)] = "?"
    return c.mirror().build()


def mirage():
    """Corvette. The decoy, drawn as an enso: the one brush circle a monk
    paints in a single breath. A ring of hull that swells thick at the stern,
    where the bridge sits, and thins on both sides toward the bow, where it
    does not quite close; past the end of the stroke the band frays into thin
    rails like the dry hairs of the brush, hung with signal buoys and
    jammers. On a scope the hairs and the gap read as a ship that is not
    there."""
    from silence_v5b import finish         # the stricter checks (one piece, symmetry, vents)
    c = Canvas("si_mirage", "Composite")
    rc, g = 8.0, 46.0                      # stroke radius, half the gap (degrees)
    for x in range(-13, 13):
        for y in range(0, 13):
            r, a = math.hypot(x, y), math.degrees(math.atan2(y, x))
            if a >= g:                     # the band: 6 thick at the stern, 3.4 at the ends
                t = 3.4 + 2.8 * ((a - g) / (180 - g)) ** 0.8
                if abs(r - rc) <= t / 2:
                    c.cells[(x, y)] = "~" if r > rc + t / 2 - 0.9 and round(a) % 24 < 6 else "#"
    # the dry hairs past the end of the stroke: an outer one, long, flaring a
    # little, and an inner one, short; ears at their tips, buoys and jammers
    # along them
    for p, ch in (((7, 7), "#"), ((7, 6), "J"), ((8, 6), "="), ((8, 5), "J"), ((9, 5), "#"),
                  ((9, 4), "="), ((9, 3), "J"), ((10, 3), "#"), ((10, 2), "="), ((10, 1), "?"),
                  ((5, 5), "#"), ((6, 5), "J"), ((6, 4), "="), ((6, 3), "J"), ((7, 3), "#"),
                  ((7, 2), "?")):
        c.cells[p] = ch
    # the stern, the thick of the stroke: bridge, helm, engines feathered
    # down the curve of the outer face
    c.R(-8, -7, 0, 1, "B")
    c.P(-6, 0, "H")
    c.P(-11, 0, "E").P(-11, 1, "E").P(-10, 2, "E")
    c.P(-9, 3, "R.O").P(-9, 4, "Q.b")
    # round the band toward the bow
    c.P(-4, 5, "M").P(-5, 6, "d")
    c.P(-3, 8, "Q").P(-1, 9, "s").P(1, 8, "O").P(3, 7, "d").P(4, 8, "M").P(5, 7, "I").P(4, 6, "A")
    # ears and a jammer on the outer face
    c.P(-10, 3, "?").P(-5, 9, "J").P(0, 10, "=").P(5, 9, "?")
    return finish(c.mirror())


def unison():
    """Corvette. The interferometer: a long nose and two arms swept back
    from a small hub, each arm ending in a rail of passive ears. Three ears
    set far apart hear as one very large one. The engines step back along
    the trailing edge of both arms like fletching."""
    c = Canvas("si_unison", "Composite")
    for y in range(0, 15):
        for x in range(-20, 6):
            if y >= 2 and -1 <= x + y <= 2:
                c.cells[(x, y)] = "#"                    # the swept arm
            if abs(x) + y <= 4 and y < 4:
                c.cells[(x, y)] = "#"                    # the hub
    for p in ((-4, 0), (-3, 1), (-3, 2), (-4, 3), (-5, 4)):
        c.cells[p] = "E"                                 # fletching, both arms
    c.R(-1, 0, 0, 1, "B")
    c.P(1, 0, "H").P(-2, 1, "O").P(1, 1, "Q").P(2, 2, "M").P(1, 2, "s")
    c.P(-3, 3, "R").P(-4, 4, "R")
    for x, y in ((-3, 5), (-6, 8), (-9, 11)):
        c.cells[(x, y)] = "~"                            # leading edge, every third row
    for x, y in ((-7, 6), (-10, 9)):
        c.cells[(x, y)] = "~"                            # trailing edge
    c.cells[(-8, 10)] = "M"
    c.P(-15, 14, "##A#")                                 # each arm ends in an ear:
    c.P(-14, 15, "#")                                    # a rail on a one-cell strut
    c.P(-19, 16, "I########?")
    c.rows(4, 1, """
#~###~###~###~##/
................A?
""")
    return c.mirror().build()


def _p(c, x0, x1, y, s):
    """Write a run of cells from x0 to x1 on row y (the length is checked)."""
    if len(s) != x1 - x0 + 1:
        raise ValueError(f"row {y}: {len(s)} cells for x {x0}..{x1}")
    c.P(x0, y, s)


def _bead(c, cx, top, r3, r2, r1):
    """A round bead on a 3-wide thread, centred on x=cx: rows y=4..1."""
    _p(c, cx - 1, cx + 1, 4, top)
    _p(c, cx - 2, cx + 2, 3, r3)
    _p(c, cx - 3, cx + 3, 2, r2)
    _p(c, cx - 4, cx + 4, 1, r1)


def pilgrim():
    """Frigate. Carries the faithful between monasteries: one long needle
    with its eye near the stern, as a sewing needle has. The pilgrims' cells
    are ranged along both sides of the eye at an even beat; the bridge sits
    in a bead just ahead of it, in the middle of the ship."""
    c = Canvas("si_pilgrim", "Composite")
    _p(c, -27, -8, 4, "E##Q#~#Q#~#Q#~#Q#~##")                   # outer wall of the range
    _p(c, -28, -7, 3, "E" + "." * 20 + "#")                      # the range's corridor
    _p(c, -2, 4, 3, "~###M##")                                   # the bead over the bridge
    _p(c, -29, 19, 2, "E" + "..O...." + "#Q#~#M#~#Q#~#" + ".." + "~###" + ".Q..s.."
       + "#~###~###~####" + "/")
    _p(c, -30, 24, 1, "E" + ".R.b.R." + "#" + " " * 13 + "#" + "..O.." + ".BB...."
       + ".Q...m...I...." + "###~#" + "/")
    _p(c, -31, 25, 0, "E" + "." * 8 + "#" + " " * 13 + "#" + "." * 6 + "BB.H" + "." * 21 + "A?")
    # an outrigger rail along the shaft
    for y in (3, 4):
        c.P(8, y, "#").P(16, y, "#")
    _p(c, 6, 18, 5, "I" + "#" * 11 + "?")
    return c.mirror().build()


def lauds():
    """Frigate. The direction finder: two long needles crossed at the bridge
    like a pair of knitting needles, each listening down its own bearing.
    Where the two bearings cross, it knows exactly where you are."""
    c = Canvas("si_lauds", "Composite")
    for x in range(-29, 30):
        for y in range(0, 10):
            if abs(y - abs(x) / 4) <= 1.5:
                c.cells[(x, y)] = "#"                    # the two needles
    c.R(-4, 4, 0, 2)                                     # the bead where they cross
    for x in (29, -29):
        c.cells[(x, 6)] = c.cells[(x, 8)] = "/"          # recut both ends below
    c.P(29, 7, "A?").P(28, 8, "#").P(28, 6, "#")
    c.cells[(29, 8)] = c.cells[(29, 6)] = "/"            # the bow points
    c.P(-29, 7, "E").P(-28, 8, "E").P(-28, 6, "E")       # feathered sterns
    c.cells.pop((-29, 8)); c.cells.pop((-29, 6))
    c.P(-6, 0, "E")
    # one segment of each needle every four cells: a fitting on its upper
    # wall and stealth coating on its lower wall, at the same even beat
    for k, (fore, aft) in enumerate((("A", "R"), ("A", "Q"), ("A", "O"), ("A", "Q"), ("?", "b"))):
        x, top = 8 + 4 * k, 3 + k
        c.cells[(x, top)] = fore
        c.cells[(-x, top)] = aft
        c.cells[(x, top - 2)] = c.cells[(-x, top - 2)] = "~"
    _p(c, -4, 4, 2, "#M#####M#")                         # the bead
    _p(c, -3, 3, 1, ".sBB.Q.")
    c.P(-1, 0, "BB.H")
    return c.mirror().build()


def compline():
    """Destroyer. The night office, and the one that leads a listening flight
    home: a long lance out in front and two shorter needles trailing it like
    wingmen in a flight of three, held on one yoke, rails on struts outboard.
    It keeps itself between the flight and harm."""
    c = Canvas("si_compline", "Composite")
    # the lead lance, 5 wide, with a bead over the bridge
    _p(c, -22, 30, 0, "E" + "." * 21 + "BB.H" + "." * 25 + "A?")
    _p(c, -21, 29, 1, "E" + "..R.O...Q..b..y..s.." + "BB" + ".....s...Q...m...I...W...." + "#/")
    _p(c, -20, 28, 2, "E" + "###~#.#~###~###~" + "." * 9 + "##~###~###~###~###~###" + "/")
    _p(c, -3, 5, 3, "#M#####M#")
    # one yoke out to the wingmen
    for y in (3, 4, 5, 6):
        _p(c, -15, -13, y, "#.#")
    # the trailing needle, 3 wide
    wall = "####~###~###~###~###.###~####"
    _p(c, -35, -5, 7, "E" + wall + "/")
    _p(c, -36, -4, 8, "E" + "..R....Q....O....Q............" + "A?")
    _p(c, -35, -5, 9, "E" + wall.replace(".", "#") + "/")
    c.cells[(-8, 9)] = "s"                               # a shield at each wingman's bow
    # an outrigger rail on two struts
    for y in (10, 11):
        c.P(-27, y, "#").P(-17, y, "#")
    _p(c, -32, -12, 12, "I" + "#" * 9 + "W" + "#" * 9 + "?")
    return c.mirror().build()


CONCEPTS = [
    ("vow", vow, "Vow", "Scout",
     "A novice's skiff: a thin needle crossed amidships by one short arm each side. The first vow is a year alone at the edge of a system, listening."),
    ("murmur", murmur, "Murmur", "Scout: courier",
     "The Silence never broadcast, so what the pickets hear goes home by hand in sealed memory cores. A thin shaft and a swallow's forked tail."),
    ("hark", hark, "Hark", "Corvette: eavesdropper",
     "Built like a Yagi aerial: five pairs of masts, longest aft, an ear at every tip. It parks off a trade lane and hears every hail."),
    ("mirage", mirage, "Mirage", "Corvette: decoy",
     "An enso, the brush circle drawn in one breath: a ring thick at the stern where the bridge sits, thinning toward a gap at the bow, its ends fraying into thin rails hung with buoys and jammers. On a scope the hairs read as a ship that is not there."),
    ("unison", unison, "Unison", "Corvette: interferometer",
     "A long nose and two arms swept back from a small hub, an ear at each tip. Three ears set far apart hear as one very large one."),
    ("pilgrim", pilgrim, "Pilgrim", "Frigate: pilgrim ship",
     "One long needle with its eye near the stern, as a sewing needle has. The pilgrims' cells line both sides of the eye; the bridge sits just ahead of it."),
    ("lauds", lauds, "Lauds", "Frigate: direction finder",
     "Two needles crossed at the bridge, each listening down its own bearing. Where the two bearings cross, it knows exactly where you are."),
    ("compline", compline, "Compline", "Destroyer: flight leader",
     "A long lance in front and two shorter needles trailing it like wingmen in a flight of three, held on one yoke. It leads a listening flight home and keeps itself between the flight and harm."),
]
