"""Gilded Throne, round 5a: eight new light and medium warships. Hand-drawn,
top-down only.

The approved grammar (gilded.py) holds at every size: a cut-gem jewel with the
bridge at its heart, a crown of an odd number of spikes at the bow (the centre
one longest, plated tips), crescent horns swept aft, filigree edges plated only
where they face forward, Titanium, shields, lasers, ion and railguns. Every
skeleton (outline, engines, main guns) is mirrored port/starboard.

Legend: see terran.py and draw.LEGEND.
"""
import math

from draw import Canvas
from gilded import FACTION, _filigree, _horn  # noqa: F401  (FACTION is read by build.py)


def appraiser():
    """Scout: the Appraiser. Before the Debt Collector comes, this prices you.
    A small jewel hung on a beam like a balance: two chevron pans at the beam
    ends carry passive radar a wide baseline apart."""
    c = Canvas("gt_appraiser")
    # the pans, a crescent at each end of the beam, cupped outward
    c.P(-11, 11, "?#").P(0, 11, "#?")
    c.P(-10, 10, "###").P(-2, 10, "###")
    c.P(-8, 9, "###s###")
    for y in range(5, 9):
        c.P(-5, y, "##")
    # the jewel and its crown
    c.P(-6, 4, "#A.s#/")
    c.P(-8, 3, "#O.M.....##")
    c.P(-9, 2, "E..R.Q.T.z")
    c.P(-10, 1, "E.b..BB.H...z#/")
    c.P(-11, 0, "E..M..BB......rri")
    return c.mirror().build()


def notary():
    """Scout: the Notary. It goes ahead of the Throne to read out the claim.
    A fleur-de-lis: a long centre petal for the crown spike, two side petals
    bulging forward and curling back into horns, a collar band across the
    stem, and a small curled foot round the engines."""
    c = Canvas("gt_notary")
    # the side petals, swelling forward and curling back to an aft tip
    c.P(-6, 10, "##")
    c.P(-5, 9, "#s#")
    c.P(-3, 8, "###")
    c.P(-1, 7, "##M#/")
    c.P(1, 6, "###")
    c.P(2, 5, "###")
    c.P(1, 4, "###/")
    # the collar band across the stem, joined to the petals
    c.P(-4, 5, "###")
    c.P(-4, 4, "#s#")
    c.P(-10, 4, "#")
    c.P(-9, 3, "##   #b####")
    # the stem, the centre petal, and the curled foot
    c.P(-7, 2, "E.O.Q.T.z###/")
    c.P(-8, 1, "E.R..BB.H...z##/")
    c.P(-9, 0, "E.bM..BB........rri")
    return c.mirror().build()


def _ring(c, cx, R, D, r, d):
    """Paint the upper half of an octagonal ring centred on (cx, 0): out to
    max-norm R and taxicab D, with an octagonal hole out to r and d."""
    for x in range(cx - R, cx + R + 1):
        for y in range(0, R + 1):
            m, t = max(abs(x - cx), y), abs(x - cx) + y
            if m <= R and t <= D and not (m <= r and t <= d):
                c.cells[(x, y)] = "#"


def auditor():
    """Corvette: the Auditor. It reads a ship's books through a lens. A
    small faceted jewel aft and, at the bow, an open octagonal loupe ringed
    with radar and lit from inside, a three-spike crown on its rim."""
    c = Canvas("gt_auditor")
    C = 8
    _ring(c, C, 5, 8, 2, 3)
    ring = set(c.cells)
    # the crown on the loupe's rim
    c.P(13, 4, "##/")
    c.P(14, 3, "##")
    c.P(14, 1, "##/")
    c.P(14, 0, "..rri")
    # the loupe: a walkway round the ring, radar and lasers on its outer rim,
    # floodlights on the inner rim shining into the lens
    hull = set(c.cells) | {(x, -y) for x, y in c.cells}
    hole = lambda x, y: max(abs(x - C), abs(y)) <= 2 and abs(x - C) + abs(y) <= 3
    for (x, y) in sorted(ring):
        n4 = [(x + a, y + b) for a, b in ((0, 1), (0, -1), (1, 0), (-1, 0))]
        if all(n in hull for n in n4):
            c.cells[(x, y)] = "."
        elif any(hole(*n) for n in n4):
            c.cells[(x, y)] = "l" if (x + y) % 2 else "#"
        elif x > C + 1:
            c.cells[(x, y)] = "z" if (x + y) % 3 == 1 else "#"
        elif x > C - 3:
            c.cells[(x, y)] = "A" if (x + y) % 3 == 0 else "#"
    # the jewel
    c.rows(-12, 6, """
        ###/
     ##s.M.s#/
    E.Q..O.q.#/
   E..R..T..A.#
  E..b..d..M...#
 E..R...BB.H....
E..M....BB.......
""")
    _horn(c, 5, 10, -7, -16, 1.6, 0.8, jewels=[(-12, 8, "s")])
    return c.mirror().build()


def retainer():
    """Corvette: the Retainer, a shareholder's bodyguard yacht. A peacock in
    full display, seen from above: a small cut-gem body at the rear centre,
    engines on its back and lasers on its shoulders, and a fan of seven
    quills spread from it through 210 degrees, forward and out past both
    beams, each ending in a diamond eye. The forward quill is walkable and
    its eye carries the railgun and ion gun; the six slender quills each
    hold a shield emitter in their eye; a filigree rib runs through all
    seven like the bar of a fan. It stands between its principal and the
    guns, far wider than it is long."""
    c = Canvas("gt_retainer")

    def eye(x, y, rows):
        """A diamond eye of radius 2 centred on (x, y): `rows` top to bottom
        are its five rows (1, 3, 5, 3, 1 cells)."""
        for k, row in enumerate(rows):
            c.P(x - len(row) // 2, y + 2 - k, row)

    def line(x0, y0, x1, y1):
        """A thin 4-connected run of hull from (x0, y0) to (x1, y1), laid
        under anything already drawn."""
        n = max(abs(x1 - x0), abs(y1 - y0)) * 4
        prev = (x0, y0)
        for k in range(n + 1):
            p = (int(round(x0 + (x1 - x0) * k / n)), int(round(y0 + (y1 - y0) * k / n)))
            if p[0] != prev[0] and p[1] != prev[1]:
                c.cells.setdefault((p[0], prev[1]), "#")
            c.cells.setdefault(p, "#")
            prev = p

    # the jewel: a small octagonal cut stone, engines down its aft facets,
    # the lasers on its shoulders
    c.rows(-6, 3, """
  zQOTQz
 Eb.....M
E.RBB.HmAs
E.MBB......
""")
    # the forward quill, walkable: the railgun and ion gun in its eye
    c.P(4, 1, "#######")
    c.P(5, 0, ".....")
    eye(12, 0, ["#", "#s#", "..rri", "#s#", "#"])
    # the feather quills, slender and solid, a shield emitter in every eye
    for x, y in ((10, 10), (4, 16), (-3, 16)):
        eye(x, y, ["#", "###", "##s##", "###", "#"])
        line(0, 0, x, y)
    # the rib of the fan: an elliptic bar through every quill
    prev = None
    for k in range(0, 106):
        t = math.radians(k)
        p = (int(round(7.5 * math.cos(t))), int(round(11 * math.sin(t))))
        if prev and p[0] != prev[0] and p[1] != prev[1]:
            c.cells.setdefault((p[0], prev[1]), "#")
        if p[1] > 0:
            c.cells.setdefault(p, "#")
        prev = p
    _filigree(c, ys=range(1, 20))
    return c.mirror().build()


def margin_call():
    """Corvette: the Margin Call. A marquise-cut striker, pointed at both
    ends, with a laser set at the end of every facet like a row of stones
    in a bracelet: when it comes, everything is called in at once. Two long
    crescent horns sweep back from its girdle."""
    c = Canvas("gt_margin_call")
    _horn(c, 4, 9, -8, -22, 1.6, 0.7, jewels=[(-15, 7, "s")], p=1.3)
    c.rows(-14, 5, """
          ######z/
     #..s.Q.M.s....z/
   E..R..O..m..T.....z/
  E..b...q.A..q...M....z/
 E..b....BB.H............z/
E.M......BB...............rri
""")
    return c.mirror().build()


def foreclosure():
    """Frigate: the Foreclosure, a repossession tug. A five-tined crown laid
    flat across the bow, a tractor beam on every tine: it takes hold of a
    defaulted ship by five points and tows it home. Behind the crown, a
    jewel of a hull with a vault of cryo racks for the crews that came with
    the ship."""
    c = Canvas("gt_foreclosure")
    _horn(c, 8, 14, -8, -22, 1.8, 0.8, jewels=[(-15, 12, "s")])
    # the jewel
    c.P(-6, 8, "#z#z#/")
    c.P(-8, 7, "##V.V.V##/")
    c.P(-10, 6, "#.V.V.V.V....")
    c.P(-11, 5, "E.............")
    c.P(-12, 4, "E.R.Q.O.Q.m.y..")
    c.P(-13, 3, "E..b..T.$.$.M...")
    c.P(-14, 2, "E.R..............")
    c.P(-15, 1, "E.b.q.O.T.BB.H.s..")
    c.P(-16, 0, "E.M........BB......")
    # the band across the bow, and the five tines: each a short root with a
    # spur of guns running out to a tractor beam at the tip
    c.P(5, 11, "#i##/")
    c.P(4, 10, "#.A..zt")
    c.P(4, 9, "#.s##/")
    c.P(3, 8, "#..#")
    c.P(3, 7, "#.s#")
    c.P(3, 6, "..s####/")
    c.P(3, 5, ".......zzt")
    c.P(3, 4, "..M####/")
    c.P(3, 3, "..b#")
    c.P(3, 2, "...#")
    c.P(3, 1, "...######/")
    c.P(3, 0, ".........rrrrt")
    return c.mirror().build()


def _gem(c, cx, cy, r, tips="#", rim="#", core="#"):
    """A small cut gem, a diamond of radius r: `tips` on its four points,
    `rim` stones (cycled) round each facet, `core` inside."""
    for x in range(cx - r, cx + r + 1):
        for y in range(cy - r, cy + r + 1):
            d = abs(x - cx) + abs(y - cy)
            if d < r:
                c.cells[(x, y)] = core
            elif d == r:
                c.cells[(x, y)] = tips if x == cx or y == cy else rim[abs(x - cx) % len(rim)]


def underwriter():
    """Frigate: the Underwriter, a convoy escort. It insures the Merchant
    Princes' cargo with its own hull: a faceted jewel with a three-spike
    crown, and on long horns two outrigger gems hung like earrings, studded
    with shield emitters and point-defence drones, thrown out wide over the
    convoy's flanks."""
    c = Canvas("gt_underwriter")
    c.rows(-13, 8, """
          ####/
        #s.M.s.##/
      #..Q.O.Q..T...#/
     E..R..m...A.....z
    E..b..y..q.q....#/
   E..R..O....M...#
  E.R..............z#/
 E.b..O.T..BB.H.s.......#/
E.M........BB............rri
""")
    _horn(c, 7, 12, -7, -17, 1.6, 1.0)
    _gem(c, -20, 14, 4, tips="p", rim="#s#")
    return c.mirror().build()


def _row(c, y, x0, x1, s):
    """Paint row y from x0 to x1 exactly (a typo in a long row fails loudly)."""
    if x0 + len(s) - 1 != x1:
        raise ValueError(f"row {y}: {len(s)} cells from {x0} don't end at {x1}")
    c.P(x0, y, s)


def poison_pill():
    """Destroyer: the Poison Pill. A round-cut stone, the biggest the line
    carries below the capitals, its table walked by a grid of corridors
    round the bridge. A five-spike crown with a railgun down the centre, two
    pairs of horns aft, lasers and ion on the top facet. Swallow it and the
    whole board makes you pay."""
    c = Canvas("gt_poison_pill")
    # the stone, round-cut: corridors across the top (y 8), through the
    # table (y 4) and down the keel (y 0); the bridge sits in the table.
    _row(c, 11, -2, 3, "##i##/")
    _row(c, 10, -5, 6, "#z#.....#z#/")
    _row(c, 9, -7, 8, "#z..Q.O.O.Q..z#/")
    _row(c, 8, -8, 11, "#" + "." * 16 + "###")
    _row(c, 7, -9, 11, "#Q.O.KK.sMs.KK.O.Q.#/")
    _row(c, 6, -10, 11, "#.RcR.m.y.A.y.m.T.A.#/")
    _row(c, 5, -11, 15, "#.b.q..q.O...O.q..q.M....#/")
    _row(c, 4, -11, 15, "E" + "." * 25 + "z")
    _row(c, 3, -11, 15, "E.bfb..Q.......Q..fc.....#/")
    _row(c, 2, -12, 12, "E.RcRM..y.....y...M.T...#")
    _row(c, 1, -12, 17, "E.b.O.Q.O..BB.H.O.Q.........#/")
    _row(c, 0, -12, 19, "E.M" + "." * 8 + "BB" + "." * 16 + "rri")
    # two pairs of horns: long ones off the top facets, short ones aft
    _horn(c, 8, 17, -9, -28, 1.5, 0.8, jewels=[(-19, 14, "s")])
    _horn(c, 5, 10, -11, -23, 1.5, 0.8, jewels=[(-18, 9, "s")])
    return c.mirror().build()


CONCEPTS = [
    ("appraiser", appraiser, "Appraiser", "Scout",
     "A small jewel hung on a beam like a balance, a crescent pan at each end. Before the Debt Collector comes, this prices you."),
    ("notary", notary, "Notary", "Scout",
     "A fleur-de-lis: a long centre petal for the crown spike, side petals curling back into horns. It goes ahead of the Throne to read out the claim."),
    ("auditor", auditor, "Auditor", "Corvette",
     "A small jewel aft and an open octagonal loupe at the bow, ringed with radar and lit from inside. It reads a ship's books through the lens."),
    ("retainer", retainer, "Retainer", "Corvette",
     "A shareholder's bodyguard yacht, a peacock in full display: a small jewel at the rear, and a fan of seven quills spread forward and past both beams, a diamond eye on every tip, shield emitters in six of them and the railgun in the seventh. It opens its tail between its principal and the guns."),
    ("margin_call", margin_call, "Margin Call", "Corvette",
     "A marquise-cut striker, pointed at both ends, a laser at the end of every facet like stones in a bracelet. When it comes, everything is called in at once."),
    ("foreclosure", foreclosure, "Foreclosure", "Frigate",
     "A repossession tug: a five-tined crown laid flat across the bow with a tractor beam on every tine. It takes hold of a defaulted ship by five points and tows it home."),
    ("underwriter", underwriter, "Underwriter", "Frigate",
     "A convoy escort: a faceted jewel with a three-spike crown and, on long horns, two outrigger gems hung like earrings, studded with shields and point defence and thrown wide over the convoy's flanks."),
    ("poison_pill", poison_pill, "Poison Pill", "Destroyer",
     "A round-cut stone with corridors round the bridge, a five-spike crown with a railgun down the centre, and two pairs of horns aft. Swallow it and the whole board makes you pay."),
]
