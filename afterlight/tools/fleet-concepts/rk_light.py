"""Recursive Kingdom, round 3: the light ships, all built from one seed.

Lore: one person copied themselves until the copies became a civilisation,
"one person, two billion times over". Every hull is crewed by the same person
at different ages, forever arguing with themselves. Individually weak, the
nearest faction to the start, they come in junk-built swarms.

Grammar, held at every size (all of it reads from above, without colour):
  - THE SEED (rk_seed.py): an arrowhead with a plated nose, two arms swept
    back at 45 degrees, a V notch cut into the stern, and a squared BARB at
    the end of each arm carrying the engines;
  - RECURSION: bigger ships are copies of the seed arranged as the seed
    again: two side by side, three as a chevron, a small copy nested in a
    big copy's notch, a copy and its mirror, a chain shrinking aft;
  - copies touch; they don't hang on necks. Each copy's own notch, and the
    voids between copies, keep every outline readable;
  - the same APARTMENT in every arm of every copy (memory core, quarters,
    battery, oxygen: the one person's cabin, copied), the main bridge in the
    middle of the ship;
  - SYMMETRY: every skeleton is mirrored port/starboard (outline, engines,
    main guns). Copy errors are small details added after the mirror: a
    shorter tip, an offset pod, a salvaged patch, a cabin laid out
    differently. Steel; gatlings, cluster rockets, heavy missiles.

Legend: see rk_seed.py (and draw.py). Seeds are painted with rk_seed.seed();
the rest of each ship is drawn here cell by cell.
"""
import rk_seed as rk

FACTION = {
    "slug": "recursive_kingdom",
    "name": "Recursive Kingdom",
    "tagline": "One person, two billion times over: every hull is the same self at different ages, still arguing with itself. Weak alone, endless in number, and the nearest kingdom to home.",
    "grammar": [
        "One seed at every size: a plated nose, arms swept back 45 degrees, a V notch in the stern, engines on a squared barb at each arm's end",
        "Bigger ships are copies of the seed arranged as the seed again: side by side, a chevron of chevrons, a copy in a bigger copy's notch, a chain shrinking aft",
        "Copies touch, never hang on necks; every copy's own notch and the voids between them keep each outline readable",
        "The same apartment in every arm of every copy (memory core, bunk, battery, air); the main bridge in the middle of the ship",
        "Mirror-symmetric skeletons with small copy errors (a shorter tip, an offset pod, a salvaged patch, a different cabin); Steel, gatlings and rockets",
    ],
    "accent": "#e08a3c",
}


def skiff():
    """Scout. One small seed, nothing added and nothing wrong with it: the
    unit every other Recursive Kingdom hull is made of."""
    c = rk.canvas("rk_skiff")
    rk.seed(c, (0, 0), "small")
    c.mirror()
    return rk.finish(c)


def mote():
    """Interceptor. The youngest, smallest copy (an xs seed) given more
    engines than it has any right to: each barb is a chamfered block round a
    large engine, and the notch is lined with a ring of nozzles, one cell
    deep, so the V is a cell shallower than a plain seed's."""
    c = rk.canvas("rk_mote")
    s = rk.seed(c, (0, 0), "xs", swap={
        (-9, 3): "#", (-9, 4): ".",                  # the barb engines move aft
        (-6, 2): ".",                                 # a way through to the nozzles
    })
    c.rows(-11, 5, """
 ##
N..
N#
""")
    c.P(-8, 2, "E")                                   # the nozzle ring down the notch
    c.P(-7, 1, "E")
    c.P(-6, 0, "E")
    c.mirror()
    rk.flaw(c, s, patch=("Composite", "nose"))       # hand-me-down plate
    return rk.finish(c)


def twin():
    """Corvette. Two seeds side by side, welded where their inner barbs meet;
    the bridge sits in that joint, between the two selves, facing the V
    between their noses, with a cross corridor behind it and a third engine
    at the back of the joint. The lower copy came out with its nose a cell
    short, so the two tips never quite line up."""
    c = rk.canvas("rk_twin")
    a = rk.seed(c, (0, 7), "small")
    b = rk.seed(c, (0, -7), "small", flip=True)
    rk.weld(c, -11, -6, 0, 0)
    # the joint, rows -1..1: engine, cross corridor, bridge
    c.rows(-11, 1, """
#..BB#
E..BB#
""")
    c.mirror()
    rk.flaw(c, b, clip="nose")                         # the misaligned tip
    rk.flaw(c, b, swap={(-7, 3): "m"})               # its cabin has a medbay for a bunk
    return rk.finish(c)


def triad():
    """Corvette. Three seeds arranged as a seed: one leads and two follow,
    swept back on the arms of a larger chevron, the first level of the
    recursion. The leader's barbs are welded to its followers' flanks (their
    engines push for it), and the bridge sits on the leader's keel. One
    follower's outer barb is a cell short; the other wears a salvaged
    titanium patch."""
    c = rk.canvas("rk_triad")
    rk.seed(c, (0, 0), "small", swap={
        (-11, 4): "#", (-11, 5): ".", (-11, -4): "#", (-11, -5): ".",    # barbs welded, no engines
        (-9, 6): "#", (-9, -6): "#",                                      # rockets the followers would block
        (-5, 1): "B", (-4, 1): "B", (-5, 0): "B", (-4, 0): "B", (-5, -1): "B", (-4, -1): "B",
        (-3, 0): "H", (-2, 1): ".", (-2, -1): ".", (-6, 1): "T", (-6, -1): "T",
    })
    follow = {(-5, -5): ".", (-6, -5): ".",          # the weld corridor runs in here
              (-4, -4): "#"}                          # gun buried against the leader
    r1 = rk.seed(c, (-8, 10), "small", swap=follow)
    r2 = rk.seed(c, (-8, -10), "small", flip=True, swap=follow)
    c.P(-13, 4, "##")                                 # welds: leader's barb to follower
    c.P(-12, 5, ".")
    c.mirror()
    rk.flaw(c, r1, clip="port")
    rk.flaw(c, r2, patch=("Titanium", "port_barb"))
    return rk.finish(c)


def echo():
    """Corvette. Mother and child: a medium seed with a small copy of itself
    riding in its notch, nose-first, so the child's leading edges follow the
    mother's notch two cells off. They share one keel corridor; the bridge
    is on the mother's keel. The child is young: one arm's memory core is
    still a battery."""
    c = rk.canvas("rk_echo")
    m = rk.seed(c, (0, 0), "medium", swap={(-8, 0): "."})
    k = rk.seed(c, (-11, 0), "small", nose=False)
    c.rows(-11, 1, """
##
...
""")
    c.mirror()
    rk.flaw(c, k, swap={(-8, 3): "b"})
    rk.flaw(c, m, patch=("Titanium", "port_barb"))
    return rk.finish(c)


def mirror():
    """Experimental. A large seed and its exact mirror welded stern to stern:
    two bows, no clear front, symmetric about both axes. The sterns meet in a
    V seam at the outer edge, and a thin keel crosses the diamond between the
    two notches to a lozenge at the dead centre, where the one bridge sits
    (each copy keeps its own helm: two helms, one bridge). The copy that
    disagrees drives from its own bow: its engines line its leading edges,
    so it can only ever push the other one forward."""
    c = rk.canvas("rk_mirror")
    keel = {(-9, 0): ".",                              # the keel corridor runs in here
            (-7, 1): "K", (-6, 1): "K", (-7, 0): ".", (-6, 0): ".",
            (-7, -1): "K", (-6, -1): "K"}              # each copy's bridge is a barracks
    r = rk.seed(c, (0, 0), "large", engines=False, swap=keel)
    l = rk.seed(c, (-41, 0), "large", rot=2, flip=True, swap=keel)
    for y in (4, 7, 9, 10):                         # the disagreeing copy's engines
        c.cells[l.w(-y, y)] = "E"
    for y in (11, -11):                             # the V seam where the sterns meet
        for x in range(-22, -18):
            del c.cells[(x, y)]
        for x in (-21, -20):
            del c.cells[(x, y - 1 if y > 0 else y + 1)]
    # the keel across the diamond between the notches (row y has room from
    # x = -30 + |y| to -11 - |y|), with a lozenge round the bridge on the
    # ship's exact centre, x = -20.5
    c.P(-23, 3, "#p##p#")
    c.P(-25, 2, "##O....O##")
    c.P(-29, 1, ".###....BB....###.")
    c.P(-30, 0, ".........BB.........")
    c.mirror()
    rk.flaw(c, l, patch=("Composite", "nose"))
    return rk.finish(c)


def scrap_barge():
    """Trader. A medium seed hauling a chain of cargo crates shaped like
    itself, each one smaller, trailing out of its notch: copies that never
    woke up, used as holds. A mining drill for a nose, tractor beams and
    salvage arms where the rockets and outer guns would be, two gatlings.
    The crates are scavenged (one titanium, one composite), and the middle
    one sits a cell closer than the others."""
    c = rk.canvas("rk_scrap_barge")
    m = rk.seed(c, (0, 0), "medium", swap={
        (0, 0): "&",
        (-14, 9): "t", (-11, 9): "t", (-14, -9): "t", (-11, -9): "t",
        (-7, 7): "X", (-7, -7): "X",
    })
    c.P(-10, 0, "##")
    big = rk.crate(c, (-11, 0), 4, 4)
    c.P(-16, 4, "p")                                  # point defence on the big crate
    c.P(-15, 0, "#")                                  # the offset crate: one cell closer
    mid = rk.crate(c, (-16, 0), 4, 3)
    c.P(-21, 0, "##")
    small = rk.crate(c, (-22, 0), 3, 2, fill=lambda x, y: "k")
    c.P(-15, 0, "kk")                                 # where a coupling closes a crate's notch
    c.P(-20, 0, "kk")
    c.mirror()
    c.patches.update({p: "Titanium" for p in big})
    c.patches.update({p: "Composite" for p in small})
    return rk.finish(c)


REVISED_LABEL = "Round 3: rebuilt on one seed"
REVISED = {
    "skiff": "Was a pod with swept wings. Now the bare seed itself: the arrowhead every other hull is made of.",
    "twin": "Was a young pod on an old one's shoulders. Now two seeds side by side, welded at the inner barbs, the bridge between them.",
}

CONCEPTS = [
    ("skiff", skiff, "Skiff", "Scout",
     "One small seed, the unit everything else is made of: a plated nose, arms swept back, a notch in the stern, an engine barb on each arm."),
    ("mote", mote, "Mote", "Scout / interceptor",
     "The smallest copy with more engines than sense: swallowtail barbs with an engine on each step, and nozzles lining its notch."),
    ("twin", twin, "Twin", "Corvette",
     "Two seeds side by side, welded at their inner barbs with the bridge between them. One nose came out a cell short, so the tips never line up."),
    ("triad", triad, "Triad", "Corvette",
     "Three seeds arranged as a seed: a leader and two followers swept back as a bigger chevron. The first level of the recursion."),
    ("echo", echo, "Echo", "Corvette",
     "Mother and child: a small copy rides nose-first in a bigger copy's notch, its edges following the mother's two cells off."),
    ("mirror", mirror, "Mirror", "Experimental",
     "A seed and its exact mirror welded stern to stern: two bows, no clear front. The copy that disagrees drives from its own bow."),
    ("scrap_barge", scrap_barge, "Scrap Barge", "Trader",
     "A seed hauling a chain of shrinking chevron crates out of its notch, copies that never woke up, used as holds. Drill, tractors, two guns."),
]
