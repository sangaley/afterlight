"""Synthesis Collective, round 5b: the great bodies, two experiments and three
traders. Hand-drawn, top-down only.

Lore: one mind in six hundred million bodies. Their ships are grown as much
as built and regrow themselves; their trade is grafts and living weapons.
"The screaming in the networks is just data corruption."

The approved grammar (synthesis.py) is kept, under the developer's symmetry
rule: every skeleton (outline, engines, main guns) is grown on y >= 0 and
mirrored, so these read as bilateral animals rather than lopsided tumours.
What stays from the grammar: bulbous lobes of rounded staircases, tendrils
three cells thick (rim, vein, rim) with the guns and the only plates on their
tips, lobes joined round open voids, veins of deck through every lobe, repair
bays everywhere. The "a little wrong" lives in small deliberate details added
after the mirror: a graft that took on one side only, an extra organ, a
Titanium plate off something they ate.

Legend (draw.py): # hull  . deck  / plate  E engine  R reactor  L large reactor
F fusion  b battery  f fuel  c cooling  B bridge  H helm  M memory core
Q quarters  K barracks  G galley  S surgery  m medbay  O oxygen  V cryo
P plasma  i ion  t tractor  X salvage arm  T targeting  y repair
% adv. repair  Z creature containment  Y specimen vault  @ AI combat core
! research lab  w wellness hub  D drone bay  = signal jammer  J signal buoy
U large cargo  k small cargo  h docking hub  d door
"""
from collections import deque

from draw import Canvas
from synthesis import FACTION  # noqa: F401  (same faction, same page)

N4 = ((0, 1), (0, -1), (1, 0), (-1, 0))
GUNS = set("Pit")
_last = {}


def finish(c, centre=0.5, graft=()):
    """Build, then check what draw.py doesn't: the hull is one piece, the
    skeleton is mirror-symmetric (outline, engines, guns) apart from the
    cells named in `graft`, no engine fires into its own hull, and the bridge
    is inside the hull, near the middle of its length."""
    d = c.build()
    _last[c.name] = dict(c.cells)
    hull = {tuple(h["grid_pos"]) for h in d["hull_cells"]}
    layer = {tuple(h["grid_pos"]): h["layer"] for h in d["hull_cells"]}
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
    graft = set(graft)
    body = hull - graft
    if {(x, -y) for x, y in body} != body:
        d["_checks"].append("outline not mirror-symmetric")
    xs = [x for x, _ in hull]
    lo, hi = min(xs), max(xs)
    bridge = []
    for (x, y), ch in c.cells.items():
        if ch in "EN" and any((xx, y) in hull for xx in range(x - 120, x)):
            d["_checks"].append(f"engine at {(x, y)} vents into hull")
        if (ch in "EN" or ch in GUNS) and c.cells.get((x, -y)) != ch and (x, y) not in graft:
            d["_checks"].append(f"'{ch}' at {(x, y)} has no mirror twin")
        if ch == "B":
            bridge.append(x)
            if layer.get((x, y)) == "Outer":
                d["_checks"].append(f"bridge on the rim at {(x, y)}")
        if ch == "/" and (x, y) not in graft and c.cells.get((x, -y)) != "/":
            d["_checks"].append(f"plate at {(x, y)} has no mirror twin")
    if not bridge:
        d["_checks"].append("no bridge")
    else:
        t = (sum(bridge) / len(bridge) - lo) / max(1, hi - lo)
        if abs(t - centre) > 0.2:
            d["_checks"].append(f"bridge not in the middle (at {t:.2f} of the length)")
    return d


def grow(name, x0, top, half, after=None, material="Composite", **kw):
    """Paint the y >= 0 half (its last line is the centreline, y = 0), mirror
    it, then let `after` add the one-sided details."""
    c = Canvas(name, material)
    c.rows(x0, top, half)
    c.mirror()
    if after:
        after(c)
    return finish(c, **kw)


# --------------------------------------------------------------------------- growing
def lobe(c, cx, cy, rx, ry=None, ch="#"):
    """A rounded lobe: every cell inside the ellipse. Interior cells left as
    '#' are deck once built (only rim cells are walls)."""
    ry = ry or rx
    for x in range(int(cx - rx) - 1, int(cx + rx) + 2):
        for y in range(int(cy - ry) - 1, int(cy + ry) + 2):
            if ((x - cx) / rx) ** 2 + ((y - cy) / ry) ** 2 <= 1.0:
                c.cells[(x, y)] = ch


def hollow(c, cx, cy, rx, ry=None):
    """Cut a rounded void (a vacuole) out of what is there."""
    ry = ry or rx
    for x in range(int(cx - rx) - 1, int(cx + rx) + 2):
        for y in range(int(cy - ry) - 1, int(cy + ry) + 2):
            if ((x - cx) / rx) ** 2 + ((y - cy) / ry) ** 2 <= 1.0:
                c.cells.pop((x, y), None)


def bud(c, cx, cy, rx, ry=None):
    """A small growth added after the mirror, on one side only: fills only
    empty cells and returns them (finish() takes them as the graft)."""
    ry = ry or rx
    new = set()
    for x in range(int(cx - rx) - 1, int(cx + rx) + 2):
        for y in range(int(cy - ry) - 1, int(cy + ry) + 2):
            if ((x - cx) / rx) ** 2 + ((y - cy) / ry) ** 2 <= 1.0 and (x, y) not in c.cells:
                c.cells[(x, y)] = "#"
                new.add((x, y))
    return new


def _steps(pts):
    """A 4-connected staircase through the waypoints, alternating x and y
    steps on the diagonals so the vein stays one cell wide."""
    out = [pts[0]]
    for (bx, by) in pts[1:]:
        x, y = out[-1]
        while (x, y) != (bx, by):
            dx, dy = bx - x, by - y
            if dx and (not dy or abs(dx) >= abs(dy) or len(out) % 2):
                x += 1 if dx > 0 else -1
            else:
                y += 1 if dy > 0 else -1
            out.append((x, y))
    return out


def tendril(c, pts, tip=None, plates=True):
    """A tendril three cells thick: a vein of deck down the waypoints with a
    rim of hull round it. `tip` puts that gun one step past the vein's end,
    flanked by plates (the grammar's only armour)."""
    vein = _steps(pts)
    vs = set(vein)
    added = set()
    for (x, y) in vein:
        for dx in (-1, 0, 1):
            for dy in (-1, 0, 1):
                n = (x + dx, y + dy)
                if n not in c.cells:
                    c.cells[n] = "#"
                    added.add(n)
    for n in added:      # bevel: a rim cell only touching the vein by a corner goes
        if n not in vs and not any((n[0] + dx, n[1] + dy) in vs for dx, dy in N4):
            del c.cells[n]
    for v in vein:
        if c.cells.get(v) == "#":
            c.cells[v] = "."
    if tip:
        (ax, ay), (bx, by) = vein[-2], vein[-1]
        dx, dy = bx - ax, by - ay
        t = (bx + dx, by + dy)
        c.cells[t] = tip
        if plates:
            for px, py in ((dy, dx), (-dy, -dx)):
                if (t[0] + px, t[1] + py) not in c.cells or c.cells[(t[0] + px, t[1] + py)] == "#":
                    c.cells[(t[0] + px, t[1] + py)] = "/"
    return vein


def show(c, y0=0):
    """Print the drawing from y0 up (scratch use while drawing)."""
    xs = [x for x, _ in c.cells]
    ys = [y for _, y in c.cells]
    lo = min(xs)
    print("     " + "".join(str(abs(x) % 10) for x in range(lo, max(xs) + 1)), f"(x0={lo})")
    for y in range(max(ys), y0 - 1, -1):
        print(f"{y:4d} " + "".join(c.cells.get((x, y), " ") for x in range(lo, max(xs) + 1)))


def _run(x0, x1, base=".", at=None):
    """A row of cells x0..x1, all `base`, with the cells in `at` ({x: ch})."""
    at = at or {}
    return "".join(at.get(x, base) for x in range(x0, x1 + 1))


# --------------------------------------------------------------------------- warships
def sinew():
    """Cruiser. A grown centipede: five lobed segments down one spine, biggest
    amidships, and four pairs of sinew limbs. Three pairs reach up and
    forward like oars with plasma on their plated tips; the fourth curls in
    front of the head as mandibles. Engines in the tail segment, crew in the
    next, the bridge and the mind's combat cores in the third, fusion and
    regrowth in the fourth, a plasma battery in the head."""
    c = Canvas("sy_sinew", "Composite")
    for cx, rx, ry in ((-28, 4.6, 5.2), (-18, 5.0, 6.4), (-6, 5.8, 7.6), (6, 5.6, 7.2), (17, 4.8, 5.8)):
        lobe(c, cx, 0, rx, ry)
    c.R(-31, 21, 0, 2)
    tendril(c, [(-18, 6), (-17, 8), (-15, 11), (-11, 13), (-7, 14)], tip="P")
    tendril(c, [(-6, 8), (-5, 10), (-2, 13), (2, 15), (6, 16)], tip="P")
    tendril(c, [(6, 7), (8, 9), (11, 11), (15, 12), (19, 12)], tip="P")
    tendril(c, [(19, 5), (22, 6), (25, 5), (27, 4)], tip="P")
    for x, y, row in (
            # tail: engines and fusion
            (-29, 5, "#y#"), (-30, 4, "#Q.Q#"), (-31, 3, "E.O.b.#"),
            (-32, 2, "E.Rc.M.d"), (-32, 1, "E.FFF."), (-32, 0, "E.FFF."),
            # crew segment
            (-21, 5, "#Q.y.Q#"), (-21, 4, "#.GG.O#"), (-22, 3, "#..GG.y.#"),
            (-22, 2, "#Q.KK..M#"),
            # the mind: bridge, combat cores, targeting, surgery
            (-8, 7, "#y.y#"), (-9, 6, "#.Q.Q.#"), (-10, 5, "#@@.T.@@#"),
            (-10, 4, "#y.M...y#"), (-11, 3, "#.SSS.T.T.#"), (-11, 2, "#.SSS.....#"),
            # fusion and regrowth
            (3, 6, "#y...y#"), (2, 5, "#%%.M.%%#"), (2, 4, "#.LL.LL.#"),
            (1, 3, "#y.c.b.c.y#"), (1, 2, "#.f.O.O.f.#"),
            # head: the plasma battery
            (15, 5, "#y.M."), (14, 4, "#..T..#"), (13, 3, "#.y...y.#"),
            (13, 2, "#y.O.m.yP")):
        c.P(x, y, row)
    c.P(-26, 1, _run(-26, 20, at={-24: "d", -12: "d", -7: "B", -6: "B", 0: "d",
                                  2: "F", 3: "F", 4: "F", 8: "F", 9: "F", 10: "F", 12: "d"}))
    c.P(-26, 0, _run(-26, 21, at={-25: "y", -21: "Q", -19: "y", -17: "Z", -16: "Z", -13: "y",
                                  -10: "@", -9: "@", -7: "B", -6: "B", -4: "H", -2: "y",
                                  2: "F", 3: "F", 4: "F", 6: "y", 8: "F", 9: "F", 10: "F",
                                  12: "y", 14: "Q", 16: "y", 18: "T", 21: "i"}))
    c.mirror()
    return finish(c)


def _sulcus(c, pts):
    """Cut a fold two cells wide into what is there."""
    for (x, y) in _steps(pts):
        c.cells.pop((x, y), None)
        c.cells.pop((x + 1, y), None)


def _rooms(c, rows):
    """{y: [(x0, x1, {x: ch}), ...]}: deck runs x0..x1 on row y with rooms at
    the given x, for interiors too big to type out whole."""
    for y, runs in rows.items():
        for x0, x1, at in runs:
            c.P(x0, y, _run(x0, x1, at=at))


def cortex():
    """Battleship. The mind's thinking organ, grown as a brain seen from
    above: two hemispheres folded by deep sulci, split by the fissure, joined
    only through the corpus callosum where the bridge sits; a ventricle
    void in each, the brainstem running aft through the fissure to a
    cerebellum full of engines and fusion. Ion disruptors crown every fold
    (it goes for your reactor); plasma on the frontal poles and on two
    olfactory tendrils forward."""
    c = Canvas("sy_cortex", "Composite")
    lobe(c, 1, 9.4, 20.5, 7.8)                      # the hemisphere
    lobe(c, -25, 0, 5.2, 8.2)                       # the cerebellum
    c.R(-27, 3, 0, 2)                               # brainstem and callosum
    for pts in ([(-15, 17), (-12, 14), (-10, 11)], [(-5, 19), (-4, 15), (-3, 13), (-3, 11)],
                [(5, 19), (5, 15), (7, 12), (7, 11)], [(14, 18), (15, 15), (13, 12), (12, 11)]):
        _sulcus(c, pts)
    hollow(c, 1, 7.5, 5.0, 1.6)                     # the ventricle
    tendril(c, [(18, 4), (21, 3), (24, 3), (26, 4)], tip="P")
    _rooms(c, {
        16: [(-1, 3, {-1: "y", 3: "y"})],
        15: [(-9, -6, {-9: "y"}), (-1, 3, {1: "M"}), (9, 11, {11: "y"})],
        14: [(-8, -6, {-8: "Q"}), (0, 4, {0: "Q", 4: "Q"}), (10, 11, {11: "Q"})],
        13: [(-15, -13, {-15: "y"}), (-7, -5, {-5: "y"}), (0, 5, {1: "!", 2: "!", 5: "y"}),
             (16, 17, {17: "y"})],
        12: [(-17, -12, {-17: "y", -14: "M"}), (-7, -5, {-7: "O"}), (0, 5, {0: "T", 5: "O"}),
             (16, 19, {17: "M", 19: "y"})],
        11: [(-18, -12, {-18: "Q", -16: "K", -15: "K", -13: "Q"}), (-7, -5, {-5: "Q"}),
             (0, 5, {2: "Y", 4: "y"}), (15, 20, {15: "Q", 20: "y"})],
        10: [(-18, -11, {-18: "y", -14: "V"}), (-8, -4, {-6: "y"}), (3, 6, {4: "y"}),
             (14, 20, {17: "V", 20: "y"})],
        9: [(-18, -4, {-18: "O", -16: "G", -15: "G", -13: "y", -10: "M", -7: "y", -5: "Y"}),
            (6, 20, {8: "Y", 10: "y", 12: "M", 14: "G", 15: "G", 17: "y", 19: "O"})],
        8: [(-18, -5, {-18: "Q", -16: "G", -15: "G", -12: "Y", -9: "@", -8: "@", -6: "T"}),
            (7, 20, {9: "T", 11: "@", 12: "@", 14: "G", 15: "G", 18: "Q", 20: "y"})],
        7: [(-17, -5, {-17: "y", -11: "y", -5: "V"}), (7, 19, {7: "V", 13: "y", 19: "y"}),
            (-26, -24, {-26: "Q", -24: "Q"})],
        6: [(-15, -4, {-15: "y", -13: "K", -12: "K", -10: "O", -8: "K", -7: "K", -5: "y"}),
            (6, 18, {6: "y", 8: "M", 11: "!", 12: "!", 15: "O", 17: "y"}),
            (-27, -23, {-27: "M", -23: "b"})],
        5: [(-13, -1, {-13: "y", -11: "Q", -9: "Q", -7: "m", -5: "Q", -3: "y"}),
            (3, 15, {3: "y", 5: "T", 7: "@", 8: "@", 10: "T", 12: "Q", 14: "y"}),
            (-28, -22, {-28: "R", -26: "c", -24: "c", -22: "R"})],
        4: [(-10, 12, {-10: "y", -7: "S", -6: "S", -5: "S", -3: "M", 0: "@", 1: "@",
                       3: "S", 4: "S", 5: "S", 7: "y", 9: "O", 11: "y"}),
            (-28, -22, {})],
        3: [(-9, 7, {-7: "S", -6: "S", -5: "S", -3: "!", -2: "!", 0: "!", 1: "!",
                     3: "S", 4: "S", 5: "S"}),
            (-28, -22, {-28: "R", -26: "L", -25: "L", -23: "R"})],
        2: [(-29, -21, {-27: "f", -23: "f", -21: "d"}), (-10, 3, {-9: "M", -6: "T", 0: "T", 3: "y"})],
        1: [(-29, 2, {-27: "F", -26: "F", -25: "F", -20: "d", -11: "d", -3: "B", -2: "B"})],
        0: [(-29, 2, {-27: "F", -26: "F", -25: "F", -22: "O", -20: "y", -18: "Z", -17: "Z",
                      -15: "y", -13: "Z", -12: "Z", -10: "y", -7: "@", -6: "@", -3: "B", -2: "B",
                      0: "H", 1: "T"})],
    })
    for x, y, ch in ((0, 17, "i"), (2, 17, "i"), (-8, 16, "i"), (-6, 16, "i"), (9, 16, "i"),
                     (10, 16, "i"), (-14, 14, "i"), (16, 14, "i"), (-18, 12, "i"), (21, 11, "i"),
                     (21, 9, "P"), (3, 0, "P"), (-17, 2, "y"), (-13, 2, "y"),
                     (-30, 0, "E"), (-30, 1, "E"), (-30, 2, "E"), (-29, 3, "E"), (-29, 4, "E")):
        c.P(x, y, ch)
    c.mirror()
    # a growth on the starboard occipital fold that the mind hasn't noticed
    return finish(c, graft=bud(c, -17, -14.5, 2.4, 2.2))


def gestalt():
    """Flagship. The mind's largest body, grown as what it is: a network.
    Ganglia of flesh (lobes) strung on sinew nerves three cells thick, the
    nerves running ganglion to ganglion like a ladder nervous system, so the
    whole ship is an open net with space showing through every mesh. The
    heart ganglion amidships holds the bridge; the mind's combat cores sit
    in the ganglion above it; fusion in the forward pair; engines in the
    aft core and the aft wings; plasma on the prow and on two mandibles
    that curl round it; ion on the outer ganglia. One aft wing ganglion
    on the port side is a Hegemony hull section it absorbed and hasn't
    finished digesting."""
    c = Canvas("sy_gestalt", "Composite")
    nodes = {"A": (-40, 0, 5.2, 6.0), "B": (-28, 11, 4.0, 3.8), "C": (-15, 4, 4.6, 4.2),
             "D": (-14, 18, 3.6, 3.4), "H": (0, 0, 7.4, 6.6), "E": (1, 14, 4.6, 4.2),
             "G": (15, 6, 4.6, 4.2), "I": (15, 19, 3.6, 3.4), "K": (28, 11, 4.0, 3.8),
             "J": (39, 0, 4.8, 4.4)}
    for x, y, rx, ry in nodes.values():
        lobe(c, x, y, rx, ry)
    for a, b, mid in (("A", "B", [(-37, 8)]), ("A", "C", [(-33, 4), (-27, 2), (-21, 4)]),
                      ("B", "D", [(-22, 18)]), ("C", "H", []), ("C", "E", [(-7, 10)]),
                      ("D", "E", [(-6, 18)]), ("H", "G", []), ("E", "I", [(8, 18)]),
                      ("G", "K", []), ("I", "K", [(23, 18)]),
                      ("G", "J", [(22, 3), (28, 4), (33, 2)]), ("K", "J", [(37, 8)])):
        pts = [nodes[a][:2]] + mid + [nodes[b][:2]]
        tendril(c, pts)
    tendril(c, [(30, 14), (35, 16), (40, 15), (44, 12), (45, 9)], tip="P")   # mandible
    _rooms(c, {
        # D, the aft crown ganglion: an ion battery
        20: [(-15, -13, {})], 19: [(-16, -12, {-16: "y", -12: "O"})],
        17: [(-16, -12, {-16: "Q", -12: "y"})], 16: [(-15, -13, {-15: "M"})],
        # I, the fore crown ganglion, and the top of E
        21: [(14, 16, {})], 18: [(-4, -1, {-2: "y"})],
    })
    _rooms(c, {
        20: [(13, 17, {13: "y", 17: "T"})],
        19: [(12, 19, {14: "Q", 18: "M"})],
        17: [(-3, 2, {-1: "y"}), (14, 16, {14: "y", 16: "O"})],
        16: [(-2, 4, {-2: "M", 1: "@", 2: "@", 4: "y"})],
        15: [(-2, 6, {-1: "!", 0: "!", 3: "T"})],
        14: [(-2, 6, {5: "M"})],
        13: [(-2, 5, {-2: "y", 0: "@", 1: "@", 3: "!", 4: "!"}),
             (-30, -25, {-29: "Q", -27: "Q"}), (26, 33, {27: "Q", 31: "y"})],
        12: [(-2, 4, {0: "Q", 2: "Q"}), (-30, -26, {-30: "G", -29: "G", -27: "m"}),
             (26, 33, {28: "M", 30: "y", 32: "Q"})],
        11: [(-3, 2, {-1: "y"}), (-31, -25, {-30: "G", -29: "G", -26: "K", -25: "K"}),
             (25, 33, {26: "y", 30: "O"})],
        10: [(-30, -26, {-28: "M"}), (26, 31, {27: "R", 30: "y"})],
        9: [(-31, -26, {-31: "y", -29: "O", -27: "Q"}), (14, 16, {15: "y"}), (26, 30, {28: "y"})],
        8: [(12, 18, {13: "%", 14: "%", 17: "y"})],
        7: [(-16, -14, {-16: "y", -14: "Q"}), (12, 19, {13: "F", 14: "F", 15: "F", 18: "R"})],
        6: [(-18, -10, {-17: "S", -16: "S", -15: "S", -12: "Z", -11: "Z"}),
            (12, 20, {13: "F", 14: "F", 15: "F", 17: "M"})],
        5: [(-18, -9, {-17: "S", -16: "S", -15: "S", -13: "M", -10: "y"}),
            (-3, 3, {-3: "y", -1: "M", 3: "y"}),
            (12, 19, {13: "F", 14: "F", 15: "F", 18: "c"}),
            (-38, -37, {})],
        4: [(-18, -9, {-18: "y", -15: "Y", -12: "Z", -11: "Z"}),
            (-8, 4, {-7: "@", -6: "@", -2: "!", -1: "!", 2: "T"}),
            (12, 18, {12: "y", 14: "b", 18: "y"}),
            (-42, -37, {-41: "R", -39: "O", -37: "y"})],
        3: [(-19, -11, {-18: "m", -16: "Q", -14: "Q", -12: "y"}),
            (-7, 5, {-6: "S", -5: "S", -4: "S", 2: "S", 3: "S", 4: "S"}),
            (13, 16, {14: "O"}), (38, 40, {38: "y"}),
            (-43, -37, {-42: "L", -41: "L", -39: "M"})],
        2: [(-20, -12, {-19: "V", -15: "V"}),
            (-6, 6, {-6: "S", -5: "S", -4: "S", 2: "S", 3: "S", 4: "S", 6: "y"}),
            (33, 42, {37: "T", 40: "M"}),
            (-43, -37, {-42: "f", -40: "c", -38: "f"})],
        1: [(-44, -36, {-43: "F", -42: "F", -41: "F", -39: "R"}),
            (-6, 7, {-5: "T", -1: "B", 0: "B", 4: "T"}),
            (35, 42, {38: "Q"})],
        0: [(-44, -33, {-43: "F", -42: "F", -41: "F", -39: "O", -37: "y"}),
            (-6, 8, {-5: "O", -3: "H", -1: "B", 0: "B", 2: "@", 3: "@", 7: "y"}),
            (36, 42, {37: "y", 39: "O"})],
    })
    for x, y, ch in ((-16, 20, "i"), (-14, 21, "i"), (-12, 20, "i"),
                     (13, 21, "i"), (15, 22, "i"), (17, 21, "i"),
                     (43, 0, "P"), (43, 1, "P"), (43, 2, "P"),
                     (-45, 0, "E"), (-45, 1, "E"), (-44, 2, "E"), (-44, 3, "E"), (-43, 4, "E"),
                     (-31, 13, "E"), (-30, 14, "E")):
        c.P(x, y, ch)
    c.mirror()
    d = finish(c)
    # the digested graft: port aft-crown ganglion D is still Hegemony Titanium
    for h in d["hull_cells"]:
        x, y = h["grid_pos"]
        if (x + 14) ** 2 / 3.6 ** 2 + (y + 18) ** 2 / 3.4 ** 2 <= 1.0:
            h["material"] = "Titanium"
    return d


# --------------------------------------------------------------------------- experiments
def dendrite():
    """Experimental: network screamer. A single neuron the size of a
    frigate. The soma holds the bridge inside a ring of signal jammers;
    dendrites branch forward and out, tipped with ion disruptors, jammers
    and a signal buoy; the axon runs aft through two myelin beads to three
    terminal boutons that carry the engines. It sits off the line and floods
    every channel with what the Collective calls data corruption: the
    screaming."""
    c = Canvas("sy_dendrite", "Composite")
    lobe(c, 0, 0, 6.6, 6.2)                         # soma
    tendril(c, [(-5, 0), (-20, 0)])                 # axon
    for x in (-10, -15):
        lobe(c, x, 0, 2.0, 2.6)                     # myelin beads
    lobe(c, -20, 0, 2.8, 3.0)                       # axon hillock knot
    tendril(c, [(-20, 2), (-22, 5), (-24, 6)])
    lobe(c, -26, 7, 2.6, 2.4)                       # upper terminal bouton
    tendril(c, [(-21, 0), (-25, 0)])
    lobe(c, -26, 0, 2.6, 2.6)                       # centre terminal bouton
    tendril(c, [(3, 4), (7, 8), (10, 12)])          # dendrites
    tendril(c, [(10, 12), (10, 16), (8, 19)], tip="=", plates=False)
    tendril(c, [(10, 12), (15, 14), (19, 14)], tip="i")
    tendril(c, [(5, 0), (13, 0)])
    tendril(c, [(13, 1), (16, 4), (21, 5)], tip="i")
    tendril(c, [(-2, 5), (-5, 10), (-8, 13)], tip="i")
    tendril(c, [(-5, 10), (-1, 14), (1, 17)], tip="J", plates=False)
    _rooms(c, {
        8: [(-27, -25, {-26: "M"})],
        7: [(-27, -25, {-26: "R"})],
        6: [(-27, -24, {-25: "y"})],
        5: [(-3, 1, {-3: "Q", 0: "M"})],
        4: [(-4, 4, {-4: "=", -2: "@", -1: "@", 1: "=", 3: "Q"})],
        3: [(-4, 4, {-4: "y", 4: "y"})],
        2: [(-5, 5, {-4: "=", -2: "T", 2: "T", 4: "="}), (-21, -20, {-20: "M"})],
        1: [(-6, 6, {-5: "O", -1: "B", 0: "B", 3: "H", 5: "="}),
            (-22, -18, {-22: "R", -18: "b"}), (-27, -25, {-26: "R"})],
        0: [(-6, 8, {-6: "R", -3: "@", -2: "@", -1: "B", 0: "B", 6: "y"})],
    })
    for x, y, ch in ((-28, 0, "E"), (-28, 1, "E"), (-28, 6, "E"), (-28, 7, "E")):
        c.P(x, y, ch)
    c.mirror()
    return finish(c)


def egg_case():
    """Experimental: drone womb. A mermaid's purse, the egg case of a skate:
    a pinched capsule with a horn at each corner, short and straight
    forward, long and curling aft. Drone embryos
    swell under its skin as blisters along both flanks, each blister a
    pair of drone bays; a womb opens in the bow between the fore horns,
    lined with more. Plasma on the fore horns, engines on the aft ones and
    across the stern. It never fights itself; it births what does."""
    c = Canvas("sy_egg_case", "Composite")
    lobe(c, -6, 0, 8.5, 8.2)
    lobe(c, 6, 0, 8.5, 8.2)
    c.R(-6, 6, 0, 7)
    hollow(c, 9, 0, 6.5, 2.4)                       # the womb
    c.K(13, 16, 0, 1)                               # ...open at the bow
    for bx in (-9, -4, 1, 6):
        lobe(c, bx, 8.4, 2.2, 2.0)                  # embryo blisters
    tendril(c, [(-10, 6), (-13, 8), (-16, 10), (-19, 11), (-23, 11)], tip="E", plates=False)
    tendril(c, [(10, 6), (13, 7), (16, 8), (19, 8)], tip="P")
    _rooms(c, {
        9: [(-10, -8, {-10: "V", -8: "V"}), (-5, -3, {-5: "y", -3: "Y"}),
            (0, 2, {0: "Y", 2: "y"}), (5, 7, {5: "V", 7: "V"})],
        8: [(-11, 7, {-11: "Q", -9: "y", -6: "Z", -5: "Z", -3: "y", 0: "Z", 1: "Z",
                      3: "y", 6: "Y", 7: "y"})],
        7: [(-14, 8, {-12: "y", -4: "M", 4: "y"})],
        6: [(-13, 13, {-12: "K", -11: "K", -9: "Q", -7: "S", -6: "S", -5: "S", -1: "!", 0: "!",
                       3: "S", 4: "S", 5: "S", 8: "Z", 9: "Z", 11: "y"})],
        5: [(-12, 12, {-11: "O", -7: "S", -6: "S", -5: "S", -3: "y", 3: "S", 4: "S", 5: "S",
                       7: "y", 10: "V"})],
        4: [(-12, 12, {-10: "m", 0: "M", 9: "y", 11: "T"})],
        3: [(-12, 5, {-11: "R", -9: "b", -5: "c", -1: "y", 3: "O"})],
        2: [(-13, 3, {-12: "f", -10: "f", -5: "@", -4: "@", -1: "T"})],
        1: [(-13, 2, {-12: "F", -11: "F", -10: "F", -7: "R", -2: "B", -1: "B", 1: "H"})],
        0: [(-13, 1, {-12: "F", -11: "F", -10: "F", -5: "y", -2: "B", -1: "B"})],
    })
    for x, y, s_ in ((-10, 10, "DD"), (-5, 10, "DD"), (0, 10, "DD"), (5, 10, "DD"),
                     (6, 3, "DD"), (9, 3, "DD"), (4, 2, "DD"),
                     (-14, 0, "E"), (-14, 1, "E"), (-14, 2, "E"), (-13, 3, "E")):
        c.P(x, y, s_)
    c.mirror()
    return finish(c)


# --------------------------------------------------------------------------- traders
def vesicle():
    """Trader, small. A peanut of two grown bulbs pinched at the waist: crew
    and engines in the aft bulb, the bridge in the waist, and the fore bulb a
    hold of containment vats, cryo and specimen vaults round a feeding
    corridor. Two short eye-stalks reach forward off its shoulders, ion on
    their tips: all it carries to defend the cargo."""
    c = Canvas("sy_vesicle", "Composite")
    lobe(c, -6, 0, 5.6, 5.2)
    lobe(c, 5, 0, 6.2, 5.8)
    c.R(-3, 2, 0, 2)
    tendril(c, [(8, 3), (10, 4), (12, 5), (14, 6), (17, 6)], tip="i")
    c.rows(-11, 4, """
  #Q.y.Q#    ZZ.Y.
 #y.M..O.# #.Y.y.V....#
E.Rb.Q.y..d.ZZ.V.ZZ.M#
E....m...BB...........#
E.Rc.f...BB.Y.V.y.V.Y.#
""")
    c.mirror()
    return finish(c)


def roe():
    """Trader, medium. A roe-stalk: one spine with six round egg sacs budding
    off it on short necks, the biggest amidships, open sky between them.
    Every sac is a sealed vat of living cargo (containment, cryo, specimen
    vaults, a graft lab), so a breach loses one sac, not the load. A fusion
    tail aft, two antennae forward with plasma on their tips."""
    c = Canvas("sy_roe", "Composite")
    lobe(c, -17, 0, 4.6, 4.4)                       # tail
    c.R(-14, 13, 0, 2)                              # spine
    lobe(c, 15, 0, 5.2, 4.2)                        # head
    for cx, cy, r in ((-9, 6.0, 3.6), (0, 7.0, 4.4), (9, 6.0, 3.6)):
        lobe(c, cx, cy, r)                          # the sacs, on their necks
        c.R(cx - 1, cx + 1, 2, int(cy - r) + 1)
    tendril(c, [(16, 3), (17, 4), (18, 5), (20, 6), (22, 6)], tip="P")
    # the big sac amidships: containment vats round a feeding vein
    for y, x, row in ((11, -1, "#y#"), (10, -3, "#V...V#"), (9, -3, "#ZZ.ZZ#"),
                      (8, -4, "#y.....y#"), (7, -4, "#ZZ.M.ZZ#"), (6, -4, "#V.....V#"),
                      (5, -3, "#ZZ.ZZ#"), (4, -3, "#y...y#"), (3, -1, "#.#")):
        c.P(x, y, row)
    # aft sac: cryo and specimens
    for y, x, row in ((9, -10, "#y#"), (8, -11, "#V.V#"), (7, -12, "#ZZ.Y.#"),
                      (6, -12, "#V...V#"), (5, -12, "#Y.ZZ.#"), (4, -11, "#y.V#"),
                      (3, -10, "#.#")):
        c.P(x, y, row)
    # fore sac: the graft lab
    for y, x, row in ((9, 8, "#y#"), (8, 7, "#Y.Y#"), (7, 6, "#!!.ZZ#"),
                      (6, 6, "#V...V#"), (5, 6, "#.ZZ.Y#"), (4, 7, "#M.V#"),
                      (3, 8, "#.#")):
        c.P(x, y, row)
    # tail: fusion, engines, crew
    c.rows(-21, 4, """
   #y#
 #Q.O.Q#
E.Rb.cM.
E.FFF...d
E.FFF.m....
""")
    # the spine: a vein both sides of the centreline, rooms between
    c.P(-14, 1, "d" + "." * 13 + "BB" + "." * 9 + "d.")
    c.P(-13, 0, "y.Q.y.ZZ.Y.y.BB.y.Y.ZZ.y.")
    for x, ch in ((-12, "V"), (-6, "y"), (-4, "V"), (4, "V"), (6, "y")):
        c.P(x, 2, ch)
    # head: crew, helm, an ion knot on the snout
    c.rows(10, 3, """
 #Q.m.Q#
#O.y.M..#
 ........#
 .H.y.Q.y.i
""")
    c.mirror()
    # a seventh egg, budding off the big starboard sac
    return finish(c, graft=bud(c, 3.6, -11.0, 1.9, 1.8))


def amnion():
    """Trader, large. A mother body aft cradling one great amnion sac in two
    arms: the sac rides ahead of her on an umbilical, held by a sinew strut
    off each arm, and the arm tips close round its front. Inside the sac,
    graft theatres and containment vats ring a nutrient heart; the bridge
    sits in the umbilical between mother and cargo."""
    c = Canvas("sy_amnion", "Composite")
    lobe(c, -15, 0, 8.5, 7.8)                       # the mother
    c.R(-7, 2, 0, 2)                                # the umbilical
    lobe(c, 11, 0, 9.0, 8.6)                        # the sac
    tendril(c, [(-12, 6), (-10, 9), (-6, 12), (-1, 14), (5, 15), (11, 14), (16, 12),
                (20, 9), (23, 6), (24, 3)], tip="P")
    tendril(c, [(13, 7), (14, 9), (15, 11)])        # sinew strut, arm to sac
    for x, y, row in (
            # the mother: engines, fusion, crew
            (-18, 7, "##y#y##"),
            (-20, 6, "##Q.O.Q.y.#"),
            (-21, 5, "#y.KK.M.KK.y#"),
            (-22, 4, "E.y.GG...CC.y.#"),
            (-22, 3, "E...GG.m.CC...#"),
            (-23, 2, "E.Rb.c.f.y.Q.y..#"),
            (-23, 1, "E.LL..FFF.Q.Q...d"),
            (-23, 0, "E.R...FFF.y.M.y.."),
            # the umbilical and the bridge
            (-6, 2, "#V#y#y#V#"),
            (-6, 1, "......BB."),
            (-6, 0, "y.Q.y.BB."),
            # the sac
            (8, 8, "##y#y#."),
            (6, 7, "#V.Y.V.Y.V#"),
            (5, 6, "#y..ZZ.ZZ..y#"),
            (4, 5, "#.SSS.ZZ.SSS..#"),
            (4, 4, "#.SSS.!!.SSS.y#"),
            (3, 3, "#y.....M.....y..#"),
            (3, 2, "#.ZZ.V.....V.ZZ.#"),
            (3, 1, "..Y...www...Y...#"),
            (2, 0, "...V.y.www.y.V....i")):
        c.P(x, y, row)
    c.mirror()
    return finish(c)


CONCEPTS = [
    ("egg_case", egg_case, "Egg Case", "Experimental: drone womb",
     "A mermaid's purse: a pinched egg case with a horn at each corner, the aft pair long and curling with engines on their tips, drone embryos swelling under its skin as blisters and a womb open at the bow. It never fights itself; it births what does."),
    ("dendrite", dendrite, "Dendrite", "Experimental: network screamer",
     "One neuron the size of a frigate: a soma with the bridge in a ring of jammers, dendrites tipped with ion and jammers, an axon aft to three engine boutons. It floods every channel with the screaming."),
    ("gestalt", gestalt, "Gestalt", "Flagship",
     "The mind's largest body, grown as a network: flesh ganglia strung on sinew nerves, an open net with space through every mesh. Bridge in the heart ganglion, plasma prow and mandibles forward; one ganglion is a Hegemony hull it is still digesting."),
    ("cortex", cortex, "Cortex", "Battleship",
     "A brain seen from above: two folded hemispheres split by the fissure and joined only at the bridge, ventricle voids inside, a brainstem running aft to a cerebellum of engines. Ion disruptors crown every fold."),
    ("sinew", sinew, "Sinew", "Cruiser",
     "A grown centipede: five lobed segments on one spine and four pairs of sinew limbs, three reaching forward like oars with plasma tips, the last curled in front of the head as mandibles."),
    ("amnion", amnion, "Amnion", "Trader: large graft carrier",
     "A mother body cradling one great sac of grafts and vats in two arms, the sac fed by an umbilical. The bridge rides between mother and cargo."),
    ("roe", roe, "Roe", "Trader: medium vat hauler",
     "A spine with six egg sacs budding off it on short necks, each sac a sealed vat of living cargo. Plasma antennae forward, fusion tail aft."),
    ("vesicle", vesicle, "Vesicle", "Trader: small vat runner",
     "Two grown bulbs pinched at the waist: crew aft, a ring of containment vats forward. It carries live specimens between Collective worlds."),
]
