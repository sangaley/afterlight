"""Corpse Stars, round 5a: the light and medium warships, hand-drawn,
top-down only. Same grammar as corpse.py (spine, swept ribs, open voids, a
plated ram, AbyssalAlloy; EMP, ion, railguns, heavy missiles), carried down
to scout, corvette, frigate and destroyer size. Each ship is drawn y >= 0 and
mirrored, so the skeleton is port/starboard symmetric; the odd deliberate
flaw (a missing bone, a cracked rib) is drawn after the mirror.
"""
from corpse import FACTION, finish

from draw import Canvas


def _segs(c, rows):
    """Paint runs of cells row by row: {y: [(x, \"cells\"), ...]}."""
    for y, runs in rows.items():
        for x, s in runs:
            c.P(x, y, s)



def _bone(c, path, fat=True):
    """A 3-wide bone with marrow: hull on every cell next to the path (king's
    move if fat, else only the four sides, which rounds the ends and keeps a
    swept rib slim), deck on the path itself. The path is drawn by hand,
    cell by cell, and must step orthogonally so the marrow is walkable."""
    for x, y in path:
        for dx in (-1, 0, 1):
            for dy in (-1, 0, 1):
                if fat or dx == 0 or dy == 0:
                    c.cells.setdefault((x + dx, y + dy), "#")
    for p in path:
        c.cells[p] = "."


def _run(x0, x1, y):
    return [(x, y) for x in range(x0, x1 + 1)]


def _col(x, y0, y1):
    return [(x, y) for y in range(y0, y1 + 1)]


def _plate_bow(c, x_min):
    """Plate every forward-facing step of the bow from x_min on (y > 0 side;
    mirror afterwards): an empty cell with hull aft of it and below it."""
    hull = {p for p, ch in c.cells.items() if ch != "/"}
    for (x, y) in sorted(hull):
        p = (x + 1, y)
        if x + 1 < x_min or y <= 0 or p in hull:
            continue
        if any((x + k, y) in hull for k in range(2, 14)):
            continue                     # an inner corner, not the bow face
        if (x + 1, y - 1) in hull and (x + 1, y + 1) not in hull:
            c.cells[p] = "/"


def _swept(x, n, run=2, y=1):
    """The marrow of a rib swept aft: from (x, y) climb `run` cells, step one
    aft, climb again, `n` cells up in all. Returns the path."""
    path = [(x, y)]
    for i in range(1, n + 1):
        y += 1
        path.append((x, y))
        if i % run == 0 and i < n:
            x -= 1
            path.append((x, y))
    return path


def _line(a, b):
    """The marrow of a straight bone from a to b in orthogonal steps (so it
    stays walkable), keeping as close to the true line as it can."""
    (x, y), (bx, by) = a, b
    path = [a]
    lx, ly = bx - a[0], by - a[1]
    off = lambda px, py: abs((px - a[0]) * ly - (py - a[1]) * lx)
    while (x, y) != (bx, by):
        opts = []
        if x != bx:
            opts.append((x + (1 if bx > x else -1), y))
        if y != by:
            opts.append((x, y + (1 if by > y else -1)))
        x, y = min(opts, key=lambda p: off(*p))
        path.append((x, y))
    return path


def _snap(c, path, keep, tip, at):
    """The deliberate flaw: break a bone drawn on y > 0 off on the y < 0 side
    after the mirror, keeping the first `keep` cells of its marrow, and put
    its weapon `tip` back at offset `at` from the last kept cell."""
    near = lambda pts: {(x + dx, y + dy) for x, y in pts
                        for dx, dy in ((0, 0), (1, 0), (-1, 0), (0, 1), (0, -1))}
    kept = near(path[:keep])
    for x, y in near(path[keep:]) - kept:
        c.cells.pop((x, -y), None)
    x, y = path[keep - 1]
    c.cells[(x + at[0], -(y + at[1]))] = tip


def _show(c):
    """Print a canvas (for drawing; not used by the build)."""
    xs = [p[0] for p in c.cells]
    ys = [p[1] for p in c.cells]
    print("     " + "".join(str(abs(x) % 10) for x in range(min(xs), max(xs) + 1)))
    for y in range(max(ys), min(ys) - 1, -1):
        print(f"{y:4d} " + "".join(c.cells.get((x, y), " ") for x in range(min(xs), max(xs) + 1)))


def psychopomp():
    """Scout. A courier that ferries one dead lord between systems: a moth of
    bone, one pair of long ribs swept back like wings, the coffin riding on
    the spine just aft of the bridge, a needle ram."""
    c = Canvas("cs_psychopomp", "AbyssalAlloy")
    c.rows(0, 8, """
     #e#
     #.#
     #..#
      #.#
      #..#
##     #..#     ###/
E.#     #..#   #.M.#/
E.R.QV#MO#.#BB#.T.s.##/
E...........BB.H......###
""")
    c.P(12, 2, "##")                 # the spine swells over the bridge
    c.mirror()
    c.P(7, -1, "b")                 # one core, not a pair
    return finish(c)


def knell():
    """Scout. The bell-ringer: a picket that runs ahead of a funeral fleet and
    tolls when it finds you. Its pelvis flares into the mouth of a bell, a
    frame of bone open in the middle with the engines along its lip and an
    EMP at each lip-tip for the toll. The spine forward is the handle,
    knuckled with radar."""
    c = Canvas("cs_knell", "AbyssalAlloy")
    c.rows(0, 7, """
#e#
#.####
E..# ###
 #.#   ##
 E..#   ##      ##/
  E.#    ##    #.M.##/
  b.#O#M#A#BB#A.s.Q###/
  E........BB.H......###
""")
    c.P(11, 2, "##")                 # the spine swells over the bridge
    c.mirror()
    c.P(7, -1, "b")
    return finish(c)


def pallbearer():
    """Corvette. The escort that carries a dead lord's coffin on its bier:
    a coffin-shaped core on the spine (the bridge lies in it, beside the
    dead) held between two long bier poles by three pairs of bearer ribs.
    Where each bearer grips the pole it holds a gun, so the coffin goes
    nowhere unguarded."""
    c = Canvas("cs_pallbearer", "AbyssalAlloy")
    _segs(c, {
        9: [(4, "#e####")],
        8: [(4, "#.######i####")],
        7: [(4, "#.#"), (9, "###.#######e##/")],
        6: [(4, "#..#"), (11, "#..#"), (16, "####.###")],
        5: [(5, "#.#"), (12, "#.#"), (19, "#.#")],
        4: [(5, "#..#"), (12, "#..#"), (19, "#..#")],
        3: [(0, "##"), (6, "#.#"), (10, "#V....VM#"), (20, "#.#")],
        2: [(0, "E.#"), (6, "#.O#Q.V...V.Q#a.#/")],
        1: [(0, "E.R.b##.a.m.M.BB.T.s.rr.##/")],
        0: [(0, "E.............BB.H........##")],
    })
    c.mirror()
    c.P(12, -1, "b")
    return finish(c)


def gravedigger():
    """Corvette. It buries what the fleet kills: a long spine for a haft, a
    crossbar pelvis for the grip, and for a blade the open frame of a
    shoulder-bone, a scapula, with salvage claws along its digging edge."""
    c = Canvas("cs_gravedigger", "AbyssalAlloy")
    _bone(c, _run(1, 31, 0))                          # the haft
    _bone(c, _col(1, 0, 3))                           # the grip
    _bone(c, [(11, 1), (11, 2), (10, 2), (10, 3), (10, 4), (9, 4), (9, 5), (8, 5)], fat=False)  # ribs
    _bone(c, _col(17, 1, 7))                          # the blade's shoulder
    _bone(c, _run(18, 22, 7) + [(22, 6), (23, 6), (24, 6), (24, 5), (25, 5), (26, 5),
              (26, 4), (27, 4), (27, 3), (28, 3), (28, 2), (29, 2), (29, 1)])
    _segs(c, {
        8: [(17, "i"), (20, "M")],
        7: [(25, "X")],
        6: [(8, "e"), (27, "X")],
        4: [(0, "#"), (1, "x")],
        3: [(0, "E"), (30, "X")],
        2: [(0, "R")],
        1: [(0, "E"), (3, "b.O.M"), (9, "Q.m"), (13, "#BB#"), (19, "T.rr.s.a")],
        0: [(0, "E"), (14, "BB.H")],
    })
    _plate_bow(c, 18)
    c.mirror()
    c.P(7, -1, "a")
    return finish(c)


def rood():
    """Corvette. A grave-cross that stands over the dead: a double cross of
    bone, a long aft bar and a short fore bar, both swept a little aft, their
    ends knuckled like the heads of long bones and armed. It holds a line
    with EMP and ion while the heavier ships close."""
    c = Canvas("cs_rood", "AbyssalAlloy")
    _bone(c, _run(1, 31, 0))                                        # spine
    _bone(c, _col(1, 0, 2))                                         # pelvis
    _bone(c, [(15, 1), (15, 2), (15, 3), (14, 3), (14, 4), (14, 5), (14, 6),
              (13, 6), (13, 7)], fat=False)                         # aft bar
    _bone(c, _run(12, 14, 8), fat=False)                            # its knuckle
    _bone(c, [(23, 1), (23, 2), (23, 3), (22, 3), (22, 4)], fat=False)  # fore bar
    _bone(c, _run(21, 23, 5), fat=False)                            # its knuckle
    _bone(c, _run(27, 33, 0))                                       # ram
    _segs(c, {
        9: [(13, "e")],
        6: [(22, "i")],
        3: [(1, "x"), (28, "##")],
        2: [(0, "E"), (6, "##"), (10, "##"), (27, "#.M.##")],
        1: [(0, "E"), (2, "#R.O#M#Q.m#a#.#TBB#sQ.#.#.rr.O.##")],
        0: [(0, "E"), (18, "BB.H"), (35, "##")],
    })
    _plate_bow(c, 27)
    c.P(18, 2, "##")                 # the spine swells over the bridge
    c.mirror()
    c.P(7, -1, "b")
    return finish(c)


def dirge():
    """Frigate. A fish's skeleton picked clean: a long spine and five pairs of
    slim open ribs, longest amidships, with a heavy missile launcher in the
    tip of every rib. It sings one note, many times. One rib snapped
    and was never mended (they don't mend; damage makes them stronger)."""
    c = Canvas("cs_dirge", "AbyssalAlloy")
    _bone(c, _run(1, 41, 0))                                        # spine
    _bone(c, _col(1, 0, 3))                                         # pelvis
    tips = []
    ribs = []
    for x, n in ((10, 4), (16, 6), (22, 7), (28, 6), (34, 4)):
        path = _swept(x, n, run=1)
        _bone(c, path, fat=False)
        ribs.append(path)
        tips.append((path[-1][0], path[-1][1] + 1))
    for p in tips:
        c.cells[p] = "x"
    _segs(c, {
        4: [(1, "i")],
        3: [(0, "N"), (38, "###")],
        2: [(0, "N"), (37, "#.M.###")],
        1: [(0, "E"), (2, "#R.ObMa#.#Qam#.#KKa#.#BB#T.#aQOa.#s.rr.f.###")],
        0: [(0, "E"), (24, "BB.H"), (42, "....###")],
    })
    _plate_bow(c, 37)
    c.mirror()
    c.P(7, -1, "a")
    _snap(c, ribs[1], 8, "x", (0, 1))           # one rib snapped and never mended
    return finish(c)


def ossifrage():
    """Frigate. The bone-breaker: a bird of prey's skeleton, wings spread.
    From each shoulder a short arm bone runs out to a wrist knuckle and three
    finger bones fan back from it, steep, raked and trailing, a gun in every
    fingertip. The beak is the ram."""
    c = Canvas("cs_ossifrage", "AbyssalAlloy")
    _bone(c, _run(1, 37, 0))                                        # spine
    _bone(c, _col(1, 0, 2))                                         # pelvis
    w = (25, 7)                                                     # the wrist
    _bone(c, _line((26, 1), w), fat=False)                          # arm
    _bone(c, _line(w, (23, 16)), fat=False)                         # steep finger
    _bone(c, _line(w, (15, 14)), fat=False)                         # raked finger
    _bone(c, _line(w, (9, 10)), fat=False)                          # trailing finger
    _bone(c, _line((10, 1), (6, 4)), fat=False)                     # talons
    _segs(c, {
        17: [(23, "i")],
        15: [(16, "e")],
        10: [(9, "i")],
        9: [(20, "M"), (24, "a")],
        8: [(22, "b")],
        5: [(7, "x")],
        3: [(1, "e"), (31, "###")],
        2: [(0, "E"), (30, "#.a.##")],
        1: [(0, "E"), (2, "#R.OM##..#Qm#aKKa#TBB#s#.#QOa.rr.s.O.###")],
        0: [(0, "E"), (21, "BB.H"), (38, "....###")],
    })
    _plate_bow(c, 30)
    c.P(21, 2, "##")                 # the spine swells over the bridge
    c.mirror()
    c.P(6, -1, "b")
    return finish(c)


def _poly(*pts):
    """A bone's marrow through several points, joined by straight runs."""
    path = [pts[0]]
    for a, b in zip(pts, pts[1:]):
        path += _line(a, b)[1:]
    return path


def reaper():
    """Destroyer. Two great scythe-blades of bone grow out of the spine just
    behind the ram and curve back past the stern, round a short ribcage of
    its own. Seen head-on it is all blade; seen from behind, an open hand.
    One blade was broken short in some old fight and left that way."""
    c = Canvas("cs_reaper", "AbyssalAlloy")
    _bone(c, _run(1, 44, 0))                                        # spine
    _bone(c, _col(1, 0, 3))                                         # pelvis
    _bone(c, _poly((37, 1), (36, 4), (33, 7), (29, 9)))             # blade heel
    blade = _poly((29, 9), (25, 12), (19, 14), (12, 15), (6, 15), (2, 14))
    _bone(c, blade, fat=False)                                      # the blade
    for (x, n), g in zip(((8, 3), (15, 4), (22, 5)), "eie"):
        path = _swept(x, n)
        _bone(c, path, fat=False)                                   # ribcage
        c.cells[(path[-1][0], path[-1][1] + 1)] = g
    _segs(c, {
        16: [(9, "i"), (16, "e")],
        14: [(1, "x")],
        6: [(36, "i")],
        4: [(38, "e")],
        4: [(1, "x")],
        3: [(0, "N"), (42, "##")],
        2: [(0, "N"), (42, ".M.##")],
        1: [(0, "E"), (2, "#RbOM#.#KKaQ#.#maOQ#.#TBB#saQRbcfa.....s.rr.O.##")],
        0: [(0, "E"), (25, "BB.H"), (45, "....###")],
    })
    _plate_bow(c, 41)
    c.P(25, 2, "##")                 # the spine swells over the bridge
    c.mirror()
    c.P(6, -1, "b")
    _snap(c, blade, len(blade) - 6, "x", (-1, 0))  # one blade broken short
    return finish(c)


CONCEPTS = [
    ("psychopomp", psychopomp, "Psychopomp", "Scout",
     "A courier that ferries one dead lord between systems: a moth of bone with one pair of long swept ribs, the coffin riding on the spine."),
    ("knell", knell, "Knell", "Scout",
     "The bell-ringer picket that runs ahead of a funeral fleet: its pelvis flares into a bell of bone, engines on the lip, an EMP at each lip-tip for the toll."),
    ("pallbearer", pallbearer, "Pallbearer", "Corvette",
     "Escort for a dead lord's coffin: a coffin-shaped core (the bridge lies in it) held between two stepped, swept bier poles by three pairs of bearer ribs, a gun where each bearer grips the pole."),
    ("gravedigger", gravedigger, "Gravedigger", "Corvette",
     "It buries what the fleet kills: a spine for a haft, a crossbar pelvis for a grip, and for a blade the open frame of a shoulder-bone with salvage claws on its edge."),
    ("rood", rood, "Rood", "Corvette",
     "A grave-cross that stands over the dead: a double cross of bone, long aft bar and short fore bar, their knuckled ends armed with EMP and ion."),
    ("dirge", dirge, "Dirge", "Frigate",
     "A fish's skeleton picked clean: a long spine and five pairs of slim open ribs, a heavy missile launcher in every rib tip. One rib snapped and was never mended; it sings one note, many times."),
    ("ossifrage", ossifrage, "Ossifrage", "Frigate",
     "The bone-breaker: a bird of prey's skeleton with wings spread, three finger bones fanning back from each wrist, a gun in every fingertip, the beak for a ram."),
    ("reaper", reaper, "Reaper", "Destroyer",
     "Two great scythe-blades of bone grow from the spine behind the ram and curve back past the stern, round a short ribcage of its own. One blade was broken short and left that way."),
]
