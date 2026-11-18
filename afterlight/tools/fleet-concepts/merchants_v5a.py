"""Merchant Princes of the Void, round 5a: seven small and medium hulls that
widen the Guild's trade. Hand-drawn, top-down only.

The faction's approved grammar (merchants.py) holds: a three-wide spine with a
walkway down its middle, standard 5x5 container pods with their corners cut on
short struts, open trusswork, Steel, gatlings and point-defence only, docking
clamps at the bow. New in this round, by the developer's symmetry rule: every
skeleton (outline, engines, guns) is mirrored port/starboard. The Guild's
utilitarian asymmetry now lives inside identical pods (cargo racked on one
side, crew and tanks on the other) and in a few small fittings after the
mirror: a crane, a hatch, a cabin laid out differently.

Legend: see terran.py and draw.LEGEND.
"""
from draw import Canvas
from merchants import FACTION, _pod, _run  # noqa: F401  (FACTION is read by build.py)


def bill_runner():
    """Scout: express courier. The fastest hull the Guild owns, carrying bills
    of exchange and a strongbox between Guild houses. A long bare spine, the
    crew cab threaded on it amidships, and two engine pods out on a stern
    crossbar, so the stern is wide and the nose is a needle."""
    c = Canvas("mp_bill_runner", "Steel")
    _pod(c, -12, 3, ["R.f", "..M", "R.c"])
    c.P(-12, 6, "N")
    c.P(-12, 5, "N")
    c.P(-12, 4, "E")
    c.P(-10, 7, "p")
    c.P(-11, 2, "#.#")
    c.P(-12, 1, _run(-12, 9, doors={-10}))
    c.P(-12, 0, "E" + "." * 22 + "o")
    c.P(10, 1, "/")
    # the cab: threaded on the spine, the bridge across the centreline
    c.rows(-3, 3, """
  ###
 #Q.$#
#.BB.M#
..BB...
""")
    # gun cheeks behind the nose
    c.P(5, 2, "#g#")
    c.P(6, 1, ".")
    c.mirror()
    c.P(2, -1, "H")
    return c.build()


def claimstaker():
    """Industrial: prospector. Finds ore for the Guild and stakes the claim.
    Two long sensor arms cross the spine at the cab, claim beacons racked
    down both sides of each and a sensor pod at each tip, so it reads a
    rock from two places at once; a pair of sample pods aft, sample racks
    on the spine and a core drill at the bow."""
    c = Canvas("mp_claimstaker", "Steel")
    c.rows(-14, 3, """
 ###
N.R#
N...
E...
""")
    c.P(-10, 1, _run(-10, 11, doors={-7}, marks={5: "k", 7: "k", 9: "k"}))
    c.P(-10, 0, "." * 23)
    _pod(c, -9, 2, ["k.k", "k.k", "k.k"])
    # the cab at the crossing
    c.rows(-3, 3, """
  #.#
 #Q.q#
#.BB.M#
..BB...
""")
    # the arms, a sensor pod at each tip
    for y in range(4, 8):
        c.P(-1, y, "J.J" if y % 2 else "#.#")
    _pod(c, -2, 8, ["...", "I.M", "k.k", ".", "#"])
    c.P(-1, 12, "AAA")
    # core drill at the bow
    c.rows(11, 2, """
 #&/
#..&/
....&
""")
    c.mirror()
    c.P(2, -1, "H")
    return c.build()


def bailiff():
    """Corvette: debt collector. The one Guild hull built to chase: nothing
    on the spine but the cab and a rack of impound holds, big engines behind
    it and a repossession clamp in front, two jaws with gatlings on them and
    grapples at their tips round a docking port, to take hold of a debtor's
    hull and tow it home."""
    c = Canvas("mp_bailiff", "Steel")
    c.rows(-17, 4, """
 ###
N.R#
N.M#
E...
E...
""")
    c.P(-13, 1, _run(-13, 9, doors={5}, marks={-12: "k", -11: "k", -10: "k", -9: "p"}))
    c.P(-13, 0, "." * 23 + "o")
    # the cab, amidships
    c.rows(-5, 3, """
  #p#
 #Q.Q#
#.BB.M#
..BB...
""")
    # the clamp: a strut up to each jaw, the jaw running forward to a grapple
    c.rows(4, 8, """
#g#  #g#
#.####.##/
#........X
#.#######/
#.#
#.#
#.#
""")
    c.mirror()
    c.P(0, -1, "H")
    return c.build()


def tidewaiter():
    """Patrol: customs. The Guild's tidewaiter waits at the jump points and
    boards whatever comes through. Two mooring clamps stand out port and
    starboard at the bow, so it can hold an inspected ship on either side;
    two bonded-store pods aft keep what it confiscates."""
    c = Canvas("mp_tidewaiter", "Steel")
    c.rows(-18, 3, """
 ###
N.R#
N...
E...
""")
    c.P(-14, 1, _run(-14, 10, doors={-11, 4}))
    c.P(-14, 0, "." * 25)
    # bonded stores aft
    c.P(-12, 2, "#.#")
    c.P(-12, 3, "#.#")
    _pod(c, -13, 4, ["k$k", "k.k", "M.k"])
    # the cab
    c.rows(-6, 3, """
  ###
 #Q.q#
#.BB.M#
..BB...
""")
    # the mooring clamps, one each side
    c.P(3, 2, "#.#")
    c.P(3, 3, "#.#")
    c.rows(0, 7, """
#g/   /g#
#.#   #.#
#.##o##.#
#.......#
 ###.###
""")
    # the scanner pod on the nose: arrays on its three outer faces
    c.rows(11, 2, """
 #A#/
#I.I#
....A
""")
    c.mirror()
    c.P(-1, -1, "H")
    return c.build()


def _vane(c, x, y0, length):
    """A radiator vane: a two-wide panel of heat vents standing out from the
    spine at x, from row y0 for `length` rows. No crew go out on it."""
    for y in range(y0, y0 + length):
        c.P(x, y, "vv")


def reefer():
    """Trader: cold goods. Luxuries from a dozen universes, kept frozen until
    the price is right: two pairs of standard pods full of cryo storage on
    the spine, each bracketed by radiator vanes standing out past it,
    dumping the heat the pumps pull out of the cargo. The vanes make the
    comb."""
    c = Canvas("mp_reefer", "Steel")
    c.rows(-22, 3, """
 ###
N.R#
N...
E...
""")
    pods = (-14, 7)
    vanes = (-17, -8, 4, 13)
    c.P(-18, 1, _run(-18, 15, doors={x + 2 for x in pods},
                     marks={x: "c" for x in vanes} | {x + 1: "c" for x in vanes}))
    c.P(-18, 0, "." * 34)
    for x in pods:
        c.P(x + 1, 2, "#.#")
        _pod(c, x, 3, ["VVV", "VcV", "V.V"])
    for x in vanes:
        _vane(c, x, 2, 8)
    c.P(-11, 6, "M")
    # the cab amidships
    c.rows(-4, 3, """
  ###
 #Q.Q#
#.BB.M#
..BB...
""")
    # bow clamp: two jaws round the docking port
    c.rows(16, 4, """
 ####/
#....o
#.##g#
#.#
...o
""")
    c.mirror()
    c.P(1, -1, "H")
    return c.build()


def rag_and_bone():
    """Trader: salvage buyer. Turns up after every battle and buys the wrecks
    by weight. Aft of the cab each side carries a cradle: two crane booms
    reaching toward each other over an open bay of sky, cranes on every
    inner face to lift wreckage in and hold it there. Scrap pods forward."""
    c = Canvas("mp_rag_and_bone", "Steel")
    c.rows(-23, 3, """
 ###
N.R#
N...
E...
""")
    c.P(-19, 1, _run(-19, 16, doors={-17, -3, 11}))
    c.P(-19, 0, "." * 36)
    _pod(c, 9, 2, ["CCk", "CCk", "k.M"])
    # the cab
    c.rows(0, 3, """
  ###
 #Q.Q#
#.BB.M#
..BB...
""")
    # the cradle
    for y in range(2, 6):
        c.P(-18, y, "#.#")
        c.P(-4, y, "#.#")
    c.P(-16, 4, "X")
    c.P(-4, 4, "X")
    c.rows(-18, 8, """
 ##g##     ##g##
#.....X   X.....#
#.##X#     #X##.#
""")
    # bow clamp: two jaws round the docking port
    c.rows(17, 4, """
 ####/
#....o
#.##g#
#.#
...o
""")
    c.mirror()
    c.P(5, -1, "H")
    return c.build()


def _fill(c, x, y, rows):
    """Re-lay the three inside rows of the pod whose bottom-left corner cell
    is (x, y), top row first. Used after mirror() to change one side only."""
    for k, row in enumerate(rows):
        c.P(x + 1, y + 3 - k, row)


def packet():
    """Trader: passenger liner. Carries passengers, mail and specie between
    the Guild's markets on a timetable, and sells passage out of universes
    the Guild has emptied. Twin-hulled: two spines side by side
    with a slot of sky between them, each with its own engines, its own
    boarding port and a row of cabin pods outboard, joined amidships by the
    saloon with the bridge in it. First class rides in the port pods,
    steerage asleep in cryo in the starboard ones."""
    c = Canvas("mp_packet", "Steel")
    c.rows(-21, 6, """
 ###
N.R#
N...
E.R#
 ###
""")
    pods = (-16, -9, 4, 11)
    c.P(-17, 5, _run(-17, 16, doors={x + 2 for x in pods}))
    c.P(-17, 4, "." * 34 + "o")
    c.P(-17, 3, _run(-17, 16, doors={0}))
    c.P(17, 5, "/")
    c.P(17, 3, "/")
    for x in pods:
        _pod(c, x, 6, ["q.q", "Q.Q", "Q.Q"])
    c.P(-14, 9, "M")
    # the saloon bridges the slot, the bridge on the centreline
    c.rows(-4, 2, """
  #Q.Q#
 #.BB.M#
#..BB...#
""")
    c.mirror()
    c.P(2, -1, "H")
    # steerage: the cabins on the starboard hull sleep their passengers in cryo
    for x in pods:
        _fill(c, x, -10, ["V.V", "V.V", "V.V"])
    c.P(14, -8, "M")
    return c.build()


CONCEPTS = [
    ("bill_runner", bill_runner, "Bill Runner", "Scout: express courier",
     "The fastest hull the Guild owns: two engine pods on a stern crossbar, a bare spine and a cab threaded on it with a strongbox inside. It carries contracts, bills and the names they bind between Guild houses, never freight."),
    ("claimstaker", claimstaker, "Claimstaker", "Industrial: prospector",
     "Two long sensor arms cross the spine at the cab, claim beacons racked along them and a sensor pod at each tip; sample pods aft, a core drill at the bow. Where it parks, the Guild owns the rock, in this universe or the next."),
    ("bailiff", bailiff, "Bailiff", "Corvette: debt collector",
     "The one Guild hull built to chase: big engines, a bare spine with the cab and a rack of impound holds, and a repossession clamp, gatlings on its jaws and grapples at the tips, that takes a debtor's ship and tows it home."),
    ("tidewaiter", tidewaiter, "Tidewaiter", "Patrol: customs",
     "Waits at the jump points and charges whatever comes through: a mooring clamp out on each side to hold an inspected ship, bonded-store pods aft for what it confiscates, a scanner pod on the nose."),
    ("reefer", reefer, "Reefer", "Trader: cold goods",
     "Luxuries from a dozen universes, kept frozen until the price is right: cryo pods on the spine, each bracketed by radiator vanes standing out past it to dump the heat."),
    ("rag_and_bone", rag_and_bone, "Rag-and-Bone", "Trader: salvage buyer",
     "Turns up after every battle and buys the wrecks by weight, from both sides: two crane booms each side reach over an open cradle aft of the cab, scrap pods forward."),
    ("packet", packet, "Packet", "Trader: passenger liner",
     "Twin hulls with a slot of sky between them, joined amidships by the saloon. Contract passengers ride first class in the port pods; refugees from emptied universes sleep in cryo in the starboard ones, at first-class prices."),
]
