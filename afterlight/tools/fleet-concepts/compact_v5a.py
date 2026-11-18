"""Free Haulers' Compact, round 5a: eight small and medium working hulls.
Hand-drawn, top-down only.

Story: a union of two hundred million haulers, miners and their families,
moving fuel, ore and salvage. Ships are handed down through families, patched
and practical, with family homes aboard; no luxury, no military polish, and
the freight shoots back.

The faction's grammar (compact.py) holds at every size: a stubby chamfered
PUSH-TUG at the stern shoving an open LOAD FRAME (a three-wide keel, ribs and
rails, open sky between) that ends in a stepped, plated BUMPER bow; loads
strapped into the frame on one-cell straps, a gap all round; BOLT-ON engine
pods on struts; a patchwork Steel hull mended with Titanium and Composite.

New in this round, by the developer's symmetry rule: every skeleton (outline,
engines, guns) is mirrored port/starboard. Bolt-on pods now come in pairs.
The loads still never match fore to aft (each bay holds a different kind of
load), and the mismatch that used to be port/starboard now lives in small
deliberate details drawn after the mirror: what is in a hopper, a family
cabin, a patched panel.

Legend: see terran.py and draw.LEGEND.
"""
from draw import Canvas
from compact import FACTION, _patched  # noqa: F401  (FACTION is read by build.py)


def _plates(c, cells):
    """Plates on the bumper's bow steps, both sides."""
    for x, y in cells:
        c.P(x, y, "/")
        c.P(x, -y, "/")


def _bumper(c, aft, rows):
    """The push-bar bow, top half (as compact.py): aft[y] is the bar's aft x
    on row y, rows[y] what it holds, aft to front."""
    for y, s in rows.items():
        c.P(aft[y], y, s)


def hotshot():
    """Courier. Trucker's slang for a rush load on a light rig. A small tug
    with two engine pods bolted up on its stern quarters, a bare keel with
    one pay-and-parcel bin strapped either side, and a small bumper: all
    stern, like a dragster."""
    c = Canvas("fhc_hotshot", "Steel")
    # bolt-on pod up on the stern quarter, on a short strut
    c.rows(-11, 7, """
 ###
E..#
EM.#
 #.#
""")
    # the tug (top half) and the keel
    c.rows(-12, 3, """
 ##d##
E.R.Q.#
E.c.BB.#
E..HBB.d
""")
    c.P(-4, 1, "#" * 11)
    c.P(-4, 0, "." * 11)
    # the bumper: a four-thick push-bar bent into a shallow chevron
    c.rows(4, 5, """
##g#
 #..#
  #..#
  #M.#
   #..#
   ...o
""")
    # a pay-and-parcel bin strapped to the keel
    c.rows(-2, 4, """
 ###
#k$k#
 # #
""")
    c.mirror()
    _plates(c, [(10, 2), (9, 4), (8, 5)])
    c.P(-8, -2, "q")            # the skipper's cabin, one side only
    c.P(0, -3, "k")             # the other bin is all parcels
    return _patched(c, [(-11, -8, 4, 7, "Titanium"), (-9, -6, -3, -2, "Composite"), (-2, 2, -4, -2, "Composite")])


def shift_bus():
    """Crew shuttle. Carries shift crews and their families out to the
    mines, depots and haulers. The tug pushes a keel threaded through one
    big rounded passenger can (seats, a galley, the bridge in the middle of
    it), a pair of engine pods bolted on top of the can, and a bumper to nose
    up against whatever it is delivering to."""
    c = Canvas("fhc_shift_bus", "Steel")
    # the tug (top half): engineering only, the crew ride forward
    c.rows(-18, 4, """
  #####
 E.R.c##
E.b...O.#
E.c.....#
E.......d
""")
    c.P(-9, 1, "#" * 17)
    c.P(-9, 0, "." * 17)
    # the passenger can, threaded on the keel
    c.rows(-6, 5, """
  ##d####
 #Q.Q.Q.Q#
#.........#
#GG.O.m.y.#
#GG..BB...#
d....BBH..d
""")
    # a pod bolted on top of the can, venting aft over the tug
    c.rows(-4, 9, """
 ###
E..#
Ef.#
 #.#
""")
    # the bumper
    c.rows(5, 6, """
##g#
 #..#
 #M.#
  #..g
  #a.#
   #..#
   ...o
""")
    c.mirror()
    _plates(c, [(11, 2), (10, 4), (9, 6)])
    c.P(-15, 0, "M")
    # the starboard row are family berths: a cabin and a cot of luggage
    c.P(-4, -4, "q")
    c.P(2, -4, "k")
    return _patched(c, [(-4, -1, 6, 9, "Composite"), (-18, -15, -3, -1, "Titanium"), (-2, 2, -5, -3, "Titanium")])


def wrecker():
    """Salvage tug, the tow truck of the lanes. The bumper is bent the
    other way into a wide push-cradle, horns forward, so a wreck sits in
    its dish: tractor beams and salvage arms down its face, plates on the
    horn tips. A scrap cage and a parts hopper strapped on the keel, and an
    engine pod bolted behind each horn."""
    c = Canvas("fhc_wrecker", "Steel")
    # the tug (top half)
    c.rows(-19, 5, """
  ######
 E.Q.Q.##
E.R.GG.O#
E...GG..##
E.c..BB..#
E....BBH.d
""")
    c.P(-9, 1, "#" * 17)
    c.P(-9, 0, "." * 17)
    # the push-cradle, horns forward
    c.rows(8, 8, """
   ##g#
   #...#
   #..#
  #..t
  #M.#
 #..X
 #.a#
#..#
...t
""")
    # an engine pod bolted behind the horn
    c.rows(7, 9, """
 ###
E..#
Ef.d
 ###
""")
    # aft load: a low scrap cage, holed in the middle
    c.rows(-8, 6, """
 #####
#kk kk#
#kkkkk#
 #####
  # #
""")
    # forward load: a parts hopper, a wedge narrowing to its straps
    c.rows(0, 6, """
#######
#UUCCk#
 #kCC#
  ###
  # #
""")
    c.mirror()
    _plates(c, [(15, 8), (15, 6)])
    c.P(-16, 0, "M")
    c.P(1, -5, "kk")            # the other hopper came back with loose parts
    return _patched(c, [(7, 10, 7, 10, "Titanium"), (-8, -2, -6, -3, "Composite"),
                        (-19, -16, 2, 5, "Composite"), (11, 14, -8, -6, "Titanium")])


def rockhound():
    """Prospector. The keel forks like a dowser's rod: two tines stepped out
    and forward, a core drill and a plated bumper on each tip, so it can
    sample two faces of a rock at once. The claim's ore rides in one wedge
    hopper strapped in the crotch of the fork, and an engine pod is bolted
    into each elbow."""
    c = Canvas("fhc_rockhound", "Steel")
    # the tug (top half)
    c.rows(-17, 4, """
  #####
 E.Q.R##
E.c.O..#
E..BB..#
E..BBH.d
""")
    # the keel, up a rib into the fork
    c.P(-9, 1, "#######.#")
    c.P(-9, 0, "........#")
    c.P(-3, 2, "#.#")
    c.P(-3, 3, "#.#")
    # the tine: an inner step, then out and forward to a drill bumper
    c.P(-3, 4, "#.########")
    c.P(-3, 5, "#........#")
    c.P(-3, 6, "######.#######.##")
    c.P(2, 7, "#...........&")
    c.P(2, 8, "#M##A####..#")
    c.P(9, 9, "#.#")
    c.P(9, 10, "#g")
    c.P(9, 5, "#.#")
    c.P(9, 4, "##")
    # the pod in the elbow
    c.rows(-2, 9, """
 ###
E..#
Ef.d
""")
    # the claim hopper, a wedge pointing into the crotch
    c.P(3, 2, "#######")
    c.P(2, 1, "#UUkUUk#")
    c.P(1, 0, "#kkkMkkk#")
    c.P(4, 3, "#")
    c.P(6, 3, "#")
    c.mirror()
    _plates(c, [(11, 10), (12, 9), (14, 8), (14, 6), (12, 5), (11, 4)])
    return _patched(c, [(-1, 2, 7, 9, "Titanium"), (5, 9, -2, 0, "Composite"), (-17, -14, -4, -2, "Titanium")])


def _ring(c, cx, r, t=3):
    """An octagonal ring of truss centred on (cx, 0): outer half-size r,
    chamfered, t thick, a walkway down its middle and open sky inside."""
    cells = set()
    for x in range(cx - r, cx + r + 1):
        for y in range(-r, r + 1):
            m, d = max(abs(x - cx), abs(y)), abs(x - cx) + abs(y)
            if m <= r and d <= r + r // 2 and (m > r - t or d > r + r // 2 - t - 1):
                cells.add((x, y))
    for p in cells:
        inner = all((p[0] + dx, p[1] + dy) in cells for dx, dy in ((0, 1), (0, -1), (1, 0), (-1, 0)))
        c.cells[p] = "." if inner else "#"


def mender():
    """Repair tender, a travelling depot. The tug pushes an octagonal ring of
    truss round an open yard (the depot's yard, small enough to fly), the
    patient held inside it by tractor beams and worked on under floodlights
    from repair bays on the ring. A short keel on to the bumper; a spares
    cage and an engine pod ride on the ring's back."""
    c = Canvas("fhc_mender", "Steel")
    _ring(c, 0, 8)
    # the tug (top half)
    c.rows(-21, 5, """
  ######
 E.Q.q.##
E.R.GG.y#
E...GG..##
E.c..BB..#
E....BBH.d
""")
    c.P(-11, 1, "###")
    c.P(-11, 0, "...d")
    # out of the ring's front, a short keel to the bumper
    c.P(8, 1, "#####")
    c.P(8, 0, "d....")
    aft = {0: 13, 1: 13, 2: 12, 3: 12, 4: 11, 5: 11, 6: 10}
    _bumper(c, aft, {0: "...o", 1: "#..#", 2: "#M.#", 3: "#..g", 4: "#..#", 5: "#..#", 6: "##g#"})
    # round the yard: floodlights, tractors, a repair arm; bays on the ring
    for p in ((-6, 3), (6, 3), (-3, 6), (3, 6)):
        c.cells[p] = "l"
    c.cells[(0, 6)] = "X"
    c.cells[(-6, 0)] = "t"
    c.cells[(6, 0)] = "t"
    for p in ((-7, 2), (7, 2), (-2, 7), (2, 7)):
        c.cells[p] = "y"
    c.cells[(0, 8)] = "M"
    c.cells[(4, 8)] = "g"
    c.cells[(-5, 7)] = "p"
    # an engine pod on the ring's back, on a short strut
    c.rows(-5, 12, """
 ####
E..f#
E...#
  #.#
""")
    c.P(-2, 8, "d")
    # a spares cage strapped beside it, holed
    c.rows(1, 14, """
 ###
#kkk#
#k k#
#kkk#
 ###
 # #
""")
    c.mirror()
    _plates(c, [(16, 2), (15, 4), (14, 6)])
    c.P(-18, 0, "M")
    c.P(2, -11, "UUk")          # the other cage holds hull plate, not parts
    c.P(2, -13, "kUU")
    return _patched(c, [(-5, -1, 9, 12, "Titanium"), (-8, -6, -5, -2, "Composite"),
                        (1, 5, -13, -9, "Composite"), (-21, -18, -5, -2, "Titanium")])


def stacker():
    """Container hauler. Three bays of mismatched boxes, stacked like a barge
    tow: a reefer box with its vents at the ends aft, a long box with a
    holed flat-rack lashed on top of it amidships, a round tank container
    forward. Two ribs between the bays carry gun tubs; an engine pod sits on
    the aft rib's tip, where there was room."""
    c = Canvas("fhc_stacker", "Steel")
    # the tug (top half)
    c.rows(-25, 5, """
  ######
 E.Q.Q.##
E.R.GG.O#
E...GG..#
E.c..BB..#
E....BBH.d
""")
    c.P(-15, 1, "#" * 35)
    c.P(-15, 0, "." * 35)
    # two ribs
    for x, tip in ((-6, 9), (8, 8)):
        c.P(x + 1, 1, ".")
        for y in range(2, tip):
            c.P(x, y, "#.#")
    c.P(8, 8, "#g#")
    # bay A: a reefer box, heat vents on its ends
    c.rows(-14, 6, """
 #####
vCCkCCv
vCCkCCv
 #####
 #   #
""")
    # bay B: a long box, and a flat-rack lashed on top of it
    c.rows(-2, 12, """
  #####
 #kkkkk#
 #kk kk#
 #kkkkk#
  #####
   # #
 #######
#UUCCkUU#
#kMCCkkk#
#########
  #   #
""")
    # bay C: a round tank container
    c.rows(12, 6, """
 ###
#fff#
#fff#
 ###
 # #
""")
    # the push-bar
    aft = {0: 20, 1: 20, 2: 19, 3: 19, 4: 18, 5: 18, 6: 17, 7: 16}
    _bumper(c, aft, {0: "...o", 1: "#..#", 2: "#M.#", 3: "#..g", 4: "#a.#", 5: "#..#",
                     6: "#..#", 7: "##g#"})
    # an engine pod on the aft rib's tip
    c.rows(-8, 11, """
 ####
E...#
E.f.#
""")
    c.mirror()
    _plates(c, [(23, 2), (22, 4), (21, 6), (20, 7)])
    c.P(-23, 0, "M")
    c.P(13, -5, "M")            # one tank went in for a core instead of fuel
    c.P(1, -10, "UU")
    return _patched(c, [(-8, -4, 9, 11, "Titanium"), (-14, -8, -6, -3, "Composite"),
                        (-1, 5, -12, -8, "Composite"), (-25, -22, 2, 5, "Titanium"), (12, 16, 3, 6, "Titanium")])


def _rail(c, segs):
    """A stepped three-wide rail, top half: segs = [(x0, x1, y_mid)], each a
    run with a walkway on y_mid; consecutive runs overlap by a cell or two
    and are joined by opening the shared rim."""
    for x0, x1, y in segs:
        for row in (y + 1, y - 1):
            for x in range(x0, x1 + 1):
                if c.cells.get((x, row)) != ".":
                    c.cells[(x, row)] = "#"
        c.P(x0, y, "#" + "." * (x1 - x0 - 1) + "#")
    for (a0, a1, ya), (b0, b1, yb) in zip(segs, segs[1:]):
        x = max(a0, b0) + 1             # first shared column, inboard of the rims
        for y in range(ya, yb + 1):
            c.cells[(x, y)] = "."


def blunderbuss():
    """Armed freighter: freight that shoots back. Its frame flares like a
    blunderbuss's bell: two stepped rails grow out of the keel just ahead of
    the tug and spread to a wide bumper across the mouth, gun tubs on every
    step and across the bumper. A fuel bladder and a scrap cage ride in the
    flare; engine pods are bolted on the rails' outer steps."""
    c = Canvas("fhc_blunderbuss", "Steel")
    # the tug (top half)
    c.rows(-25, 5, """
  ######
 E.Q.Q.##
E.R.GG.O#
E...GG..#
E.c..BB..#
E.M..BBH.d
""")
    c.P(-15, 1, "#" * 29)
    c.P(-15, 0, "." * 29)
    # the flaring rail, stepping out of the keel to the bumper's tip
    _rail(c, [(-13, -8, 3), (-9, -4, 5), (-5, 0, 7), (-1, 4, 9), (3, 9, 11)])
    c.P(-12, 1, ".")
    c.P(-12, 2, ".")
    # the wide bumper across the mouth
    aft = {0: 14, 1: 14, 2: 13, 3: 13, 4: 12, 5: 12, 6: 11, 7: 11, 8: 10, 9: 10, 10: 9, 11: 9, 12: 9}
    _bumper(c, aft, {0: "...o", 1: "#..#", 2: "#M.#", 3: "#..g", 4: "#..#", 5: "#a.#", 6: "#..#",
                     7: "#..g", 8: "#..#", 9: "#..#", 10: "#..#", 11: "...#", 12: "#p"})
    # gun tubs on the rail's steps
    for p in ((-11, 4), (-7, 6), (1, 10), (6, 12)):
        c.cells[p] = "g"
    # an engine pod on the rail's outer step
    c.rows(-6, 11, """
 ###
E.f#
E..#
""")
    c.P(-4, 8, "d")
    # aft load: a small fuel bladder
    c.rows(-2, 4, """
 ###
#fff#
 # #
""")
    # forward load: a hollow scrap cage
    c.rows(3, 7, """
 #####
#kkkkk#
#kk kk#
#kkkkk#
 #####
  # #
""")
    c.mirror()
    _plates(c, [(17, 2), (16, 4), (15, 6), (14, 8), (13, 10), (11, 12)])
    c.P(-23, 0, "M")
    c.P(5, -4, "UU")            # one cage carries crates
    return _patched(c, [(-6, -3, 9, 11, "Titanium"), (3, 9, -7, -3, "Composite"),
                        (13, 16, -5, -1, "Titanium"), (-25, -22, -5, -2, "Composite")])


def flagman():
    """Convoy picket: the Compact's escort car, which runs ahead of a convoy,
    lights the lane and marks it. One long yard across the keel behind the
    bumper, a flag head on each end of it: an engine pod aft, a radar array,
    a gun tub and a floodlight on the tip. A rack of marker buoys rides on
    the keel to drop along the route."""
    c = Canvas("fhc_flagman", "Steel")
    # the tug (top half)
    c.rows(-16, 4, """
  #####
 E.Q.O##
E.R.c..#
E..BB..#
E.MBBH.d
""")
    c.P(-8, 1, "#" * 19)
    c.P(-8, 0, "." * 19)
    # the yard: a rib out to a flag head
    c.P(5, 1, ".")
    for y in range(2, 10):
        c.P(4, y, "#.#")
    c.rows(0, 12, """
 ###g##
E.A....l
Ef...d#
""")
    # the bumper
    aft = {0: 11, 1: 11, 2: 10, 3: 10, 4: 9, 5: 9, 6: 8}
    _bumper(c, aft, {0: "...o", 1: "#..#", 2: "#M.#", 3: "#..g", 4: "#..#", 5: "#..#", 6: "##g#"})
    # a rack of marker buoys strapped to the keel
    c.rows(-6, 6, """
 ###
#JJJ#
#J J#
 ###
 # #
""")
    c.mirror()
    _plates(c, [(14, 2), (13, 4), (12, 6)])
    c.P(-4, -4, "#")            # one rack has been emptied down the lane and patched shut
    return _patched(c, [(0, 3, 10, 12, "Titanium"), (-6, -2, -6, -2, "Composite"),
                        (-16, -14, -4, -2, "Composite"), (4, 6, -9, -6, "Titanium")])


CONCEPTS = [
    ("hotshot", hotshot, "Hotshot", "Trader: courier",
     "A rush-load rig: a tug with two engine pods bolted on its stern quarters, a long bare keel with a pay-and-parcel bin strapped either side, and a small bumper. All stern, like a dragster."),
    ("shift_bus", shift_bus, "Shift Bus", "Civilian: crew shuttle",
     "Carries shift crews and their families to the mines and depots: the tug pushes a keel threaded through one big rounded passenger can, two engine pods bolted on top of it, and a bumper to nose up against wherever it is delivering."),
    ("wrecker", wrecker, "Wrecker", "Civilian: salvage tug",
     "The tow truck of the lanes: its bumper bent the other way into a wide push-cradle, horns forward, tractor beams and salvage arms down the dish. A scrap cage and a parts hopper on the keel, an engine pod bolted behind each horn."),
    ("rockhound", rockhound, "Rockhound", "Civilian: prospector",
     "The keel forks like a dowser's rod: two tines stepped out and forward, a core drill and plated bumper on each tip, so it samples two faces of a rock at once. The claim's ore rides in one wedge hopper strapped in the crotch; an engine pod is bolted into each elbow."),
    ("mender", mender, "Mender", "Civilian: repair tender",
     "A travelling depot: the tug pushes an octagonal ring of truss round an open yard, the patient held inside by tractor beams and worked on under floodlights, then a short keel on to the bumper. A spares cage and an engine pod ride on the ring's back."),
    ("stacker", stacker, "Stacker", "Trader: container hauler",
     "Three bays of mismatched boxes stacked like a barge tow: a reefer box with vents on its ends, a long box with a holed flat-rack lashed on top, a round tank container. Gun tubs on the ribs; an engine pod on the aft rib, where there was room."),
    ("blunderbuss", blunderbuss, "Blunderbuss", "Trader: armed freighter",
     "Freight that shoots back. The frame flares like a blunderbuss's bell: two stepped rails grow out of the keel and spread to a wide bumper across the mouth, gun tubs on every step and across the bumper, a fuel bladder and a scrap cage in the flare."),
    ("flagman", flagman, "Flagman", "Escort: convoy picket",
     "The Compact's escort car: it runs ahead of a convoy, lights the lane and marks it. A long yard across the keel behind the bumper with a flag head on each end (engine pod, radar, gun tub, floodlight), and a rack of marker buoys to drop along the route."),
]
