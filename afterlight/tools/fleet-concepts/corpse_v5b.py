"""Corpse Stars, round 5b: eight new hulls in the approved grammar.

Spine down the centreline, ribs that sweep aft with space between them, a
plated ram forward, bones only. Drawn y >= 0 and mirrored; any asymmetry is a
small deliberate detail drawn after the mirror.
"""
import math

from corpse import FACTION, finish, _segs
from draw import Canvas

FACTION = FACTION


def ferryman():
    """Small trader. A ferry for the dead: a short spine, two pairs of ribs
    that each hold a rack of coffins at the tip, and a lantern on the prow."""
    c = Canvas("cs_ferryman", "AbyssalAlloy")
    c.rows(0, 7, """
        #VV#     #VV#
        #.VV#    #.VV#
         #...#    #...#
  ##      #.#      #.#     ###/
 E..#     #..#     #..#    #M.##/
E.R.f######.###i####.###i###.s..##/
E..c.b.O.....m.Q...BB..H.a.k.k.....##/
E..................BB..................l
""")
    c.mirror()
    c.P(5, 1, "M")                     # the third core, port side only
    return finish(c)


def tithe():
    """Medium trader. The tithe of Abyssal Alloy and relics: three pairs of
    ribs, each ending in a knuckle of bone that is a cargo hold, so the ship
    reads as a spine hung with long bones. The relic vault rides the spine."""
    c = Canvas("cs_tithe", "AbyssalAlloy")
    for r in (13, 23, 33):
        c.rows(r - 4, 10, """
 ## ##
#CC.CC#
#CC.CC#
 #...#
 #..#
  #.#
  #..#
   #.#
""")
        c.P(r - 3, 3, "###.##")       # the knuckle where the rib meets the spine
    c.rows(0, 4, """
 ##                                          ###/
E..#                                         #M.##/
E.R.f#######.######i##.#########.##########..s.e.##/
E..c.b.O.Q.Q..$.$.b.....m.BB.H.....V.$.$.O.a.......##/
E.........................BB..........................#
""")
    c.mirror()
    c.P(18, 1, "M")                    # the third core, port side only
    return finish(c)


def _grave_cross(c, r, y0):
    """A rib that is a grave cross: a swept stem lined with coffin niches on
    both sides of a walkway, a crossbar of niches, and a short head. r is
    where the stem's walkway meets the spine, y0 the first row off it."""
    c.rows(r - 7, y0 + 12, """
   #####
   #V.V#
   #V.V#
####...####
#V.......V#
####...####
   #V.V#
   #V.V#
   #V..V#
    #V.V#
    #V.V#
    #V..V#
     #V.V#
""")


def catacomb():
    """Large trader. A funeral barge of the faithful dead: its ribs are grave
    crosses, every one lined with coffin niches like a catacomb gallery.
    Abyssal Alloy rides in the spine."""
    c = Canvas("cs_catacomb", "AbyssalAlloy")
    for r in (17, 33, 49):
        _grave_cross(c, r, 3)
    c.rows(0, 4, """
 ###                                                         ###/
E..f#                                                        #M.##/
E.R.c#############.###############.###############.##########..e.##/
E..b.O.UU.UU.Q.Q...m.UU.UU.$.$...BB.H.$.$.UU.UU.O....UU.Q.Q.......##/
E................................BB.................................l
""")
    for r in (17, 33, 49):
        c.P(r, 2, ".")
    c.P(35, 11, ".i")                  # an ion on the forward arm of the middle cross
    c.mirror()
    c.P(18, 1, "M")                    # the third core, port side only
    return finish(c)


def _vertebra(c, v, rooms, tip="x"):
    """One vertebra of the Cortege: a swollen lens-shaped centrum round the
    spine with a short transverse process swept aft to a gun. v is the
    centrum's middle; rooms fills the eight room cells of its upper half."""
    a, b, e, d, f, g, h, k = rooms
    c.rows(v - 10, 10, f"""
    #{tip}#
    #.#
    #..#
     #.#
     #..#
      #..#####
     #..{a}.{b}.{e}.##
    #.{d}..{f}..{g}..##
   #.{h}.........{k}.#
""")


def cortege():
    """Cruiser. A funeral procession in bone: the spine is a chain of four
    swollen vertebrae pinched apart by bare links, each throwing a short
    process swept aft to a gun. Railguns in the ram."""
    c = Canvas("cs_cortege", "AbyssalAlloy")
    for v, rooms, tip in ((15, "QM.O.b.c", "x"), (33, "Q.Qa.T.s", "e"),
                          (51, "QMQ.Om.a", "e"), (69, "s.a.b.cM", "x")):
        _vertebra(c, v, rooms, tip)
    # the pelvis: engines, reactors, and the stern of the spinal cord
    c.rows(0, 4, """
 ###
E..f#
E.R.c###
E..b.O.R
E.......
""")
    for x0, x1 in ((23, 25), (41, 43), (59, 61), (77, 78)):
        c.R(x0, x1, 2, 2)                # the bare links between vertebrae
    row1 = list("." * 71)                # x = 8..78, the spinal cord's side rooms
    for x, m in ((11, "Q"), (13, "Q"), (16, "R"), (18, "R"), (21, "LL"), (29, "m"), (31, "b"),
                 (34, "Q"), (36, "Q"), (39, "y"), (46, "LL"), (50, "BB"), (53, "H"), (64, "R"),
                 (66, "R"), (70, "O"), (72, "a")):
        row1[x - 8:x - 8 + len(m)] = m
    c.P(8, 1, "".join(row1))
    c.P(8, 0, "." * 71)
    c.P(50, 0, "BB")
    for x in (24, 42, 60, 77):
        c.P(x, 0, "d")
    # the ram, plated on every step
    c.rows(79, 6, """
###/
#.M##/
#.a.s##/
#..T.e.##/
..O..b..i##/
.....rr.......##/
.....rr.........##
""")
    return finish(c.mirror())


def requiem():
    """Battleship. Two great thighbones crossed over the spine, memento mori
    from above: the forward ends are split knees carrying missiles and
    railguns, the aft ends are ball joints carrying engines. The bridge is
    where the bones cross; the ram runs on ahead of them."""
    c = Canvas("cs_requiem", "AbyssalAlloy")
    X0 = 34
    # the shafts, one row at a time: 45 degrees, a walkway on the inner face
    aft = ["..", ".Q", ".O", "..", ".b", "..", ".M", "..", ".c", "..", ".a", "..", ".."]
    fwd = ["..", "..", "a.", "..", "T.", "..", "b.", "..", "M.", "..", "a.", "..", ".."]
    for k, y in enumerate(range(4, 17)):
        c.P(X0 - k - 3, y, "#" + aft[k] + "..#")
        c.P(X0 + k - 2, y, "#.." + fwd[k] + "#")
    # the knees: two lobes, missiles on the lobes, railguns and an ion
    c.rows(44, 22, """
 ##x#  #x##
#....##....#
#.M.a..rr..i
#...T..b..#
#........##
#..s..###
""")
    # the ball joints: round heads on short necks, engines on their aft faces
    c.rows(12, 23, """
  ####
 E...M#
E..R.c.#
E.f...b.#
E..R.a...#
 E....Q...#
  ###......#
""")
    # the knuckle where the bones cross the spine
    c.P(30, 4, "##.....###")
    c.P(27, 3, "#####.....#####")
    # the spine
    c.rows(13, 3, """
 ###
E..f###################.....###############################
E.R.c.LL.R.R..m.a......Q.Q...BB..H...a.R.R.b.c..Q.Q..O.m.
E.......d...............d....BB..............d...........
""")
    # the ram, plated on every step
    c.rows(70, 5, """
###/
#M.##/
#..e.##/
..a.T...##/
KK.s.rr.i...##/
......rr.......##
""")
    return finish(c.mirror())


def see():
    """Flagship. The seat of a dead pontiff who still holds office: a ringed
    cross inside a pointed halo (a mandorla) of bone, its four arms running
    out past the halo. The pontiff lies in state where the arms cross, beside
    the bridge; dead cardinals keep their stalls along the crossbar. Voids
    everywhere else: you see space through the halo."""
    c = Canvas("cs_see", "AbyssalAlloy")
    CX, HL, HW, T = 5, 46, 22, 3.4           # halo centre, half-length, half-width, bone
    R = (HL * HL + HW * HW) / (2 * HW)
    D = R - HW

    def halo(x, y, shrink=0.0):
        x -= CX
        return math.hypot(x, y - D) <= R - shrink and math.hypot(x, y + D) <= R - shrink

    for x in range(CX - HL - 1, CX + HL + 2):
        for y in range(0, HW):
            if halo(x, y) and not halo(x, y, T):
                c.cells[(x, y)] = "#"
    X = 12                                   # where the arms cross
    for x in range(X - 12, X + 13):          # the ring of the cross (its nimbus)
        for y in range(0, 13):
            if 8.6 <= math.hypot(x - X, y) <= 11.2:
                c.cells[(x, y)] = "#"
    c.R(-44, 56, 0, 2)                       # the shaft of the cross: the spine
    c.R(X - 2, X + 2, 0, 27)                 # the arm
    # knuckles on the outer face of the halo, a gun on each, deck behind it
    top = lambda x: max(y for (xx, y) in c.cells if xx == x)
    for x0, ch in ((-31, "x"), (-22, "e"), (-13, "i"), (-4, "x"), (24, "i"), (33, "e"), (42, "x")):
        t = {dx: top(x0 + dx) for dx in range(-2, 3)}
        for dx in range(-2, 3):
            c.cells[(x0 + dx, t[dx] + 1)] = "#"
        for dx in range(-1, 2):
            c.cells[(x0 + dx, t[dx] + 2)] = "#"
        c.cells[(x0, t[0] + 2)] = ch
        for k in range(-1, 2):
            c.cells[(x0, t[0] + k)] = "."
    # the shaft: stores and quarters aft, the pontiff's chapel, guns forward
    c.P(-44, 1, "..R.c.Q.Q.O.m...b.a....Q..Q.Q...y.LL.O.a...b.")
    c.P(-44, 0, "." * 101)
    for x in (-36, -16, 30, 46):
        c.P(x, 0, "d")
    c.P(18, 1, ".a.T..Q.Q.O..b.s.....a..T..c.a.c..")
    c.P(-44, 2, "#" * 9)                    # the stern end of the shaft is plain bone
    # the chapel where the arms cross: the pontiff in state beside the bridge
    c.rows(X - 5, 5, """
 #########
#V.q...q.V#
#.........#
#..$.V.$..#
..q.BB.H.q.
....BB.....
""")
    # the arm: dead cardinals in their stalls, and the arm's end flared like a
    # cross pattee, guns on it
    for y in range(6, 27):
        c.P(X - 2, y, "#V.q#" if y % 3 == 0 else "#...#")
    c.P(X - 1, 16, "M")
    c.rows(X - 5, 30, """
  #e#i#e#
 #.......#
 ##V.V.V##
  #.....#
""")
    # the pelvis: the shaft runs out past the halo's stern point to the engines
    c.rows(-56, 5, """
  ####
 N....###
 N.LL.f.c####
N..R..M..b..##
N.............#
E.............##
""")
    # the head: the shaft runs out past the halo's bow point into the ram
    c.rows(54, 4, """
#####/
#.M..###/
#..a.e...###/
..i.rr.T...i.###/
.....rr...........##
""")
    return finish(c.mirror())


def _bone(c, p0, p1, w, ch="#"):
    """A straight bone: every cell within w of the segment p0-p1."""
    (x0, y0), (x1, y1) = p0, p1
    L2 = (x1 - x0) ** 2 + (y1 - y0) ** 2
    for x in range(min(x0, x1) - 3, max(x0, x1) + 4):
        for y in range(min(y0, y1) - 3, max(y0, y1) + 4):
            t = max(0, min(1, ((x - x0) * (x1 - x0) + (y - y0) * (y1 - y0)) / L2))
            if math.hypot(x - x0 - t * (x1 - x0), y - y0 - t * (y1 - y0)) <= w:
                c.cells[(x, y)] = ch


def _wall(c, p0, p1):
    """A 3-thick bone you can walk along: a deck down its middle that stays
    joined edge to edge wherever the bone steps up or down a row."""
    (x0, y0), (x1, y1) = p0, p1
    prev = None
    for x in range(x0, x1 + 1):
        y = round(y0 + (y1 - y0) * (x - x0) / (x1 - x0))
        for k in (-1, 0, 1):
            c.cells.setdefault((x, y + k), "#")
        c.cells[(x, y)] = "."
        if prev is not None and prev != y:
            for k in (-1, 0, 1):
                c.cells.setdefault((x, prev + k), "#")
            c.cells[(x, prev)] = "."
        prev = y


def hourglass():
    """Experimental. The cult's clock for the end of everything: an hourglass
    of bone lying on its side, two triangular bulbs meeting at a narrow
    waist where the bridge sits. The forward bulb has run empty; the last of
    the sand (its batteries and fuel) is heaped against the stern."""
    c = Canvas("cs_hourglass", "AbyssalAlloy")
    W = 30                                    # the waist
    _bone(c, (5, 15), (W - 2, 3), 1.3)        # aft bulb
    _bone(c, (W + 2, 3), (W + 25, 15), 1.3)   # forward bulb
    c.R(0, W + 28, 0, 2)                      # the neck, through which the sand ran
    # the end plates, each bowed outward: engines across the stern one's aft
    # face, guns across the bow one's forward face
    stern = "...b...R...M...c."
    bow = "#e##e##i##e##e###"
    for y in range(0, 16):
        k = round(3 * (y / 15) ** 2)
        c.P(k, y, "E." + stern[y] + ".#")
        g = bow[y]
        c.P(55 - k, y, "#.." + g if g != "#" else "#..#")
    c.P(4, 16, "###")
    c.P(52, 16, "####")
    # the sand: heaped against the stern end, batteries and fuel
    c.rows(5, 8, """
#
##
#b#
#.b##
#f..b##
#.b.f..##
#.f..b..b##
""")
    # the spine through the neck, the bridge at the waist
    c.P(0, 1, "E...b.....O.Q.a.m....Q...BB.H...a.T.Q.O.....s........")
    c.P(0, 0, "E.........d..............BB.................d.........")
    # a short heavy ram off the bow plate
    c.rows(59, 4, """
##/
#M.##/
..a.e.##/
.T.rr.i..##/
.....rr.......##
""")
    c.mirror()
    c.P(19, 1, "M")                    # the third core, port side only
    return finish(c)


def sarcophagus():
    """Experimental. An open coffin with its lord still in it: a coffin of
    bone, narrow at the feet and wide at the shoulders, and inside it a whole
    skeleton laid out in state, skull, ribcage, pelvis and legs, with space
    all round it. The coffin carries the guns; the dead lord commands from
    the ribcage. Break the coffin and what is inside is still there."""
    c = Canvas("cs_sarcophagus", "AbyssalAlloy")
    # the coffin: foot, flank to the shoulder, flank to the head
    _wall(c, (1, 8), (40, 16))
    _wall(c, (41, 16), (56, 9))
    for y in range(0, 9):                     # the foot: engines across it
        c.P(0, y, "E.#" if y % 2 == 0 else "E..")
    c.P(0, 9, " ###")
    for y in range(0, 10):                    # the head of the coffin
        c.P(55, y, "#.#")
    # the legs, from the feet to the hips, a knuckle at each knee
    c.rows(3, 6, """
     ###
##########
....d.....
##########
     ###
""")
    # the pelvis: two wings of bone, each round a hole
    c.rows(13, 8, """
  ####
 #....#
#..  ..#
#.#  #.#
...##..#
#.R..b.#
#.......
#..O....
#.......
""")
    # the spine and the ribcage, the bridge at its heart
    c.rows(20, 2, """
######################
.Q.a.c.m..b..BB.H.s.Q.
.d...........BB.....d.
""")
    for r, tip in ((26, "e"), (32, "i"), (38, "e")):
        c.rows(r - 3, 9, f"""
 {tip}#
#.#
#..#
 #.#
 #..#
  #.#
  #..#
""")
    # the skull, face up, crown toward the head of the coffin
    c.rows(42, 6, """
    ######
  ##.M....##
 #..........#
#.....  ....#
#.....  ....#
#............
#.  .........
""")
    c.rows(58, 4, """
###/
#M.##/
..a.e.##/
.T.rr.i..##/
.....rr.......##
""")
    # knuckles on the outer face of the coffin, a gun on each, deck behind it
    top = lambda x: max(y for (xx, y) in c.cells if xx == x)
    for x0, ch in ((12, "x"), (24, "i"), (36, "x"), (47, "i")):
        t = top(x0)
        for x in (x0 - 1, x0 + 1):
            c.cells.setdefault((x, t), "#")
        c.P(x0 - 1, t + 1, "#" + ch + "#")
        c.cells[(x0, t)] = "."
    return finish(c.mirror())


CONCEPTS = [
    ("cortege", cortege, "Cortege", "Cruiser",
     "A funeral procession in bone: the spine is a chain of swollen vertebrae pinched apart by bare links, each throwing a short process aft to a gun."),
    ("requiem", requiem, "Requiem", "Battleship",
     "Two great thighbones crossed over the spine: split knees forward carrying missiles and railguns, ball joints aft carrying engines, the bridge where they cross."),
    ("see", see, "The Holy See", "Flagship",
     "The seat of a dead pontiff still in office: a ringed cross inside a pointed halo of bone, its arms running out past the halo, the pontiff lying in state beside the bridge."),
    ("hourglass", hourglass, "Hourglass", "Experimental: entropy clock",
     "An hourglass of bone on its side: two triangular bulbs meet at a waist where the bridge sits. The forward bulb has run empty; the last sand is heaped against the stern."),
    ("sarcophagus", sarcophagus, "Sarcophagus", "Experimental: open coffin",
     "An open coffin of bone with its dead lord still laid out inside, skull, ribs, pelvis and legs, space all round him. The coffin carries the guns; he commands from the ribcage."),
    ("ferryman", ferryman, "Ferryman", "Trader: small",
     "A ferry for the dead: two pairs of short ribs each cradling a rack of coffins, and a lantern on the prow."),
    ("tithe", tithe, "Tithe", "Trader: medium",
     "Carries the tithe of Abyssal Alloy and relics: three pairs of ribs each ending in a knuckle of bone that is a cargo hold."),
    ("catacomb", catacomb, "Catacomb", "Trader: large",
     "A funeral barge for the faithful dead: its ribs are grave crosses, each one lined with coffin niches like a catacomb gallery."),
]
