"""The Shepherd, round 5a: the flock between the Lamb and the Ram, and a little
above. Eight light and medium ships. Hand-drawn, top-down only.

Story: the Shepherd is an apex hunter that wants you to become one. It engages
anything within 12,000 units, and what it catches it keeps: it gathers ships
into its flock. So its lesser ships are the flock's working animals and its
hands: dogs that run the edge, a bellwether the flock follows, hunters that
cut one ship out of a group, a drover that drives it toward the hook, pens
that hold what was caught.

Same grammar as shepherd.py (FACTION["grammar"] there), held at every size:
  - a STAFF spine and a HOOK (crook, barb, horn, gatepost) open inside, the
    hook's inner face lined with tractor beams `t`;
  - curves as diagonal staircases, plates only on the forward steps and the
    hook tips;
  - attendants launch from cradles `D`; AbyssalAlloy.

New in this round, by the developer's symmetry rule: every skeleton (outline,
engines, main guns, bridge on the centreline amidships) is mirrored port and
starboard, so the crook comes in pairs (horns, barbs, gateposts) or is centred
on the staff. The approved ships curl one way; here a one-sided curl survives
only as a small detail drawn after the mirror.

Legend: see terran.py and draw.LEGEND. 't' tractor beam, 'P' plasma caster,
'i' ion disruptor, 'D' drone bay (cradle), 'g' gatling, 'p' point defence.
"""
from collections import deque

from draw import Canvas
from shepherd import FACTION, MAT, finish as _sh_finish  # noqa: F401  (FACTION is read by build.py)

N4 = ((0, 1), (0, -1), (1, 0), (-1, 0))
MAIN = set("ENPit")          # engines and main weapons must mirror exactly
SIZE = {  # class -> hull cells (BRIEF.md size ladder; attendants a little over the Lamb's 120)
    "attendant": (110, 220), "corvette": (180, 320), "frigate": (350, 520), "destroyer": (500, 720),
}


def finish(c, size, asym=0.06):
    """shepherd.finish (every gun has an open side, every engine clear aft),
    plus: the hull is one piece; the skeleton is mirror-symmetric (a few
    detail cells may differ, at most `asym` of the hull); engines and main
    guns mirror exactly; the bridge is inside the hull, on the centreline,
    near the middle; the hull count fits the class."""
    d = _sh_finish(c)
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
        d["_checks"].append(f"hull in pieces: {len(hull) - len(seen)} cells cut off")
    odd = hull ^ {(x, -y) for x, y in hull}
    if len(odd) > asym * len(hull):
        d["_checks"].append(f"skeleton not mirror-symmetric ({len(odd)} odd cells)")
    for (x, y), ch in c.cells.items():
        if ch in MAIN and c.cells.get((x, -y)) != ch:
            d["_checks"].append(f"'{ch}' at {(x, y)} has no mirror image")
    bridge = [p for p, ch in c.cells.items() if ch == "B"]
    if not bridge:
        d["_checks"].append("no bridge")
    elif any(layer.get(p) == "Outer" for p in bridge):
        d["_checks"].append("bridge on the rim")
    else:
        xs = [p[0] for p in hull]
        mid, half = (min(xs) + max(xs)) / 2, (max(xs) - min(xs)) / 2
        bx = sum(p[0] for p in bridge) / len(bridge)
        by = sum(p[1] for p in bridge) / len(bridge)
        if abs(bx - mid) > 0.45 * half or abs(by) > 0.5:
            d["_checks"].append(f"bridge at ({bx:.0f},{by:.1f}) is off the middle ({mid:.0f},0)")
    for need, what in (("Q", "quarters"), ("O", "oxygen"), ("R", "reactor")):
        if not any(ch in need or (need == "Q" and ch in "Kq") or (need == "R" and ch in "LF")
                   for ch in c.cells.values()):
            d["_checks"].append(f"no {what}")
    lo, hi = SIZE[size]
    if not lo <= len(hull) <= hi:
        d["_checks"].append(f"{len(hull)} hull cells, outside {size} {lo}-{hi}")
    return d


# --------------------------------------------------------------------------- ships
def sheepdog():
    """Picket. The dog that runs the edge of the flock and brings the strays
    back: a small crescent, its round bow out front and two arms trailing
    back round an open pocket either side, each arm's tip turned in as a
    hook, tractors on the inner faces. What the arms sweep up, they hold."""
    c = Canvas("sh_sheepdog", MAT)
    c.rows(-14, 8, """
          #####/
      #####O..##/
    ######t#..M.#/
  ####      t..s.#/
 ###        #..c..#/
 ##/        t.A.M.T#/
       #i###..s..q..#/
      E.R.QBB..O.....P
      E..f.BB.H...M..#
""")
    return finish(c.mirror(), "attendant")


def bellwether():
    """Attendant. The wether that wears the bell, and the flock follows it: a
    short staff with a round lantern-pod held out ahead on a neck, a
    floodlight in its face and a signal buoy ringing in it, and two small
    horns curled back over the shoulders, tractors inside the curls. Where
    the light goes, the caught ships go."""
    c = Canvas("sh_bellwether", MAT)
    c.rows(-10, 7, """
             ####/
            ##  ##/
           ##    ##
          ##     ##   ###/
          ##    ##/  #.J.#/
 ####i####.t        #.M.s.#/
E.R..Q.O.BB.##t#t###...O...i
E..b..c..BBH.........T...M..l
""")
    return finish(c.mirror(), "attendant")


def lurcher():
    """Corvette. The poacher's dog, bred to run down one animal and bring it
    back: a long lean staff, all engine and corridor, ending in a harpoon
    head whose two long barbs sweep back low over the staff, tips turned
    in, open aft. It runs into a group, takes one ship on the barbs, and the
    tractors inside them hold it while it turns for home."""
    c = Canvas("sh_lurcher", MAT)
    c.rows(-15, 6, """
                   ######/
                #####  ####/
              ####      t.M#/
 #######/               t.s.##/
E..c.O..#i####P#########..O...#/
E.R..Q.f.O.KK..BB.T.m.c...A....i
E..b......s....BBH..........M...P
""")
    return finish(c.mirror(), "corvette")


def shears():
    """Corvette. Sheep shears: a looped spring at the stern (the engines on
    its back), a hinge-block amidships with the bridge, and two blades
    running forward with a slit between them, their tips turned in to
    nearly close it. Tractors line the slit; the blades close on one ship
    and cut it from the group."""
    c = Canvas("sh_shears", MAT)
    c.rows(-16, 6, """
                 ####/
  #####          #.M.####/
 #..O..####i#####..T..A..#P#i#/
#..###..Q.O..m.q....Qc........#/
E.#   #.R..f.GG...##t##t##t##..#/
E.#   #....BBGG..P           t###
E.#   #..M.BBH...t
""")
    return finish(c.mirror(), "corvette")


def harrier():
    """Frigate, the herder. A drover walks behind the herd and drives it: a
    broad shallow sweep across the bow, concave, ion disruptors all along
    its face to push and scatter, and at each end the sweep rolls outward
    into a crook, tractors inside, to turn back anything that breaks to the
    side. Whatever is in front of it is driven toward the Shepherd's hook."""
    c = Canvas("sh_harrier", MAT)
    c.rows(-18, 18, """
                       ######/
                      ##  t..#/
                      #   t.O.#/
                      #  #.M.q.#
                         P..s.#/
                        #.Q...i
                        #.c..#/
                       #..m..i
                       #.O..#/
                      P..M..i
                      #.b..#/
                     #..Q..i
                     #.f..#/
     #p#    #p#     P..T..i
 #######i######P####..y..#/
E.R..Q.f.O.KK..Q...A..m..i
E..b.............M.......#
E.R..c..O.T...BB..s..y...i
E.....M.......BBH........P
""")
    return finish(c.mirror(), "frigate")


def hurdle():
    """Frigate, the fence. A hurdle is the woven panel shepherds stake across
    a gap to pen the flock: a short staff crossed by two long rails that
    stand across the ship's course, joined by bars with open windows
    between, every rail end rolled outward into a gatepost crook with
    tractors inside. Cradles on the rails launch the flock. It sets itself
    across a lane, and what runs from the drover runs into it."""
    c = Canvas("sh_hurdle", MAT)
    c.rows(-15, 17, """
    #####/      #####/
   ##  ###     ###  ##/
  ##   t.#     #.t   ##
   ##  t.#     #.t  ##/
       i.#     #.P
       D.#     #.D
       D..#####..D
       i...O.Q...P
       #..#####..#
       #.#     #.#
       D.#     #.P
       D.#     #.#
       #.#     #.i
       #.#     #.#
 ##i###...##p##...###/
E.R..Q.f..O.KK..c.M...#/
E..b..m....BB..O..T.A..#/
E...c.M....BBH.......y..P
""")
    return finish(c.mirror(), "frigate")


def pinfold():
    """Destroyer, the pound. A pinfold is where strays are penned until
    they are claimed: a long staff carrying a round pen at its bow, a wall
    all the way round but for a narrow gate in front, the wall's ends turned
    in at the gate like barbs so what goes in does not come out. Tractors
    line the whole inside of the wall. Cradles under the staff carry its
    flock."""
    c = Canvas("sh_pinfold", MAT)
    c.rows(-34, 11, """
                               ###P###/
      #########             #i#Q.M.O.Q#i#/
      #.......#           ##c.KK..s..m..c##/
      #DD#DD#.#          P.................#/
      ##    #.#         #.O..R.#t#t#t#.R..O.#
            t.#         #.q.M.t       t.M.q.#/
 ####i#######.####P####i.b.m.t         t.Q.b.P
E.R.Q.f.O.KK...Q.GG.m...c...t           t.f..i
N.b..y...M.......GG....O.A..#           #.O.#/
N.R..................M......t          #####/
E..c..O..q...s..T..O..BB....#
E.LL....M.............BBH...t
""")
    return finish(c.mirror(), "destroyer")


def briar():
    """Destroyer, the snare. Sheep that stray into the briar are held there
    by the fleece until the shepherd comes: a long thorned stem, its waist
    swollen amidships and tapering to a plated point, four pairs of hooked
    thorns along it, each a strut curling aft into a barb with a tractor in
    its crook. A ship that passes close is snagged, and held, and the stem
    keeps going."""
    c = Canvas("sh_briar", MAT)
    c.rows(-27, 8, """
             #####/   #####/   #####/
 #####/      # t..#/  # t..#/  # t..#/
 # t..#/     #  t..#  #  t..#  #  t..#
 #  t..#        ##..#    ##..#    ##..#
    ##..#  ##it###..##it###..##it###..##/
 it###..###.Q..O.m..Q.c.M.f..Q.b.O.A..y.##P#/
E..R....c.......s................M.......T..#i#/
E.Q..O.f.b..Q.y.KK..O...BB..q..m..T..Q..c.A...s#i/
E..LL........M..........BBH..........s.....M.....P
""")
    return finish(c.mirror(), "destroyer")


CONCEPTS = [
    ("sheepdog", sheepdog, "Sheepdog", "Picket",
     "A small crescent that runs the edge of the flock, its round bow out front and two arms trailing back round open pockets, the tips turned in as hooks. What the arms sweep up, the tractors hold."),
    ("bellwether", bellwether, "Bellwether", "Attendant",
     "The wether that wears the bell: a round lantern-pod held out on a neck, a floodlight in its face and a signal buoy ringing inside, two horns curling forward round its neck. Where the light goes, the caught ships follow."),
    ("lurcher", lurcher, "Lurcher", "Corvette",
     "The poacher's dog: a long lean staff ending in a harpoon head, two long barbs swept back low over the staff. It runs into a group, takes one ship on the barbs, and turns for home."),
    ("shears", shears, "Shears", "Corvette",
     "Sheep shears: a looped spring at the stern, two blades forward with a slit between them, tips turned in to nearly close it. Tractors line the slit; the blades close on one ship and cut it from the group."),
    ("harrier", harrier, "Harrier", "Frigate (herder)",
     "A broad concave sweep across the bow, ion disruptors along its face, each end rolling outward into a crook with tractors inside. It drives whatever is in front of it toward the Shepherd's hook."),
    ("hurdle", hurdle, "Hurdle", "Frigate (fence)",
     "The woven panel shepherds stake across a gap: two long rails standing across its course, joined by bars round open windows, every rail end rolled into a gatepost crook. It sets itself across a lane, and what runs from the drover runs into it."),
    ("pinfold", pinfold, "Pinfold", "Destroyer (pound)",
     "Where strays are penned until claimed: a round pen on a long staff, walled all round but for a narrow gate in front, the wall's ends turned in like barbs. Tractors line the whole inside of the wall."),
    ("briar", briar, "Briar", "Destroyer (snare)",
     "A long thorned stem with four pairs of hooked thorns curling aft along it, a tractor in every crook. Whatever passes close is snagged, as sheep are on the briar, and held until the Shepherd comes."),
]
