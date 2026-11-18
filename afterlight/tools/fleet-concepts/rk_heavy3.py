"""Recursive Kingdom, round 3: the Parliament and the Hive, built from the seed.

Lore: one person copied themselves until the copies became a civilisation,
"one person, two billion times over". Every hull is crewed by the same person
at different ages, forever arguing with themselves.

These two take the recursion round in a circle instead of down a line:
  - PARLIAMENT (cruiser): eight copies in a ring, every nose aimed into one
    open chamber, the floor where the Kingdom argues with itself; the real
    guns point outward from the copies' backs;
  - HIVE (station): two rings of copies, every nose aimed at one core, the
    outer ring a closed wall round four courtyards.
Both are open frames round voids; copies touch or are welded where barbs
meet, never hung on necks. Mirror-symmetric skeletons (the hive four ways),
copy errors only after the mirror, Steel with salvaged patches.

Legend: see rk_seed.py (and draw.py). Seeds are painted with rk_seed.seed();
the welds and the hive's core are drawn here cell by cell.
"""
import rk_seed as rk
from rk_light import FACTION  # noqa: F401  (same faction: build.py reads it from here)

N4 = ((0, 1), (0, -1), (1, 0), (-1, 0))
LAUNCHERS = set("x*^")
GUNS = set("gnpz")


# --------------------------------------------------------------------------- helpers
def _hull(c):
    return {p for p, ch in c.cells.items() if ch != "/"}


def silence(c):
    """Clean-up once copies are packed together: a launcher without four
    clear cells outside it, or a gun buried on every side, becomes plain hull.
    Run it before the copy errors, so it stays symmetric."""
    hull = _hull(c)
    for (x, y), ch in sorted(c.cells.items()):
        if ch in LAUNCHERS:
            if not any((x + dx, y + dy) not in hull
                       and all((x + dx * k, y + dy * k) not in hull for k in range(1, 5))
                       for dx, dy in N4):
                c.cells[(x, y)] = "#"
        elif ch in GUNS and all((x + dx, y + dy) in hull for dx, dy in N4):
            c.cells[(x, y)] = "#"


def put(c, cells):
    """Write {(x, y): ch} (' ' deletes)."""
    for p, ch in cells.items():
        if ch == " ":
            c.cells.pop(p, None)
        else:
            c.cells[p] = ch


# --------------------------------------------------------------------------- parliament
# Eight copies in a ring round an open floor, the Kingdom's assembly. Bow +x.
#   SPEAKER  a large copy on the keel at the stern, nose to the floor. It is
#            the only copy whose barbs face aft, so it drives; its bridge has
#            moved up behind its nose, at the head of the floor.
#   BAR      a large copy on the keel at the bow, nose to the floor; its
#            engine mounts face forward and carry the bow battery.
#   BENCHES  three small copies a side, side by side, noses down at the
#            floor, welded where their barbs meet; their engine mounts face
#            outboard and carry the broadside cannons. The two at the stern
#            end bolt their engines onto the aft face of their outer barb.
# The rule the drawing keeps: every NOSE aims into the chamber at every other
# copy (eight nose guns), and every other gun points outward from a copy's
# BACK. The speaker and the bar are welded to the end benches at the corners,
# leaving a slit between each pair, so every copy's outline still reads.
PA = 10                 # speaker and bar tips at (-PA, 0) and (PA, 0)
PB = 8                  # bench tips at (x, +-PB)
BENCH_X = (-14, 0, 14)

# A bench copy (pointing at the floor): a cannon on each barb's back where its
# engines were, and no shoulder guns (they could only shoot along the bench).
BENCH = {(-11, 5): "n", (-11, -5): "n", (-11, 4): "#", (-11, -4): "#",
         (-4, 4): "#", (-4, -4): "#"}
# The speaker and the bar: their shoulder and flank guns would face the floor;
# only the nose aims in. Their own 2x3 bridge becomes barracks.
ELDER = {(-8, 8): "#", (-8, -8): "#", (-5, 5): "#", (-5, -5): "#",
         (-7, 1): "K", (-6, 1): "K", (-7, 0): ".", (-6, 0): ".", (-7, -1): "K", (-6, -1): "K"}


def parliament():
    """Cruiser. The Kingdom's assembly: eight copies of the seed in a ring,
    every nose aimed into one open chamber. Two large copies close the ends
    (the speaker at the stern, the only one whose engines face aft, and the
    bar at the bow); three small copies sit on each bench, noses down at the
    floor. Every copy aims at every other; the real guns point outward from
    their backs. The bridge sits behind the speaker's nose, at the head of
    the floor, the nearest the ship's middle a bridge can be."""
    c = rk.canvas("rk_parliament")
    rk.seed(c, (-PA, 0), "large", swap={                     # the speaker
        **ELDER,
        (-3, 1): "B", (-2, 1): "B", (-3, 0): "B", (-2, 0): "B", (-3, -1): "B", (-2, -1): "B",
    })
    bar = rk.seed(c, (PA, 0), "large", rot=2, swap={
        **ELDER, (-7, 0): "K", (-6, 0): "K",
        (-20, 9): "n", (-20, -9): "n", (-20, 10): "g", (-20, -10): "g",   # the bow battery
        (-20, 11): "/", (-20, -11): "/",                                   # plated prong tips
    })
    benches = []
    for x in BENCH_X:
        benches.append(rk.seed(c, (x, PB), "small", rot=3, swap=BENCH))
        benches.append(rk.seed(c, (x, -PB), "small", rot=1, flip=True, swap=BENCH))
    # the stern benches' outer barbs face aft: two engines each
    put(c, {(-20, 15): "E", (-20, 16): "E"})
    # corners: the speaker's and the bar's barbs welded to the end benches,
    # with a passage through each joint
    for s in (1, -1):
        put(c, {(s * 20, 10): ".", (s * 19, 10): ".", (s * 19, 11): ".", (s * 19, 12): ".",
                (s * 18, 12): ".",
                (s * 20, 11): "#", (s * 18, 10): "#", (s * 18, 11): "#", (s * 20, 12): "#",
                (s * 20, 13): "#"})
    # benches: neighbours welded at the ends of their barbs, a door between
    for gx in (-7, 7):
        put(c, {(gx, 19): "#", (gx, 17): "#", (gx, 18): "d",
                (gx - 1, 18): ".", (gx + 1, 18): "."})
    c.mirror()
    silence(c)
    # copy errors
    rk.flaw(c, bar, clip="nose", patch=("Composite", "port_barb"))   # the bar falls a cell short
    rk.flaw(c, benches[2], swap={(-7, 3): "m"})                       # a medbay for a bunk
    rk.flaw(c, benches[5], clip="stbd", patch=("Titanium", "stbd_barb"))
    rk.flaw(c, benches[1], patch=("Titanium", "nose"))
    return rk.finish(c)


# --------------------------------------------------------------------------- hive
# Two rings of copies round a core, every nose pointed in. Drawn for the east
# side (copies pointing west, -x) and placed four ways; every copy off an
# axis has its mirror twin, so the station is symmetric about both axes and
# both diagonals (which is why rk_seed.finish()'s port/starboard checks fit).
#   CORE        an octagon: the docking hub in the middle, the brood round it;
#   INNER RING  four small copies, noses welded into the core's four faces;
#   OUTER RING  a closed wall of twelve copies: on each axis a small copy
#               riding nose-first in an inner copy's notch (one keel runs from
#               the hub out through both to a berth in its notch), and at each
#               corner a pair of xs copies, noses aimed into a courtyard. The
#               axis copies stand proud of the corners, so the wall is round.
HI = 7                  # inner tips at (HI, 0)
HO = 16                 # outer axis tips at (HO, 0): three cells into the inner notch
HC = (14, 12)           # corner copy tip (and its twin across the diagonal)


def _turn(p, k):
    x, y = p
    for _ in range(k % 4):
        x, y = -y, x
    return x, y


def d4(cells):
    """{(x, y): ch} -> the same cells at all eight symmetric places."""
    out = {}
    for (x, y), ch in cells.items():
        for p in ((x, y), (x, -y), (y, x), (y, -x)):
            for k in range(4):
                out[_turn(p, k)] = ch
    return out


def ring(c, size, at, **kw):
    """Paint a copy pointing west (-x) with its tip at `at`, and its three
    turns; off the axis, also its twin at (x, -y). A station has no engines.
    Returns the copies."""
    x, y = at
    out = []
    for k in range(4):
        out.append(rk.seed(c, _turn((x, y), k), size, rot=2 + k, engines=False, **kw))
        if y:
            out.append(rk.seed(c, _turn((x, -y), k), size, rot=2 + k, flip=True, engines=False, **kw))
    return out


# The core, one eighth of it (x >= y >= 0), placed eight ways by d4(): an
# octagon (|x|, |y| <= 6, |x| + |y| <= 8) with the docking hub in the middle,
# a ring corridor round it, a corridor out along each axis to an inner copy's
# keel, and between the corridors the brood: cryo vats where the Kingdom grows
# its new copies, with their reactor, air and memory.
CORE = {
    (0, 0): "h", (1, 0): "h", (1, 1): "h",
    (2, 0): ".", (2, 1): ".", (2, 2): ".",
    (3, 0): ".", (4, 0): ".", (5, 0): ".", (6, 0): ".",
    (3, 1): "O", (4, 1): "R", (5, 1): "V", (6, 1): "b",
    (3, 2): ".", (4, 2): "V", (5, 2): "M", (6, 2): "#",
    (3, 3): ".", (4, 3): "V", (5, 3): "#",
    (4, 4): "p",
    (7, 1): "#",                                  # the neck into an inner copy's nose
}


def hive():
    """Station. The Kingdom's hive: two rings of copies, every nose pointed
    at one core. Four copies make the inner ring, noses welded into the core.
    Twelve more make a closed outer wall: on each axis a copy rides
    nose-first in an inner copy's notch, and at each corner a pair of the
    smallest copies aim across a courtyard at the inner ring. The core holds
    the docking hub and the brood, the vats where new copies are grown; the
    bridge is beside the hub. Visitors berth in the notches of the outer
    copies, whose engine rooms became holds. Light guns on the outer copies'
    backs, point defence on the corner noses and the core."""
    c = rk.canvas("rk_hive")
    inner = ring(c, "small", (HI, 0), nose=False, swap={
        (-5, 0): ".", (-6, 0): ".",                    # the keel runs on, out through the notch
        (-4, 4): "#", (-4, -4): "#", (-9, 6): "#", (-9, -6): "#",   # no guns into the courtyards
    })
    outer = ring(c, "small", (HO, 0), nose=False, swap={
        (-5, 0): ".", (-6, 0): "o",                    # a berth in the notch
        (-11, 5): "g", (-11, -5): "g", (-11, 4): "#", (-11, -4): "#",   # guns on its back
        (-10, 4): "k", (-9, 4): "k", (-8, 4): "k",     # the engine rooms are holds
        (-10, -4): "k", (-9, -4): "k", (-8, -4): "k",
        (-4, 4): "#", (-4, -4): "#", (-9, 6): "#", (-9, -6): "#",
    })
    corner = ring(c, "xs", HC, swap={
        (0, 0): "p", (-3, 3): "#", (-3, -3): "#",     # point defence on the nose
        (-7, 5): "#", (-7, -5): "#",
        (-5, 0): "o",                                  # a berth in its notch
    })
    put(c, d4(CORE))
    # an inner copy's keel runs on into the outer copy's nose
    put(c, d4({(HO - 3, 0): ".", (HO - 2, 0): ".", (HO - 1, 0): ".",
               (HO - 1, 1): "#", (HO, 1): "#"}))
    # each corner copy sits on the axis copy beside it, a door between
    put(c, d4({(21, 6): "#", (22, 6): ".", (22, 7): "d"}))
    # the corner pairs are welded where their barbs meet, closing the wall
    put(c, d4({(16, 16): "#", (17, 16): "#", (17, 17): "#", (18, 17): "#"}))
    silence(c)
    # the one bridge, beside the hub
    put(c, {(3, 1): "B", (4, 1): "B", (5, 1): "B", (3, 2): "B", (4, 2): "B", (5, 2): "B"})
    # copy errors
    rk.flaw(c, corner[5], clip="port")                                # a barb cut short
    rk.flaw(c, outer[2], swap={(-7, 3): "m"})                         # a medbay for a bunk
    rk.flaw(c, inner[3], patch=("Titanium", "port_arm"))              # salvaged plate
    rk.flaw(c, corner[2], patch=("Composite", "nose"))
    return rk.finish(c)


CONCEPTS = [
    ("parliament", parliament, "Parliament", "Cruiser",
     "Eight copies in a ring, every nose aimed into one open chamber: the Kingdom arguing with itself. "
     "The real guns point outward from their backs."),
    ("hive", hive, "Hive", "Station",
     "Two rings of copies with every nose turned on one core: four nosed into it, twelve more in a closed "
     "wall round four courtyards. Ships berth in the copies' notches; the brood grows in vats round the hub."),
]
