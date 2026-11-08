"""Gilded Throne: hand-drawn, top-down only.

Lore: the ultra-rich of a thousand civilisations, who bought their way out of
three dying universes. A corporate monarchy under the Eternal CEO, a merger of
ten thousand board members. Consciousness is a commodity to them, and their
Debt Collectors repossess it. In a fight they are elite mercenaries: they flank,
they retreat at 30% hull, and they are allied with the Terran Hegemony.

Grammar, held at every size (all of it reads from above):
  - a JEWEL at the heart: a faceted, cut-gem hull with the bridge in its middle;
  - a CROWN at the bow: an odd number of spikes, the centre one longest,
    a plate on every spike tip;
  - swept CRESCENT WINGS curving back from the jewel to aft-pointing horns;
  - FILIGREE edges: long runs of single-cell diagonal steps, plates only on
    the forward-facing ones;
  - obscenely symmetric, Titanium, shields, lasers, ion and railguns.

Legend: see terran.py and draw.LEGEND.
"""
from draw import Canvas

FACTION = {
    "slug": "gilded_throne",
    "name": "Gilded Throne",
    "tagline": "The ultra-rich of a thousand civilisations, who bought their way out of three dying universes. Their Debt Collectors repossess consciousness.",
    "grammar": [
        "A jewel at the heart: a faceted, cut-gem hull with the bridge in its middle",
        "A crown at the bow: an odd number of spikes, the centre one longest, a plate on every tip",
        "Swept crescent wings curving back from the jewel to aft-pointing horns",
        "Filigree edges: long runs of single-cell diagonal steps, plates only on the forward ones",
        "Obscenely symmetric, Titanium; shields, lasers, ion and railguns",
    ],
    "accent": "#c9a227",
}


# ---------------------------------------------------------------- helpers
def _horn(c, y0, y1, root_x, tip_x, w0, w1, jewels=(), p=1.6):
    """Paint one crescent horn (upper side; mirror() makes the pair): rows from
    y0 at the root to y1 at the tip, curving aft, thinning from w0 to w1.
    Plates go on the forward steps of its leading edge. `jewels` are
    (x, y, ch) stones set into it."""
    prev = None
    for y in range(y0, y1 + 1):
        u = (y - y0) / (y1 - y0)
        cx = root_x + (tip_x - root_x) * u ** p
        w = w0 + (w1 - w0) * u
        a, b = int(round(cx - w)), int(round(cx + w))
        if prev is not None and b < prev[0] + 1:
            b = prev[0] + 1                   # keep every row joined to the last
        for x in range(a, b + 1):
            c.cells.setdefault((x, y), "#")
        prev = (a, b)
    for x, y, ch in jewels:
        c.cells[(x, y)] = ch
    _filigree(c, ys=range(y0, y1 + 2), xmax=root_x + 8)


def _filigree(c, ys, xmax=99, xmin=-99):
    """Plate every forward-facing step (an empty cell with hull to its left and
    below, nothing above or right) within the rows ys."""
    hull = {p for p, ch in c.cells.items() if ch != "/"}
    for y in ys:
        for x in range(xmin, xmax + 1):
            p = (x, y)
            if p in c.cells:
                continue
            if (x - 1, y) in hull and (x, y - 1) in hull and (x + 1, y) not in hull and (x, y + 1) not in hull:
                c.cells[p] = "/"


def courier():
    """Scout. A small cut gem: plated fore facets, engines stepped down the aft
    facets, a three-spike crown and two crescent horns."""
    c = Canvas("gt_courier")
    c.rows(-12, 7, """
####/
   ####/
      ###/ ###/
       ####.s.#/
         #Q...M####
        E...H...z
       E.b.BB.T..#z##/
      E.RM.BB...d....i
""")
    return c.mirror().build()


def dividend():
    """Corvette. The jewel grown up: a faceted diamond with a ring corridor
    running just inside its facets, the bridge at the heart, a three-spike
    crown with a railgun down the centre spike, and two crescent horns."""
    c = Canvas("gt_dividend")
    c.rows(-16, 11, """
####
  #####/
    #####/
      #####/    #/
        ##s### #s#/
          ##O.#...#/
            #...M..#/
            #..Q.Q..#z##/
           E..m...O....###
          E..R.H.T.A..z
         E..b..BB...s..#z##/
        E..M..dBB.d.......rri
""")
    return c.mirror().build()


def debt_collector():
    """Frigate. The jewel with two great crescent grapple claws round a crown
    spike. Tractor beams line the inner edge of each claw, salvage arms close
    the tips, and the claw roots hold cryo racks for repossessed minds."""
    c = Canvas("gt_debt_collector")
    # crescent horns, aft (shields set in them like stones)
    c.P(-17, 10, "####")
    c.P(-15, 9, "#####/")
    c.P(-13, 8, "##s##/")
    c.P(-11, 7, "##b##/")
    c.P(-9, 6, "##s##GG....#/")
    # the jewel and the claw roots
    c.P(-7, 5, "###GG..M..#/")
    c.P(-4, 4, "E..O.m..#/")
    c.P(-5, 3, "E..Q...Q..#/")
    c.P(-6, 2, "E..b....T...z")
    c.P(-7, 1, "E..R..BB.H....#i#z##/")
    c.P(-8, 0, "E..M..dBBd...........rr")
    c.P(-4, 7, "#V.V.#")
    c.P(-4, 8, "#V.V.$##")
    c.P(-3, 9, "#V..$$.###")
    # the claw: arc corridor, tractor beams on the inner edge
    c.P(-2, 10, "#V....VV.#t#t#t...##/")
    c.P(-1, 11, "##.............##/")
    c.P(1, 12, "###sQ.A.Qs###/")
    c.P(4, 13, "##z#z##/")
    # the claw's front, closing on the mouth: salvage arms at the tip
    c.P(13, 9, "#t...##/")
    c.P(15, 8, "t.y..#/")
    c.P(16, 7, "#t...#/")
    c.P(18, 6, "X...X")
    c.P(19, 5, "X..#")
    c.P(20, 4, "XX#")
    return c.mirror().build()


def throne_barge():
    """Flagship yacht. A great cut diamond set like a jewel in a ring: the
    throne room floats in the middle of a courtyard garden open to the stars,
    held only by the keel. A five-spike crown at the bow, engines down the aft
    facets, and two long crescent horns."""
    c = Canvas("gt_throne_barge")
    R, IN = 24, 16                  # the ring runs from |x|+|y| = 16 to 24
    # The ring, band by band, drawn along each facet from the keel (y = 0)
    # upward. fore[s][y] is the cell (s - y, y); aft[s][y] is (-(s - y), y).
    fore = {
        24: "###z##" "###" "#i##" "#" "###" "#z##z###",       # hull rim, guns
        23: "." * 24,                                           # outer corridor
        22: "." * 23,
        21: "..TqsqOmqsqAqbqOqsqyq.",                           # state rooms
        20: "..b.$.$.c.M.$.$.b.$..",
        19: "." * 20,                                           # inner corridor
        18: "." * 19,
        17: "..k.V.k.V.k.V.k.V.",                               # bonded stores
        16: "#" "l##l##l##l##l##l",                             # garden lights
    }
    aft = {
        24: "EEEEEEEE" "#z##" "#####" "#z##z##",                # engines on the facet
        23: "." * 23,
        22: "." * 22,
        21: "..RcRbRcRbOQsQmQOQsQy",
        20: "..f.f.f.f.M.V.$.V.$.",
        19: "." * 19,
        18: "." * 18,
        17: "..$.V.$.V.$.V.$.V",
        16: "#" "##l##l##l##l##l",
    }
    for s in range(IN, R + 1):
        for y, ch in enumerate(fore[s]):
            c.cells[(s - y, y)] = ch
        for y, ch in enumerate(aft[s]):
            c.cells[(-(s - y), y)] = ch
    # The keel spokes holding the throne room, through the courtyard.
    for x in list(range(-R + 1, -7)) + list(range(8, R)):
        c.cells[(x, 0)] = "."
        if abs(x) < IN:
            c.cells[(x, 1)] = "p" if x % 4 == 0 else "l" if x % 4 == 2 else "#"
    # The throne room: bridge at the heart, the Eternal CEO's merged board
    # (an AI core) above it, wellness gardens either side.
    c.rows(-7, 7, """
       #
      #s#
     #qMq#
    #.@@..#
   www...www
  #www...www#
 #.wwwBB.www.#
......BBH......
""")
    c.P(-7, 1, "#").P(7, 1, "#")     # where the keel meets the throne room
    # The crown: a centre spike carrying a railgun and an ion lance, and two
    # pairs of jewelled spikes on the bow facets.
    c.P(23, 2, "####/")
    c.P(23, 1, "....#z#z/")
    c.P(23, 0, ".......rri")
    for y, (end, tip) in ((8, (21, 24)), (7, (24, 24)), (6, (21, 24))):
        c.R(R - y + 1, end, y, y)
    c.cells[(22, 8)] = "/"
    c.cells[(22, 6)] = "/"
    c.R(9, 13, 16, 16).R(10, 16, 15, 15).R(11, 13, 14, 14)
    c.cells[(14, 16)] = "/"
    c.cells[(14, 14)] = "/"
    _horn(c, 12, 28, -10, -42, 3.0, 1.0,
          jewels=[(-13, 14, "s"), (-17, 18, "s"), (-23, 21, "s")])
    _filigree(c, ys=range(1, R + 2), xmin=1)
    return c.mirror().build()


def bullion_freighter():
    """Trade. A string of cut gems on one keel: an engine gem with the horns,
    two strongroom gems packed with reinforced vaults, and the bridge gem set
    between them so the bullion is never at either end. The forward strongroom
    wears a three-spike crown."""
    c = Canvas("gt_bullion_freighter")
    c.rows(-25, 5, """
     #
    #s#
   E.R.#
  E..c..#
 E.R.b.R..
E....M.....
""")
    c.rows(-18, 7, """
       #
      #$#
     #$.$#
    #$$.$$#
   #$$$.$$$#
  #s$$$.$$$s#
 ..$$$$.$$$$..
...............
""")
    c.rows(4, 7, """
       #
      #$#
     #$.$#z/
    #$$.$$..##
   #$$$.....#
  #$$$$.$$.z
 ..$$$$.$$$..##z#/
.................rri
""")
    c.rows(-7, 7, """
       #
      #s#
     #QMQ#
    #m.A.y#
   #s.O.O.s#
  #b..q.q..b#
 ..T..BB.H.T..
......BB.......
""")
    _horn(c, 4, 12, -19, -36, 2.0, 0.8, jewels=[(-22, 7, "s")])
    _filigree(c, ys=range(1, 9), xmin=-17)
    return c.mirror().build()


def exchange():
    """Station. A gilded sigil: an octagonal jewel of a trading floor with four
    small light-wells, four crescent harbours on the axes lined with docking ports
    round a docking hub, and four crown spikes on the diagonals carrying the
    guns it turns on you when your standing is bad enough. One eighth is
    drawn; it is reflected across the diagonal and turned four ways."""
    c = Canvas("gt_exchange")
    # One octant (0 <= y <= x). Cells above the diagonal are left blank and
    # come from the reflection.
    c.rows(0, 19, """
                   #
                  .#
                 .#/
                ..z
               ..#
              ..z
             ..#
            ..i         #z#z###/
           ..#        ##....z####
          ..z       #z...Ms#
         ..#       z.Q.A.o#
        ..#       #O...ko
       ..#       z..CC.ko
      lO.Q#      #Q.CCko
       #.Ms#    z....ko
    l #q.Qm#    #.CC..o
   .Q#qq.Oy#   z..CC.o
  ..........#p#Q..kk.o
 .$$kkOkkbR..kk.k..hhh
...................hhh
""")
    for (x, y), ch in list(c.cells.items()):
        if 0 < y < x:
            c.cells[(y, x)] = ch
    c.rot4()
    # The counting-house at the very centre: the bridge, and the helm.
    c.rows(-2, 2, """
.....
.BBB.
.BBB.
..H..
.....
""")
    return c.build()


CONCEPTS = [
    ("courier", courier, "Courier", "Scout",
     "A small cut gem with a three-spike crown and two swept horn wings. Carries offers you can't refuse."),
    ("dividend", dividend, "Dividend", "Corvette",
     "A faceted diamond with a ring corridor inside its facets and the bridge at the heart. A railgun runs down the crown's centre spike."),
    ("debt_collector", debt_collector, "Debt Collector", "Frigate",
     "Two great crescent grapple claws round a crown spike, tractor beams down their inner edges and salvage arms at the tips. It repossesses ships; the claw roots hold cryo racks for the minds."),
    ("throne_barge", throne_barge, "Throne Barge", "Flagship yacht",
     "A great cut diamond set like a jewel in a ring. The throne room floats in a courtyard garden open to the stars, held only by the keel; a five-spike crown at the bow."),
    ("bullion_freighter", bullion_freighter, "Bullion Freighter", "Trade",
     "A string of cut gems on one keel: two strongrooms packed with reinforced vaults, the bridge gem set between them so the bullion is never at either end."),
    ("exchange", exchange, "The Exchange", "Station: trade hub",
     "A gilded sigil: an octagonal trading floor with four light-wells, crescent harbours lined with docking ports on every axis, and crown spikes on the diagonals. It sells at prices set by your standing; bad enough, and the spikes open fire."),
]
