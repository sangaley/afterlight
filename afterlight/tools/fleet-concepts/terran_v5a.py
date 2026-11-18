"""Terran Hegemony, round 5a: eight light and medium warships that fill the
navy's ladder from scout to destroyer, between the approved Lancer, Warden,
Bulwark and Sovereign. Hand-drawn, top-down only.

The faction's approved grammar (terran.py) holds: an arrowhead bow plated on
its steps, gun sponsons out on port and starboard with the gun on the outer
face and a magazine behind it, engine nacelles at the stern corners, a keel
corridor with the bridge on the centreline amidships, mirror symmetry,
Titanium. The approved set was passed with "keep it but it looks too
rectangular", so nothing here is a box: every outline is raked, stepped,
clipped or open.

Legend: see terran.py and draw.LEGEND.
"""
from draw import Canvas
from terran import FACTION  # noqa: F401  (read by build.py)


def picket():
    """Scout: radar picket. A small lozenge, widest amidships, with a radar
    array on each beam point. It runs ahead of the line and feeds the
    Oracle's thirty seconds."""
    c = Canvas("th_picket")
    c.rows(-7, 4, """
       #A#/
     ##..M##/
   ##Q.O.s.b##/
 E#.R.BB...T.a#g/
E...MdBBH.......g
""")
    return c.mirror().build()


def dispatch():
    """Scout: dispatch courier. Under martial law nothing important goes by
    open channel: sealed orders ride in a vault behind the bridge. A slim
    lance and a broad engine yoke at the stern, all of it built to run."""
    c = Canvas("th_dispatch")
    c.rows(-11, 5, """
 E##/
 E.b#/
 E.R.##/
 #.M...####g#A/
E...O..dQ$BB..saTg#/
E.....M...BBH......g
""")
    c.mirror()
    c.P(-2, -1, "Q")   # one vault, to port; a courier officer's berth opposite
    return c.build()


def provost():
    """Corvette: interdiction. The martial-law cutter. Two engine booms trail
    aft round an open cradle where a stopped ship is clamped for inspection;
    floodlights on the bow and an ion disruptor to do the stopping."""
    c = Canvas("th_provost")
    c.rows(-12, 9, """
      g#g/
     #.a.#/
    #.....#/
  ##.......###/
 ER.M.Q.O.b.Q.##/
E..........d....##/
E##########.KK.mM.#l/
           l...s.T..#g/
           #.BB..a....#l/
           o.BBH..A.....i
""")
    return c.mirror().build()


def halberd():
    """Corvette: gunboat. A gun head on a narrow neck: the head is a plated
    diamond carrying a railgun pair and a sponson each side, the neck one
    armoured corridor with the bridge in it, the stern the engines. A crew
    that loses the head can still bring the rest home."""
    c = Canvas("th_halberd")
    c.rows(-14, 6, """
                   g#g/
 ####/            #.a.#/
N.R.O##/         #.....##/
N.b.M..##/      #.Q...Q..##/
E.Q..KK..##g##A#..m.a..T...#/
#.LL.c....d..BB..a...a...rr.#/
E.........d..BBH...M...s.....n
""")
    return c.mirror().build()


def javelin():
    """Frigate: missile. A long spear, one raked face from the stern to the
    point, stepped every seven cells; each step ends in a launcher raised on
    its own plated rack, so the faces are serrated, with the magazine just
    inboard. Engine nacelles at the stern corners."""
    c = Canvas("th_javelin")
    c.rows(-24, 8, """
 ####/
N....#/     x/
N##...######.#/    x/
    #R.b.Q.O.a##p#a.#/    x/
   E.....d...........##p#a.#/    x/
   #.LL.c.f.KK.Q.m.O.M.T.Q.a##p#a.#/
   E.....d........................a##p##g/
   #.R.b.d.Q.Q.m.s.O..BB..T.s.Q.a..rr.a..####g/
   E..................BBH.....M...............n
""")
    return c.mirror().build()


def augur():
    """Frigate: fire control. The keel runs through an open diamond frame
    hung with the Oracle's arrays, two courtyards of empty sky inside it.
    One Augur sees for a squadron; the guns on its forward faces are an
    afterthought."""
    c = Canvas("th_augur")
    c.rows(-16, 10, """
              A#A#A/
            ##...s.##/
          #A...MOT...##/
        #A...Q#   #Q...#g/
      #A...b#       #a...#g/
    #A...T#           #a...#g/
  #A....O               s....#g/
E#.R.b.Q.#A#####A#####A#.KK.m..#/
N.LL.c.f.Q.Q.m.O.T.b.M.Q.a.A.rr.##/
N.....d.........BB................#g/
E...............BBH.................n
""")
    return c.mirror().build()


def claymore():
    """Destroyer: gun line. A long blade with a cross-guard: two arms swept
    forward from amidships like a claymore's quillons, a gun sponson on each
    tip, cannons on every step of the blade and railguns down its middle.
    Engine nacelles at the stern corners."""
    c = Canvas("th_claymore")
    c.rows(-26, 13, """
                    n#n/
                   #...#/
                  #...aT#
                 A..a#/
                #..s#/
 ####/         A..a#/
N....#/       #..M#
N###..#g#g####.....#g####/
    E.R.b.c.a.KK.Q.Q.a.O.####n#/
    #......d.......d..........####n#/
    E.LL.f.O.SSS.m.GG.T.M.Q.a.s.A..####n#/
    #........SSS...GG.......a..rr..T....####g#/
    E..FFF.c.Q.KK.O.y.d.BB..s.Q.a...rr.a.....###g#/
    E..FFF..............BBH.......M...............n
""")
    return c.mirror().build()


def tribune():
    """Destroyer: twin hull. Two gun hulls side by side, tied amidships by a
    lozenge of bridge deck, open sky fore and aft between them. A hit that
    would break a single keel leaves a Tribune fighting on two."""
    c = Canvas("th_tribune")
    c.rows(-19, 12, """
          n#n/          n#n/
 ####/   #.a.#/        #.a.#/
N....#/ #.....#/      #.....#/
N###..##.a...a.#g###g#..a.a..###/
  #R.b.Q.a.O.a.Q.Q.m.KK.a.T.a.M.##g/
 E.......d...................d.......g/
 ELL.c.M.O.Q.y.T.s.m.a.Q.b.a.rr.a.rr..n
 E.......d...................d.......#/
  #R.b.Q.O.KK..............Q.a.m.###/
   ###A######..d.......d..##A###/
             #.Q.O...s.Q.#
           ##.m...BB..T...##/
          g.......BBH.......g
""")
    return c.mirror().build()


CONCEPTS = [
    ("picket", picket, "Picket", "Scout: radar picket",
     "A small lozenge with a radar array on each beam point. It runs ahead of the line and feeds the Oracle its thirty seconds."),
    ("dispatch", dispatch, "Dispatch", "Scout: dispatch courier",
     "Under martial law nothing that matters goes by open channel. A slim lance with sealed orders in a vault behind the bridge, and a broad engine yoke built to run."),
    ("provost", provost, "Provost", "Corvette: interdiction",
     "The martial-law cutter. Two engine booms trail round an open cradle where a stopped ship is clamped and searched; floodlights forward, and an ion gun to do the stopping."),
    ("halberd", halberd, "Halberd", "Corvette: gunboat",
     "A plated gun head on a narrow neck, the bridge riding in the neck. Railguns in the point, a sponson on each beam; lose the head and the stern still comes home."),
    ("javelin", javelin, "Javelin", "Frigate: missile",
     "A long spear stepped every seven cells, a launcher on every step with its magazine behind it. It stands off and empties its racks before the line closes."),
    ("augur", augur, "Augur", "Frigate: fire control",
     "The keel runs through an open diamond frame hung with the Oracle's arrays. One Augur sees thirty seconds ahead for a whole squadron; its guns are an afterthought."),
    ("claymore", claymore, "Claymore", "Destroyer: gun line",
     "A long blade with a cross-guard: two arms swept forward like a claymore's quillons, a gun sponson on each tip and a cannon on every step of the blade."),
    ("tribune", tribune, "Tribune", "Destroyer: twin hull",
     "Two gun hulls side by side, tied amidships by a lozenge of bridge deck, open sky fore and aft between them. A hit that would break one keel leaves a Tribune fighting on two."),
]
