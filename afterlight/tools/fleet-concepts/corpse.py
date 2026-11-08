"""Corpse Stars: hand-drawn, top-down only.

Lore: an entropy cult that worships dead stars and heat death. They come from
the previous universe and know what is coming. Their leaders literally decay
in office, and their ships get stronger as they take damage. In game they are
territorial deep lords (danger 2.5): they ram intruders and aim for engines.

Grammar, held at every size (all of it reads from above):
  - a VERTEBRAL SPINE down the centreline: a 3-wide column that swells into
    a knuckle wherever a rib joins it and pinches between;
  - RIBS: 3-wide bones off the spine that sweep aft as they go out, with open
    voids between them, so you can see space through the ship;
  - a long heavy RAM PROW at the bow, plated on every step of both faces;
  - nothing else: no plating over the ribs, no fill between them. It should
    look like the bones of a ship;
  - AbyssalAlloy; EMP, ion, railguns and heavy missiles.

Legend (draw.py): # hull  . deck  / plate  E engine  N large engine  R reactor
L large reactor  F fusion reactor  b battery  f fuel  c cooling  v vent
B bridge  H helm  M memory core  s shield  A radar  T targeting  Q quarters
K barracks  G galley  m medbay  O oxygen  a ammo  V cryo  $ vault
e EMP  i ion  r railgun  x heavy missile  d door  y repair
"""
import math
from collections import deque

from draw import Canvas

FACTION = {
    "slug": "corpse_stars",
    "name": "Corpse Stars",
    "tagline": "An entropy cult from the previous universe that worships dead stars. Their lords decay in office; their ships grow stronger as they break.",
    "grammar": [
        "A vertebral spine down the centreline, swelling into a knuckle at every rib",
        "Ribs that sweep aft as they go out, with open voids between them",
        "A long heavy ram prow at the bow, plated on every step",
        "Bones only: no fill between the ribs; you see space through the ship",
        "AbyssalAlloy; EMP, ion, railguns and heavy missiles, aimed at engines",
    ],
    "accent": "#8a63d2",
}

N4 = ((0, 1), (0, -1), (1, 0), (-1, 0))


def finish(c):
    """Build, then check the hull is one piece and no engine fires into its
    own hull (draw.py doesn't check either)."""
    d = c.build()
    hull = {tuple(h["grid_pos"]) for h in d["hull_cells"]}
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
        if ch in "EN" and any((xx, y) in hull for xx in range(x - 60, x)):
            d["_checks"].append(f"engine at {(x, y)} vents into hull")
    return d


def _segs(c, rows):
    """Paint runs of cells row by row: {y: [(x, "cells"), ...]}."""
    for y, runs in rows.items():
        for x, s in runs:
            c.P(x, y, s)


def mourner():
    """Ram. A heavy plated ram on a short spine, two pairs of swept ribs and
    a pelvis of engines. The bridge sits in the root of the ram."""
    c = Canvas("cs_mourner", "AbyssalAlloy")
    c.rows(0, 12, """
    #i
    #.#   #e
    #.#   #.#
    #..M  #..#
     #.#  #..#
     #.#   #.#
     #..#  #.#
      #.#  #..# ###/
      #.#   #.# #M.###/
 #x#  #.#   #.# #Q.O.s###/
E..c# #.#   #.# #..a.b.a.###/
E.R.b#m.Q###O.b#.BB.T.LL.c..###/
E................BB..H....rr..###
""")
    return finish(c.mirror())


def pyre():
    """Cruiser. A full ribcage: five pairs of swept ribs closed by a thin
    outer bone, so the voids between them are windows onto space. A pelvis
    of engines aft, the bridge on the spine amidships, a long ram forward."""
    c = Canvas("cs_pyre", "AbyssalAlloy")
    c.rows(0, 12, """
             #e######i######e####
           ##...#M##...####...#M###
        x####..#    #.#    #.#   #.i#
       #..#  #..#   #..#   #..#   #..##
      #..#    #.#    #.#    #.#    #..###
     ##..#    #.#    #.#    #.#    #.# #####/
    ## #..#   #..#   #..#   #..#   #..#  #M..##/
   ##   #.#    #.#    #.#    #.#    #.#   #Q.Q.##/
#x#..#  #.#    #.#    #.#    #.#    #.#   #.O.GG.##/
E.R.c#  #..#   #..#   #..#   #..#   #..#  #LL.GG.s.##/
E..b.#   #.#    #.#    #.#    #.#    #.#  #..T.m.a.M.##/
E.R.f.#a#...#b##Q.Q#a##m.O#BB#Q.Q#b##KK.##.R.KK..rr.e..##/
E..........................BB....................H....rr..##
""")
    return finish(c.mirror())


def _disc(c, r0, r1, ch="#", reach=30):
    """Every cell whose centre lies between radius r0 and r1."""
    for x in range(-reach, reach + 1):
        for y in range(-reach, reach + 1):
            if r0 <= math.hypot(x, y) <= r1:
                c.cells[(x, y)] = ch


def _curved_rib(c, a0, r0, r1, twist, w0, w1):
    """One rib of a ring cage: a bone from radius r0 out to r1 that bends
    through `twist` degrees on the way and thins from w0 to w1."""
    pts = []
    for i in range(80):
        t = i / 79
        r, a = r0 + (r1 - r0) * t, math.radians(a0 + twist * t)
        pts.append((r * math.cos(a), r * math.sin(a), w0 + (w1 - w0) * t))
    for x in range(-int(r1) - 3, int(r1) + 4):
        for y in range(-int(r1) - 3, int(r1) + 4):
            if any(math.hypot(x - px, y - py) <= w for px, py, w in pts):
                c.cells.setdefault((x, y), "#")


def _mirrored(c, rows):
    """_segs, and the same runs copied to -y (for the parts of an otherwise
    asymmetric drawing that are port/starboard symmetric)."""
    _segs(c, rows)
    _segs(c, {-y: runs for y, runs in rows.items() if y > 0})


def dead_sun():
    """Capital. A dead star caged in bone: fourteen ribs curl out of a dark
    core into an outer ring like the ribs of a ribcage wrapped round a wheel.
    The spine runs through it all, a pelvis of engines aft and the ram
    forward. The bridge is in the dead core."""
    c = Canvas("cs_dead_sun", "AbyssalAlloy")
    _disc(c, 19.4, 22.6)                       # the outer bone
    for k in range(14):                       # the ribs
        _curved_rib(c, k * 360 / 14 + 6, 5.5, 20.5, 38, 1.5, 1.05)
    c.R(-23, 22, -1, 1)                       # the spine
    for (x, y) in list(c.cells):              # bare bone has no deck
        if c.cells[(x, y)] == "#" and math.hypot(x, y) <= 6.4:
            del c.cells[(x, y)]
    # the dead core, drawn room by room
    c.rows(-6, 6, """
    #####
   #Q...Q#
  #FFF.O.c#
 #.FFF.b.M.#
#.mFFF.LL.s.#
..T..BB.H.A..
.....BB......
..A..BB..T...
#.s.LL.FFFm.#
 #.M.b.FFF..#
  #c.O.FFF#
   #Q...Q#
    #####
""")
    # spine: a corridor with the odd stores on its rims
    c.P(-23, 0, "." * 17)
    c.P(7, 0, "." * 16)
    c.P(-14, 1, "M")
    c.P(14, -1, "M")
    c.P(-10, -1, "a")
    c.P(10, 1, "a")
    # pelvis of engines
    _mirrored(c, {
        4: [(-29, "##x###")],
        3: [(-29, "E.R.f#")],
        2: [(-29, "E..b.#")],
        1: [(-29, "E.R.c.")],
        0: [(-29, "E.....")],
    })
    # the ram, plated on every step of both faces
    _mirrored(c, {
        5: [(23, "###/")],
        4: [(23, "#.Q.##/")],
        3: [(23, ".KK.O.###/")],
        2: [(23, "..a.M.s..###/")],
        1: [(23, ".R.m.rr.e...###/")],
        0: [(23, ".............i####")],
    })
    # weapons round the outer bone, on its outer face, each with deck behind
    hull = {p for p, ch in c.cells.items() if ch != "/"}
    walk = lambda p: c.cells.get(p) == "." or (p in hull and all(
        (p[0] + dx, p[1] + dy) in hull for dx, dy in N4))
    for ang, ch in ((30, "e"), (60, "i"), (90, "x"), (120, "i"), (150, "e"),
                    (210, "e"), (240, "i"), (270, "x"), (300, "i"), (330, "e")):
        a = math.radians(ang)
        p = (round(22.5 * math.cos(a)), round(22.5 * math.sin(a)))
        for _ in range(4):        # step in to the outer face if we start outside it
            if p in hull:
                break
            p = (round(p[0] - math.cos(a)), round(p[1] - math.sin(a)))
        c.cells[p] = ch
    return finish(c)


def ossuary():
    """Experimental. A ship built out of the bones of other ships. A Titanium
    engine block off a Hegemony hull, one pair of square Steel girders off
    something big, three swept ribs of their own (long on one side, short on
    the other), a forward-swept rib off something that was never human, a
    Choir organ pipe lashed on as a rib, and a Hegemony arrowhead bow worn
    as the ram. Only the spine is theirs all the way."""
    c = Canvas("cs_ossuary", "AbyssalAlloy")
    _segs(c, {
        10: [(14, "##x#")],
        9: [(15, "#.#"), (20, "#i#"), (34, "#e#")],
        8: [(10, "#ee#"), (15, "#.#"), (20, "#.#"), (34, "#.#")],
        7: [(10, "#..#"), (15, "#..#"), (20, "#..#"), (25, "##e#"), (33, "#..#")],
        6: [(10, "#.M#"), (16, "#.#"), (21, "#.#"), (26, "#.#"), (33, "#.#")],
        5: [(10, "#..#"), (16, "#.#"), (21, "#.#"), (26, "#.#"), (33, "#.#"), (44, "###/")],
        4: [(0, "####"), (10, "#.b#"), (16, "#..#"), (21, "#..#"), (26, "#..#"), (32, "#..#"), (44, "#Q.##/")],
        3: [(0, "E..#"), (10, "#..#"), (17, "#.#"), (22, "#.#"), (27, "#.#"), (32, "#.#"), (44, "#.O.s##/")],
        2: [(0, "E.R.####"), (10, "#..#"), (17, "#.#"), (22, "#.#"), (27, "#.#"), (32, "#.#"), (44, "#.KK.a.##/")],
        1: [(0, "E.b..O.f." "##" ".." "##" "a" "##" "." "##" "M" "#" "." "#" "BB" "#" "." "##" "s" "#" "." "##" "T" "#######"),
            (44, ".R.m.rr..##/")],
        0: [(0, "E" + "." * 24 + "BB" + "." * 17 + "...H........#")],
        -1: [(0, "E.c..Q.a." "##" ".." "##" "v" "##" "." "####" "." "#" "BB" "#" "." "####" "M" "#" "." "########"),
             (44, ".LL.y.rr.##/")],
        -2: [(0, "E.R.####"), (10, "#..#"), (17, "#.#"), (22, "#.#"), (27, "#.#"), (34, "#.#"), (44, "#.Q.Q.c##/")],
        -3: [(0, "E..#"), (10, "#..#"), (17, "#.#"), (22, "#.#"), (27, "#.#"), (34, "#.#"), (44, "#.M.b##/")],
        -4: [(0, "####"), (10, "#.a#"), (16, "#..#"), (21, "#..#"), (26, "#..#"), (31, "#?##.###=#/"), (44, "#.O##/")],
        -5: [(10, "#..#"), (16, "#.#"), (21, "#.#"), (26, "#e#"), (31, "#.........i"), (44, "###/")],
        -6: [(10, "#i#"), (16, "#i#"), (21, "#.#"), (31, "###=####?#/")],
        -7: [(20, "#..#")],
        -8: [(20, "#x#")],
    })
    d = finish(c)
    # whose bone is whose
    for h in d["hull_cells"]:
        x, y = h["grid_pos"]
        if x <= 9 or x >= 44:
            h["material"] = "Titanium"          # Hegemony engine block and bow
        elif 10 <= x <= 13 and abs(y) >= 2:
            h["material"] = "Steel"             # the square girders
        elif x >= 31 and (y <= -4 or (y <= -2 and 34 <= x <= 36)):
            h["material"] = "Steel"             # the Choir pipe and its lashing
        elif x >= 32 and y >= 2:
            h["material"] = "Composite"         # the rib that bends the wrong way
    return d


def reliquary():
    """Station. A reliquary: a vault of cryo coffins where the cult keeps its
    decayed lords, with four skeletons laid head-outward around it like the
    points of a compass. Each has a spine, three pairs of ribs swept back
    toward the vault, and a plated ram for a skull. One half-arm drawn,
    reflected into a whole arm, then turned four ways."""
    c = Canvas("cs_reliquary", "AbyssalAlloy")
    _segs(c, {
        10: [(16, "##i#")],
        9: [(17, "#.#")],
        8: [(17, "#.#")],
        7: [(17, "#..#"), (24, "##e#")],
        6: [(1, "###"), (11, "#x#"), (18, "#.#"), (25, "#.#")],
        5: [(1, "V.V#"), (11, "#.#"), (18, "#.#"), (25, "#.#")],
        4: [(1, ".$.M#"), (11, "#..#"), (18, "#..#"), (25, "#..#"), (34, "###/")],
        3: [(1, "..V.V#"), (12, "#.#"), (19, "#.#"), (26, "#.#"), (34, "#M.##/")],
        2: [(1, "$.R..#"), (12, "#.#"), (19, "#.#"), (26, "#.#"), (34, "#.e.a##/")],
        1: [(1, "..s.O." "#a###" "Q.Q" "#b##" "m.O" "#s##" "Q.Q" "#a###" ".LL.i..##/")],
        0: [(1, "." * 36 + "rr...###")],
    })
    for (x, y), ch in list(c.cells.items()):   # the other side of the arm
        if y >= 1 and x >= 7:
            c.cells[(y, x)] = ch
    c.rot4()
    c.cells[(0, 0)] = "."
    c.R(-1, 1, 1, 2, "B")                       # the bridge, among the coffins
    return finish(c)


CONCEPTS = [
    ("mourner", mourner, "Mourner", "Ram",
     "A plated ram on a short spine with two pairs of swept ribs. It comes straight at you and goes for the engines."),
    ("pyre", pyre, "Pyre", "Cruiser",
     "A full ribcage: five pairs of swept ribs closed by a thin outer bone, the voids between them open to space. A long plated ram forward."),
    ("dead_sun", dead_sun, "Dead Sun", "Capital",
     "A dead star caged in bone: fourteen ribs curl out of a dark core into an outer ring. The spine runs through it, engines aft, ram forward, bridge in the core."),
    ("ossuary", ossuary, "Ossuary", "Experimental: bone ship",
     "Built from the bones of other ships: a Hegemony engine block and bow, square girders, a rib that bends the wrong way, a Choir pipe lashed on as a rib. Only the spine is theirs."),
    ("reliquary", reliquary, "Reliquary", "Station",
     "A vault of cryo coffins for the cult's decayed lords, with four skeletons laid head-outward round it like compass points, each with a plated ram for a skull."),
]
