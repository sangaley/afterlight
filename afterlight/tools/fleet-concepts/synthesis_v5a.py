"""Synthesis Collective, round 5a: light and medium warships, hand-drawn.

Grown, not built: every hull is a mirror-symmetric body (drawn on y >= 0,
then mirrored), with the faction's wrongness kept to small grafts added after
the mirror (a tumour, an extra growth). Tendrils three cells thick, plasma
and plates on their tips, repair bays along every vein.

Legend: draw.py. P plasma, i ion, t tractor beam, X salvage arm, y repair
bay, % advanced repair bay, Z creature containment, Y specimen vault,
@ AI combat core, J signal buoy, I thermal imager, ? passive radar,
D drone bay, = signal jammer.
"""
from draw import Canvas
from synthesis import FACTION

assert FACTION["slug"] == "synthesis_collective"


def grow(name, x0, y_top, block):
    c = Canvas(name, "Composite")
    c.rows(x0, y_top, block)
    return c


def vein(c, *pts):
    """A tendril three cells thick along a staircase through pts: a vein of
    deck with rim grown round it. Rim never overwrites what is already drawn."""
    path = [pts[0]]
    for (x1, y1) in pts[1:]:
        x, y = path[-1]
        while (x, y) != (x1, y1):
            dx, dy = x1 - x, y1 - y
            if abs(dx) >= abs(dy):
                x += (dx > 0) - (dx < 0)
            else:
                y += (dy > 0) - (dy < 0)
            path.append((x, y))
    for (x, y) in path:
        for ex in (-1, 0, 1):
            for ey in (-1, 0, 1):
                c.cells.setdefault((x + ex, y + ey), "#")
    for p in path:
        if c.cells.get(p, "#") == "#":
            c.cells[p] = "."
    return path


def tip(c, x, y, gun="P", plates=True):
    """A gun on a tendril tip facing bow, with a plate either side."""
    c.cells[(x, y)] = gun
    if plates:
        for k in (1, -1):
            c.cells[(x, y + k)] = "/"


def tipn(c, x, y, gun="i"):
    """A gun on a tendril tip pointing outboard (+y), a plate either side."""
    c.cells[(x, y)] = gun
    c.cells[(x - 1, y)] = "/"
    c.cells[(x + 1, y)] = "/"


def walkable(c, p):
    ch = c.cells.get(p)
    if ch is None or ch == "/":
        return False
    if ch in "#.":
        return all((p[0] + dx, p[1] + dy) in c.cells and c.cells[(p[0] + dx, p[1] + dy)] != "/"
                   for dx, dy in ((0, 1), (0, -1), (1, 0), (-1, 0)))
    return ch not in "MbfcvsATWkCUaGJ?=I~$Yd"   # crew posts and doors


def stud(c, faces, pattern):
    """Line the rim cells that look into the open cells `faces` (a test on a
    coordinate) with `pattern`, repeated in order along x then y. Only rim
    cells with walkable deck behind them are used. y >= 0 only."""
    rim = []
    for (x, y), ch in sorted(c.cells.items()):
        if ch != "#" or y < 0:
            continue
        n4 = [(x + dx, y + dy) for dx, dy in ((0, 1), (0, -1), (1, 0), (-1, 0))]
        if any(n not in c.cells and faces(*n) for n in n4) and any(walkable(c, n) for n in n4):
            rim.append((x, y))
    for k, p in enumerate(rim):
        ch = pattern[k % len(pattern)]
        if ch != " ":
            c.cells[p] = ch
    return rim


def lamprey():
    """Corvette, latcher. An eel with a round sucking mouth for a bow: two
    lips round an open cup, every face inside it a tractor beam or a salvage
    arm, plasma teeth on the lip tips. Gill pores notched down both flanks;
    one lip carries a growth that doesn't match the other."""
    c = grow("sy_lamprey", -21, 4, """
        #i #i# #i#
    ####Q.My.O#y.Q...
  ##Rb.ZZ.y.SSS.yO..X
E.fc..d.m...SSS.yBB.t
E.R....M....@@..HBB.X
""")
    vein(c, (-2, 4), (-1, 4), (-1, 5), (1, 5), (1, 6), (6, 6), (6, 5), (8, 5), (8, 4), (9, 4))
    tip(c, 10, 4)
    c.cells[(3, 6)] = "y"
    stud(c, lambda x, y: 0 <= x <= 9 and y <= 4, "t#X#")
    c.mirror()
    c.R(2, 4, -8, -8).R(3, 3, -9, -9)          # the growth on the port lip
    return c.build()


def stalker():
    """Corvette, hunter. A mantis: a segmented abdomen joined by graft seams,
    a thorax with the bridge, a triangular head whose eyes are thermal
    imagers, and two raptorial arms held forward with plasma on the tips and
    a hooked spur folded under each."""
    c = Canvas("sy_stalker", "Composite")
    # abdomen: three segments, each seam a narrow neck with a bulkhead in it
    c.P(-15, 3, "####").P(-8, 3, "#i#")
    c.P(-21, 2, "####").P(-16, 2, "#Q.yO#").P(-9, 2, "#y.O#")
    c.P(-22, 1, "E.R.b#..ZZ..#.R.b.")
    c.P(-22, 0, "E....d..@@..d.....")
    # thorax and head
    c.P(-3, 4, "######")
    c.P(-4, 3, "#y.M.Qy#").P(6, 3, "II")
    c.P(-4, 2, ".ZZ.yO..##..#")
    c.P(-4, 1, ".m.BB......T.#/")
    c.P(-4, 0, "...BB.H..M....P")
    # raptorial arms
    vein(c, (0, 3), (0, 5), (2, 5), (2, 6), (5, 6), (5, 7), (10, 7))
    tip(c, 11, 7)
    vein(c, (9, 7), (9, 5))
    c.P(8, 4, "/X/")
    stud(c, lambda x, y: y >= 8, "#i#")
    c.mirror()
    c.R(-14, -13, -4, -4)                    # a growth on one side of the abdomen
    return c.build()


def spore():
    """Scout, seeder. A seed pod with two wings swept back like a maple
    key's, their edges studded with signal buoys it sheds as it drifts:
    wherever a Spore has been, the Collective can hear."""
    c = Canvas("sy_spore", "Composite")
    c.P(-2, 3, "#####")
    c.P(-4, 2, "#My.OJQ##")
    c.P(-6, 1, "E#R.b.BB...##/")
    c.P(-6, 0, "E.d..HBB.M...P")
    vein(c, (-1, 2), (-1, 5), (-2, 5), (-2, 6), (-3, 6), (-3, 7), (-4, 7), (-4, 8), (-5, 8))
    c.cells[(-6, 8)] = "J"
    stud(c, lambda x, y: x < -2 and y > 5, "J#")
    c.mirror()
    return c.build()


def mite():
    """Scout, swarm picket. A tick: a round flat body on six short legs,
    plasma on the forelegs, ions on the middle pair, an engine at the end of
    each hind leg. They come in dozens and all report to the same mind."""
    c = Canvas("sy_mite", "Composite")
    c.P(-3, 3, "######")
    c.P(-4, 2, "#yM.O.Q##")
    c.P(-5, 1, "#R.b.BB...#/")
    c.P(-5, 0, "#.d.HBB.M..P")
    vein(c, (2, 2), (4, 2), (4, 4), (6, 4))                 # foreleg
    tip(c, 7, 4)
    vein(c, (-1, 2), (-1, 5))                                # middle leg
    c.P(-1, 6, "i").P(-2, 6, "/").P(0, 6, "/")
    vein(c, (-3, 2), (-3, 3), (-5, 3), (-5, 4), (-7, 4))    # hind leg
    c.cells[(-8, 4)] = "E"
    c.mirror()
    return c.build()


def howl():
    """Corvette, jammer. A small body with its jaws wide open and a frill of
    six spines fanned out behind the head, every spine lined with signal
    jammers and tipped with an ion disruptor. It screams on every channel the
    enemy owns. The Collective calls it data corruption."""
    c = Canvas("sy_howl", "Composite")
    c.P(-7, 3, "#######")
    c.P(-9, 2, "##y.M.=.Q##")
    c.P(-12, 1, "E##R.b.=.BB..O..i")
    c.P(-12, 0, "E...d...HBB.M...i")
    vein(c, (1, 2), (1, 3), (7, 3))                          # upper jaw
    tip(c, 8, 3)
    stud(c, lambda x, y: x >= 5 and y <= 1, "i#")
    vein(c, (2, 3), (2, 9))                                     # the frill
    tipn(c, 2, 10)
    vein(c, (-3, 3), (-3, 4), (-4, 4), (-4, 5), (-5, 5), (-5, 6), (-6, 6), (-6, 7), (-7, 7), (-7, 8), (-8, 8), (-8, 9))
    tipn(c, -8, 10)
    vein(c, (-7, 3), (-8, 3), (-8, 4), (-10, 4))
    c.P(-11, 4, "i").P(-11, 5, "/").P(-11, 3, "/")
    c.P(1, 7, "=").P(3, 5, "=").P(-9, 7, "=").P(-4, 7, "=")
    c.mirror()
    return c.build()


def pouch(c, cx, cy):
    """A brood pouch on a neck: a round lobe of drone bays, its neck running
    down to the body at (cx, cy - 5)."""
    c.rows(cx - 3, cy + 3, """
  ###
 #DD.#
#y...D#
#D.y.D#
#D...Q#
 #y.y#
  #.#
  #.#
""")
    c.cells[(cx, cy - 5)] = "."


def brood():
    """Frigate, swarm-mother. A broad oval body carrying four brood pouches
    on short necks, each a round lobe of drone bays, a blunt plated snout
    of guns. It hangs back and empties its pouches at you."""
    c = Canvas("sy_brood", "Composite")
    c.P(-10, 6, "###       ###")
    c.P(-11, 5, "#..#i###i#...#i#")
    c.P(-13, 4, "#i.yQ.mM.O.Qy.KK...##")
    c.P(-14, 3, "#.R.b.d.ZZ.y..SSS.Q..##")
    c.P(-17, 2, "N##.f.y...O......SSS..T..##")
    c.P(-17, 1, "N.R.b.c.d..@@.BB...m.y..ZZ.##/")
    c.P(-17, 0, "E....d.......HBB......M......P")
    pouch(c, -9, 11)
    pouch(c, 1, 11)
    c.P(8, 3, "P").P(9, 2, "i").P(10, 1, "i")                 # the bow's guns
    c.mirror()
    c.R(-14, -13, -10, -9).R(-13, -13, -11, -11)              # one pouch swollen
    return c.build()


def conjoined():
    """Frigate. Twins grown from one body: two fusiform hulls fused at a
    heart in the middle where the bridge sits, tails and heads apart, one
    strand of sinew still stretched between the heads. Every face that looks
    into the gap between them is an ion disruptor."""
    c = Canvas("sy_conjoined", "Composite")
    c.P(-8, 10, "########")
    c.P(-12, 9, "####y.Q.M.Oy####")
    c.P(-15, 8, "###R.b..Q.y.KK.Q.y.###")
    c.P(-18, 7, "E##.R..c.f....m....T.O...####/")
    c.P(-18, 6, "E..d.....y.......y.....d.....P")
    c.P(-18, 5, "E##.b.ZZ..SSS.y..Q.y.ZZ..####/")
    c.P(-15, 4, "###.O..SSS..M..Q.y.###")
    c.P(-12, 3, "####.y..O.y.####")
    c.P(-9, 2, "#..@@..y.#")
    c.P(-9, 1, "#...HBB..T#")
    c.P(-9, 0, "i....BB....#")
    vein(c, (7, 5), (7, 0))                                    # the strand between the heads
    stud(c, lambda x, y: y >= 9 or (y >= 8 and not -12 <= x <= 3), "#i#")
    stud(c, lambda x, y: 0 <= y <= 4, "i##")
    c.mirror()
    c.R(-12, -10, 10, 10).R(-11, -11, 11, 11)                  # a growth on one twin
    return c.build()


def segment(c, s, h, rooms, keel, leg=None):
    """One body segment of a centipede, x = s..s+6, half-height h. rooms
    are the 5-wide interior rows from y = h-1 down to 1, keel the 7-wide
    centreline row; a bulkhead in the neck behind it is the graft seam. A
    jointed leg grows outboard from the top room, `leg` on its plated tip."""
    c.P(s + 2, h, "###")
    for k, row in enumerate(rooms):
        c.P(s, h - 1 - k, "#" + row + "#")
    c.K(s, s, h - 1, h - 1).K(s + 6, s + 6, h - 1, h - 1)     # rounded shoulders
    c.P(s, 0, keel)
    c.P(s - 1, 1, "#").P(s - 1, 0, "d")
    if leg:
        vein(c, (s + 3, h - 1), (s + 3, h + 1), (s + 4, h + 1), (s + 4, h + 3))
        tipn(c, s + 4, h + 4, leg)


def myriad():
    """Destroyer. A centipede: five body segments on graft seams, each with
    a pair of jointed legs that end in plated guns, a tail of engines and a
    head with two hooked mandibles. One mind, a hundred legs; it walks its
    broadside down your hull."""
    c = Canvas("sy_myriad", "Composite")
    c.P(-35, 3, "###").P(-37, 2, "N#R.b#")
    c.P(-37, 1, "N.c.f.").P(-37, 0, "E.....")                  # tail
    segment(c, -30, 4, ["#y.Q#", "RM.bc", "....."], ".......", "i")
    segment(c, -22, 5, ["#Q.y#", "ZZ.KK", "..y..", "f...b"], ".@@....", "P")
    segment(c, -14, 5, ["#y.Q#", "SSS.M", "SSS..", ".BB.."], "..BBH..", "P")
    segment(c, -6, 5, ["#Q.y#", "GG.M.", "GG.O.", "....."], ".......", "P")
    segment(c, 2, 4, ["#y.Q#", "KK.T.", "....."], "...M...", "i")
    c.P(11, 3, "###").P(10, 2, "#y.I#").P(10, 1, "..T..#").P(10, 0, ".....#")
    c.P(9, 1, "#").P(9, 0, "d")
    vein(c, (12, 2), (12, 4), (16, 4), (16, 3), (17, 3))      # mandible
    tip(c, 18, 3)
    c.mirror()
    c.R(-16, -14, -4, -3)                    # flesh grown over one seam
    return c.build()


CONCEPTS = [
    ("spore", spore, "Spore", "Scout",
     "A seed pod with two wings swept back like a maple key's, edged with signal buoys it sheds as it drifts. Wherever a Spore has been, the Collective can hear."),
    ("mite", mite, "Mite", "Scout",
     "A tick on six short legs: plasma forelegs, ion middle legs, an engine on each hind leg. They come by the dozen and all report to one mind."),
    ("lamprey", lamprey, "Lamprey", "Corvette",
     "An eel with a round sucking mouth for a bow, lined with tractor beams and salvage arms. It latches on and feeds."),
    ("howl", howl, "Howl", "Corvette",
     "Jaws wide open and a frill of six spines fanned behind the head, each lined with jammers and tipped with an ion disruptor. It screams on every channel; the Collective calls that data corruption."),
    ("stalker", stalker, "Stalker", "Corvette",
     "A mantis: segmented abdomen, a head with thermal-imager eyes, and two raptorial arms held forward with plasma on the tips. It hunts alone."),
    ("brood", brood, "Brood", "Frigate",
     "A broad body carrying four brood pouches on short necks, each a round lobe of drone bays. It hangs back and empties its pouches at you."),
    ("conjoined", conjoined, "Conjoined", "Frigate",
     "Twins grown from one body, fused at a heart that holds the bridge, tails and heads apart and one strand of sinew still stretched between the heads. Every inner face is an ion disruptor."),
    ("myriad", myriad, "Myriad", "Destroyer",
     "A centipede: five segments on graft seams, a pair of gun-tipped legs on each, hooked mandibles at the head. One mind, a hundred legs; it walks its broadside down your hull."),
]
