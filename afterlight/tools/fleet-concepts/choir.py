"""Broken Choir: hand-drawn, top-down only.

Lore: a galactic telepathic gestalt that shattered. Millions of voices in
screaming consensus; they hear everyone's thoughts and can't turn it off. The
shattering was intentional. Their ships are ghost ships: already damaged,
erratic, never fleeing, attacking anything.

Grammar, held at every size (all of it reads from above):
  - SHARDS: every hull is broken into solid fragments with deliberate
    crack-gaps of open space between them;
  - STRUTS: the shards are held together only by thin 3-wide bridges, so the
    ship is still one piece, barely;
  - ORGAN PIPES: long thin 3-wide resonance spines laid side by side at
    staggered lengths, studded with listening gear;
  - BROKEN SYMMETRY: nearly mirrored, then snapped: a pipe missing here, a
    shard drifted there;
  - Steel, heavy missiles, cluster rockets and gatlings.

Legend (draw.py): # hull  . deck  / plate  E engine  N large engine  R reactor
L large reactor  b battery  f fuel  c cooling  v vent  B bridge  H helm
M memory core  s shield  A radar  ? passive radar  = jammer  J signal buoy
T targeting  Q quarters  K barracks  G galley  m medbay  O oxygen  a ammo
x heavy missile  * cluster rocket  g gatling  d door  y repair
"""
from collections import deque

from draw import Canvas

FACTION = {
    "slug": "broken_choir",
    "name": "Broken Choir",
    "tagline": "A telepathic gestalt that broke itself on purpose. Millions of voices, one scream; their ghost ships never flee.",
    "grammar": [
        "Every hull is broken into solid shards with crack-gaps of open space between them",
        "Shards held together only by thin 3-wide struts: one piece, barely",
        "Organ-pipe resonance spines: long 3-wide pipes side by side at staggered lengths",
        "Broken symmetry: nearly mirrored, then snapped, a pipe missing or a shard drifted",
        "Steel; heavy missiles, cluster rockets and gatlings",
    ],
    "accent": "#9fc4a8",
}

N4 = ((0, 1), (0, -1), (1, 0), (-1, 0))


def finish(c):
    """Build, then add the checks the shard grammar needs on top of draw.py's:
    the hull must still be one piece, and no engine may fire into its own hull."""
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
        if ch in "EN" and any((xx, y) in hull for xx in range(x - 40, x)):
            d["_checks"].append(f"engine at {(x, y)} vents into hull")
    return d


def _segs(c, rows):
    """Paint runs of cells row by row: {y: [(x, "cells"), ...]}. The same as
    drawing it as text, but each run says where it starts, so long sparse
    rows don't depend on counting spaces."""
    for y, runs in rows.items():
        for x, s in runs:
            c.P(x, y, s)


def shard():
    """Corvette. Two shards split by a slanting crack, one strut across it,
    and a bank of three organ pipes run out of the fore shard at three
    lengths."""
    c = Canvas("bc_shard", "Steel")
    c.rows(0, 6, """
  #g#     ####/
 #M..#   #Q.m.#?##M##/
E.R..b#  #...........g
#.Q.O.#  #.c.##=#####/
#.m.f#  #.H.#
E...b.##.BB..#?###M####=##/
E.R......BB...............x
E.c.###..BBa.#####?#######/
#.O#  #.Q.m#
#.M#  #.f.M.##=###/
 ##  #............*
     #.R.b#####?##/
      #g##/
""")
    return finish(c)


def chorus():
    """Frigate. Three shards strung on one spine pipe, wide cracks between
    them: a leaning engine slab, a tall bridge shard with a horn, and a
    triangular bow shard that the organ pipes run out of."""
    c = Canvas("bc_chorus", "Steel")
    c.rows(0, 10, """
                #g/
                #.#/
                #..#*/
###            #Q.M..##/
#M#g###        #...Q.m.##
#.f.c.b###     #O.GG.s.A#      #?##M##=#/
E.R..LL.v.##   #..GG..T.#   ###.........g
E...b..f...#  #......M.a#  #.M....#=####/
#.c.O.M...#   #KK....y.a#  #.a.a.#
E..........###..H.BB.s.###.T.s....##?###=###/
E.................BB........................x
E.R..LL.b.###.KK..BB.T.##.f.a.a..###?#######/
E...c..f.#  #.Q.....O.#  #.Q.M..#
#.Q.m.Q.#  #.M.b..s.b.#  #.......####/
 #..M.O.#  #..Rc...Q.#   #.R.........*
  #g...#   #.b.v..a.m#    #.##=######/
    ####   #g###..O..#    #g/
               ##x##.#
                   #g#
""")
    return finish(c)


def _pipe(c, yc, x0, tip, weapon, gear=0):
    """One organ pipe: 3 wide, deck down the middle, listening gear studded
    along both rims at a fixed pitch, a weapon at the mouth flanked by two
    plates. gear shifts where along the pipe the studs fall."""
    for x in range(x0, tip):
        c.cells[(x, yc + 1)] = "#"
        c.cells[(x, yc)] = "."
        c.cells[(x, yc - 1)] = "#"
        k = (x - x0 + gear) % 6
        if x < tip - 2:
            if k == 2:
                c.cells[(x, yc + 1)] = "?"
            elif k == 5:
                c.cells[(x, yc - 1)] = "="
    c.cells[(tip, yc)] = weapon
    c.cells[(tip, yc + 1)] = "/"
    c.cells[(tip, yc - 1)] = "/"


def requiem():
    """Capital. A cathedral organ: a console of three shards, cracked apart
    and joined by two struts, sounding a bank of seven pipes. One pipe has
    snapped; its mouth still hangs off its neighbour by a strut."""
    c = Canvas("bc_requiem", "Steel")
    c.rows(0, 18, """
       #gx*/
    ###...M##/
    E.R.Q.Q.O#/
    E.........#g/
    E.R.bc.GG.m.#/
     #.f.v.GG.a.M#
     E............
     E.Q.Q.KK.O.s#
     ##...f.b.c.#
       #....T..A.#
        #..#M#....
 #g#    #.#   ####
 #.#?#=#.d.#
 E.R.Q.Q..m.#?##=#
 N................
 N.LL.a.GG.W.M.a..
 #.bc.a.GG.T.s.y.#
 N.R.KK.HBB.@@.A..
 N.......BB.......
 E.R.KK..BB.q.m.M.
 #.bf.O.Q.Q.v.c.s#
  E.LL.a.M.a.?.J..
  E...............
   E.O.b..#=####?#
   #g##.d#
       #.#    ####
       #.# #M#....
       #..#..T.A.#
     ##..Q.Q.b.O#
     E.R.f.c.KK.s#
     E............
     E.R.GG.m.a.M#
     E.b.GG.v...g/
     ###.Q.Q..##/
        #g..g#/
          x*
""")
    for yc, tip, weapon, gear in ((12, 36, "*", 0), (8, 44, "x", 3), (4, 51, "x", 1),
                                  (0, 57, "x", 4), (-4, 48, "x", 2), (-12, 38, "*", 5)):
        _pipe(c, yc, 18, tip, weapon, gear)
    c.P(30, 1, "M")
    c.P(41, -1, "M")
    # the snapped pipe: a stub with a ragged end, a gap, then its mouth,
    # hung off the next pipe up by one strut
    _pipe(c, -8, 18, 30, "#", 1)
    c.K(30, 30, -9, -7)
    c.P(29, -8, "#")
    c.P(30, -9, "#")
    _pipe(c, -8, 33, 41, "g", 0)
    c.K(33, 33, -7, -7)
    c.P(33, -8, "#")
    c.P(35, -6, "#.#")
    return finish(c)


def echo_chamber():
    """Experimental. The Choir's voice weapon: a megaphone that has burst. A
    reed pipe and four launchers in the throat fire down the bore of a horn
    whose walls have cracked into pieces, each held on by one strut. The
    lower lip broke short; the piece it lost hangs off it on a strut."""
    c = Canvas("bc_echo_chamber", "Steel")
    # runs are split where one shard ends and a strut or the next shard begins
    _segs(c, {
        20: [(34, "#g#/")],
        19: [(31, "###...#")],
        18: [(29, "##.M....#/")],
        17: [(30, "#.a.O.Q.x")],
        16: [(23, "g#"), (30, "#.b.?.a.x")],
        15: [(20, "###..##" "###" "..s.M..#" "/")],
        14: [(20, "#.Q.m.." "..." "...y...g")],
        13: [(21, "#.GG.." "###" "#.T..##" "/")],
        12: [(12, "##g##"), (21, "#.GG.#"), (31, "g###/")],
        11: [(10, "##.Q.M.#" "###" ".O.s.a#")],
        10: [(10, "#.b.c.f." "..." "..KK..*")],
        9: [(10, "#.Q.m.R." "###" "#g.#x#")],
        8: [(10, "#.O.a.vg"), (23, "g")],
        7: [(10, "#..KK.f.g")],
        6: [(2, "#g##g###" ".R.M.Q." "##")],
        5: [(0, "##..LL.Q.O.y..g##")],
        4: [(0, "E.c...f.b....g")],
        3: [(0, "E.Q.m.Q.a...*")],
        2: [(0, "#.s.A..H....x")],
        1: [(0, "E.R..BB.W...." "#?###M###=##" "/")],
        0: [(0, "E....BB" + "." * 18 + "x")],
        -1: [(0, "E.R..BB.T...." "###=####?###" "/")],
        -2: [(0, "#.s.O..q....x")],
        -3: [(0, "E.Q.m.Q.a...*")],
        -4: [(0, "E.c...f.b....g")],
        -5: [(0, "##..LL.Q.O.y..g##")],
        -6: [(2, "#g##g###" ".R.M.Q.#")],
        -7: [(10, "#..KK.g")],
        -8: [(10, "#.O.a.g"), (22, "g#")],
        -9: [(10, "#.Q.m.." "###" "##..x#")],
        -10: [(10, "#.M.bc." "..." ".Q..#")],
        -11: [(10, "##.Q.f#" "###" ".GG.#"), (30, "g#")],
        -12: [(12, "#g##"), (20, "#GG.s#" "##" "##..#")],
        -13: [(20, "#.O.m." ".." ".M.a#")],
        -14: [(20, "#.Q.##" "##" "..y..#"), (37, "##/")],
        -15: [(20, "#g##"), (28, "#.b.R." "##" "#..#")],
        -16: [(29, "#c..." ".." "...x")],
        -17: [(30, "#.M#" "##" "...#")],
        -18: [(31, "g#"), (36, "#.#/")],
        -19: [(37, "g#")],
    })
    return finish(c)


def broken_spire():
    """Station. A spire seen from straight above is a pyramid: four faces
    meeting at the apex. This one has shattered along its four edges, so the
    faces are held only where they meet the hub, and each face sounds its own
    fan of organ pipes, long, short, shorter, turning like a pinwheel. One
    quadrant drawn and turned four ways; then a pipe snapped and one blown off."""
    c = Canvas("bc_broken_spire", "Steel")
    c.rows(1, 30, """
/
#
#
?
#
#
=
# /*/
# #.#
# ?.#
# #.#
M #.=
# #.# /g/
# #.# #.#
= ?.# #.?
# #.# #.#
# #.= =.#
# #.# #.#
##A.A##.###
.Q...Q...g
O.KK.m.b#  o
..Q.GG.g  #.##?####/
M.Q.GG#  g.........g
..R.c#  #a.M####=##/
b..f#  #Q.Q#
s.O#  #.R.y.###?####=####/
..#  #...................*
..# #.T.LL..####=####?###/
...#.Q.O.s.#
...KK.m.a.b.##?####=####?####/
.............................x
""")
    c.rot4()
    c.cells[(0, 0)] = "."
    c.R(-1, 1, 1, 2, "B")            # the bridge, in the apex
    # the west pipe has snapped: a gap, and its mouth hung off the pipe below
    c.K(-22, -21, -1, 1)
    c.P(-25, -2, "#.#")
    # the south fan's short pipe is gone altogether
    for (x, y) in list(c.cells):
        if 7 <= x <= 9 and y <= -13:
            del c.cells[(x, y)]
    return finish(c)


CONCEPTS = [
    ("shard", shard, "Shard", "Corvette",
     "Two shards split by a slanting crack with one strut across it, and three organ pipes run out of the bow at three lengths."),
    ("chorus", chorus, "Chorus", "Frigate",
     "Three broken shards strung on one spine pipe with open cracks between them. The bridge rides the tall middle shard."),
    ("requiem", requiem, "Requiem", "Capital",
     "A cathedral organ: three cracked console shards sounding a bank of seven pipes. One pipe has snapped and its mouth hangs off its neighbour."),
    ("echo_chamber", echo_chamber, "Echo Chamber", "Experimental: voice weapon",
     "A megaphone that has burst: horn walls cracked into pieces held by single struts, the lower lip broken short with its lost piece hanging off it. A reed pipe and four launchers fire down the bore."),
    ("broken_spire", broken_spire, "Broken Spire", "Station",
     "A spire seen from above: a pyramid of four faces, shattered along its edges and held only at the apex. Each face sounds a fan of organ pipes; one has snapped, one is gone."),
]
