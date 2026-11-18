"""Recursive Kingdom, round 3: two heavy ships, both built from the one seed.

Lore: one person copied themselves until the copies became a civilisation,
"one person, two billion times over". Every hull is crewed by the same person
at different ages, forever arguing with themselves. Weak alone, the nearest
kingdom to home, junk-built.

These two carry the recursion past the light ships (rk_light.py), and each is
drawn so it reads from above, without colour:
  - SIERPINSKI (Majority of One): two levels. A medium seed leads two small
    copies swept back on its own arm lines, joined barb to barb: one
    chevron of chevrons, rk_light's Triad with an elder in front. Three of
    those make the ship, arranged the same way again, so the outline is the
    seed's arrowhead three times over and the voids are its notch three
    times over: in every copy, behind every chevron, and one big one in the
    middle, closed at the stern by the two innermost copies welded keel to
    keel. Nine copies, nine helms, one bridge (the elder in front's; the
    other two elders' bridges are barracks).
  - SPIRAL (Regress): one keel and two curls. A large seed on the keel; on
    each of its swept-back barbs its eldest child, and from each child a
    line of ever smaller copies curls away round an open eye, every copy a
    quarter turn on from the one before (medium, small, xs, then two copies
    too small to wake: uncrewed chevron crates, a magazine and a battery).
    The arms of each copy and the next run on one line, so each curl is one
    square spiral with a copy's nose at every corner. The keel copy has no
    engines of its own: its barbs are welded to its children's, and the
    children push.

Rules kept on both hulls: the skeleton is mirror-symmetric (copies and joints
are drawn on the port side and mirrored; copy errors are added after the
mirror and stay small); engines only on faces with open space aft; one main
bridge, on the keel nearest the ship's middle; launchers only where four
cells outside are clear; Steel with salvaged patches. Joints are short welds
or plain contacts with a corridor through them: no copy hangs on a neck.

Legend: see rk_seed.py (and draw.py). Seeds are painted with rk_seed.seed(),
crates with rk_seed.crate(); joints, the stern block and the turned copies'
engines are drawn here cell by cell.
"""
import rk_seed as rk
from rk_light import FACTION  # same faction, same page

N4 = ((0, 1), (0, -1), (1, 0), (-1, 0))
LAUNCHERS = set("x*^")
GUNS = set("gnpz")


# --------------------------------------------------------------------------- helpers
def hull_of(c):
    return {p for p, ch in c.cells.items() if ch != "/"}


def silence(c):
    """Symmetric clean-up once copies are packed together: a launcher
    without four clear cells outside it, a gun shut in on every side, or an
    engine that would fire into hull becomes plain hull."""
    hull = hull_of(c)
    for (x, y), ch in sorted(c.cells.items()):
        if ch in LAUNCHERS:
            lanes = [(dx, dy) for dx, dy in N4 if (x + dx, y + dy) not in hull]
            if not any(all((x + dx * k, y + dy * k) not in hull for k in range(1, 5)) for dx, dy in lanes):
                c.cells[(x, y)] = "#"
        elif ch in GUNS and all((x + dx, y + dy) in hull for dx, dy in N4):
            c.cells[(x, y)] = "#"
        elif ch in "EN" and any((xx, y) in hull for xx in range(x - 80, x)):
            c.cells[(x, y)] = "#"


def demote_bridge(c, s):
    """A medium copy's own 2x3 bridge when it isn't the ship's: two barracks
    with the keel corridor running between them."""
    for ly in (1, -1):
        for lx in (-6, -5):
            c.cells[s.w(lx, ly)] = "K"
    for lx in (-6, -5):
        c.cells[s.w(lx, 0)] = "."


def mirrored(s):
    """The Seed that c.mirror() made of the port copy s (for copy errors)."""
    return rk.Seed(s.size, (s.at[0], -s.at[1]), (4 - s.rot) % 4, not s.flip, s.short)


def swap(c, cells):
    for p, ch in cells.items():
        c.cells[p] = ch


# --------------------------------------------------------------------------- sierpinski
L1 = (13, 13)        # leader to follower: 13 aft, 13 out, on the leader's own arm line
L2 = (22, 22)        # front chevron to rear chevron: the same again, one level up


def l1_joint(c, X, Y, s):
    """Weld the barb on side s (+1 port, -1 starboard) of the medium leader
    whose tip is (X, Y) to the inner barb of the small follower beside it,
    with the barb corridor running straight through."""
    c.P(X - 18, Y + s * 8, "...")         # follower's rim, weld, leader's engine
    c.P(X - 17, Y + s * 9, ".")           # the follower's shoulder gun, now shut in
    c.P(X - 18, Y + s * 7, "###")         # weld (over the follower's plate, the other engine)


def l2_joint(c, fx, fy, s):
    """Weld the outer barb (side s) of the follower whose tip is (fx, fy) to
    the inner arm of the next chevron's leader resting against it."""
    c.P(fx - 13, fy + s * 5, "...")       # leader's shoulder gun, weld, follower's engine
    c.P(fx - 13, fy + s * 4, "###")       # weld (and the follower's other engine)


def chevron(c, at, flip=False):
    """One chevron of the first level: a medium leader with a small follower
    swept back on each of its arms, joined barb to barb. The leader is the
    elder, with the big guns (cannons where its gatlings would be)."""
    X, Y = at
    a, b = L1
    m = rk.seed(c, at, "medium", flip=flip, swap={(0, 0): "n", (-4, 4): "n", (-4, -4): "n"})
    s1 = rk.seed(c, (X - a, Y + b), "small")
    s2 = rk.seed(c, (X - a, Y - b), "small", flip=True)
    l1_joint(c, X, Y, 1)
    l1_joint(c, X, Y, -1)
    return m, s1, s2


def sierpinski():
    """Battleship. Three chevrons, each made of three chevrons: two levels
    of recursion, nine copies, the voids between them left open. A medium
    elder leads two small copies swept back on its own arm lines, welded
    barb to barb (rk_light's Triad, with an elder in front); three of those
    lead and follow the same way, the rear chevrons' inner arms resting on
    the front chevron's followers. The big void in the middle is closed at
    the stern, where the two innermost copies are welded keel to keel round
    an engine bank. The one bridge is on the front elder's keel; the rear
    elders' bridges are barracks; every copy keeps its own helm."""
    c = rk.canvas("rk_sierpinski")
    A, B = L2
    a, b = L1
    m0, s01, _ = chevron(c, (0, 0))                   # the front chevron
    m1, s11, s12 = chevron(c, (-A, B))                # the port rear chevron
    l2_joint(c, -a, b, 1)                             # front follower to rear elder
    demote_bridge(c, m1)
    swap(c, {
        (-11, 9): "x",                                # the front elder's silo
        (-44, 41): "x",                               # the outermost copies' silos
        (-17, 17): "p", (-39, 39): "p",               # point defence on the outer shoulders
    })
    # the stern block: the two innermost rear copies welded keel to keel, an
    # engine bank between their barbs, point defence facing into the big void
    c.rows(-46, 3, """
   .
#.b.f#
E.R.b#
E....p
""")
    c.mirror()
    silence(c)
    # copy errors (after the mirror; small)
    rk.flaw(c, mirrored(s11), clip="nose")           # an outer copy a cell short
    rk.flaw(c, s01, patch=("Titanium", "port_barb"))  # salvaged plate
    rk.flaw(c, mirrored(m1), patch=("Composite", "nose"))
    rk.flaw(c, s12, swap={(-7, 2): "O", (-6, 2): "b"})   # its cabin laid out the other way
    return rk.finish(c)


# --------------------------------------------------------------------------- spiral
def spiral():
    """Cruiser. One keel and two curls of ever smaller copies. A large seed
    on the keel; behind each of its barbs, welded barb to barb, its eldest
    child (a medium); and from each child a line of copies curls away round
    an open eye, every copy a quarter turn on from the last and smaller: a
    small copy pointing outboard, an xs pointing aft, then two copies too
    small to crew, chevron crates used as a magazine and a battery, the last
    pointing back at the child's notch. Each copy's arm runs on into the
    next one's, so each curl is a square spiral with a nose at every
    corner. The keel copy has no engines: its children push it (the aft-
    pointing copies drive from their own bows)."""
    c = rk.canvas("rk_spiral")
    k = rk.seed(c, (0, 0), "large")
    a = rk.seed(c, (-15, 17), "medium", swap={(0, 0): "n"})
    b = rk.seed(c, (-35, 38), "small", rot=1)
    cc = rk.seed(c, (-51, 24), "xs", rot=2)
    d = rk.crate(c, (-40, 13), 3, 3, fill=lambda x, y: "a", rot=3)
    rk.crate(c, (-32, 20), 3, 2, fill=lambda x, y: "b")
    demote_bridge(c, a)
    # the keel copy's barb welded to its child's, corridor straight through
    c.P(-22, 10, "...")                   # (the child's barb gun, the weld, an engine)
    c.P(-22, 9, "###")
    # the curl's other joints are plain contacts with a corridor through them
    c.P(-30, 26, ".")                     # child's barb up into the outboard copy's
    c.P(-30, 27, ".")
    c.P(-42, 28, "..")                    # outboard copy's barb across to the aft one's
    # engines on the turned copies' aft-facing faces
    c.P(-41, 30, "E")                     # the outboard copy's barb face
    c.P(-41, 31, "E")
    for p in ((-50, 25), (-50, 23), (-49, 26), (-49, 22)):
        c.cells[p] = "E"                  # the aft-pointing copy drives from its bow
    for p in ((-51, 25), (-51, 23), (-50, 26), (-50, 22)):
        c.cells.pop(p, None)
    swap(c, {(-39, 34): "p", (-31, 34): "p"})        # point defence on the outer copy
    c.mirror()
    silence(c)
    # copy errors (after the mirror; small)
    rk.flaw(c, mirrored(a), clip="nose")              # one child a cell short
    rk.flaw(c, b, patch=("Titanium", "stbd_barb"))    # salvaged plate
    rk.flaw(c, mirrored(cc), swap={(-6, 4): "m"})     # a medbay where the bunk was
    c.patches.update({p: "Composite" for p in d if c.cells.get(p) != "/"})
    return rk.finish(c)


CONCEPTS = [
    ("sierpinski", sierpinski, "Majority of One", "Battleship",
     "Three chevrons, each made of three chevrons: nine copies in two levels of recursion, the voids between them left open. "
     "Nine helms, one bridge; whatever it argues about, it wins the vote."),
    ("spiral", spiral, "Regress", "Cruiser",
     "A keel copy with two curls of its own copies, each a quarter turn on and smaller than the last, "
     "until the youngest are too small to wake. The keel copy has no engines; its children push."),
]
