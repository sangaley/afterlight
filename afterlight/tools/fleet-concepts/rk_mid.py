"""Recursive Kingdom, round 3: the middle ships (frigates and destroyers), all
built from the one seed in rk_seed.py.

Same faction and grammar as rk_light.py: every hull is copies of the seed
arranged as the seed again, and these take the recursion a level deeper:
  - Brood Raft: a triad carrying a twin of children in its notch;
  - Recycler: one seed whose notch has become a jaw, a dead copy in its throat;
  - Ram: three generations nested one inside the next, every bow plated;
  - The Argument: two triads, port and starboard, mouth to mouth;
  - Lattice: an H-tree of forks, each half the last, a seed on every tip.
Skeletons are mirrored port/starboard; copy errors are added after the mirror.

Legend: see rk_seed.py (and draw.py). Seeds are painted with rk_seed.seed(),
crates with rk_seed.crate(); the rest of each ship is drawn here cell by cell.
"""
import rk_seed as rk
from rk_light import FACTION  # noqa: F401  (same faction; build.py reads FACTION from here)


def bow_plates(c, edge, skip=()):
    """Plate every step of a bow staircase: edge = [(x, y), ...] the outermost
    hull cell of each row going out (y > 0); the plate goes just past each
    shorter row's end, above the longer row's last cell."""
    for (x0, y0), (x1, y1) in zip(edge, edge[1:]):
        if y1 in skip:
            continue
        p = (x0, y1)
        if p not in c.cells:
            c.cells[p] = "/"


def brood_raft():
    """Frigate (carrier). A raft of copies tied into a bigger chevron, with
    its children aboard: three small seeds lashed into a triad (a leader and
    two followers on its arms), and in the triad's notch two xs copies, the
    youngest, nosed into the followers' inner barbs and welded to each other
    like a twin. Every grown copy has swapped its fuel and coolant for a drone
    bay in each arm. The bridge sits on the leader's keel."""
    c = rk.canvas("rk_brood_raft")
    bays = {(-9, 4): "D", (-8, 4): "D", (-9, -4): "D", (-8, -4): "D"}
    lead = rk.seed(c, (0, 0), "small", swap={
        **bays,
        (-11, 4): "#", (-11, 5): ".", (-11, -4): "#", (-11, -5): ".",   # barbs lashed to the followers
        (-9, 6): "#", (-9, -6): "#",                                     # rockets the followers would block
        (-5, 1): "B", (-4, 1): "B", (-5, 0): "B", (-4, 0): "B", (-5, -1): "B", (-4, -1): "B",
        (-3, 0): "H", (-2, 1): ".", (-2, -1): ".", (-6, 1): "T", (-6, -1): "T",
    })
    follow = {**bays,
              (-5, -5): ".", (-6, -5): ".", (-4, -4): "#",              # lashing corridor; buried gun
              (-11, -4): ".", (-11, -5): "#"}                            # the inner barb a child noses into
    f1 = rk.seed(c, (-8, 10), "small", swap=follow)
    f2 = rk.seed(c, (-8, -10), "small", flip=True, swap=follow)
    child = {(-7, -5): "#"}                                              # rocket the twin weld would bury
    k1 = rk.seed(c, (-20, 6), "xs", nose=False, swap=child)
    k2 = rk.seed(c, (-20, -6), "xs", nose=False, flip=True, swap=child)
    c.P(-13, 4, "##")                                                    # leader to follower
    c.P(-12, 5, ".")
    c.P(-20, 7, "##")                                                    # child to follower
    c.P(-20, 5, "#")
    c.P(-29, 0, "#####")                                                 # the children's twin weld
    c.mirror()
    rk.flaw(c, f2, clip="nose")                                          # a follower's tip came out short
    rk.flaw(c, k2, swap={(-6, 4): "m"})                                  # one child bunks in a medbay
    rk.flaw(c, f1, patch=("Titanium", "port_barb"))                      # salvaged plate
    return rk.finish(c)


def recycler():
    """Frigate (salvager). A large seed whose notch has become a jaw: its
    barbs grow on aft into two mandibles that hook in toward each other like
    a beetle's, their inner faces lined with breaker drills (salvage arms)
    and tractor beams, their stepped outer backs carrying the engines. A
    dead copy sits in the throat as the scrap hold, so what the jaw chews
    goes straight into it."""
    c = rk.canvas("rk_recycler")
    s = rk.seed(c, (0, 0), "large", swap={
        (0, 0): "g", (-8, 8): "g",                     # under 450 cells: no cannons
        (-20, 8): "X", (-20, 9): ".", (-20, 10): ".",  # the engines move out to the mandibles
        (-18, 8): "t",
    })
    # one mandible, row by row: hooked in, engines down its back, teeth inside
    for y, x, run in (
        (11, -23, "###"),
        (10, -25, "##.Q."),
        (9, -27, "E#....."),
        (8, -28, "E.R..tXX"),
        (7, -29, "E..XtX"),
        (6, -30, "#..X"),
        (5, -31, "tXXX"),
    ):
        c.P(x, y, run)
    # the hold: a dead copy in the throat, one cell off the notch faces
    rk.crate(c, (-12, 0), 5, 5, fill=lambda x, y: "U" if abs(y) % 2 and x < -1 - abs(y) else "k")
    c.P(-11, 0, "#")                                  # its coupling to the keel
    c.mirror()
    rk.flaw(c, s, patch=("Titanium", "stbd_barb"))   # a salvaged barb
    rk.flaw(c, s, swap={(-13, -5): "m"})             # one bunk is a medbay
    return rk.finish(c)


def ram():
    """Frigate (ram). Three generations packed one inside the next: a small
    copy nested in a large one's notch and an xs copy in the small one's, each
    two cells off its parent, and every one of their bow faces plated, so the
    seams between the copies are lines of armour. The battering head is the
    large copy's nose, plated on every step; the bridge sits in the middle
    copy, on the keel at the ship's centre."""
    c = rk.canvas("rk_ram")
    big = rk.seed(c, (0, 0), "large", swap={
        (0, 0): "g",                                   # under 450 cells: no cannons
        (-5, 5): "#", (-8, 8): "#",                    # the head is armour, not gun ports
        (-10, 0): ".",                                 # the keel runs on into the child
        (-7, 1): "K", (-6, 1): "K", (-7, 0): ".", (-6, 0): ".",   # its bridge is a barracks
    })
    mid = rk.seed(c, (-13, 0), "small", nose=False, swap={
        (-4, 4): "#",                                  # gun buried against the parent
        (-11, 4): "#", (-11, 5): "#",                  # engines would fire into the child
        (-6, 0): ".", (-5, 0): ".",                    # the keel runs on through
        (-3, 1): "B", (-2, 1): "B", (-3, 0): "B", (-2, 0): "B",   # the one bridge
    })
    low = rk.seed(c, (-22, 0), "xs", nose=False, swap={
        (-3, 3): "#",                                  # gun buried against the parent
    })
    c.rows(-13, 1, """
##
 ..
""")                                                    # keel welds, large to small
    c.rows(-22, 1, """
##
 ..
""")                                                    # small to xs
    bow_plates(c, [big.w(-k, k) for k in range(0, 12)])
    bow_plates(c, [mid.w(-k, k) for k in range(0, 7)], skip=(1,))
    bow_plates(c, [low.w(-k, k) for k in range(0, 6)], skip=(1,))
    c.mirror()
    rk.flaw(c, big, patch=("Titanium", "nose"))      # the battering head is salvaged plate
    rk.flaw(c, rk.Seed("xs", (-22, 0), 0, False), swap={(-6, -4): "m"})   # the youngest bunks in a medbay
    return rk.finish(c)


def argument():
    """Destroyer. The faction arguing with itself: two triads (three seeds
    arranged as a seed) turned to face each other mouth to mouth across the
    keel, one pointing to port and its mirror image to starboard, so the
    whole ship is its own reflection. A lozenge across the gap, pointed fore
    and aft like two noses, holds the one bridge between them, and every barb
    and notch that faces the gap carries a gun aimed at its other self. The
    aft copies drive. The starboard self came out a little different: a
    patched nose, a salvaged barb, a cabin laid out its own way."""
    c = rk.canvas("rk_argument")
    # one triad, pointing to port: a leader and two followers on its arms
    L = rk.seed(c, (0, 22), "small", rot=1, swap={
        (-11, 4): "#", (-11, 5): ".", (-11, -4): "#", (-11, -5): ".",    # barbs welded to the followers
        (-9, 6): "#", (-9, -6): "#",                                      # rockets the followers would block
        (-6, 0): "g", (-5, 0): ".",                                       # a gun down its mouth at its twin
    })
    follow = {(-5, -5): ".", (-6, -5): ".", (-4, -4): "#"}               # weld corridor; buried gun
    A = rk.seed(c, (-10, 14), "small", rot=1, swap=follow)               # aft follower
    F = rk.seed(c, (10, 14), "small", rot=1, flip=True, swap=follow)     # fore follower
    for side in (1, -1):                                                  # leader's barbs to followers
        for p, ch in (((-13, 4), "#"), ((-12, 4), "#"), ((-12, 5), ".")):
            c.cells[L.w(p[0], side * p[1])] = ch
    for s in (A, F):
        for p, ch in (((-11, 5), "g"),                                    # outer barb: gun across the gap
                      ((-11, -5), "."), ((-11, -6), "."),                  # inner barb: door to the lozenge
                      ((-9, -6), "#")):                                    # rocket the lozenge would block
            c.cells[s.w(*p)] = ch
    for lx in range(-10, -6):                                             # the aft copy drives
        c.cells[A.w(lx, 6)] = "E"
    # the lozenge across the gap, holding the one bridge: an engine at its
    # aft point, a gun at its fore point
    c.rows(-8, 2, """
  #.#########.#
 #..s...BB.A...#
E......HBB......g
""")
    bow_plates(c, [(8, 0), (7, 1), (6, 2)])
    c.mirror()
    # the starboard self, as the mirror painted it
    L2 = rk.Seed("small", (0, -22), 3, True)
    A2 = rk.Seed("small", (-10, -14), 3, True)
    F2 = rk.Seed("small", (10, -14), 3, False)
    rk.flaw(c, L2, patch=("Composite", "nose"))
    rk.flaw(c, A2, patch=("Titanium", "port_barb"))
    rk.flaw(c, F2, swap={(-7, 3): "m"})
    return rk.finish(c)


def lattice():
    """Destroyer. Branching recursion, an H-tree grown forward: a keel that
    splits into two branches, each splitting again at half the size, and a
    seed socketed on every tip with a cannon in its nose. Every fork is the
    seed's own V turned round (two arms at 45 degrees, each fork half the
    one before), and the voids between the branches are open sky. Engines
    line the trunk's trailing edges; the bridge sits in the keel's tongue,
    at the point where the first split begins."""
    c = rk.canvas("rk_lattice")
    # the four tips: xs seeds socketed on the branch ends, cannon noses
    rk.seed(c, (33, 18), "xs", swap={
        (0, 0): "n", (-5, 0): ".",                       # cannon; the branch runs in here
        (-9, -3): "#", (-9, -4): "#", (-7, -5): "#",     # engines and a rocket the branch would block
    })
    rk.seed(c, (33, 6), "xs", swap={
        (0, 0): "n", (-5, 0): ".",
        (-9, -3): "#", (-9, -4): "#", (-9, 3): "#", (-9, 4): "#",
        (-7, -5): "#", (-7, 5): "#",
    })
    # one half of the tree, row by row (x where each run starts)
    for y, x, run in (
        # the keel and its tongue (rows 0..2), the bridge on the centreline
        (0, 0, "E.R......BBH..g"),
        (1, 1, "#..KK.s.BB.A#"),
        (2, 2, "E..MQ.T.###"),
        # the first fork's arm (band 7): engines aft, corridor, the apartment
        (3, 3, "#f..as#"),
        (4, 4, "ER..QM#"),
        (5, 5, "#c..Ob#"),
        (6, 6, "ER..AT#"),
        (7, 7, "#f..QM#"),
        (8, 8, "#a..Ob#"),
        (9, 9, "#c..as#"),
        # where the first fork's arm becomes the second fork
        (10, 10, "#b..m.##s..A#"),
        (11, 11, "#b........#"),
        (12, 12, "####..y.#"),
        # the second fork's outer arm (band 6) running to the outer tip
        (13, 16, "#b..O#"),
        (14, 17, "ER..M#"),
        (15, 18, "#c..Q#"),
        (16, 19, "#a..T.."),
        (17, 20, "#a....."),
        (18, 21, "######."),
        # the second fork's inner arm running to the inner tip
        (9, 18, "#b..O#"),
        (8, 19, "#c..T.."),
        (7, 20, "#a....."),
        (6, 21, "######."),
    ):
        c.P(x, y, run)
    bow_plates(c, [(14, 0), (13, 1), (12, 2), (9, 3)])  # the tongue's nose is plated
    c.mirror()
    rk.flaw(c, rk.Seed("xs", (33, -18), 0, True), patch=("Composite", "nose"))   # a salvaged tip
    rk.flaw(c, rk.Seed("xs", (33, 6), 0, False), swap={(-6, 4): "m"})           # a cabin made a medbay
    return rk.finish(c)


REVISED_LABEL = "Round 3: rebuilt on one seed"
REVISED = {
    "brood_raft": "Was a flock of younger selves climbing an adult on swept necks. Now a triad of small seeds lashed into a chevron, with two xs children docked in its notch round a brood well.",
    "recycler": "Was a chain of heads ending in a salvage maw. Now one large seed whose notch has become the jaw: hooked mandibles lined with breaker drills, a dead copy in the throat as the hold.",
    "argument": "Was two halves facing fore and aft on a yoke. Now two triads, port and starboard, each the other's mirror image, mouth to mouth across the keel with the bridge between them.",
}

CONCEPTS = [
    ("brood_raft", brood_raft, "Brood Raft", "Frigate (carrier)",
     "Three small seeds lashed into a chevron carry their children: two xs copies docked in its notch round an open brood well, and a drone bay in every grown arm."),
    ("recycler", recycler, "Recycler", "Frigate (salvager)",
     "A seed whose notch has become a jaw: hooked mandibles lined with breaker drills and tractors, and a dead copy in the throat as the scrap hold."),
    ("ram", ram, "Ram", "Frigate (ram)",
     "Three generations packed one inside the next, every bow face plated, so the seams between the copies are lines of armour. The one Recursive ship built not to break."),
    ("argument", argument, "The Argument", "Destroyer",
     "Two triads facing each other mouth to mouth across the keel, each the other's mirror image, guns aimed across the gap at themselves. The bridge sits on a lozenge between them."),
    ("lattice", lattice, "Lattice", "Destroyer",
     "Branching recursion: a keel that splits in two, each branch splitting again at half the size, and a seed socketed on every tip with a cannon in its nose."),
]
