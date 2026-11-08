"""Recursive Kingdom: one person, cloned into a civilisation.

Lore: a single founder copied themselves until the copies became a kingdom.
Every hull is crewed by the same person at different ages, forever arguing
with themselves. The original is still alive somewhere, hiding, and disgusted
by what they became. In game they are the faction nearest the start: raid
waves of 3-5 small hulls that attack anything nearby, go kamikaze below 25%
hull and pick over the wrecks afterwards.

Grammar, held at every size (all of it reads from above):
  - THE POD is the clone's head, and every pod has the same face: a domed
    stern stepped round its engines, and a MOUTH at the bow, a notch between
    two tusk-like jaws with a gun at the back of the throat. Same person,
    same face, at every age.
  - Four ages of pod, drawn once each by hand: the YOUNG pod (small, sleek,
    one gatling in its mouth), the ADULT (the workhorse, rockets in the
    mouth, a gun on the crown), the OLD pod (heavier, lopsided, a broken
    tusk, a bolted-on gun blister and patches of salvaged Titanium and
    Composite plate) and the ELDER (a huge head with a wide mouth full of
    teeth, the only pod with a bridge).
  - Ships are pods GRAFTED together, never laid on a grid: staggered flocks,
    chevrons, chains and pinwheels, joined by shaped necks: diagonal
    staircase tubes, doubled tubes and bare struts. Children ride on the
    backs of the old.
  - The pods do not agree. Some face each other, mouth to mouth, guns aimed
    across a gap.
  - Steel (with salvaged patches); cluster rockets and gatlings, salvage
    arms on the scavengers.

Legend: see draw.py. * cluster rocket, g gatling, X salvage arm, o docking port.
"""
from collections import deque

from draw import Canvas

FACTION = {
    "slug": "recursive_kingdom",
    "name": "Recursive Kingdom",
    "tagline": "One person cloned into a whole civilisation, forever arguing with themselves at different ages. The original is alive, hiding, and disgusted.",
    "grammar": [
        "Every pod is the same head: a domed, stepped stern and a mouth at the bow, a notch between two tusks with a gun in the throat",
        "Four ages of that head: sleek young, workhorse adult, lopsided old (broken tusk, bolted-on blister, salvaged patches), huge elder",
        "Pods grafted into real shapes: chevron flocks, staggered chains, pinwheels, joined by diagonal, doubled or bare-strut necks",
        "Children ride on the backs of the old; some pods face each other mouth to mouth, guns aimed across the gap",
        "Steel with salvaged Titanium and Composite patches; cluster rockets, gatlings, salvage arms",
    ],
    "accent": "#e08a3c",
}

N4 = ((0, 1), (0, -1), (1, 0), (-1, 0))
N8 = N4 + ((1, 1), (1, -1), (-1, 1), (-1, -1))

# --------------------------------------------------------------------------- the pods
# Each pod is drawn whole, bow to the right, the middle row is y = 0 and
# column 0 is x = 0. Same face at every age: the stern steps round the
# engines like a dome, the bow opens into a mouth between two tusks.

YOUNG = """
   #####/
  #M.Q.a##/
 E.R.....H###
E..b.c...g
 E.R.....T###
  #O.Q.a##/
   #####/
"""

ADULT = """
      ##g##/
    ##M.Q..a####/
  E#R.c..O..s.####
 E..R....H...g
E......d.....*
 E..R....T...g
  E#R.c..O..b.####
    ##M.Q..a####/
      ##g##/
"""

# Old: the adult grown heavy and lopsided. A crown bump on top, the upper
# tusk long and re-plated, the lower one snapped off flush, a gun blister
# bolted under the belly, and the hull patched (see OLD_PATCHES).
OLD = """
        ##g##/
      ###a.M.####/
    E#R.c..O..s.Q######/
  E...f.R....H......#####
 E..b..c...........g
E..........d..M....*
 E..b..c...........g
  E..f..R....T...###
   E#R.c..Q..b.##
     ###.##g###/
      #..a#
     *..Q.g
      #*##
"""
OLD_PATCHES = [
    ((8, 12, 4, 5), "Titanium"),        # a slab of someone else's plate on the crown
    ((17, 24, 2, 3), "Composite"),      # the re-plated upper tusk
    ((5, 11, -7, -5), "Titanium"),      # the bolted-on blister
    ((15, 19, -3, -2), "Composite"),    # where the lower tusk snapped
]


def parse(art, mid=None):
    """Template art to cells; row `mid` (default the middle row) is y = 0."""
    lines = [l for l in art.strip("\n").split("\n")]
    mid = len(lines) // 2 if mid is None else mid
    return {(x, mid - r): ch for r, l in enumerate(lines) for x, ch in enumerate(l) if ch != " "}


def half(rows):
    """A symmetric pod from its upper half: rows (start column, cells) from the
    top row down to the centre row, reflected below."""
    out = {}
    n = len(rows) - 1
    for k, (x0, s) in enumerate(rows):
        for i, ch in enumerate(s):
            if ch != " ":
                out[(x0 + i, n - k)] = ch
                out[(x0 + i, k - n)] = ch
    return out


# Maw: an adult grown into a scavenger. Its jaws have stretched into long
# plated mandibles with a crawlway down each, salvage arms at the tips, along
# the inside of the jaws and down the throat.
MAW = half([
    (7, "##g##/"),
    (5, "##M.Q..a#############/"),
    (3, "E#R.c.sO...............X"),
    (2, "E..R....H..##X###X###X##/"),
    (1, "E.....c.....X"),
    (0, "E......d.....X"),
])


# Elder: the head grown huge. A wide mouth with five teeth in a V-shaped
# throat, long tusks, the bridge (the only one in the kingdom's pods) in the
# middle of the skull, barracks and a galley for the children it carries.
ELDER = half([
    (13, "##g###g##/"),
    (10, "####.Q.O.Q.M####/"),
    (7,  "###f..R.c..y...s...a####/"),
    (5,  "E#R..b...KK..GG.....A...##########/"),
    (4,  "E..R.c..d.....GG..Q..T.....s.##########"),
    (3,  "E..b..f..LL..m...........H........g"),
    (2,  "E..R..c..............BB......a....*"),
    (1,  "E.........d..M........BB..d.......g"),
])
for _k, _ch in {(21, -6): "#", (14, -6): "M", (19, -2): "T", (29, -1): "s",
                (28, 3): "a", (28, -3): "O"}.items():
    ELDER[_k] = _ch
ELDER_PATCHES = [
    ((15, 22, 6, 7), "Titanium"),
    ((24, 33, -5, -4), "Composite"),
    ((3, 6, -3, -1), "Titanium"),
    ((30, 38, 3, 4), "Composite"),
]


POD = {"young": parse(YOUNG), "adult": parse(ADULT), "old": parse(OLD, mid=5), "elder": ELDER, "maw": MAW}
PATCHES = {"old": OLD_PATCHES, "elder": ELDER_PATCHES}


def _xf(p, rot, flip):
    x, y = p
    if flip:
        y = -y
    for _ in range(rot % 4):
        x, y = -y, x
    return x, y


class Pod:
    def __init__(self, at, rot, flip):
        self.at, self.rot, self.flip = at, rot, flip

    def w(self, x, y):
        dx, dy = _xf((x, y), self.rot, self.flip)
        return self.at[0] + dx, self.at[1] + dy


def stamp(c, kind, at, rot=0, flip=False, engines=None, swap=None):
    """Stamp a pod. Engines only survive on a pod facing the bow (rot 0) and
    only if engines isn't False; otherwise they become plain hull. swap maps
    local cells to replacement characters (per-pod variations)."""
    p = Pod(at, rot, flip)
    keep_e = (rot % 4 == 0) if engines is None else engines
    cells = dict(POD[kind])
    for k, ch in (swap or {}).items():
        if ch == " ":
            cells.pop(k, None)
        else:
            cells[k] = ch
    for (x, y), ch in cells.items():
        if ch == "E" and not keep_e:
            ch = "#"
        c.cells[p.w(x, y)] = ch
    for (x0, x1, y0, y1), mat in PATCHES.get(kind, []):
        for x in range(x0, x1 + 1):
            for y in range(y0, y1 + 1):
                if (x, y) in cells:
                    c.patches[p.w(x, y)] = mat
    return p


def _line(a, b):
    """Grid path from a to b: straight runs and 45-degree staircases."""
    (x, y), out = a, [a]
    while (x, y) != b:
        dx = (b[0] > x) - (b[0] < x)
        dy = (b[1] > y) - (b[1] < y)
        if dx and dy:                    # staircase: step x, then y
            x += dx
            out.append((x, y))
            y += dy
        else:
            x, y = x + dx, y + dy
        out.append((x, y))
    return out


def tube(c, *pts, ch="."):
    """A walkable neck through waypoints: deck down the middle, hull round
    every deck cell. Existing cells are only overwritten on the path."""
    path = []
    for a, b in zip(pts, pts[1:]):
        path += _line(a, b)
    for p in path:
        c.cells[p] = ch
    for x, y in path:
        for dx, dy in N8:
            n = (x + dx, y + dy)
            if c.cells.get(n, "/") == "/":
                c.cells[n] = "#"
    return path


def strut(c, *pts, w=1):
    """A bare structural strut: hull only, no deck, w cells either side."""
    for a, b in zip(pts, pts[1:]):
        for x, y in _line(a, b):
            for dx in range(-w + 1, w):
                for dy in range(-w + 1, w):
                    c.cells.setdefault((x + dx, y + dy), "#")


def _prune_plates(c):
    """Drop any plate that grafting has left out of a corner of hull."""
    hull = {p for p, ch in c.cells.items() if ch != "/"}
    for (x, y), ch in list(c.cells.items()):
        if ch != "/":
            continue
        n, s, w, e = ((x, y + 1) in hull, (x, y - 1) in hull, (x - 1, y) in hull, (x + 1, y) in hull)
        if (n + s + w + e) != 2 or (n and s) or (w and e):
            del c.cells[(x, y)]


def finish(c):
    """Build; apply salvaged patches; check the hull is one piece and no
    engine vents into its own hull."""
    _prune_plates(c)
    d = c.build()
    hull = {tuple(h["grid_pos"]) for h in d["hull_cells"]}
    for h in d["hull_cells"]:
        mat = c.patches.get(tuple(h["grid_pos"]))
        if mat:
            h["material"] = mat
    start = next(iter(hull))
    seen, q = {start}, deque([start])
    while q:
        x, y = q.popleft()
        for dx, dy in N4:
            n = (x + dx, y + dy)
            if n in hull and n not in seen:
                seen.add(n)
                q.append(n)
    if len(seen) != len(hull):
        d["_checks"].append(f"hull in pieces: {len(hull) - len(seen)} cells cut off from {start}")
    for (x, y), ch in c.cells.items():
        if ch in "EN" and any((xx, y) in hull for xx in range(x - 80, x)):
            d["_checks"].append(f"engine at {(x, y)} vents into hull")
    return d


def canvas(name):
    c = Canvas(name, "Steel")
    c.patches = {}
    return c


# --------------------------------------------------------------------------- ships
def skiff():
    """Raider. One young pod with a pair of swept-back wings off its
    shoulders, a memory core and a drop tank in each wing for the kamikaze
    run, and a rocket on each cheek just forward of the wing root."""
    c = canvas("rk_skiff")
    stamp(c, "young", (0, 0), swap={(6, 3): "*", (6, -3): "*"})
    for s in (1, -1):
        for y, x0, x1 in ((4, 0, 5), (5, -2, 2), (6, -4, 0), (7, -5, -2)):
            c.R(x0, x1, y * s, y * s)
        c.cells[(-1, 5 * s)] = "M"
        c.cells[(-3, 6 * s)] = "f"
        c.cells[(-1, 7 * s)] = "/"
    return finish(c)


def twin():
    """An old pod with a young one riding on its shoulders, grafted on by a
    short neck. Two helms, one argument about where to go."""
    c = canvas("rk_twin")
    old = stamp(c, "old", (0, 0), swap={(10, 5): "#"})
    kid = stamp(c, "young", (6, 10))
    tube(c, old.w(12, 4), kid.w(6, -2))
    return finish(c)


def brood_raft():
    """Corvette. A chevron flock: an adult leads, and two young selves climb
    back up each flank, each hanging off the shoulder of the one ahead on a
    short swept neck, so the leading edge is a staircase of open mouths. A
    bare rib from each child's jaw to its parent's tusk closes a small open
    triangle under every neck."""
    c = canvas("rk_brood_raft")
    lead = stamp(c, "adult", (0, 0))
    for s in (1, -1):
        f = s < 0
        k1 = stamp(c, "young", (-2, 10 * s), flip=f)
        k2 = stamp(c, "young", (-9, 19 * s), flip=f)
        tube(c, lead.w(5, 2 * s), lead.w(5, 4 * s), k1.w(4, -3), k1.w(4, -2))
        tube(c, k1.w(2, 1), k1.w(2, 3), k2.w(6, -3), k2.w(6, -2))
        strut(c, k1.w(11, -2), lead.w(14, 4 * s))
        strut(c, k2.w(11, -2), k1.w(9, 3))
    return finish(c)


def recycler():
    """Scavenger. A chain of heads, each biting the stern of the one ahead:
    an old pod has a young one by the tail, and the young one has the maw,
    an adult whose jaws have grown into long plated mandibles lined with
    salvage arms. A child rides on the old one's back facing aft, watching
    the way they came. It parks its mouth over a wreck and eats."""
    c = canvas("rk_recycler")
    old = stamp(c, "old", (0, 0), swap={(19, 1): "#", (19, 0): ".", (19, -1): "#", (10, 5): "#"})
    rider = stamp(c, "young", (16, 10), rot=2)
    tube(c, old.w(12, 4), rider.w(4, 2))
    stamp(c, "young", (20, 0), swap={(0, 0): ".", (1, 1): "#", (1, -1): "#", (9, 0): "."})
    c.cells[(20, 1)] = c.cells[(20, -1)] = "#"
    stamp(c, "maw", (30, 0), swap={(0, 0): ".", (1, 1): "#", (1, -1): "#", (2, 2): "#",
                                   (2, -2): "#", (3, 3): "#", (3, -3): "#"})
    return finish(c)


def argument():
    """Experimental. Two factions of the same self in one hull. On the left
    an old pod and the child under its chin, which drive; on the right an
    adult and its own child, turned round, which refuse to. Parent faces
    parent and child faces child across an open gap, every throat gun aimed
    at the other side. A single yoke over the top holds them together,
    because neither will let go."""
    c = canvas("rk_argument")
    old = stamp(c, "old", (0, 0))
    kid = stamp(c, "young", (10, -11), flip=True)
    tube(c, old.w(16, -2), kid.w(6, -3), kid.w(6, -2))
    rival = stamp(c, "adult", (44, 0), rot=2, swap={(8, -4): "#"})
    rkid = stamp(c, "young", (40, -11), rot=2, flip=True)
    tube(c, rival.w(10, 3), rkid.w(6, -3), rkid.w(6, -2))
    tube(c, old.w(12, 4), (12, 5), (22, 15), (27, 15), (37, 5), rival.w(7, -3))
    return finish(c)


def elder_carrier():
    """Carrier. An elder with its children riding on it: young pods and an
    adult stand up off its back like spines and hang off its belly, each
    grafted on by its stern and ready to drop, with open bays between every
    spine. The elder's skull is patched with whatever it has salvaged."""
    c = canvas("rk_elder_carrier")
    el = stamp(c, "elder", (0, 0), swap={(15, 7): "#", (19, 7): "#", (15, -7): "#", (19, -7): "#"})
    # (deck in the elder, where the stern of the child sits, kind)
    top = [((6, 3), (6, 6), "young"), ((18, 6), (18, 8), "adult"), ((30, 3), (30, 6), "young")]
    bot = [((x, -y), (sx, -sy), kind) for (x, y), (sx, sy), kind in top]
    for row, r in ((top, 1), (bot, 3)):
        for deck, stern, kind in row:
            p = stamp(c, kind, stern, rot=r)
            tube(c, deck, p.w(1, 0))
    return finish(c)


def _turn(p, k):
    x, y = p
    for _ in range(k % 4):
        x, y = -y, x
    return x, y


def hive():
    """Station. A nest of heads round a brood chamber: four adults look out
    along the axes, and between each pair a young pod is turned to look the
    way the nest turns, so the whole thing reads as a slow pinwheel. The
    chamber is pierced by four open wells ringed with the cryo vats where new
    copies are grown; the bridge sits in the middle of them."""
    c = canvas("rk_hive")
    c.rows(1, 8, """
#####
.....#
.Q.M.V#
....V..#
.O.V  s.#
...V  .R.#
....y.....#
.s......b.
.........
""")
    c.rot4()
    c.rows(-2, 2, """
.....
.BB.H
.BB..
.BB..
.....
""")
    for k in range(4):
        a = stamp(c, "adult", _turn((10, 0), k), rot=k, engines=False)
        tube(c, _turn((6, 0), k), a.w(1, 0))
        y = stamp(c, "young", _turn((14, 9), k), rot=k + 1, engines=False)
        tube(c, _turn((7, 6), k), _turn((9, 8), k), y.w(1, 0))
    return finish(c)


CONCEPTS = [
    ("skiff", skiff, "Skiff", "Raider (waves of 3-5)",
     "A young pod with swept-back wings off its shoulders, a rocket on each cheek and a drop tank in each wing. They come in waves and dive into you below a quarter hull."),
    ("twin", twin, "Twin", "Scout",
     "An old pod, patched and lopsided, with a young one riding on its shoulders. Two helms, one argument about where to go."),
    ("brood_raft", brood_raft, "Brood Raft", "Corvette",
     "A chevron flock: an adult leads and four younger selves climb back up its flanks on swept necks, so the leading edge is a staircase of open mouths."),
    ("recycler", recycler, "Recycler", "Scavenger",
     "A chain of heads each biting the stern of the next, ending in a maw whose jaws have grown into long plated mandibles lined with salvage arms. A child rides the old one's back facing aft. It parks its mouth over a wreck and eats."),
    ("argument", argument, "The Argument", "Experimental",
     "Two factions of the same self in one hull: an old pod and its child drive, an adult and its child turned round refuse to. Parent faces parent and child faces child, every throat gun aimed across the gap; one yoke holds them together."),
    ("elder_carrier", elder_carrier, "Elder Carrier", "Carrier",
     "A huge, patched elder with its children standing off its back and hanging off its belly like spines, each grafted on by its stern and ready to drop, open bays between them."),
    ("hive", hive, "Hive", "Station",
     "A nest of heads round a brood chamber: adults look out along the axes, young pods between them turned the way the nest turns. Four open wells in the chamber hold the vats where new copies are grown."),
]


# All seven sent back as "to basic"; redrawn since. Every ship is now the same
# clone head at different ages: young, adult, old (patched), elder.
_AGES = "Redrawn from one clone head at four ages (young, adult, patched old, elder)."
REVISED = {
    "skiff": _AGES + " A young head with swept wings.",
    "twin": _AGES + " A young head riding an old one's shoulders.",
    "brood_raft": _AGES + " A chevron of children climbing an adult.",
    "recycler": _AGES + " A chain of heads, each biting the one ahead, ending in a salvage maw.",
    "argument": _AGES + " Parent faces parent and child faces child, guns mouth to mouth.",
    "elder_carrier": _AGES + " An elder with its children grafted on like spines.",
    "hive": _AGES + " A brood chamber with cryo wells, heads looking out.",
}
