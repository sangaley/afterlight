"""Broken Choir, round 5a: the light and medium warships, hand-drawn, top-down.

The approved set (choir.py) has a corvette, a frigate, a capital, an
experimental and a station. These fill the light and middle of the ladder:
two scouts, three corvettes, two frigates and a destroyer, each with its own
job and its own outline, all speaking the same grammar (FACTION["grammar"]):
shards split by open cracks, held on by thin 3-wide struts, sounding banks of
3-wide organ pipes studded with listening gear.

Symmetry: every skeleton is drawn on y >= 0 and mirrored (outline, engines,
main guns). The Choir's broken symmetry is kept to small details added after
the mirror: a cracked-off plate, a missing stud, a chipped corner.

Legend (draw.py): # hull  . deck  / plate  E engine  R reactor  L large reactor
b battery  f fuel  c cooling  v vent  B bridge  H helm  M memory core
s shield  A radar  W advanced radar  ? passive radar  = jammer  J signal buoy
T targeting  Q quarters  K barracks  G galley  m medbay  O oxygen  a ammo
y repair  q officer  g gatling  x heavy missile  * cluster rocket  d door
"""
from choir import FACTION, finish
from draw import Canvas

assert FACTION["slug"] == "broken_choir"


def pipe(c, yc, x0, tip, weapon, top="", bot="", plates=True):
    """One organ pipe from x0 to its mouth at tip: rim, deck, rim, with the
    weapon in the mouth between two plates. top / bot paint listening gear
    along the two rims from x0 (a space or '#' leaves plain rim)."""
    for x in range(x0, tip):
        c.cells[(x, yc + 1)] = "#"
        c.cells[(x, yc)] = "."
        c.cells[(x, yc - 1)] = "#"
    for row, studs in ((yc + 1, top), (yc - 1, bot)):
        for i, ch in enumerate(studs):
            if ch not in " #":
                c.cells[(x0 + i, row)] = ch
    if weapon:
        c.cells[(tip, yc)] = weapon
    if plates:
        c.cells[(tip, yc + 1)] = "/"
        c.cells[(tip, yc - 1)] = "/"


# --------------------------------------------------------------------- scouts

def whisper():
    """Scout, picket. A tuning fork struck and left ringing: two long
    listening tines on a U-shaped yoke, cracked off the engine stem and held
    on by one strut. More ear than ship. One tine has lost a plate."""
    c = Canvas("bc_whisper", "Steel")
    c.rows(0, 4, """
          ##A##
  #M#    #MO...
 #.O.#  #..Q...
E.R.b.##b.BBW#
E.........BBH#
""")
    pipe(c, 3, 15, 23, "g", top=" ?  =  ", bot="  =  ? ")
    c.mirror()
    del c.cells[(23, -4)]             # a plate cracked off the lower tine
    return finish(c)


def keen():
    """Scout, interceptor. A broken arrow: a nocked body shard with the
    engines, a shaft pipe out to a rocket at the bow, and two barbs swept back
    from the arrowhead, cracked off the shaft and each held on by one strut.
    The barbs carry engines of their own."""
    c = Canvas("bc_keen", "Steel")
    pipe(c, 0, 8, 22, "*", top="  ?  =     ?")
    c.rows(0, 7, """
           g#/
           EM##/
           E..f##/
            #...Q##/
             ##.#####
  #M##g#      #.#
 E.R.BB.#
E.O.bBB..
""")
    c.mirror()
    return finish(c)


# ------------------------------------------------------------------ corvettes

def lament():
    """Corvette, brawler. A struck gong: a round hull shattered into a boss
    and three pieces, a Y of cracks running out from a ring crack round the
    boss, each piece held on by one strut. Three stub pipes jut from the
    front piece and gatlings ring the rim. It closes, and keeps hitting."""
    c = Canvas("bc_lament", "Steel")
    c.rows(0, 8, """
     ##g###
   #gQ.m.M# g#/
  #f...KK# #.ag/
 #...c#..# #O.Q#
 E.QO# #.# #....
E..b#  #.#  #a...
E.R#  #M.T#  #.R#
#### #..BB.##b.a.
     #.HBB.......
""")
    pipe(c, 0, 17, 22, "*", top=" ?  ")
    pipe(c, 4, 16, 20, "g", top=" = ")
    c.mirror()
    del c.cells[(14, -7)]             # one plate shaken off the lower rim
    return finish(c)


def syrinx():
    """Corvette, missile boat. Pan pipes: five pipes side by side, cut level
    at the mouths and graduated at the tail, bound by a band across their
    middle; the bridge sits in the knot where the band crosses the longest
    pipe. Every mouth is a launcher, every tail an engine."""
    c = Canvas("bc_syrinx", "Steel")
    pipe(c, 0, 0, 18, "x", top="  =  ?        ?=")
    pipe(c, 4, 3, 18, "*", top="  ?  =  ?  =  ", bot=" =  ")
    pipe(c, 8, 6, 18, "x", top=" ?  =M g  ?", bot="    ")
    for yc, x0 in ((0, 0), (4, 3), (8, 6)):
        c.cells[(x0, yc)] = "E"
    c.rows(6, 3, """
 Q.O.m.
#R.M.ba#
..BB.T..
.HBB....
""")
    c.rows(9, 6, "#.#")
    c.mirror()
    del c.cells[(18, -9)]             # a plate cracked off the lowest mouth
    return finish(c)


def nocturne():
    """Corvette, escort. An hourglass: two wedge shards flaring fore and aft
    from a pinched waist, cracked off a small hub that holds the bridge and
    held to it by two struts. The wide fore face is a wall of gatlings and
    stub pipes it keeps between the fleet and whatever comes; the aft face is
    all engines, to keep station."""
    c = Canvas("bc_nocturne", "Steel")
    c.rows(0, 8, """
                      ##g
                     #M..
##                  #Q...
E.#                #.O.a.
#..#              #.KK..g
E.f.#            #.Q.m.a.
#.R.c#   #M###  #..R.c...
E.b.Q.###.BBT.##.b.s.....
E.R.O.....BBH...........g
""")
    pipe(c, 2, 25, 29, "g", top="  ")
    pipe(c, 6, 25, 27, "g")
    c.mirror()
    return finish(c)


# ------------------------------------------------------------------- frigates

def segs(c, rows):
    """Paint runs of cells row by row: {y: [(x, "cells"), ...]}; spaces skip."""
    for y, runs in rows.items():
        for x, run in runs:
            c.P(x, y, run)


def outcry():
    """Frigate, hunter. A cry flung out in every direction: four shards
    blown off one core in an X, each still held by a single strut. The
    forward pair hook into short pipes like claws, a heavy missile in each;
    the aft pair carry a staircase of drives down their trailing edges."""
    c = Canvas("bc_outcry", "Steel")
    segs(c, {
        13: [(5, "#g##"), (28, "###")],
        12: [(5, "E.MR##"), (26, "##...")],
        11: [(6, "E.R..#"), (25, "gQ..f#")],
        10: [(7, "E.b..#"), (24, "#M..a#")],
        9: [(8, "#R...#"), (23, "gO..b#")],
        8: [(9, "#bO..#"), (22, "gQ..a#")],
        7: [(10, "##Q..#"), (21, "#...##")],
        6: [(12, "##..#"), (20, "#..##")],
        5: [(14, "#.#"), (20, "#.#")],
        4: [(14, "#..#M#..#")],
        3: [(13, "#Q.O..m.Q.#")],
        2: [(12, "#R.b.KK.a.s.#")],
        1: [(11, "E.LL.HBB.W.a..#")],
        0: [(11, "E.....BB.......")],
    })
    pipe(c, 0, 26, 34, "x", top=" ?  =  ")
    pipe(c, 12, 31, 36, "x", top=" ? =", bot="")
    c.cells[(30, 10)] = "/"           # plates on the inside of each claw
    c.cells[(28, 8)] = "/"
    c.mirror()
    del c.cells[(28, -8)]             # one claw has shed a plate
    return finish(c)


def verdict():
    """Frigate, line. The scales the Choir weighs the living on: a keel hub
    with its needle pipe pointing at the enemy, and a beam out to port and
    starboard with a round pan hung from each end. Each pan has cracked off
    its beam and is held by a single strut; each carries guns all round and
    a drive bank on its aft rim."""
    c = Canvas("bc_verdict", "Steel")
    segs(c, {
        18: [(6, "#g#g#")],
        17: [(5, "#.Q.m.#")],
        16: [(4, "#.a.M.O.g")],
        15: [(4, "E.R...b..")],
        14: [(4, "E.....Q..")],
        13: [(4, "E.c.a..M.")],
        12: [(4, "#.......g")],
        11: [(5, "g.....g")],
        10: [(6, "#...#")],
        9: [(7, "#.#")],
        8: [(7, "#.#")],
        7: [(7, "#.#")],
        6: [(7, "#.#")],
        5: [(7, "#.#")],
        4: [(2, "##M##...##g##")],
        3: [(1, "#.Q.O.m.Q.a..s#")],
        2: [(0, "E.R.KK..T.c.a..#")],
        1: [(0, "E.b.f.HBB.W.y...")],
        0: [(0, "E......BB........")],
    })
    pipe(c, 0, 16, 26, "x", top="  ?  =  ")
    pipe(c, 14, 13, 18, "x")
    c.mirror()
    return finish(c)


# ------------------------------------------------------------------ destroyer

def rib(c, rx, y0, y1, cells):
    """One rib: a 5-wide band swept back from its root at (rx, y0) to its
    tip at row y1, one cell aft per row. cells paints it row by row from the
    root up: each string runs from the band's aft edge (a space leaves hull)."""
    for k, y in enumerate(range(y0, y1 + 1)):
        x0 = rx - k - 2
        c.R(x0, x0 + 4, y, y)
        if k < len(cells):
            c.P(x0, y, cells[k])


def threnody():
    """Destroyer. The carcass of something vast that still sings: a backbone
    with the bridge in its heart, three pairs of ribs swept back from it,
    each cracked off at the root and hung on one strut, a pelvis full of
    drives, and a skull at the bow sounding three pipes."""
    c = Canvas("bc_threnody", "Steel")
    #   pelvis x0..7, backbone x8..37, strut x38, skull x39..45
    segs(c, {
        6: [(2, "#g#g#")],
        5: [(1, "#.Q.O.#"), (40, "#g##")],
        4: [(0, "E.R.b.c#"), (13, "#.#"), (21, "#.#"), (29, "#.#"), (39, "#.Q..")],
        3: [(0, "#.f.Q..." + "##?###.###M###.##=####.###M#?#"), (39, "#M.a.")],
        2: [(0, "E.c.m.O." + ".Q.Om..KK.a.Q..y.Q.a...c.O.m.#"), (39, "#.O.m.#")],
        1: [(0, "#.R.b..." + ".LL.b..T.A..H.BB.W.R.f..s.a..." + "#"), (39, "..b.s..")],
        0: [(0, "E......." + "..............BB.............." + "."), (39, ".......")],
    })
    pipe(c, 0, 46, 54, "x", top="  ?  = ")
    pipe(c, 4, 44, 49, "*")
    # each rib from its root row up, strings running from the band's aft
    # edge: the walkway climbs the aft side, rooms face forward
    rib(c, 14, 5, 11, [" ... ", " ..Q ", "E..O#", "E..a#", "E..M#", "E..Q#", "  g  "])
    rib(c, 22, 5, 11, [" ... ", " ..a ", " ..Q ", " ..m ", " ..y ", " ..O ", "  x  "])
    rib(c, 30, 5, 11, [" ... ", " ..Q ", " ..M ", " ..a ", " ..O ", " ..Q ", "  x  "])
    for x in (11, 19, 27):            # a plate on each rib's tip
        c.cells[(x, 11)] = "/"
    c.mirror()
    del c.cells[(27, -11)]            # one rib tip has lost its plate
    return finish(c)


CONCEPTS = [
    ("whisper", whisper, "Whisper", "Scout",
     "A tuning fork struck and left ringing: two long listening tines on a yoke, cracked off the engine stem and held by one strut. More ear than ship; it hears your crew think before it sees you."),
    ("keen", keen, "Keen", "Scout",
     "A broken arrow built to run things down: a shaft pipe to a rocket at the bow, and two swept barbs cracked off it, each hanging on by one strut and carrying its own drives."),
    ("lament", lament, "Lament", "Corvette",
     "A struck gong: a round hull shattered into a boss and three pieces by a Y of cracks, each hung on one strut. Stub pipes jut from the front, gatlings ring the rim; it closes and keeps ringing."),
    ("syrinx", syrinx, "Syrinx", "Corvette",
     "A voice box made of pan pipes: five pipes cut level at the mouths and graduated at the tail, bound by one band with the bridge in its knot. Every mouth a launcher, every tail a drive."),
    ("nocturne", nocturne, "Nocturne", "Corvette",
     "An hourglass: two wedges flaring from a pinched waist, cracked off the bridge hub and held by struts. Its wide fore face is a wall of gatlings and stub pipes kept between the fleet and the enemy."),
    ("outcry", outcry, "Outcry", "Frigate",
     "A cry flung out in every direction: four shards blown off one core in an X, each held by one strut. The forward pair hook into short pipes like claws; the aft pair carry a staircase of drives."),
    ("verdict", verdict, "Verdict", "Frigate",
     "The scales the Choir weighs this reality on, and finds it wrong: a keel hub whose needle pipe points at the enemy, and a beam to port and starboard with a round gun pan hung from each end on one strut."),
    ("threnody", threnody, "Threnody", "Destroyer",
     "The carcass of a dead civilisation, still singing: three pairs of ribs swept back off a backbone, each hung on one strut, a pelvis of drives and a skull at the bow sounding three pipes."),
]
