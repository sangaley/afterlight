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


def memento():
    """Battleship. Memento mori: a skull seen from above, face forward. A
    broad round cranium of bone, two eye sockets and a nose hole open right
    through the ship, cheekbones flaring out into gun knuckles, and a row of
    teeth across the front, each tooth a gun. The engines sit across the back
    of the cranium; the bridge is between the eyes."""
    c = Canvas("cs_memento", "AbyssalAlloy")
    # x = -21 (the crown) to 18 (the teeth), y = 21 down to the centreline
    c.rows(-21, 21, """
                         ##rr#
                        #.a...#
                        #.....#
               #..i...# #..a..x
            ##Q.Q...s.........#
          ##.........M....O.T.#
        #.....Q.Q.............#
       #........................#
      #.R...M...........M....a...###a###
     e...........#      #..a...c.......x
    #...b.R.....#        #.Q.......#####
   #............#        #........#
   #.Q...KK.m..M#        #.T.......#####
  #.b.c.LL.R...s#        #.............i
  E.R.f..R..b...#        #.......b.#####
 E.......Q......#        #.......c#
 E..............#        #.........#####
 E.c.R.....M.O...#      #.............rr
E.f.LL.m..Q.b..s..######...........#####
E.R.....O.....................  #.#
E..b.Q.....Q........BBH.T..#    #.a#####
E........d.......d..BB.....     #.....rr
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
    """Experimental. The dead lord lying in state, inside his mausoleum: a
    square-ish cross-shaped tomb seen from above. A round dome sits over the
    lord at the centre, its drum pierced by a ring of windows, with the
    bridge beside his bier. Four short wings run out from it, each lined on
    both edges by a colonnade of bone and each ending in a small domed
    chapel on a narrow neck; open courts lie between the wings. The chapels
    carry the guns; the rear chapel carries the engines on its back face."""
    c = Canvas("cs_sarcophagus", "AbyssalAlloy")
    for x in range(-28, 29):
        for y in range(0, 29):
            ax, r = abs(x), math.hypot(x, y)
            if r <= 12.4:                          # the dome
                a = math.degrees(math.atan2(y, ax)) % 45
                if not (7.6 < r <= 8.6 and min(a, 45 - a) > 9):   # drum windows
                    c.cells[(x, y)] = "#"
                continue
            for u, v in ((ax, y), (y, ax)):        # u along a wing, v across it
                if ((u <= 16 and v <= 4)           # the wing
                        or (u <= 18 and v <= 2)    # the chapel's neck
                        or (12 <= u <= 16 and 7 <= v <= 8)        # architrave
                        or (v in (5, 6) and u in (12, 14, 16))    # columns
                        or math.hypot(u - 22.5, v) <= 5.4):      # the chapel
                    c.cells[(x, y)] = "#"
    # the rear chapel: engines on its back face, reactors behind them
    _segs(c, {
        5: [(-24, "E##e")],
        4: [(-26, "N...R.f#")],
        3: [(-26, "N.LL.c.#")],
        2: [(-27, "N.b.O.b.")],
        1: [(-27, "N..c.R.m.")],
        0: [(-27, "E" + "." * 52 + "rr")],
    })
    # the rear wing: the dead lord's household
    _segs(c, {3: [(-16, "Q...Q")], 1: [(-16, "....m")]})
    # the dome: the lord on his bier (cryo) at the centre, grave goods and
    # his cardinals round him, the bridge at his head
    _segs(c, {
        7: [(-2, ".$.$.")],
        6: [(1, "T")],
        5: [(-3, "M.O.s.M")],
        3: [(-6, "q...........q")],
        1: [(-6, "q.....$..BBH")],
        0: [(-1, "VVV.BB")],
    })
    # the dome's outer ring, beyond the drum: guns into the courts
    _segs(c, {
        10: [(-5, "c")],
        9: [(-8, "i"), (8, "e")],
        4: [(10, "a")],
        2: [(-10, "b")],
    })
    # the side wing and its chapel: quarters, stores, guns
    _segs(c, {
        27: [(-2, "##x##")],
        26: [(-2, ".a...")],
        23: [(-5, "i..Q.Q....i")],
        22: [(-4, ".........")],
        21: [(-4, "M........e")],
        20: [(-3, ".R.....")],
        15: [(0, "M")],
        14: [(-3, "c.....b")],
        13: [(4, "O")],
    })
    # the front wing and chapel: targeting, ammunition, railguns in the apse
    _segs(c, {
        5: [(23, "x")],
        4: [(19, "..T...a")],
        3: [(13, "T.M"), (24, "m.i")],
        2: [(19, "..s.a..rr")],
        1: [(13, "b.c"), (25, "a")],
    })
    for x in (-17, 17):
        c.cells[(x, 0)] = "d"                       # bulkheads in the necks
    return finish(c.mirror())


CONCEPTS = [
    ("cortege", cortege, "Cortege", "Cruiser",
     "A funeral procession in bone: the spine is a chain of swollen vertebrae pinched apart by bare links, each throwing a short process aft to a gun."),
    ("memento", memento, "Memento", "Battleship",
     "Memento mori: a skull seen from above, as wide as it is long. Eye sockets and a nose hole open right through it, cheekbones flare into gun knuckles, a row of teeth across the front carries the guns, engines across the back of the cranium, the bridge between the eyes."),
    ("see", see, "The Holy See", "Flagship",
     "The seat of a dead pontiff still in office: a ringed cross inside a pointed halo of bone, its arms running out past the halo, the pontiff lying in state beside the bridge."),
    ("hourglass", hourglass, "Hourglass", "Experimental: entropy clock",
     "An hourglass of bone on its side: two triangular bulbs meet at a waist where the bridge sits. The forward bulb has run empty; the last sand is heaped against the stern."),
    ("sarcophagus", sarcophagus, "Sarcophagus", "Experimental: mausoleum",
     "The dead lord's mausoleum, seen from above: a cross-shaped tomb with a round windowed dome over his bier, the bridge beside him. Four short wings lined with bone colonnades, open courts between them, each wing ending in a small domed chapel that carries guns; engines on the rear chapel."),
    ("ferryman", ferryman, "Ferryman", "Trader: small",
     "A ferry for the dead: two pairs of short ribs each cradling a rack of coffins, and a lantern on the prow."),
    ("tithe", tithe, "Tithe", "Trader: medium",
     "Carries the tithe of Abyssal Alloy and relics: three pairs of ribs each ending in a knuckle of bone that is a cargo hold."),
    ("catacomb", catacomb, "Catacomb", "Trader: large",
     "A funeral barge for the faithful dead: its ribs are grave crosses, each one lined with coffin niches like a catacomb gallery."),
]
