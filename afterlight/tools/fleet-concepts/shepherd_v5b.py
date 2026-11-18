"""The Shepherd, the heavy hulls and the kept ships (v5b). Hand-drawn,
top-down only.

Same grammar as shepherd.py (FACTION["grammar"] there), held at every size:
  - a STAFF spine, and at the bow a HOOK, open inside, its inner face lined
    with tractor beams `t`;
  - curves are diagonal staircases; plates only on the forward steps and the
    hook tips;
  - smaller hooks (barbs, horns, gateposts) repeat the crook;
  - the flock's Lambs launch from cradles `D`;
  - AbyssalAlloy.

What this round adds. By the developer's symmetry rule every skeleton is
drawn on y >= 0 and mirrored, so the crooks come in pairs (a lyre, an
anchor's flukes, an angler's horns, a fleece of curls), and the one-sided
curl of the approved ships lives on only in small details drawn after the
mirror (one pod of the Kept Hauler still in its old steel).

The kept ships. What the Shepherd catches it keeps, and what it keeps
becomes the flock. The three non-combat hulls were traders once: each keeps
its hauler frame (pods on a spine, a tanker's tank train, a barge), re-hulled
in AbyssalAlloy, with the faction's mark fused onto the bow, a NOSE RING:
a small crook from each side curling round an open ring, tractors inside.
They carry for the flock now, and their crews ride in cryo and memory.

Lore: an apex boss (danger 8) that "wants you to become one"; it engages
anything within 12,000 units and what it catches it keeps. Shepherd ships
preach that the only peace is unity, and the only unity is upload.

Legend (draw.py): # hull  . deck  / plate  d door  E engine  N large engine
R reactor  L large reactor  b battery  f fuel  c cooling  B bridge  H helm
M memory core  s shield  A radar  T targeting  Q quarters  q officer quarters
K barracks  G galley  m medbay  O oxygen  y repair  a ammo  V cryo
k small cargo  C bulk cargo  U large cargo  J signal buoy  l floodlight
t tractor beam  P plasma caster  i ion disruptor  D drone bay (cradle)
g gatling  p point-defence drone
"""
from collections import deque

from draw import Canvas
from shepherd import FACTION, MAT, plate_front  # noqa: F401  (FACTION is read by build.py)

N4 = ((0, 1), (0, -1), (1, 0), (-1, 0))
SKELETON = set("ENtPiD")          # these must have a mirror image
SIZE = {  # class -> hull cells (BRIEF.md ladder; the approved Ram is 314, The Shepherd 1788)
    "trader": (200, 700), "cruiser": (700, 950), "battleship": (1050, 1350),
    "carrier": (850, 1250), "experimental": (600, 1250),
}


# --------------------------------------------------------------------------- helpers
def finish(c, size, details=0.10, patches=()):
    """Build, then check what draw.py leaves out: the hull is one piece; the
    skeleton is mirror-symmetric (hull to within `details` of its cells,
    engines and main guns exactly); every gun has an open side; no engine
    fires into its own hull; the bridge is inside the hull near the middle;
    the hull count fits the class. `patches` (x0, x1, y0, y1, material)
    leave parts of a kept ship in the material it was built in."""
    d = c.build()
    for cell in d["hull_cells"]:
        x, y = cell["grid_pos"]
        for x0, x1, y0, y1, mat in patches:
            if x0 <= x <= x1 and y0 <= y <= y1:
                cell["material"] = mat
    hull = {tuple(h["grid_pos"]) for h in d["hull_cells"]}
    layer = {tuple(h["grid_pos"]): h["layer"] for h in d["hull_cells"]}
    plates = {p for p, ch in c.cells.items() if ch == "/"}
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
    odd = len(hull - {(x, -y) for x, y in hull})
    if odd > details * len(hull):
        d["_checks"].append(f"skeleton not symmetric: {odd} hull cells without a mirror image")
    for (x, y), ch in c.cells.items():
        if ch in "EN" and any((xx, y) in hull for xx in range(x - 90, x)):
            d["_checks"].append(f"engine at {(x, y)} vents into hull")
        if ch in SKELETON and c.cells.get((x, -y)) != ch:
            d["_checks"].append(f"'{ch}' at {(x, y)} has no mirror image")
        if ch in "tPigpnX" and all((x + dx, y + dy) in hull or (x + dx, y + dy) in plates for dx, dy in N4):
            d["_checks"].append(f"'{ch}' at {(x, y)} has no open side")
    bridge = [p for p, ch in c.cells.items() if ch == "B"]
    if not bridge:
        d["_checks"].append("no bridge")
    elif any(layer.get(p) == "Outer" for p in bridge):
        d["_checks"].append("bridge on the rim")
    else:
        xs = [p[0] for p in hull]
        mid, half = (min(xs) + max(xs)) / 2, (max(xs) - min(xs)) / 2
        bx = sum(p[0] for p in bridge) / len(bridge)
        if abs(bx - mid) > 0.45 * half:
            d["_checks"].append(f"bridge at x={bx:.0f} is far from the middle ({mid:.0f})")
        if abs(sum(p[1] for p in bridge) / len(bridge)) > 1:
            d["_checks"].append("bridge off the centreline")
    lo, hi = SIZE[size]
    if not lo <= len(hull) <= hi:
        d["_checks"].append(f"{len(hull)} hull cells, outside {lo}-{hi}")
    return d


def plates_at(c, cells):
    """Hand-placed plates, both sides (call after mirror())."""
    for x, y in cells:
        c.cells[(x, y)] = "/"
        c.cells[(x, -y)] = "/"


# --------------------------------------------------------------------------- ships
def lyre_cruiser():
    """Cruiser. A lyre: the staff forks at the bow into two crooks that
    sweep out and forward and curl in toward each other, tips almost
    meeting. Inside them, ahead of the staff, an open pocket a ship wide,
    tractors all round its inner face and an ion gun in the staff's blunt
    bow to still what is held. The only way in is the mouth between the
    tips, and the tips are tractors too."""
    c = Canvas("sh_lyre_cruiser", MAT)
    c.rows(-35, 17, """
                                                 ##P###
                                              ###Q..O.Q##
                                          #i##...d.......#
                                         #Q...Q.s.#t#a.O..#
                                        #..Q.a#t##   #t.a.#
                                       #..O#t#         #Q..P
                                      PQ..t            t..s#
                                     #a..M#            #O.a#
                                     #..Qt             t...i
                                    i..O#              #Q.M#
                                   #...t               t...P
                                  P..Qt         #t##t##.a.#
    #######P#####i#####P#####i####...t         #.Q.O..Q..s#
  ##f.Qc.bQ.aa.GG.Qs.V..VQ.M..sQ.a...#         t..........P
##cb..cf.Q..KK.GG.Q.O.m..Qq.s.A.T.a..t         #f.QM.aO.c#
N..............d................d....#          #t##t##t#
Nb.R.R.b.LL.c.Q.O.y.q.m.s.A.BB.T.a.s.t
E.....R.f..M.KK....M........BBH......i
""")
    c.mirror()
    plate_front(c, test=lambda p: p[0] >= 20)
    plates_at(c, [(12, 6), (12, 2)])
    return finish(c, "cruiser")


def great_hook():
    """Battleship. An anchor, the greatest hook a ship can carry: a long
    staff, a crossbar stock at the stern carrying the engines (its ends
    curl forward into small crooks), and at the bow a round crown whose two
    arms sweep back along the staff. Each arm ends in a fluke barbed back
    toward the staff, so the pockets between arm and staff open aft: it
    runs past what it hunts, and what the tractors pull into the pockets
    can't slide out."""
    c = Canvas("sh_great_hook", MAT)
    c.rows(-44, 19, """
                                             #P#
                                        ##i##...##i##
                                      #PQ..OQ..MQa.Q.P#
                                   #i#.................##
       ###                         #.QO.aQ.#t##t##..OQ..a#
  #########                       #..a.##t#       #t#...Qa#
 #..OQ######                     #...#t              #t..Os#
#.L.Q#   ###                    #..#                   t..Q.P
#.L.a#    ##                    #.#                     t..Ma#
E.c.O#                          #.#                      #..Q.i
#.R.QP                          #t#                       t..O#
N.bdf#                                                     #...P
N.R.M#                                                     t..a#
#.L.c#                                                      #...i
N.L...########P########i#####P#######t######t########t####t#s..M#
N...Q.ff.QO.KK.Q.aGG.Qs.VV.qm.QO.M.Qa.VQ.s.O.Qa.TQ.yM.Qa.OQ.s.aQ#
#.b.R.cb.Q..ay.Q..GG.Q..Q..qm.Q..A.Q..VQ..O.Q..TQ..a.Q..OQ..f.b.#
N.......d...............d...............d.............d.........#
N..R.LL.c.R.b.f.KK.O.y.Q.m.A.T.BB.s.Q.a.a.O.Q.y.a.s.Q..O.b.c.R.s.P
E...R...M...R.......M.....O....BBH.......M.......................i
""")
    c.mirror()
    plate_front(c, test=lambda p: p[0] >= 9)
    plates_at(c, [(-33, 14)])
    return finish(c, "battleship")


def flock_mother():
    """Carrier. A ewe in full fleece: a broad oval body, and standing up
    from each flank three crooks, small, large, small, each curling aft over
    an open pocket. The pockets are cradles: a drone bay under every curl
    launches a Lamb, a tractor on the stem and the tip draws it home, and
    the Lambs ride there between hunts the way the boss carries its own."""
    c = Canvas("sh_flock_mother", MAT)
    c.rows(-29, 17, """
                       #P#
                     ##...##
         #P#        #.......#        #P#
       ##...##      #..#t#..#      ##...##
      #...t...#    #..#   #..#    #...t...#
      #..# #..#    #..#   #..#    #..# #..#
      #.#   #.#    #..#   #..#    #.#   #.#
      #D#   #.#     #D#    #.#    #D#   #.#
      #D#   #.#     #D#    #.#    #D#   #.#
       #    #.#      #     #.#     #    #.#
            #.#            #.#          #.#
            #.#P##P####i####.##P####i####.#
       ##P##........d.............d........##P##
   ##P#.GG.Qa.Q.a.mSSS.QGG.VV.QVV.Qa.QQ.Qs.VV.M.i###
###cf.Q.GG.QO.QKK.mSSS.QGG.VV.QVV.QO.KK.Qa.VV.QO.bcf###
N..............d......................d................#
N.b.LL.c.LL.f.O.y.m.Q.A.T.s.BB.s.Q.a.O.q.a.y.Q.a.O.s.a..P
E..R..M...R...KK..M....O....BBH....O...M...q...T...M.....i
""")
    c.mirror()
    plate_front(c, test=lambda p: p[0] >= 17 or abs(p[1]) >= 12)
    return finish(c, "carrier")


def bleater():
    """Experimental: the distress lure. An angler: two great crooks curl
    forward from the stern into a wide crescent, their tips hooked in, and
    out of the crescent's heart a long thin staff reaches far past them to
    a small round pod. The pod is the bait. It cries like a lamb, a
    distress call on every band from its signal buoys, its floodlights
    flickering like fires in a wreck, and whatever comes to help it passes
    between the horns. The tractors on the horns' inner faces do the rest."""
    c = Canvas("sh_bleater", MAT)
    c.rows(-17, 15, """
                  ###P###
               ###Q.O.Q.k###
             #iQ.a..M...O.Q.P#
            P.......d.........i
           #.Q.VV.QO.V.VQ.a..#
          #..O.Q.f#t##t##s..#
         #..Q.m.t#       ##.#
        P..O.Q.t          #.#
        #.Q.a.t           #t#
  ###i##..M.Qt
 E.b.R.LL.OQt
E.c.KK.GG.Qy#                                  #l#
E.R.QO.GG.Qm.####t###t####                   ##J.J##
N.....d..................#                   #k....l
N.LL.b.Q.A.T.s...BB.Q.O.s.###################.J.A.J.#
E..M..R..M....O..BBH.......d...............d....M...l
""")
    c.mirror()
    plate_front(c, test=lambda p: p[0] >= 6 and abs(p[1]) >= 8)
    return finish(c, "experimental")


def kept_hauler():
    """Trader (kept hauler). A saddle-bag freighter the Shepherd caught and
    kept: a plain spine with two long cargo pods slung on short necks each
    side, re-hulled in the Shepherd's alloy all but one pod, still in the
    steel it was built in. On its bow the flock's mark, a nose ring: two
    small crooks curling round an open ring, tractors inside, a gatling on
    top. Its crew ride in cryo now; it carries for the flock."""
    c = Canvas("sh_kept_hauler", MAT)
    c.rows(-22, 8, """
     ##########     ##########
     #UU.UU.UU.#    #UU.UU.UU.#
    #k.........k#  #k.........k#
    #####...#####  #####...#####  ##g##
         #.#            #.#      #.O.Q.#
 #########.##############.#######..#t#..#
N............d.............d......#   #t#
N.R.bc.fQ.O.VV.Qm.ABB.QVV.Os.a.k.t
E..LL.M..kk.M.q..O.BBH..M.kk.V...i
""")
    c.mirror()
    plate_front(c, test=lambda p: p[0] >= 12)
    return finish(c, "trader", patches=[(-18, -6, -8, -4, "Steel")])


def kept_tanker():
    """Trader (kept tanker). A tanker the Shepherd caught and kept: three
    round tanks strung on a spine like beads, the crew's sphere in the
    middle (cryo, the bridge, a memory core for whoever chose to stay
    awake), fuel fore and aft, and the engine block at the stern. Re-hulled
    in the Shepherd's alloy, the flock's nose ring on its bow. It fuels the
    flock."""
    c = Canvas("sh_kept_tanker", MAT)
    c.rows(-31, 7, """
           ###          #p#          ###
         ##f.f##      ##s.s##      ##f.f##
        #fff.fff#    #Q.O.A.Q#    #fff.fff#   ##g##
 ##### #ffff.ffff#  #Q.V.M.V.Q#  #ffff.ffff# #.O.Q.#
E.R.O# #ffcf.fcff#  #Q.VV.VV.Q#  #ffcf.fcff.#..#t#..#
N.bcR.#ff.ff.ff.ff##Q.O.....m.Q##ff.ff.ff.b...#   #t#
N........................BB..................t
E.LL.M.ff.ff.ff.M..O.Q.q.BBH.V..O.ff.M.ff.c..#
""")
    c.mirror()
    plate_front(c, test=lambda p: p[0] >= 14)
    return finish(c, "trader")


def kept_barge():
    """Trader (kept barge). A broad cargo barge the Shepherd caught and
    kept, pushed by its own stubby tug: a wide raked deck with two open
    cargo wells a side. The Shepherd's alloy has grown over it, and the deck
    edge now curls over each well like a crook, so the wells open only at
    their aft corners; tractors line them and hold what the flock has
    caught. The flock's nose ring is on its bow. It hauls the catch home."""
    c = Canvas("sh_kept_barge", MAT)
    c.rows(-28, 9, """
           ##  #####p######  #######
         ##O#  #..........#  #......##
        #Qc.#  ##t##.Q.M.Q#  ##t##.Q.a##
       #Q.a.#       #O.V.Q#       #Q.V.O#
       #Q.O.t       t.KK..t       t.Q.m.Q#     ##g##
 ######.CC.Q#       #CC.Q.#       #CC.Q.k.#   #.O.Q.#
E.fR.QO.CC.Q.##t#t##.CC.QO.##t#t##.CC.Qa.f.# #..#t#..#
N..........d..........d...............d.....#..#   #t#
N.R.bc.Q.kk.k.kk.k.Q.A.sBB.Q.kk.k.O.kk.k.kk.a.t
E.LL.M.R.UU.kk.UU.M.O.q.BBH.V.UU.M.UU.kk.UU...i
""")
    c.mirror()
    plate_front(c, test=lambda p: p[0] >= 9)
    return finish(c, "trader")


CONCEPTS = [
    ("lyre_cruiser", lyre_cruiser, "Lyre", "Cruiser",
     "Two crooks curled into a lyre round an open pocket ahead of the bow, tractors all round its inner face. The only way in is the mouth between the tips, and the tips pull."),
    ("great_hook", great_hook, "Great Hook", "Battleship",
     "An anchor: a crossbar stock of engines at the stern, a long staff, and a round crown whose arms sweep back to barbed flukes. It runs past what it hunts; what the tractors draw into the pockets cannot slide out."),
    ("flock_mother", flock_mother, "Flock-Mother", "Carrier",
     "A ewe in full fleece: a broad oval body with three crooks standing from each flank, curling aft over open cradles. A Lamb launches from under every curl, and the tractors on stem and tip draw it home."),
    ("bleater", bleater, "Bleater", "Experimental: distress lure",
     "An angler: two great crooks curl forward into a crescent, and from its heart a long staff reaches past the horns to a small pod that cries like a lamb on every band, its lights flickering like a wreck's fires. Whatever comes to help passes between the horns."),
    ("kept_hauler", kept_hauler, "Kept Hauler", "Trader (kept hauler)",
     "A saddle-bag freighter it caught and kept: two long pods a side on a plain spine, one still in its old steel, and on the bow the flock's nose ring, tractors inside. Its crew sleep in cryo. It carries for the flock now."),
    ("kept_tanker", kept_tanker, "Kept Tanker", "Trader (kept tanker)",
     "A tanker it caught and kept: three round tanks strung on a spine like beads, the crew asleep in the middle one, the flock's nose ring on the bow. It fuels the flock."),
    ("kept_barge", kept_barge, "Kept Barge", "Trader (kept barge)",
     "A broad cargo barge it caught and kept, pushed by its own tug. The deck edge has grown over its cargo wells like crooks, tractors line them, and the flock's nose ring is on its bow. It hauls the catch home."),
]
