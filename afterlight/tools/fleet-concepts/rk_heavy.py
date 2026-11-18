"""Recursive Kingdom, round 3: the heavy ships, all built from one seed.

Lore: one person copied themselves until the copies became a civilisation,
"one person, two billion times over". Every hull is crewed by the same person
at different ages, forever arguing with themselves. The original founder is
still alive somewhere, hiding, disgusted by what they became.

The light ships (rk_light.py) show the seed and the first level of the
recursion. These heavy ships carry it further, each drawn so the recursion
reads from above, without colour:
  - ELDER CARRIER: a copy within a copy within a copy: a huge seed drawn as
    a frame (its outline doubled, a hangar slot between the walls), its
    eldest child in its notch and a grandchild in the child's;
  - THE ORIGINAL: three levels, 27 copies, a chevron of chevrons of
    chevrons, the founder's cabin on the keel at the dead centre;
  - COPY PRESS: copies on a line at every stage of being printed, with more
    copies leaning over the line printing them.
(sierpinski() is an unused draft: that key, the spiral, the parliament and
the hive are drawn in another module.)

Rules kept on every hull: mirror-symmetric skeletons (copy errors only after
c.mirror(), small); engines only on faces with open space aft; one main
bridge, on the keel nearest the ship's middle (a copy's own bridge becomes
barracks when it isn't the main one); launchers only where they have four
clear cells outside; Steel with salvaged patches.

Legend: see rk_seed.py (and draw.py). Seeds are painted with rk_seed.seed();
the rest of each ship (welds, the huge seed, the chamber, the line) is drawn
here cell by cell.
"""
from collections import deque

import rk_seed as rk
from draw import DrawError
from rk_light import FACTION  # same faction, same page

N4 = ((0, 1), (0, -1), (1, 0), (-1, 0))
LAUNCHERS = set("x*^")
GUNS = set("gnpz")


# --------------------------------------------------------------------------- helpers
def paint(c, rows, mirror_rows=False):
    """Paint hand-drawn rows: a list of (y, x_first, "cells"); spaces skip.
    With mirror_rows, rows with y > 0 are also painted at -y."""
    for y, x0, s in rows:
        c.P(x0, y, s)
        if mirror_rows and y > 0:
            c.P(x0, -y, s)


def outline_of(rows):
    """Hull cells (no plates, no spaces) of hand-drawn rows."""
    return {(x0 + i, y) for y, x0, s in rows for i, ch in enumerate(s) if ch not in " /"}


def expect(rows, want, name):
    """A drawing must cover exactly the outline it was planned on."""
    have = outline_of(rows)
    if have != want:
        raise DrawError(f"{name}: drawing doesn't match its outline: missing {sorted(want - have)[:6]} "
                        f"extra {sorted(have - want)[:6]}")


def hull_of(c):
    return {p for p, ch in c.cells.items() if ch != "/"}


def silence(c):
    """Structural clean-up after copies are packed together (symmetric, so
    it is skeleton, not a copy error): a launcher without four clear cells
    outside it, a gun buried on every side, or an engine that would vent
    into hull becomes plain hull. Returns what it changed."""
    hull = hull_of(c)
    changed = []
    for (x, y), ch in sorted(c.cells.items()):
        if ch in LAUNCHERS:
            open_sides = [(dx, dy) for dx, dy in N4 if (x + dx, y + dy) not in hull]
            ok = any(all((x + dx * k, y + dy * k) not in hull for k in range(1, 5)) for dx, dy in open_sides)
            if not ok:
                c.cells[(x, y)] = "#"
                changed.append((ch, (x, y)))
        elif ch in GUNS:
            if all((x + dx, y + dy) in hull for dx, dy in N4):
                c.cells[(x, y)] = "#"
                changed.append((ch, (x, y)))
        elif ch in "EN":
            if any((xx, y) in hull for xx in range(x - 80, x)):
                c.cells[(x, y)] = "#"
                changed.append((ch, (x, y)))
    return changed


def demote_bridges(c, keep):
    """Every bridge cell not in `keep` becomes barracks (KK pairs) or deck:
    a copy's own bridge, when it isn't the ship's main one."""
    for p, ch in list(c.cells.items()):
        if ch == "B" and p not in keep:
            c.cells[p] = "K"
    # a 2x3 bridge became 6 K: keep the barracks to 2x1 pairs, the rest deck
    seen = set()
    for (x, y), ch in sorted(c.cells.items()):
        if ch != "K" or (x, y) in seen:
            continue
        comp, q = {(x, y)}, deque([(x, y)])
        while q:
            a = q.popleft()
            for dx, dy in N4:
                n = (a[0] + dx, a[1] + dy)
                if c.cells.get(n) == "K" and n not in comp:
                    comp.add(n)
                    q.append(n)
        seen |= comp
        if len(comp) % 2:
            for p in sorted(comp)[len(comp) - len(comp) % 2:]:
                c.cells[p] = "."


# --------------------------------------------------------------------------- elder carrier
# The elder: a huge seed (band 17, half 21: bigger than any standard size)
# drawn as a FRAME. Its band is split down the middle by a hangar slot that
# follows the arms, so the elder's own outline reads twice, a thin chevron
# inside a thick one; the two walls meet at the keel and at the barbs.
# Upper half, by hand, x from -35 (the barbs' aft face) to 0 (the nose).
E_BAND, E_HALF, E_BARB, E_EXT = 17, 21, 5, 3
E_NROW = E_HALF - E_BARB                 # 16
E_NOTCH = -(E_BAND - 1)                  # -16
E_STERN = E_NOTCH - E_NROW - E_EXT       # -35
SLOT = (2, 16)                           # rows of the hangar slot


def _elder_outline():
    out = set()
    for y in range(0, E_HALF + 1):
        lo = E_NOTCH - y if y <= E_NROW else E_STERN
        for x in range(lo, -y + 1):
            if SLOT[0] <= y <= SLOT[1] and -y - 10 <= x <= -y - 5:
                continue                 # the hangar slot
            out.add((x, y))
    return out


def _elder_rows():
    rows = [
        (21, -35, "##*####x####*##/"),
        (20, -35, "E..ff.c..m.A...#"),
        (19, -35, "E................"),
        (18, -35, "E.RLLb.QM..T..a..n"),
        (17, -35, "###aQ.Ob.DD#DD#...g"),
    ]
    # the arms: rear wall (6) | hangar slot (6) | front wall (5)
    # (rooms only on the slot side of the front wall's 3-wide stair, so the
    # corridor up the bow never closes)
    front = {16: "#...#", 15: "DD..p", 14: "#...#", 13: "#...g", 12: "DD..#", 11: "#M..#",
             10: "#...p", 9: "DD..#", 8: "#T..#", 7: "#...g", 6: "DD..#", 5: "#M..#",
             4: "#a..p", 3: "#Q..#", 2: "DD..#"}
    rear_rows = {16: "#MQ.DD", 15: "#bO..#", 14: "#cR..#", 13: "#MQ.DD", 12: "#bO..#",
                 11: "#KK..#", 10: "#MQ.DD", 9: "#bO..#", 8: "#cR..#", 7: "#MQ.DD",
                 6: "#bO..#", 5: "#KK..#", 4: "#MQ.DD", 3: "#bO..#", 2: "#s...#"}
    for y in range(SLOT[1], SLOT[0] - 1, -1):
        rows.append((y, E_NOTCH - y, rear_rows[y]))
        plate = "/" if y <= 4 else ""
        rows.append((y, -y - 4, front[y] + plate))
    rows += [
        (1, -17, "#.KK.DD#DD#.T.s.#/"),
        (0, -16, "#...........H...n"),
    ]
    return rows


def elder_carrier():
    """Carrier. A copy within a copy within a copy. The elder is a huge seed
    drawn as a frame: a hangar slot follows its arms, so its outline reads
    twice, a thin chevron of plated bow inside a thick one of cabins, the
    two walls meeting at the keel and the barbs, and the slot lined with
    drone bays on both faces. Its eldest child (a large seed) rides in its
    notch, nose welded to the elder's keel, and the child's own child (a
    small seed) rides in the child's notch. The ship's one bridge is the
    child's, in the middle of the hull; the elder keeps a helm in its bow."""
    c = rk.canvas("rk_elder_carrier")
    rows = _elder_rows()
    expect(rows, _elder_outline(), "elder")
    paint(c, rows)
    big = rk.seed(c, (-19, 0), "large", nose=False)
    small = rk.seed(c, (-32, 0), "small", nose=False)
    rk.weld(c, -19, -17, 1, 1)                    # elder keel to child's nose
    c.P(-18, 0, "..")
    rk.weld(c, -32, -30, 1, 1)                    # child's keel to grandchild's nose
    c.P(-31, 0, "..")
    c.mirror()
    silence(c)
    rk.flaw(c, big, clip=("stbd", 2), patch=("Titanium", "port_barb"))
    rk.flaw(c, small, swap={(-8, 3): "b"})       # the youngest's cabin: a battery for a core
    c.patches.update({p: "Composite" for p in c.cells
                      if p[1] < -16 and p[0] > -27 and c.cells[p] != "/"})   # one barb re-plated
    return rk.finish(c)


# --------------------------------------------------------------------------- sierpinski
# One CHEVRON = a medium seed leading two small ones, swept back on its arms
# 13 cells aft and 13 out, each follower's inner barb lying beside the
# leader's barb (as in rk_light's triad). Three chevrons arranged the same
# way, 22 aft and 22 out, the next chevron's inner arm resting on the corner
# of the leader's follower: nine seeds, two levels, the voids left open.
L1 = (13, 13)
L2 = (22, 22)


def _chevron(c, at, flip=False):
    """Paint one chevron of three seeds with its leader's tip at `at`, and
    weld each follower's inner barb to the leader's barb."""
    mx, my = at
    a, b = L1
    m = rk.seed(c, at, "medium", flip=flip, swap={(0, 0): "n", (-4, 4): "n", (-4, -4): "n"})
    s1 = rk.seed(c, (mx - a, my + b), "small")             # the upper follower,
    s2 = rk.seed(c, (mx - a, my - b), "small", flip=True)  # the lower its mirror
    for sy in (1, -1):                         # the barb-to-barb welds
        rk.weld(c, mx - 18, mx - 17, my + sy * 7, my + sy * 7)
        rk.weld(c, mx - 17, mx - 17, my + sy * 8, my + sy * 8)
    return m, s1, s2


def sierpinski():
    """Battleship. Three chevrons, each made of three chevrons: nine seeds in
    two levels of recursion, the voids between them left open. A medium
    leads two small copies swept back on its arms; three of those lead and
    follow the same way, the rear chevrons' inner arms resting on the
    corners of the front one's followers. Its stern is closed by an engine
    block welded between the two innermost rear copies, so the outline is a
    triangle of triangles round one big triangular void. The one bridge is
    on the lead copy's keel; the other copies' bridges are barracks."""
    c = rk.canvas("rk_sierpinski")
    A, B = L2
    lead = _chevron(c, (0, 0))
    up = _chevron(c, (-A, B))
    down = _chevron(c, (-A, -B), flip=True)
    for sy in (1, -1):                         # the level-two joints
        rk.weld(c, -26, -25, sy * 17, sy * 17)
        rk.weld(c, -25, -25, sy * 18, sy * 18)
    # the stern block between the two innermost rear copies (rows -2..2)
    c.rows(-46, 2, """
E.Rb#
E....a
""")
    c.P(-46, 0, "E..p.#")
    c.mirror()
    demote_bridges(c, keep={p for p in c.cells if c.cells[p] == "B" and p[0] > -10})
    silence(c)
    rk.flaw(c, up[1], clip="nose")                         # a follower one cell short
    rk.flaw(c, down[0], patch=("Titanium", "port_barb"))
    rk.flaw(c, lead[2], swap={(-8, 3): "m"})               # a medbay for a memory core
    rk.flaw(c, down[2], patch=("Composite", "nose"))
    return rk.finish(c)


# --------------------------------------------------------------------------- the original
# 27 xs seeds, three levels, every level the same arrangement scaled by two:
# a chevron is a leader and two followers 7 aft / 8 out; a chevron of
# chevrons puts the next two chevrons 15 aft / 15 out; the ship puts two more
# of those 30 aft / 30 out. Every place two copies touch has the same
# geometry (one copy's barb beside the other's), so every joint is the same
# three-cell weld behind the front copy's barb.
O_L1, O_L2, O_L3 = (7, 8), (15, 15), (30, 30)


def _joint(c, tip, s):
    """Weld the barb on side s (+1 port, -1 starboard) of the xs seed whose
    tip is `tip` to the copy lying beside it."""
    lx, ly = tip
    rk.weld(c, lx - 10, lx - 10, ly + s * 3, ly + s * 4)
    rk.weld(c, lx - 11, lx - 11, ly + s * 3, ly + s * 3)


def _o_chevron(c, at, seeds):
    x, y = at
    a, b = O_L1
    lead = rk.seed(c, at, "xs", flip=y < 0, swap={(0, 0): "n"})
    up = rk.seed(c, (x - a, y + b), "xs")
    down = rk.seed(c, (x - a, y - b), "xs", flip=True)
    _joint(c, at, 1)
    _joint(c, at, -1)
    seeds += [lead, up, down]


def _o_group(c, at, seeds):
    x, y = at
    a, b = O_L1
    A, B = O_L2
    _o_chevron(c, at, seeds)
    _o_chevron(c, (x - A, y + B), seeds)
    _o_chevron(c, (x - A, y - B), seeds)
    _joint(c, (x - a, y + b), 1)
    _joint(c, (x - a, y - b), -1)


def the_original():
    """Flagship. The founder's own ship, the oldest hull in the kingdom and
    the leader of its battle groups: three levels of recursion, 27 of the
    smallest copies. Three make a chevron, three chevrons make a chevron of
    chevrons, three of those make the ship, every joint the same weld, the
    voids between them open. On the keel at the dead centre, between the
    two innermost copies of the front chevron, sits a seed smaller than any
    copy: the founder's cabin, with the ship's one bridge, a memory core and
    a vault no copy has opened. Patched everywhere, in everything."""
    c = rk.canvas("rk_the_original")
    seeds = []
    A3, B3 = O_L3
    A2, B2 = O_L2
    a, b = O_L1
    _o_group(c, (0, 0), seeds)
    _o_group(c, (-A3, B3), seeds)
    _o_group(c, (-A3, -B3), seeds)
    _joint(c, (-A2 - a, B2 + b), 1)           # the two level-three joints
    _joint(c, (-A2 - a, -B2 - b), -1)
    # the founder's cabin: a tiny seed on the keel at the middle of the hull,
    # under the barbs of the two innermost copies (rows -1..1)
    c.rows(-31, 1, """
#BB.q#/
#BB.$M#
""")
    c.mirror()
    silence(c)
    # the oldest, most patched hull: a salvaged patch on every chevron, in
    # whatever the kingdom had; a few copies came out short or different
    mats = ("Titanium", "Composite", "AbyssalAlloy")
    regions = ("port_barb", "nose", "stbd_barb", "keel", "port", "stbd_arm", "port_arm", "nose", "stbd")
    for k in range(0, 27, 3):
        rk.flaw(c, seeds[k + (k // 3) % 3], patch=(mats[(k // 3) % 3], regions[k // 3]))
    for k in (4, 14, 23):                     # three copies a cell short in the nose
        rk.flaw(c, seeds[k], clip="nose")
    rk.flaw(c, seeds[10], swap={(-7, 3): "m"})          # a medbay where the core should be
    rk.flaw(c, seeds[20], swap={(-6, 4): "K", (-5, 4): "K"})   # a bunk room knocked through
    c.patches.update({(x, y): "Titanium" for x in range(-31, -24) for y in (-1, 0, 1)})
    return rk.finish(c)


# --------------------------------------------------------------------------- copy press
# The stages on the line are seeds without their barbs (the barbs, the guns
# and the engines go on last): the chevron body only, rows -3..3, band 7,
# tip at local (0, 0), so each nests in the notch of the one ahead exactly
# as a small seed nests in a medium's. Upper half and centre row, by hand,
# x relative to the stage's tip.
STAGES = {
    # a bare frame: the outline printed two cells thick, nothing inside
    "frame": [(3, -9, "#######"), (2, -8, "##   ##"), (1, -7, "##   ##"), (0, -6, "##   ##")],
    # walls printed and decked over; the memory goes in first
    "partial": [(3, -9, "#######"), (2, -8, "#..M..#"), (1, -7, "#.....#"), (0, -6, "#......")],
    # the rooms are in and its bridge is lit: the copy nearest done steers
    "near": [(3, -9, "#M##Q##"), (2, -8, "#bO...#"), (1, -7, "#..BB.#"), (0, -6, "#MBB...")],
}


def _stage(c, kind, tip):
    tx, ty = tip
    paint(c, [(y + ty, tx + x0, s) for y, x0, s in STAGES[kind]])


def _keel_link(c, front_notch, back_tip):
    """Weld a copy's tip into the notch of the copy ahead (as rk_light's echo)."""
    c.P(back_tip, 1, "##")
    c.P(back_tip, 0, "." * (front_notch - back_tip))


def copy_press():
    """Experimental. A factory that prints copies of itself. Down the keel,
    a line of copies at every stage, each nested in the notch of the one
    ahead: a bare frame at the stern, then walls with only the memory in,
    then one with its rooms in and its bridge lit (the copy nearest done
    steers the press; it is the ship's one bridge, at the dead middle),
    then a finished skiff with barbs, guns and engines, held by the nose in
    the notch of the foreman at the bow. Over the line, on both sides, three
    of the smallest copies lean in nose-first, a repair bay for a nose, their
    barbs welded side by side; the foreman's barbs carry drone bays. The
    line hangs only from the skiff, so the skiff's engines fire down the
    slot between the line and the workers."""
    c = rk.canvas("rk_copy_press")
    foreman = rk.seed(c, (0, 0), "medium", swap={
        (-8, 0): ".",                                        # its notch opens onto the skiff
        (-10, 8): "D", (-9, 8): "D", (-10, -8): "D", (-9, -8): "D"})
    # finished, held by the nose; its helm and core trade places so the keel
    # runs on through its notch to the line behind
    skiff = rk.seed(c, (-11, 0), "small", nose=False, swap={(-5, 0): "H", (-4, 0): "M"})
    _keel_link(c, -8, -11)
    _stage(c, "near", (-20, 0))
    _keel_link(c, -17, -20)
    _stage(c, "partial", (-29, 0))
    _keel_link(c, -26, -29)
    _stage(c, "frame", (-38, 0))
    _keel_link(c, -35, -38)
    # the workers: xs copies facing down onto the line, noses at row 7, side
    # by side (11 apart, so their outer barbs touch)
    print_head = {(0, 0): "y", (-7, 3): "D", (-6, 3): "D", (-7, -3): "D", (-6, -3): "D"}
    workers = [rk.seed(c, (x, 7), "xs", rot=3, swap=print_head) for x in (-21, -32)]
    workers.append(rk.seed(c, (-43, 7), "xs", rot=3,
                           swap={**print_head, (-6, -5): "E", (-8, -5): "E"}))   # the last one pushes
    rk.weld(c, -17, -15, 8, 12)                              # foreman's barb to the first worker
    c.mirror()
    demote_bridges(c, keep={p for p in c.cells if c.cells[p] == "B" and p[0] < -20})
    silence(c)
    rk.flaw(c, workers[1], clip="nose")                      # one print head set a cell back
    rk.flaw(c, foreman, patch=("Titanium", "port_barb"))
    rk.flaw(c, workers[2], patch=("Composite", "stbd_barb"))
    c.patches.update({(x, y): "Composite" for x in range(-47, -37) for y in range(-3, 4)
                      if (x, y) in c.cells})                 # the frame is printed in scrap
    return rk.finish(c)


CONCEPTS = [
    ("elder_carrier", elder_carrier, "Elder Carrier", "Cruiser (carrier)",
     "A copy within a copy within a copy: a huge seed drawn as a frame, a drone hangar following its arms, "
     "its eldest child in its notch and a grandchild in the child's."),
    ("the_original", the_original, "The Original", "Flagship",
     "The founder's own ship and the kingdom's capital: 27 copies in three levels, a chevron of chevrons of "
     "chevrons with the voids left open. Its bridge sits in a seed smaller than any copy, on the keel at the "
     "dead centre, beside a vault no copy has opened."),
    ("copy_press", copy_press, "Copy Press", "Experimental",
     "A factory that prints copies of itself: down its keel, a line of copies from a bare frame to a finished "
     "skiff, each in the notch of the next, with more copies leaning in over the line, repair bays for noses."),
]
# sierpinski() above is a finished draft kept for reference only: that key
# was handed to another agent, so it is deliberately not in CONCEPTS.
