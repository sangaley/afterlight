"""Eternal Hegemony: hand-drawn, top-down only.

Lore: the Terran Hegemony's own past, frozen at its peak. It believes it is
two hundred years ago, takes orders from emperors long dead, slips in and out
of time, and can call back past versions of ships it has lost. A boss faction
(danger 6), met as a lone patrol far out.

Grammar: the Terran grammar made monumental and ceremonial.
  - a stepped CROWN BOW: a long central spire flanked by crown points, every
    raked step plated;
  - DOUBLE-TIER SPONSONS: each gun sponson is a ziggurat, a broad gallery with
    a narrower turret tier stacked outboard of it, plated on its forward steps;
  - ENGINE PODS at the stern corners round a recessed centre engine bank;
  - a PROCESSIONAL NAVE down the centreline, lined with shield and core
    "pillars", leading to the throne (the bridge) in the middle of the ship;
  - cathedral symmetry, open courtyards inside the big hulls, Titanium.

Legend: see terran.py and draw.LEGEND.
"""
from draw import Canvas, AIMED, ENGINES, LEGEND
import fleet

FACTION = {
    "slug": "eternal_hegemony",
    "name": "Eternal Hegemony",
    "tagline": "The Hegemony's own past, frozen at its peak. It obeys dead emperors, slips in and out of time, and calls back the ships it has lost.",
    "grammar": [
        "Stepped crown bow: a long central spire between crown points, every raked step plated",
        "Double-tier gun sponsons: a broad gallery with a narrower turret tier stacked outboard",
        "Engine pods at the stern corners round a recessed centre engine bank",
        "A processional nave down the centreline, lined with pillars, ending at the throne-bridge",
        "Cathedral symmetry, open courtyards in the big hulls, Titanium",
    ],
    "accent": "#d4a35a",
}


def finish(c):
    """c.build(), plus two checks the shared checker leaves out: every gun and
    launcher has an open side to fire from, and every engine has clear space
    aft of it."""
    d = c.build()
    hull = {p for p, ch in c.cells.items() if ch != "/"}
    for p, ch in c.cells.items():
        kind = LEGEND.get(ch)
        x, y = p
        if kind in AIMED and kind != "DockingPort":
            if all((x + dx, y + dy) in hull or c.cells.get((x + dx, y + dy)) == "/"
                   for dx, dy in ((0, 1), (0, -1), (1, 0), (-1, 0))):
                d["_checks"].append(f"{kind} at {p} has no open side")
        if kind in ENGINES and any((xx, y) in hull for xx in range(x - 30, x)):
            d["_checks"].append(f"{kind} at {p} is blocked aft")
    return d


def echo_escort():
    """Frigate. The 'old pattern' line ship: the Terran Bulwark two centuries
    ago, heavier and dressed for parade. Crown bow, two ziggurat sponsons a
    side, the forward pair a step lower, a nave running to the bridge."""
    c = Canvas("eh_echo_escort")
    c.rows(-18, 9, "\n".join([
        "           #n#/",
        "          ##.##/      #n#/",
        "         g.a.a.#/    ##.##/",
        " #####   #O.d.Q.#   g.a.a.#/",
        "E.f.c.###.Q...Q..#/ #m.d.O.#",
        "E.R.b.d.KK.KK.m..." + "##" + "..GG.a.T" + "##/",
        " ##.cMf.Q.O.Q.y.d.M...GG.a.A.###",
        "   E......d.........d.M.....#",
        "   E.LL.s.c.s.m.s..BB.H.s.T.#zn##/",
        "   E" + "." * 15 + "BB" + "." * 13 + "#",
    ]))
    return finish(c.mirror())


def herald():
    """Destroyer. Three ziggurat sponsons a side, each smaller and lower than
    the last, so the hull steps down from a broad stern to a crown bow; a nave
    of shield pillars runs from the engine bank to the throne-bridge. The
    Herald goes ahead of the old fleet and announces it."""
    c = Canvas("eh_herald")
    y3 = list("   E.LL.M.Q.SSS.O.y.Q.m.M.b.c.....a.T.s.M.###")
    y1 = list("." * 50)
    y1[:4] = list("   E")
    y1[5] = "R"
    for i in (7, 10, 13, 16, 19, 22, 31, 34, 37, 40):
        y1[i] = "s"                         # the pillars of the nave
    y1[25] = y1[26] = "B"
    y1[28] = "H"
    y1[42:50] = list("zn#####/")
    c.rows(-26, 10, "\n".join([
        "          #n#n#/",
        "         #a.a.a#/       #n#/",
        "        g.O...Q.#/     ##.##/",
        " #####  #.KK.d.KK#    g.a.a.#/      #nn#/",
        "E.f.c.##.m.....y../   #GG.d.Q#/    ga..a#/",
        "E.R.b.v.KK.KK.O.Q." + "#g##" + ".GG.q.a." + "##/ " + "#.O..s.#/",
        " ##" + "." * 15 + "d" + "." * 11 + "d." + "##" + "." * 8 + "#/",
        "".join(y3),
        "   E.f.c..d.SSS.....Q.....b.c.d..........#",
        "".join(y1),
        "   E" + "." * 21 + "BB" + "." * 22 + "n",
    ]))
    return finish(c.mirror())


def throne_of_ages():
    """Boss dreadnought. The Terran body made monumental and crowned: the
    processional nave with three ziggurat sponsons a side, stepping down
    toward the bow, carried behind a great tiara whose plated spires fan out
    like a sunburst. Over each waist a pointed arch of flying buttresses
    springs from the aft sponson to the throne's sponson and lifts a gun
    bastion at its apex; the courtyard under it is open to space. Behind each
    arch hangs its echo: the same arch a beat out of time, a hollow lattice in
    a different hull. The throne (bridge) sits on the keel under the middle
    sponson, engine pods at the stern corners round a recessed centre bank."""
    import math
    c = Canvas("eh_throne_of_ages")
    hull, ghost = set(), set()

    def R(x0, x1, y0, y1, s=hull):
        s |= {(x, y) for x in range(x0, x1 + 1) for y in range(y0, y1 + 1)}

    def zig(x0, x1, y0, tiers):
        """A ziggurat sponson: tiers of (rows, step in at the bow, step in aft)."""
        y, a, b = y0, x0, x1
        for rows, fore, aft in tiers:
            for _ in range(rows):
                R(a, b, y, y)
                y += 1
            a, b = a + aft, b - fore

    def stroke(pts, w, s=hull):
        for px, py in pts:
            s |= {(x, y) for x in range(int(px) - 2, int(px) + 3) for y in range(int(py) - 2, int(py) + 3)
                  if y >= 0 and math.hypot(x - px, y - py) <= w}

    def bez(p0, p1, p2, n=90):
        return [((1 - t) ** 2 * p0[0] + 2 * (1 - t) * t * p1[0] + t * t * p2[0],
                 (1 - t) ** 2 * p0[1] + 2 * (1 - t) * t * p1[1] + t * t * p2[1]) for t in (i / n for i in range(n + 1))]

    def spire(x0, y0, ang, ln, w=1.5):
        a = math.radians(ang)
        for x in range(int(x0) - ln - 3, int(x0) + ln + 3):
            for y in range(max(0, int(y0) - ln - 3), int(y0) + ln + 3):
                al = (x - x0) * math.cos(a) + (y - y0) * math.sin(a)
                ac = -(x - x0) * math.sin(a) + (y - y0) * math.cos(a)
                if -0.5 <= al <= ln and abs(ac) <= w * (1 - al / (ln + 1.0)) + 0.3:
                    hull.add((x, y))

    def arch(dx, w, s):
        """A pointed arch: two flying buttresses meeting at a diamond bastion."""
        stroke(bez((dx - 11, 11), (dx - 8, 22), (dx, 24)), w, s)
        stroke(bez((dx + 11, 11), (dx + 8, 22), (dx, 24)), w, s)
        s |= {(x, y) for x in range(dx - 4, dx + 5) for y in range(20, 29) if abs(x - dx) + abs(y - 24) <= 4}

    # The crown: a tiara arc at the bow with spires radiating from it.
    CX, RO, RI, TH = 2, 28, 24, 68
    for x in range(CX, CX + RO + 1):
        for y in range(0, RO + 1):
            r, t = math.hypot(x - CX, y), math.degrees(math.atan2(y, x - CX))
            if RI + max(0, t - (TH - 12)) * 0.28 <= r <= RO and t <= TH:   # horns taper
                hull.add((x, y))
    for ang, ln in ((0, 16), (12, 13), (24, 11), (36, 9), (48, 7)):
        a = math.radians(ang)
        spire(CX + (RO - 0.5) * math.cos(a), (RO - 0.5) * math.sin(a), ang, ln)
    spire(CX + 27.5 * math.cos(math.radians(60)), 27.5 * math.sin(math.radians(60)), 60, 5)
    # The body: nave, three ziggurat sponsons shrinking toward the bow, pods.
    R(-35, 6, 0, 3)
    R(6, CX + 24, 0, 2)
    zig(-30, -14, 4, [(3, 3, 1), (3, 3, 2), (2, 2, 2)])
    zig(-10, 5, 4, [(3, 3, 1), (3, 3, 2), (2, 2, 2)])
    zig(10, 19, 3, [(2, 2, 1), (2, 2, 2)])
    hull |= {(x, y) for x in range(-41, -29) for y in range(4, 14) if abs(x + 35) + abs(y - 9) <= 7.5}
    # Flying buttresses: the arch over the courtyard, and one out to the crown.
    arch(-12, 1.65, hull)
    stroke(bez((15, 6), (20, 10), (CX + 25 * math.cos(math.radians(44)), 25 * math.sin(math.radians(44)))), 1.1)
    hull -= {(x, y) for x, y in hull if not any(q in hull for q in ((x + 1, y), (x - 1, y), (x, y + 1), (x, abs(y - 1))))}
    # The time-echo: the same arch, a beat behind.
    arch(-22, 0.9, ghost)
    ghost -= hull
    ghost -= {(x, y) for x, y in ghost if abs(x + 22) + abs(y - 24) <= 1}   # hollow the ghost bastion
    for p in hull | ghost:
        c.cells[p] = "#"

    def room(x, y, s):
        for i, ch in enumerate(s):
            if ch != " ":
                assert (x + i, y) in hull, f"room cell {(x + i, y)} is off the hull"
                c.cells[(x + i, y)] = ch

    # The nave: the processional corridor on the keel, lined with shield
    # pillars, the throne-bridge at the crossing under the mid sponson.
    room(-35, 0, "E" + "." * 31 + "BB.H" + "." * 25)
    y1 = list("E.R.f." + "." * 55)                 # x -35..25
    for x in list(range(-29, -4, 3)) + list(range(0, 20, 3)):
        y1[x + 35] = "s"                          # the pillars of the nave
    y1[32:34] = "BB"
    room(-35, 1, "".join(y1))
    room(-35, 2, "E.LL.bc.M.Q.KK.O.Q.m.y.Q.q.@@.q.T.q.W.q.A.n#z.a.T.a.M.a#n#z#n")
    room(-34, 3, ".." + "  " + "." * 18 + "#l#" + "." * 16)
    room(11, 3, ".Q.O.Q.a")
    # The crown: a walk round the inside of the tiara, stores and posts on its
    # outer side, guns on its rim between the spires, a railgun lance down
    # the centre spire.
    def inside(p):
        return all((p[0] + dx, abs(p[1] + dy)) in hull for dx, dy in ((0, 1), (0, -1), (1, 0), (-1, 0)))
    tiara = {}
    for (x, y) in hull:
        r, t = math.hypot(x - CX, y), math.degrees(math.atan2(y, x - CX))
        if RI <= r <= RO + 0.6 and t <= TH + 4 and not (y <= 2 and x <= CX + 24):
            tiara[(x, y)] = (r, t)
    walk = lambda p: c.cells.get(p) in ("#", ".") or LEGEND.get(c.cells.get(p)) in fleet.POSTS
    for k, (tg, ch) in enumerate(zip(range(3, 66, 6), "TaQsaOymaTQ")):
        outer = [p for p, (r, t) in tiara.items() if inside(p) and abs(t - tg) < 3 and p[1] > 0]
        if outer:
            c.cells[max(outer, key=lambda p: tiara[p][0] - abs(tiara[p][1] - tg) / 3)] = ch
    for tg, ch in zip((6, 18, 30, 42, 54, 62), "nzinzn"):
        rim = [p for p, (r, t) in tiara.items() if not inside(p) and abs(t - tg) < 3 and r > 26.5
               and any(inside(q) and walk(q) for q in ((p[0], p[1] - 1), (p[0] - 1, p[1]), (p[0], p[1] + 1), (p[0] + 1, p[1])))]
        if rim:
            c.cells[min(rim, key=lambda p: abs(tiara[p][1] - tg))] = ch
    lance = min(x for x in range(CX + RO, CX + RO + 20) if (x, 1) not in hull)
    room(lance, 0, "rr")
    # Engine pods at the stern corners, a fusion reactor in each.
    room(-38, 5, "E....")
    room(-39, 6, "E.f.M.f..")
    room(-40, 7, "E..........")
    room(-41, 8, "E.LL.FFF.b..")
    room(-41, 9, "E....FFF....")
    room(-41, 10, "E.LL.FFF.c.")
    room(-40, 11, "E.........")
    room(-39, 12, "E.b.R.c..")
    room(-37, 13, " g.g.")
    # The aft sponson: barracks and quarters, guns on every aft-facing step.
    room(-29, 4, "Q.Q.O.Q.Q.m.Q.a")
    room(-29, 5, "...............")
    room(-29, 6, "b.KK.M.KK.c.y n")
    room(-28, 7, "...........")
    room(-28, 8, "Q.O.T.s.O.Q")
    room(-28, 9, "n.......  n")
    room(-27, 10, "g.a.T.a")
    room(-26, 11, "n")
    # The mid sponson: the throne hall over the bridge.
    room(-9, 4, "Q.q.m.s.q.O.q.")
    room(-9, 5, "..............")
    room(-9, 6, "y.Q.s.A.s.Q. n")
    room(-8, 7, "..........")
    room(-8, 8, "Q.T.M.a.Q.")
    room(-8, 9, "n.......")
    room(-7, 10, "g..Ta.")
    room(-5, 11, "n")
    # The fore sponson.
    room(12, 5, ".....")
    room(12, 6, "n")
    # The gun bastion at the head of the arch.
    room(-12, 28, "n")
    room(-14, 26, "z...z")
    room(-14, 25, "s...s")
    room(-16, 24, "n.T...T.n")
    room(-16, 23, "M...O...M")
    # Plates, selectively: the forward faces of the crown and its spires, the
    # forward step of every sponson tier, and the bastion's forward faces.
    c.mirror()
    solid = {p for p, ch in c.cells.items() if ch != "/"}
    steps = {(-13, 4), (-16, 7), (-19, 10), (6, 4), (3, 7), (1, 10), (18, 5), (20, 3)}
    for x in range(-45, 60):
        for y in range(-32, 33):
            p = (x, y)
            if p in c.cells or not ((x >= CX + 19 and abs(y) <= 26) or (x, abs(y)) in steps
                                    or (x > -12 and abs(x + 12) + abs(abs(y) - 24) <= 6)):
                continue
            n, s, w, e = ((x, y + 1) in solid, (x, y - 1) in solid, (x - 1, y) in solid, (x + 1, y) in solid)
            if w and n != s and not e:
                c.cells[p] = "/"
    for p, ch in list(c.cells.items()):          # never close a gun's last open side
        if LEGEND.get(ch) in AIMED:
            nb = [(p[0] + dx, p[1] + dy) for dx, dy in ((0, 1), (0, -1), (1, 0), (-1, 0))]
            if all(q in solid or c.cells.get(q) == "/" for q in nb):
                q = next(q for q in nb if c.cells.get(q) == "/")
                for m in (q, (q[0], -q[1])):        # and its mirror image
                    if c.cells.get(m) == "/":
                        del c.cells[m]
    d = finish(c)
    gs = ghost | {(x, -y) for x, y in ghost}
    for h in d["hull_cells"]:
        if tuple(h["grid_pos"]) in gs:
            h["material"] = "Composite"
    return d


def reliquary():
    """Experimental carrier. A mandorla, the pointed almond that frames a relic
    in a cathedral window: an almond ring round two open windows, with the keel
    run straight through the middle carrying the throne. Drone bays line both
    sides of each window and face each other like mirrors; that is where the
    echoes of lost ships come through. Engines fan up the stern quarter, every
    forward step is plated."""
    c = Canvas("eh_reliquary_of_echoes")
    y1 = list("." * 56)                     # x -24..31
    y1[0], y1[2] = "E", "R"
    for x in (-19, -16, -13, -10, -7, -4, 3, 6, 9, 12, 15, 18):
        y1[x + 24] = "s"
    y1[23] = y1[24] = "B"
    y1[26], y1[46] = "H", "T"
    y1[51:56] = "#nzn#"
    c.rows(-24, 16, "\n".join([
        "                       #n#/",
        "                      g...#/",
        "                    #g.a.a.##/",
        "                 E#n.Q.O.O.Q.#n#/",
        "              Eg#.R.b.M...M.b.R.#g#/",
        "           Eg#" + "." * 21 + "#g#/",
        "         E....." + "s.a.T.@@.W.@@.T.a.s" + ".....#/",
        "        E..V.V.!!." + "DD#DD#l#DD#DD" + ".!!.V.V..#/",
        "       E..Q.O.QDDl" + " " * 13 + "lDDQ.O.Q..#/",
        "      E..KK.mDD" + " " * 19 + "DDm.KK..#/",
        "     E..M.y.l" + " " * 23 + "l.y.M..#/",
        "    E..Q.QDD" + " " * 25 + "DDQ.Q..#/",
        "   E..b.Ol" + " " * 29 + "lO.b..#/",
        "  E..R.cl" + " " * 31 + "lc.R..#/",
        " E..LL.f." + "#DD#l#DD#DD#l##l##l#DD#DD#l#DD#" + ".a.T..." + "#zn#/",
        "".join(y1) + "/",
        "E" + "." * 22 + "BB" + "." * 31 + "n",
    ]))
    return finish(c.mirror())


def octant(c, top, block):
    """Paint the wedge 0 <= x <= y of a station (rows from y=top down, x from 0),
    reflect it across the diagonal, then turn it four ways: eightfold symmetry."""
    for k, line in enumerate(block.strip("\n").split("\n")):
        y = top - k
        for x, ch in enumerate(line):
            if ch == " ":
                continue
            assert x <= y, f"octant cell {(x, y)} is past the diagonal"
            c.cells[(y, x)] = ch
            if x >= 1:
                c.cells[(x, y)] = ch
    return c.rot4()


def citadel():
    """Station. A grander, double-walled star fort: a sanctum with the throne at
    the centre, an inner wall, an open courtyard crossed by four gatehouses,
    then the curtain wall with a ziggurat gate tower on every face and a
    crowned bastion on every corner. Drawn as one eighth, reflected and turned."""
    c = Canvas("eh_citadel")
    octant(c, 29, """
                     /n/
                    /#.#/
                   /#a.a#/
                   g.O.O.g
                  g.Q.M.Q.
n/               g.b.....
.#/             #.c..FFF
..#g/          n.R.d.FF
.a..#/          #.b..F
.a.a.g           #...
......#n#x#g#x#n#...
.a.s.aa.a.a.a.a.T..
........d.........
.Q.KK.O.m.Q.KK.y.
...##l###p###l##
..#
.s#
..#
.s#
...##l##p##
.Q.q.m.O.Q
....d....
.b.M.R.c
..#l###
.#
.#
""")
    c.rows(-3, 3, """
###d###
#q.s.q#
#.HBB.#
d..BB.d
#..BB.#
#q.s.q#
###d###
""")
    return finish(c)


CONCEPTS = [
    ("echo_escort", echo_escort, "Echo Escort", "Frigate",
     "An 'old pattern' Terran line ship, two centuries out of date and dressed for parade: crown bow, ziggurat sponsons, a nave to the throne."),
    ("herald", herald, "Herald", "Destroyer",
     "Three ziggurat sponsons a side, shrinking toward a crown bow; a nave of shield pillars runs to the throne-bridge. It goes ahead of the old fleet and announces it."),
    ("throne_of_ages", throne_of_ages, "Throne of Ages", "Boss dreadnought",
     "A crowned procession: a sunburst tiara of plated spires at the bow, ziggurat sponsons stepping down the nave, and over each waist a pointed arch of flying buttresses holding a gun bastion. Behind each arch hangs its echo, the same arch a moment out of time."),
    ("reliquary_of_echoes", reliquary, "Reliquary of Echoes", "Experimental: echo carrier",
     "A mandorla, the almond frame round a cathedral relic: two open windows lined with facing drone bays like mirrors, the throne carried on the keel between them. The echoes of lost ships come through the windows."),
    ("citadel", citadel, "Citadel", "Station",
     "A double-walled star fort: a throne sanctum, an inner wall, a courtyard crossed by four gatehouses, then a curtain wall with ziggurat gate towers and crowned corner bastions."),
]


# Sent back as "it looks to basic"; redrawn since.
REVISED = {
    "throne_of_ages": "Redrawn: a crown of nine plated spires on a curved band, flying-buttress arches over gun bastions, and a ghostly time-echo of each arch trailing behind.",
}
