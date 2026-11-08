"""Terran Hegemony, round 2: hand-drawn, top-down only.

Lore: the present-day Hegemony. Humanist, military, and losing. Their past self
(the Eternal Hegemony) still fights as if it won; this one is racing to stop
its own future, and its sensors are said to see thirty seconds ahead.

Grammar, held at every size (all of it reads from above):
  - an ARROWHEAD BOW with a plate on every step of both raked faces;
  - square GUN SPONSONS out on port and starboard: a gun on the outer face,
    a magazine behind it, a deck running back to the keel;
  - ENGINE NACELLES as pods at the stern corners, plus a centre engine bank;
  - a KEEL corridor down the centreline with the bridge in the middle of the
    ship, never at an edge;
  - port/starboard symmetric, Titanium.

Legend (draw.py): # hull  . deck  / plate  E engine  N large engine  R reactor
L large reactor  F fusion reactor  b battery  f fuel  c cooling  v heat vent
B bridge  H helm  M memory core  s shield  A radar  T targeting  Q quarters
K barracks  G galley  S surgery  m medbay  O oxygen  a ammo  C cargo  k small cargo
U large cargo  n cannon  g gatling  r railgun  x missile  z laser  p PD drone
i ion  d door  D drone bay  y repair  l floodlight  o docking port  V cryo
"""
from draw import Canvas

FACTION = {
    "slug": "terran_hegemony",
    "name": "Terran Hegemony",
    "tagline": "The present-day Hegemony: humanist, military, and losing. Their sensors are said to see thirty seconds ahead.",
    "grammar": [
        "Arrowhead bow with a plate on every step of both raked faces",
        "Square gun sponsons out on port and starboard: gun outside, magazine behind, deck back to the keel",
        "Engine nacelles as pods at the stern corners, plus a centre engine bank",
        "A keel corridor down the centreline, bridge in the middle of the ship",
        "Port/starboard symmetric, Titanium, interiors drawn room by room",
    ],
    "accent": "#a6b3cc",
}


def lancer():
    """Scout. An arrowhead with an engine pod each side: fast, light, all sensor."""
    c = Canvas("th_lancer")
    c.rows(-10, 5, """
 ##/
 #####/
 E..###g##/
E.R.Q.M.s.###/
#.b....H.A.T.#g#/
 E.d.....M..z...n
""")
    return c.mirror().build()


def warden():
    """Corvette. One pair of sponsons. About the Pincer's size: your peer."""
    c = Canvas("th_warden")
    c.rows(-14, 6, """
           g#g/
 ##       #.a.#/
E..#     #.....#/
E...#####O....Qmg##/
  E..R.b.M...GG..s.##/
  E..c.KK.m..GG.BB.A.###/
  E..R..d.......BBM..z..n
""")
    return c.mirror().build()


def bulwark():
    """Frigate. Two pairs of sponsons, engine pods, a railgun bow. The navy's
    line ship."""
    c = Canvas("th_bulwark")
    c.rows(-20, 7, """
           n#n/      n#n/
 ####     #.a.#/    #.a.#/
N....#   #.....#/  #.....#/
N###..###O.....m#g#Q.....Q#g##/
    E.LLb.KK.QOM..GG.a.aT.s.A.##/
    E......d......GG....d.......##/
    E.R.bc.KK.mO.y....BB.a.a.s.rr.##/
    E....d............BB....M....z..n
""")
    return c.mirror().build()


def sovereign():
    """Battleship. A keel hull and two outboard gun hulls held apart by two
    crossbeams, open courtyards between them. Three sponsons on each gun hull."""
    c = Canvas("th_sovereign")
    c.rows(-26, 16, """
          #n#n#     #n#n#     #n#n#
          #.a.#     #.a.#     #.a.#
          #...#/    #...#/    #...#/
##########O...Q##g##Q...O##g##m...Q##g###/
N.fc.Rb.KK.....aa.Ta.....as.aa.....aT.as.#/
N...............d............d............#/
####.Rv.QQ.Om...KK.QQO.y.aa.M.a.....as.aT.A#/
    ########.....#####g######g##.....#g######
            #...#               #...#
            #.p.#               #.p.#
            #...#               #...#
            #.d.#               #.d.#
#################...#####g#######g###...########/
E..FFF.bc.v......KK.SSS.GG.Q.O.........aa.T.a.s.#/
E..FFF...d..........SSS.GG........d.............##/
E.RFFF.LL.b.y.m.QQ.O.......BB.H......a.rr.a.T.rr..##/
E......d......M............BB.......d...........z...n
""")
    return c.mirror().build()


def oracle():
    """Experimental. The Hegemony's precognition picket: an octagonal ring of
    radar arrays round a small shielded core, joined by four spokes, open sky
    in between. Lore: it sees the fight thirty seconds early and calls the
    line in."""
    c = Canvas("th_oracle")
    for x in range(-11, 12):
        for y in range(-11, 12):
            m, d = max(abs(x), abs(y)), abs(x) + abs(y)
            if 8 <= m <= 11 and d <= 16 and (m >= 8 and d >= 8):
                c.cells[(x, y)] = "#"
    for x in range(-11, 12):        # spokes
        for y in (-1, 0, 1):
            if 3 < abs(x) < 9:
                c.cells[(x, y)] = "#"
                c.cells[(y, x)] = "#"
    hull = set(c.cells)
    inside = lambda p: all((p[0] + dx, p[1] + dy) in hull for dx, dy in ((0, 1), (0, -1), (1, 0), (-1, 0)))
    for (x, y) in sorted(hull):
        m, d = max(abs(x), abs(y)), abs(x) + abs(y)
        if not inside((x, y)):
            if (m == 11 or d == 16) and (x + y) % 2 == 0:
                c.cells[(x, y)] = "A"
            continue
        if m == 10 or d == 15:
            c.cells[(x, y)] = "."       # the ring's walkway, right behind the arrays
        elif m == 9 or d == 14:
            c.cells[(x, y)] = "s" if (x * 3 + y) % 7 == 0 else "b" if (x - y) % 5 == 0 else "."
    for (x, y), ch in list(c.cells.items()):
        if ch == "A" and not any(c.cells.get((x + dx, y + dy)) == "." for dx, dy in ((0, 1), (0, -1), (1, 0), (-1, 0))):
            c.cells[(x, y)] = "#"
    for k in range(4, 9):           # spoke walkways
        for p in ((k, 0), (-k, 0), (0, k), (0, -k)):
            c.cells[p] = "."
    c.rows(-3, 3, """
###d###
#M.s.R#
#.....#
d.HBB.d
#..BB.#
#R.BBM#
###d###
""")
    c.P(0, -3, "d")
    c.P(9, 3, "M")
    return c.build()


def homeward():
    """Civilian evacuation ark. A long keel with habitation ribs branching off
    both sides, every rib its own sealed block of cryo berths and quarters.
    The Hegemony is losing; this is what carries its people out."""
    c = Canvas("th_homeward", "Steel")
    c.rows(-27, 11, """
         #s#     #s#     #s#     #s#
         #V#     #V#     #V#     #V#
         #V#     #V#     #V#     #V#
         #Q#     #Q#     #Q#     #Q#
         #Q#     #Q#     #Q#     #Q#
         #.#     #.#     #.#     #.#
         #d#     #d#     #d#     #d#
 ########.#######.#######.#######.###############/
 E.RbcfOQ.QKKmOQQ.QQmKKOQ.QVVmyOQ.QMQVVOmQsaTAsk.#/
 E............d......................d............#/
 E.RLLb.ymOQQKKQQm.OQQKKQQm.OQQKKQQm.Oqq.y.BB.H.p..#/
 E...M................d....................BB.......g
""")
    return c.mirror().build()


def hauler():
    """Convoy hauler. An armoured tug pulling a spine of container pods, a gap
    between every pod so each container reads as its own block."""
    c = Canvas("th_convoy_hauler")
    c.rows(-25, 7, """
      ###   ###   ###   ###
     #CC.# #CC.# #CC.# #CC.#
     #CC.# #CC.# #CC.# #CC.#
     #kk.# #kk.# #kk.# #kk.#       ######/
 ### #...# #...# #...# #...#       #Q.O.s##/
E.R.#.....#.....#.....#.....#######.M.KKO.f#/
E.b.....................d...........Q...BB.p#/
E.M.UU.UU.UU.UU.UU.UU.UU.UU.UU.UU.......BB...g
""")
    return c.mirror().build()


def gun_platform():
    """Defense platform: a square battery with a gun arm on each side, radial,
    no engines. Rings a Hegemony station or protected planet."""
    c = Canvas("th_gun_platform")
    c.rows(1, 6, """
/
#
#
#g#
M.#
.s.##/
.R...n
""")
    c.rot4()
    c.cells[(0, 0)] = "."
    return c.build()


def bastion():
    """Battlestation by the Hegemony capital: a star fort. A square curtain wall
    round a docking basin, a diamond bastion on each corner so every wall is
    covered from both sides. One quadrant drawn, turned four ways."""
    c = Canvas("th_bastion")
    c.band(6, "#g##g#" + "#g###o")
    c.band(7, "QQm.yOM" + "aKKa.T.")
    c.band(8, "...d...." + "....d...")
    c.band(9, "QKKQOQmy." + "aaTaaUUa.")
    c.band(10, "bcfbcfbcf." + "bLLcbLLcb.")
    c.band(11, "." * 22)
    c.band(12, "#n#g#n#g...." + "..." + "#g#n#g#n#")
    for x in range(8, 19):           # the corner bastion, a diamond on (13, 13)
        for y in range(8, 19):
            d = abs(x - 13) + abs(y - 13)
            if d > 5 or (max(x, y) <= 12 and d >= 4):
                continue                 # where it meets the wall, the wall wins
            if d == 5:
                c.cells[(x, y)] = "n" if (x + y) % 4 == 0 else "g" if (x + y) % 4 == 2 else "#"
            elif d == 4:
                c.cells[(x, y)] = "."
            elif abs(x - 13) <= 1 and abs(y - 13) <= 1:
                c.cells[(x, y)] = "F"
            elif x == 13 or y == 13:
                c.cells[(x, y)] = "."
            else:
                c.cells[(x, y)] = "a" if (x + y) % 2 else "O"
    c.cells[(18, 13)] = "x"
    c.cells[(13, 18)] = "x"
    for k in (9, 10):
        c.cells[(k, k)] = "."
    c.rot4()
    for x in range(2, 5):             # the one bridge, north wall
        for y in (9, 10):
            c.cells[(x, y)] = "B"
    return c.build()


# Approved with the note "keep it but it look too rectangular"; reshaped since.
REVISED = {
    "lancer": "Swept fins at the stern and a stepped tail, so it reads as a dart.",
    "warden": "Gun sponson now a raked diamond with plated front facets; rounded engine pods.",
    "bulwark": "Both sponson pairs raked into diamonds with plated fronts; rounded engine pods.",
    "hauler": "Containers have clipped corners, the tug's engine block is chamfered.",
}

CONCEPTS = [
    ("lancer", lancer, "Lancer", "Scout",
     "An arrowhead with two engine pods. The first Hegemony hull a new captain meets: fast, light, mostly sensor."),
    ("warden", warden, "Warden", "Corvette",
     "One pair of gun sponsons, the bridge on the keel. About the Pincer's size: your peer."),
    ("bulwark", bulwark, "Bulwark", "Frigate",
     "Two pairs of sponsons and a railgun bow. The navy's line ship, and the one you'll meet most."),
    ("sovereign", sovereign, "Sovereign", "Battleship",
     "A keel hull and two gun hulls joined by crossbeams, with open courtyards between. Eight sponsons a side."),
    ("oracle", oracle, "Oracle", "Experimental: sensor picket",
     "A ring of radar arrays round a shielded core. Lore: it sees the fight thirty seconds early and calls the line in."),
    ("homeward", homeward, "Homeward", "Civilian: evacuation ark",
     "A keel with habitation ribs on both sides, each rib its own sealed block. The Hegemony is losing; this carries its people out."),
    ("hauler", hauler, "Convoy Hauler", "Trade",
     "An armoured tug pulling four container pods on a spine, with gaps so each container reads on its own."),
    ("gun_platform", gun_platform, "Gun Platform", "Defense platform",
     "A small cross-shaped battery that rings a Hegemony station or protected planet."),
    ("bastion", bastion, "Bastion", "Battlestation",
     "A star fort by the Hegemony capital: curtain wall round a docking basin, with arrowhead bastions covering every wall."),
]
