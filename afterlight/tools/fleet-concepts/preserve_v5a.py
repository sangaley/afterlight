"""Stellar Preserve, light and medium ships (v5a): hand-drawn, top-down only.

Same grammar as preserve.py (read FACTION["grammar"] there):
  - an ARK: a soft, rounded spine with the bridge in its middle;
  - SEED PODS: octagonal rooms on short stalks off the spine, a gap round each;
  - TRACTOR BOOMS: 3-wide arms at the bow, a tractor beam at every tip;
  - a couple of defensive gatlings, no more: they run, they don't fight;
  - port/starboard symmetric, Steel, plates only on the rounded prow and boom tips.

Legend: see preserve.py and draw.py. Also J signal buoy, I thermal imager,
? passive radar, S surgical bay (3x2). Weapons are the faction's own
placeholders only: g gatling, t tractor beam.
"""
from draw import Canvas
from preserve import FACTION

# Seed pods, drawn stalk-down like preserve.py's and stamped with
# c.rows(x, top, POD). The ones ending in "#.#" bring their own short stalk to
# sit on the spine's wall; the others sit on an angled stalk (_lean_stalk) or
# straight on the spine. Every door opens onto deck.
POD5_VAULT = """
 ###
#YMY#
#...#
#ZZ.#
 #d#
 #.#
"""
POD5_BUOY = """
 ###
#JMJ#
#...#
#Y.Y#
 #d#
"""
POD5_COLD = """
 ###
#VMV#
#...#
#ZZ.#
 #d#
"""
POD7_PEN = """
  ###
 #Y.Y#
#ZZ.ZZ#
#.....#
#ZZ.ZZ#
 #.O.#
  #d#
  #.#
"""
POD7_ISO = """
  ###
 #Y.Y#
#ZZ.ZZ#
#..m..#
#ZZ.ZZ#
 #...#
  #d#
  #.#
"""
POD7_SEAL = """
  ###
 #Y.Y#
#ZZ.ZZ#
#.....#
#..m..#
 #...#
  #d#
"""
POD9_PEN = """
  #####
 #.Y.Y.#
#ZZ...ZZ#
#...O...#
#ZZ...ZZ#
 #.....#
  ##d##
   #.#
"""


def _lean_stalk(c, a, y0, lean, rows=3):
    """An angled stalk: rows of 4 hull cells, the bottom one at x = a..a+3
    on row y0, each row above shifted one cell by `lean` (-1 sweeps it aft,
    +1 sweeps it forward). Four wide, so its two middle diagonals are deck."""
    for k in range(rows):
        c.R(a + lean * k, a + 3 + lean * k, y0 + k, y0 + k)


def spoor_hound():
    """Scout. A tracker: a small round body full of ears and eyes, two
    sample pods at its sides, and one long nose of a boom that plucks shed
    scales off a creature's trail. It marks the trail with signal buoys."""
    c = Canvas("sp_spoor_hound", "Steel")
    c.rows(-5, 9, POD5_VAULT)
    c.rows(-9, 3, """
    #?.I#/
  g#O..MQJA/
E#.R.BBH.Q.##########/
E..bfBB..............t
""")
    return c.mirror().build()


def ringer():
    """Scout. Rings creatures the way a bird-ringer rings birds: pins one
    between its two short prongs and fixes a signal buoy to its hide, so the
    herders can find the herd again. Buoy pods ride at its stern quarters."""
    c = Canvas("sp_ringer", "Steel")
    c.rows(-8, 8, POD5_BUOY)
    c.rows(-8, 3, """
 #.##g.........t
E..OQ.AMQ.#####/
E.R.BBH.s#/
E.b.BB..J#
""")
    c.P(-2, 4, "#########/")
    return c.mirror().build()


def snare_setter():
    """Corvette. A net-layer: two booms spread wide in a Y, a tractor beam
    at each tip, so a net can be strung across a migration lane between
    them. Containment pods at its sides; it marks each net with buoys."""
    c = Canvas("sp_snare_setter", "Steel")
    c.rows(-7, 12, POD7_PEN)
    c.rows(-10, 9, """
                       ###/
                     ##...t
                   ##...##/
                 ##...##
               ##...##
  ####.##A#J###...##
 g..............##
EcRfOQQm.MQQsJ.#/
E.b.y..BBH.Q.O.#/
E...f..BB......M#
""")
    return c.mirror().build()


def pruner():
    """Corvette. The culler, for the Preserve's "pruning of diseased
    branches": two curved booms close in front of the bow like the blades of
    a pair of shears, and what is held between them is cut out of the herd.
    Isolation pods at its sides, two surgeries amidships."""
    c = Canvas("sp_pruner", "Steel")
    c.rows(-7, 12, POD7_ISO)
    c.rows(-8, 7, """
             #####
            #.....##
           #..###...##
  ##.###g###.#   ##...#
 #...........#     ##..#/
ERf.SSSOM.Q!!#/      #..t
EKK.SSSBBHQ.s.#/      ##/
E.b....BB.....M#
""")
    return c.mirror().build()


def gleaner():
    """Corvette. A ranger that walks the far edges of a preserve for months,
    gathering stragglers and abandoned eggs. Its four cold pods are swept
    back off a slim spine like grains on an ear of barley, and its one long
    boom is the awn."""
    c = Canvas("sp_gleaner", "Steel")
    for b in (-6, 2):                      # two grains a side, swept aft
        _lean_stalk(c, b - 3, 4, -1)
        c.rows(b - 6, 11, POD5_COLD)
    c.rows(-12, 3, """
 g#....####....#/
E.R.O.Q.I.Q.A..M##/
E.bf.m.QBBH.Q..s.y#######/
E....c..BB...............t
""")
    return c.mirror().build()


def drover():
    """Frigate. Walks a herd alongside a creature transport: a herding bar
    across its bow, a tractor at each end to turn strays off the
    transport's flanks, and a lance of a boom ahead to lead. Two big pens
    hang at its sides for whatever it gathers on the way."""
    c = Canvas("sp_drover", "Steel")
    c.rows(-12, 13, POD9_PEN)
    c.rows(-17, 11, """
                        /t/
                        #.#
                        #.#
                        #.#
                        #.#
                        #.#
    #####.####/         #.#
  #gQOQ.M..KKQ##/       #.#
 E..............##/     #.#
E.Rf.LL.O.Q.GG.s.A#/    #.#
E.b.c.Q..BBHGG.Q.y.#################/
E...f..d.BB...M.....................t
""")
    return c.mirror().build()


def lazaret():
    """Frigate. The quarantine ship: four sealed pods held out in an X on
    angled stalks, as far from one another as the hull allows, so nothing
    passes between them. A sick creature waits in one until it mends, or
    until the Pruner comes."""
    c = Canvas("sp_lazaret", "Steel")
    _lean_stalk(c, -10, 5, -1)             # aft pair swept back
    _lean_stalk(c, -2, 5, +1)              # fore pair swept forward
    c.rows(-14, 14, POD7_SEAL)
    c.rows(-1, 14, POD7_SEAL)
    c.rows(-15, 4, """
     #...#g##...#/
  ###............##/
 E.R.O.Q.m.M.!!.Q.A##/
E.b.m.KKy.BBH.s.Q.m..########/
E...f..d..BB.....M...........t
""")
    return c.mirror().build()


def verderer():
    """Destroyer. The poacher-hunter, named for the officers of the old royal
    forests who kept the deer and judged the poachers: it doesn't shoot
    poachers, it catches them. A long
    neck ends in a fork lined with tractor beams that closes on a hull and
    holds it; the poachers go into the pens like any other specimen."""
    c = Canvas("sp_verderer", "Steel")
    for x0 in (-18, -8):
        c.rows(x0, 14, POD7_PEN)
    c.rows(-21, 7, """
                                   #########/
    ##.###g#####.##/              #.........t
  ##...............##/           #....#t##t#/
 E.R.QQO.KK.M.KK.Q.m.A#/        #....#
E......d.......d.......#/      #....#
E.cLL.b.SSS.O..GG.Q.s.Qy#/    #....#
E.b.R.c.SSS.BBHGG.Q......#####....#
E...f...d...BB......M.............
""")
    return c.mirror().build()


CONCEPTS = [
    ("spoor_hound", spoor_hound, "Spoor Hound", "Scout",
     "A tracker: a small round body of ears and eyes, two sample pods at its sides and one long nose of a boom that plucks shed scales off a creature's trail."),
    ("ringer", ringer, "Ringer", "Scout",
     "Rings creatures the way a bird-ringer rings birds: pins one between two short prongs and fixes a signal buoy to its hide."),
    ("snare_setter", snare_setter, "Snare-setter", "Corvette",
     "A net-layer: two booms spread in a Y so a tractor net can be strung across a migration lane between their tips."),
    ("pruner", pruner, "Pruner", "Corvette",
     "The culler: two curved booms close in front of the bow like shears, and what is held between them is cut out of the herd."),
    ("gleaner", gleaner, "Gleaner", "Corvette",
     "A ranger gathering stragglers on a preserve's far edges: four cold pods swept back off a slim spine like grains on an ear of barley."),
    ("drover", drover, "Drover", "Frigate",
     "Walks a herd beside a creature transport: a herding bar across the bow with a tractor at each end, a lance of a boom ahead, two big pens at its sides."),
    ("lazaret", lazaret, "Lazaret", "Frigate",
     "The quarantine ship: four sealed pods held apart in an X on angled stalks, so nothing passes between them."),
    ("verderer", verderer, "Verderer", "Destroyer",
     "The poacher-hunter: it doesn't shoot poachers, it catches them, in a fork lined with tractor beams at the end of a long neck."),
]
