"""Broken Choir, round 5b: the heavies, the flagship, two experimentals and
three traders. Hand-drawn, top-down only, in the approved grammar (choir.py):
shards with crack-gaps between them, thin 3-wide struts holding them, organ
pipes side by side at staggered lengths, symmetry snapped only in small
details, Steel, gatlings, heavy missiles and cluster rockets.

Every skeleton is drawn on y >= 0 and mirrored (outline, engines, guns);
what breaks the symmetry is added after the mirror and kept small: a stud
gone, a plate gone, a panel cracked out.

Legend: see choir.py (and draw.py). C bulk cargo (2x2), U large cargo (2x1),
k small cargo.
"""
from collections import deque

from choir import FACTION  # same faction, same page
from draw import Canvas

N4 = ((0, 1), (0, -1), (1, 0), (-1, 0))


def finish(c):
    """Build, then the checks the shard grammar needs on top of draw.py's
    (the same as choir.finish): the hull is one piece, and no engine fires
    into its own hull."""
    d = c.build()
    hull = {tuple(h["grid_pos"]) for h in d["hull_cells"]}
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
    for (x, y), ch in c.cells.items():
        if ch in "EN" and any((xx, y) in hull for xx in range(x - 60, x)):
            d["_checks"].append(f"engine at {(x, y)} vents into hull")
    return d


def _segs(c, rows):
    """Paint runs of cells row by row: {y: [(x, "cells"), ...]}; spaces skip."""
    for y, runs in rows.items():
        for x, s in runs:
            c.P(x, y, s)


def paint(c, runs):
    """Paint runs of cells: [(y, x, "cells"), ...]; spaces skip. A list,
    so several runs on one row never overwrite each other."""
    for y, x, s in runs:
        c.P(x, y, s)


def snap(c, gone=(), dead=()):
    """The broken symmetry, added after the mirror and kept small: plates
    lost (gone) and listening studs gone dead to bare hull (dead)."""
    for p in gone:
        assert c.cells.get(p) == "/", p
        del c.cells[p]
    for p in dead:
        assert c.cells.get(p) in ("?", "="), p
        c.cells[p] = "#"


def pipe(c, yc, x0, tip, weapon, gear=0):
    """A forward organ pipe, as in choir.py: 3 wide, deck down the middle,
    listening gear on both rims at a pitch of six, a weapon at the mouth
    between two plates."""
    for x in range(x0, tip):
        c.cells[(x, yc + 1)] = "#"
        c.cells[(x, yc)] = "."
        c.cells[(x, yc - 1)] = "#"
        k = (x - x0 + gear) % 6
        if x < tip - 2:
            if k == 2:
                c.cells[(x, yc + 1)] = "?"
            elif k == 5:
                c.cells[(x, yc - 1)] = "="
    c.cells[(tip, yc)] = weapon
    c.cells[(tip, yc + 1)] = "/"
    c.cells[(tip, yc - 1)] = "/"


def vpipe(c, xc, y0, tip, weapon, gear=0):
    """The same pipe standing out to port (+y), mouth outboard; the mirror
    gives its twin to starboard."""
    for y in range(y0, tip):
        c.cells[(xc - 1, y)] = "#"
        c.cells[(xc, y)] = "."
        c.cells[(xc + 1, y)] = "#"
        k = (y - y0 + gear) % 6
        if y < tip - 2:
            if k == 2:
                c.cells[(xc - 1, y)] = "?"
            elif k == 5:
                c.cells[(xc + 1, y)] = "="
    c.cells[(xc, tip)] = weapon
    c.cells[(xc - 1, tip)] = "/"
    c.cells[(xc + 1, tip)] = "/"


# --------------------------------------------------------------------------- heavies
def antiphon():
    """Cruiser. Antiphony is two half-choirs answering each other from either
    side of the church. Here they are the two broadsides: a bank of five
    pipes stands out to port and the same five to starboard, longest aft, so
    the ship sings one side and is answered from the other. The keel is
    three shards, cracked apart in chevrons and held by keel struts: an
    engine shard, the wind chest the pipes rise from (bridge in it), and an
    arrowhead bow."""
    c = Canvas("bc_antiphon", "Steel")
    _segs(c, {
        # the port choir: a wind chest the pipes stand on
        11: [(11, "#" * 25)],
        10: [(10, "#Q..a.m.a.O.a.Q.a.M.a.y.a.#")],
        9: [(9, "#" + "." * 27 + "#")],
        8: [(8, "###.#####################.#####")],
        # two struts across the crack
        7: [(10, "#.#"), (32, "#.#")],
        6: [(10, "#.#"), (32, "#.#")],
        # the keel: engine shard, chevron crack, bridge shard, chevron
        # crack, arrowhead bow
        5: [(1, "#g########.##"), (17, "################.#####g##")],
        4: [(0, "E.............#"), (18, "#.Q.O.M.KK.Q.y...s.a.Q.O#"), (46, "##g#/")],
        3: [(0, "E.R.LL.c.Q.m.a.#"), (19, "#.GG.b.f.R.T...A.LL.c.m.#"), (47, "#.O###/")],
        2: [(0, "E.f.M.b.v.O.KK..#"), (20, "#GG.Q.c.LL.H...R.b.f.Q..#"), (48, "#.a.M.##/")],
        1: [(0, "E.R.LL.c.y.T.b....###.O.KK.m.BB.W.KK.Q.a.s.y.####.m.T.Q.####/")],
        0: [(0, "E" + "." * 28 + "BB" + "." * 29 + "g")],
    })
    for xc, tip, weapon, gear in ((13, 25, "x", 0), (17, 23, "*", 2), (21, 21, "x", 4),
                                  (25, 19, "*", 1), (29, 17, "x", 3), (33, 15, "g", 5)):
        vpipe(c, xc, 11, tip, weapon, gear)
    c.mirror()
    snap(c, gone=[(22, -21)], dead=[(18, -14)])
    return finish(c)


def schism():
    """Battleship. The Choir broke itself on purpose; this hull broke down
    its keel and was never mended. Two blades sail side by side with open
    space between them, held only by struts across the crack, like the
    rungs of a ladder. The bridge sits on the one shard that still spans the
    crack, amidships. Each blade sounds three pipes forward, longest by the
    crack, so the split runs on between the two longest pipes."""
    c = Canvas("bc_schism", "Steel")
    c.rows(0, 13, """
                          ###g#####g####
   #g##g#            ##g##..............
 ##Q.O..M##      ####.Q.M.GG.O.T.Q.KK.a.
E.R.LL.f.b..#   #.Q.b.f.R.GG.c.LL.m.a.Q..#
N............###.O.Q.m.y.a.R.b.M.Q.O.T.a.a.#
N.c.O.Q.m.y.....................................
E.R.LL.v.b.Q..####.GG.Q.LL.c.f.O.KK.m.a.a..#
E.............#   #GG.m.Q.b.R.T.Q.f.R.c.b.Q.##
N.f.M.KK.O.a..#   #.Q.a.KK.O.M.y.a.Q.b.A.O.a.a##
N.c.b.Q.y.T.R..#   #............................
E######.########   #####.#######.....#####.#####
      #.#                #.#     #.....#   #.#             #.#
      #.#                #.#     #.BB.H#   #.#             #.#
      #.#                #.#     #.BB..#   #.#             #.#
""")
    pipe(c, 4, 48, 64, "x", 0)
    pipe(c, 8, 44, 57, "*", 3)
    pipe(c, 12, 40, 50, "g", 1)
    c.P(58, 3, ".")
    c.mirror()
    snap(c, gone=[(64, -5)], dead=[(47, -13)])
    return finish(c)


def cathedral():
    """Flagship, and the seat of the Precentor who leads the Choir: a
    cathedral seen from above, east end forward. Its roof has fallen in, so
    the nave is open space between two aisle walls; flying buttresses (struts)
    hold gun piers off the aisles; the twin west towers and the portal carry
    the engines. The bridge is in the crossing. Each transept arm, cracked off
    the crossing and held by one strut, sounds a case of three pipes forward
    either side of the choir; the apse ends in a chapel hung on a strut."""
    c = Canvas("bc_cathedral", "Steel")
    # the twin west tower (port)
    c.rows(0, 17, """
  #g###g#
 #.Q.O.Q.#
E.........#
E.R.f.M.c.#
N.........#
N.LL.b.Q.v#
E.........#
E.R.O.m.a.#
#.Q.b.T.Q..###
 #............
  ####.#######
""")
    # the portal on the keel, hung off the tower by one strut
    c.rows(2, 6, """
   #.#
   #.#
 ###.####
E.Q.O..Q.#
E.R.LL.M.#
E.c.b.y.Q#
E........#
""")
    # the port aisle wall: inner rim on the open nave, buttress roots out
    c.rows(14, 9, """
######.#######.#######.###########
..................................
.O.b.Q.c.a.Q.M.b.Q.O.a.Q.c.b.y.###
###############################
""")
    # flying buttresses and their gun piers
    for x in (20, 28, 36):
        c.rows(x - 2, 16, """
#gxg/
#...#
#a.a#
##.##
 #.#
 #.#
 #.#
""")
    # the crossing: the bridge in the middle of the ship
    c.rows(48, 10, """
 #####.####
#.Q.m.O.a.q#
.KK.R...R.KK#
#...........#
#GG.LL.LL.GG#
#GG.c.M.b.GG#
#.Q.f...f.Q.#
#.O.y.H.y.O.#
#.s.A...W.s.#
#....BB......
#....BB......
""")
    # the transept strut, and the port transept arm sounding three pipes
    c.rows(48, 24, """
   ##g##g####
  #.Q..O.M...
 #.a.KK.Q.y..
#...........#
#.Q.R.a.c.R..
#..M.Q.m.Q...
#.b.O.LL.O.c.
#.a.Q.a.Q.a.#
#.....GG.....
#.s.A.GG.A.s.
#.Q.m.O.Q.y.#
 #####.#####
     #.#
     #.#
""")
    for yc, tip, weapon, gear in ((15, 79, "x", 0), (19, 76, "*", 2), (23, 73, "x", 4)):
        pipe(c, yc, 61, tip, weapon, gear)
    # the choir and apse, cracked off the crossing, a chapel hung off the apse
    c.rows(61, 7, """
    ###g#####g##/
   #.Q..O.M.Q...##/
   #.KK.a.R.a.KK..##/
   #.b.GG.T.GG.c....##/
   #.O.GG.y.GG.O......#/
   #.f.Q.c.LL.b.Q.A....#/   ####/
###.Q.KK.m.O.Q.y.a.T....#####.a.#/
.................................*
""")
    c.mirror()
    snap(c, gone=[(30, -16), (81, -5)], dead=[(66, -14)])
    return finish(c)


# --------------------------------------------------------------------------- experimentals
def tympanum():
    """Experimental: the ear. The Choir hears every mind near it and cannot
    stop; this is the ear it built to hear further. Its pipes are bent into a
    dish, each one a resonator, their mouths on a curve that focuses on one
    receiver shard held out on the keel pipe. It hears a whole system think,
    and screams it back. The dish has cracked into a hub and two wing shards
    a side, each held by one strut; the engines line its convex back."""
    c = Canvas("bc_tympanum", "Steel")
    _segs(c, {
        # the outer wing shard
        21: [(18, "#g###g####")],
        20: [(16, "E.Q..O..M...")],
        19: [(15, "E............")],
        18: [(14, "###.##########")],
        17: [(16, "#.#")],
        16: [(16, "#.#")],
        # the inner wing shard: a stepped band, its back the dish's back
        15: [(10, "#g#####.###")],
        14: [(9, "E..Q.O......")],
        13: [(8, "#.R.LL.b.c.a#")],
        12: [(7, "E.m.Q.T.A.#")],
        11: [(6, "#.O.c.M.Q.#")],
        10: [(5, "#.Q.b.y.a..")],
        9: [(4, "E...........")],
        8: [(4, "###.########")],
        7: [(6, "#.#")],
        6: [(6, "#.#")],
        # the hub, bridge in it; the keel pipe runs on to the receiver
        5: [(1, "######.#####")],
        4: [(0, "E.Q.O.M.b.a..")],
        3: [(0, "E.R.LL.Q.c..."), (24, "#=?=#/")],
        2: [(0, "E...........#"), (23, "#.A.W.#/")],
        1: [(0, "#.f.T.BB..W.." + "#" * 10), (23, ".T.J.?.#/")],
        0: [(0, "E..H..BB" + "." * 23 + "g")],
    })
    for x in (16, 20):
        c.P(x, 1, "?")
    for yc, x0, tip, weapon, gear in ((4, 13, 20, "g", 0), (9, 16, 24, "*", 2), (14, 21, 30, "g", 4),
                                      (19, 28, 38, "x", 1)):
        pipe(c, yc, x0, tip, weapon, gear)
    c.mirror()
    snap(c, gone=[(30, -15)], dead=[(19, -8)])
    return finish(c)


def shatterpoint():
    """Experimental: the note that breaks glass. The Choir sings hulls apart
    at their breaking note; this one sang its own first and never stopped.
    A diamond hull caught in the instant of shattering: six fragments burst
    out round a core (the bridge in it), the cracks between them widening
    outward, each fragment held to the core by one strut. The fragments
    flung highest each sound a pipe away from the blast."""
    c = Canvas("bc_shatterpoint", "Steel")
    paint(c, [
        # the two upper fragments, either side of a straight crack
        (15, 24, "#.#"), (15, 30, "#.#"),
        (14, 22, "#.Q.#"), (14, 30, "#.Q.#"),
        (13, 21, "#.O..#"), (13, 30, "#..O.#"),
        (12, 19, "#.a.M.y#"), (12, 30, "#y.M.a.#"),
        (11, 18, "#.Q.b.Q.#"), (11, 30, "#.Q.b.Q.#"),
        (10, 17, "g.R.LL.c.#"), (10, 30, "#.c.LL.R.g"),
        (9, 18, "#.......#"), (9, 30, "#.......#"),
        (8, 19, "#.O.m..#"), (8, 30, "#..m.O.#"),
        (7, 20, "####.#"), (7, 31, "#.####"),
        # struts across the open ring round the core
        (6, 23, "#.#"), (6, 31, "#.#"),
        # the aft fragment (engines on its broken back), the core, the bow fragment
        (7, 11, "#g#g#"), (7, 41, "#g#g#"),
        (6, 10, "#.Q.O.##"), (6, 39, "##.Q.O.#/"),
        (5, 8, "E.M.c.f.b.#"), (5, 23, "#.#######.#"), (5, 38, "#.M.a.T.y##/"),
        (4, 7, "#.R.LL.Q.y.KK#"), (4, 23, "#.Q.O.m.Q.#"), (4, 36, "#.Q.KK.R.LL.c#/"),
        (3, 5, "E.f.Q.O.a.m.T.O.#"), (3, 23, "#.R.LL.M.R#"), (3, 36, "#.a.O.m.f.A.O..#"),
        (2, 4, "#...............#"), (2, 23, "#.T.A...W.#"), (2, 36, "#.Q.y.b.Q.s.W...#"),
        (1, 3, "E.b.LL.c.O.Q.R...." + "##" + ".c.H.BB.s.." + "##" + ".KK.O.R.a.m.Q.....##/"),
        (0, 3, "E" + "." * 24 + "BB" + "." * 26 + "x"),
    ])
    vpipe(c, 25, 16, 18, "x", 2)
    vpipe(c, 31, 16, 18, "*", 5)
    c.mirror()
    snap(c, gone=[(26, -18), (49, -5)])
    return finish(c)


# --------------------------------------------------------------------------- traders
def alms():
    """Trader, small hauler. A round cargo box cracked straight across like
    a broken coin, the lid held open on one keel strut. The box carries the
    engines and the bridge; the lid carries the goods (recorded voices in
    memory cores and sealed berths, among plain crates) and two short pipes.
    Its trade is nothing anyone can name: it leaves what it carries and goes."""
    c = Canvas("bc_alms", "Steel")
    paint(c, [
        (8, 6, "#g#"), (8, 11, "##"),
        (7, 5, "#.Q#"), (7, 11, "#M#"),
        (6, 4, "#.O.#"), (6, 11, "#.k#"),
        (5, 3, "#k.M.#"), (5, 11, "#CC.#"),
        (4, 2, "#.UU..#"), (4, 11, "#CC.V#"),
        (3, 1, "E.R.k..#"), (3, 11, "#.UU..#"),
        (2, 1, "E......#"), (2, 11, "#.CC..."),
        (1, 1, "E.b..BB." + "##" + "..CC.k."),
        (0, 1, "E....BB" + "." * 9 + "g"),
    ])
    pipe(c, 2, 18, 21, "g", 0)
    c.mirror()
    snap(c, gone=[(21, -3)])
    return finish(c)


def bier():
    """Trader, medium hauler. A bier: the hold is laid out like a body, a
    coffin-shaped shard on the keel, carried between two poles (long pipes)
    that run past it fore and aft, held to them by struts. A bearer shard at
    each pole end: engines on the two behind, gatlings on the two ahead.
    What it carries is a dead civilisation's last recorded voices, sealed in
    berths among the crates; the bridge rides in the middle of the coffin."""
    c = Canvas("bc_bier", "Steel")
    pipe(c, 8, 5, 45, "g", 0)
    paint(c, [
        # the bearers at the pole ends
        (10, 0, "##g##"), (10, 40, "#g#g#"),
        (9, 0, "E.Q.."), (9, 40, "....."),
        (8, 0, "E.R.."), (8, 40, ".a..."),
        (7, 0, "E.O.."), (7, 40, "....."),
        (6, 0, "#####"), (6, 40, "#####"),
        # struts from the poles down to the coffin
        (7, 19, "."), (7, 33, "."),
        (6, 18, "#.#"), (6, 32, "#.#"),
        (5, 18, "#.#"), (5, 32, "#.#"),
        # the coffin, narrow at the foot, widest at the shoulders
        (4, 16, "###.#############.#/"),
        (3, 13, "#.V.V..CC.M.Q.CC.V.V..k.#/"),
        (2, 11, "E.k.V.V..CC.O.H.CC.V.V.UU.M.#"),
        (1, 11, "E.R.b.k.Q.UU.BB.UU.Q.k.k.c..#/"),
        (0, 11, "E............BB..............g"),
    ])
    c.mirror()
    snap(c, gone=[(38, -3)], dead=[(22, -7)])
    return finish(c)


def _reliquary(c, X, bridge=False):
    """One cargo shard of the Procession, its left end at x = X: a long
    hexagon on the keel, crates round sealed berths and a memory core."""
    paint(c, [
        (7, X + 5, "#####"),
        (6, X + 4, "#.V.V.#"),
        (5, X + 3, "#.CC.CC.#"),
        (4, X + 2, "#..CC.CC..#"),
        (3, X + 1, "#.UU.k.M.UU.#"),
        (2, X, "#.V.Q.k.O.k.Q.V#"),
        (1, X, ".k.UU.BB.UU.k.." if bridge else ".k.UU.y.UU.k.c."),
        (0, X, "......BB......." if bridge else "..............."),
    ])


def procession():
    """Trader, large hauler. A procession in single file: three reliquary
    shards on the keel, each cracked off the one ahead and held by a keel
    strut, pushed by an engine shard and led by a shard sounding three short
    pipes. Each reliquary holds one destroyed civilisation's archive; the
    bridge walks in the middle of the line."""
    c = Canvas("bc_procession", "Steel")
    paint(c, [
        # the engine shard pushing from behind
        (6, 2, "#g###g#"),
        (5, 1, "#.Q.O.Q.#"),
        (4, 0, "E.........#"),
        (3, 0, "E.R.LL.f.c.#"),
        (2, 0, "E.f.M.b.Q...#"),
        (1, 0, "E.R.LL.O.y..."),
        (0, 0, "E" + "." * 12),
        # the lead shard
        (6, 70, "#####/"),
        (5, 70, "#.Q.O.#"),
        (4, 70, "#.a.M.."),
        (3, 70, "#.k.k.#"),
        (2, 70, "#.UU..#"),
        (1, 70, "..y.Q.."),
        (0, 70, "......."),
    ])
    for X, br in ((16, False), (34, True), (52, False)):
        _reliquary(c, X, br)
    for x0 in (13, 31, 49, 67):           # keel struts across the cracks
        paint(c, [(1, x0, "###"), (0, x0, "...")])
    pipe(c, 0, 77, 84, "g", 0)
    pipe(c, 4, 77, 81, "g", 2)
    c.mirror()
    snap(c, gone=[(75, -6)], dead=[(77, -5)])
    return finish(c)


CONCEPTS = [
    ("antiphon", antiphon, "Antiphon", "Cruiser",
     "Two half-choirs answering each other across the keel: five pipes stand out to port and five to starboard, "
     "longest aft, so it sings one broadside and is answered from the other. Three keel shards cracked in chevrons."),
    ("schism", schism, "Schism", "Battleship",
     "Broken down its keel and never mended: two blades side by side with open space between them, held by struts "
     "like the rungs of a ladder. The bridge rides the one shard still spanning the crack."),
    ("cathedral", cathedral, "Cathedral", "Flagship",
     "The Precentor's own ship, which leads the Choir: a cathedral seen from above with its roof fallen in. Buttress "
     "struts hold gun piers off the open nave, the transept arms sound the great organ either side of the choir, and "
     "the bridge stands in the crossing."),
    ("tympanum", tympanum, "Tympanum", "Experimental: listening dish",
     "The ear the Choir built to hear further: its pipes bent into a dish whose mouths focus on one receiver shard "
     "held out on the keel pipe. It hears a whole system think, and screams it back."),
    ("shatterpoint", shatterpoint, "Shatterpoint", "Experimental: the shattering",
     "The note that breaks glass, sung at itself: a diamond hull caught in the instant of shattering, six "
     "fragments bursting out round a core, the cracks widening outward, each fragment held by one strut."),
    ("alms", alms, "Alms", "Trader: small hauler",
     "A round cargo box cracked across like a broken coin, the lid held open on one strut. It carries recorded "
     "voices sealed among plain crates, and leaves them where it stops: trade in nothing anyone can name."),
    ("bier", bier, "Bier", "Trader: medium hauler",
     "Its hold is laid out like a body: a coffin-shaped shard carried between two pole pipes, with bearers at the "
     "pole ends. It carries a dead civilisation's last recorded voices, sealed in berths among the crates."),
    ("procession", procession, "Procession", "Trader: large hauler",
     "Reliquary shards in single file on keel struts, pushed by an engine shard and led by three short pipes. Each "
     "reliquary holds one destroyed civilisation's archive; the bridge walks in the middle of the line."),
]
