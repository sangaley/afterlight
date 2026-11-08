"""The Silence: hand-drawn, top-down only.

Lore: they found perfect peace by removing ambition; they lobotomised their
entire species. They cannot start a fight, and what hits them is reflected.
In the game they carry no weapons at all: they shadow you at a distance,
flee when shot, and are the fastest thing in the sky.

Grammar, held at every size (all of it reads from above):
  - NEEDLES: long, thin lance hulls only 3-5 cells wide, tapering to a point
    at the bow (a plate on each step of the point, nowhere else);
  - THIN OUTRIGGERS: single-cell rails held off the hull on one-cell struts,
    carrying the passive sensors; nothing else sticks out;
  - a FEATHERED STERN: engines stepped back along the tail like fletching;
  - quiet interiors: one keel corridor, a few rooms, stealth coating set into
    the walls at an even beat;
  - perfectly calm symmetry; Composite; no weapons anywhere.

Legend (draw.py): # hull  . deck  / plate  E engine  R reactor  b battery
B bridge  H helm  M memory core  s shield  Q quarters  O oxygen  m medbay
A radar  ? passive radar  I thermal imager  ~ stealth coating  J signal buoy
d door  y repair
"""
import math

from draw import Canvas

FACTION = {
    "slug": "the_silence",
    "name": "The Silence",
    "tagline": "They found perfect peace by removing ambition from their whole species. They never start a fight; they watch, and they leave.",
    "grammar": [
        "Needles: long lance hulls only 3-5 cells wide, tapering to a point",
        "Thin outriggers: single-cell rails on one-cell struts, carrying the passive sensors",
        "A feathered stern: engines stepped back along the tail like fletching",
        "Quiet interiors: one keel corridor, few rooms, stealth coating set into the walls at an even beat",
        "Perfectly calm symmetry, Composite, and no weapons anywhere",
    ],
    "accent": "#e6ecf2",
}


def listener():
    """Scout. One needle and a pair of whiskers. It sits at the edge of your
    sensor range and listens."""
    c = Canvas("si_listener", "Composite")
    c.rows(-14, 5, """
       I#######?
           #
           #
   E##~##M##~###~##M#/
 E..R.Q.O...BB..s..I.#/
E...........BB.H......A?
""")
    return c.mirror().build()


def hush():
    """Corvette. Twin needles, a yoke between them holding the bridge and a
    thin bridge forward. Every room sits on one corridor; the walls carry the
    rest."""
    c = Canvas("si_hush", "Composite")
    c.rows(-17, 6, """
 E##~#b#~#M#~###~#I#~###~#M#~##/
E..R..Q....O..........Q........A?
 E##~###~###~#s#~.##~###~##.~##/
                #.#       #.#
               #...#      #.#
              #.BB..#     #.#
              #.BB.H#     #.#
""")
    return c.mirror().build()


def vigil():
    """Frigate-weight picket. A very long sensor lance: a quiet body aft, a
    bead amidships holding the bridge and a cross-tree of passive sensors,
    and a long spar of radar arrays ahead. It sees you long before you see
    it, and it keeps exactly that distance."""
    c = Canvas("si_vigil", "Composite")
    c.rows(-42, 9, """
                                           ?
                                           #
                                           #
                                           I
       J#################?
          #           #                    #
          #           #                    #        ?###################?
   E###~###~#M#~#s#~###~#M#~###         #M###~#       #               #
 E...R..b...Q..O....Q...m...y..##~###~##.BB..s.###A##~##A##~##A##~##A##~##A##~#/
E........................................BB.H..................................A?
""")
    return c.mirror().build()


def monastery():
    """Station. A still halo: a thin ring with eight needles pointing out
    from it, nothing in the middle at all. The four long needles carry radar
    at their tips, the four diagonals carry only passive ears. Everything is
    placed in mirrored pairs, so the station looks the same from every side.
    One quadrant drawn, turned four ways."""
    c = Canvas("si_monastery", "Composite")
    for x in range(1, 36):
        for y in range(0, 36):
            if 11.6 <= math.hypot(x, y) <= 16.6:                 # the ring
                c.cells[(x, y)] = "#"
            if abs(x - y) <= 1 and x + y >= 20 and max(x, y) <= 22:
                c.cells[(x, y)] = "#"                            # diagonal needle
    for k in range(14, 32):                                      # long needle
        c.cells[(k, 0)] = "."
        c.cells[(k, 1)] = c.cells[(1, k)] = "#"
    c.P(32, 0, "A?").P(32, 1, "/").P(1, 32, "/")
    pairs = {(15, 3): "Q", (14, 6): "Q", (13, 2): "O", (12, 5): "R", (13, 8): "M",
             (11, 7): "s", (16, 4): "~", (15, 7): "I", (16, 16): "~", (18, 18): "I",
             (22, 22): "?", (18, 1): "~", (22, 1): "~", (26, 1): "~", (30, 1): "~"}
    for (x, y), ch in pairs.items():          # every item has its mirror across the diagonal
        c.cells[(x, y)] = c.cells[(y, x)] = ch
    c.rot4()
    c.R(-1, 1, 13, 14, "B")                   # the one bridge, north arc
    c.cells[(0, 15)] = "H"
    return c.build()


CONCEPTS = [
    ("listener", listener, "Listener", "Scout",
     "One needle and a pair of sensor whiskers. It sits at the edge of your sensor range and listens."),
    ("hush", hush, "Hush", "Corvette",
     "Twin needles joined by a yoke with the bridge and one thin bridge forward. Every room sits on a single corridor."),
    ("vigil", vigil, "Vigil", "Frigate: sensor picket",
     "A very long sensor lance: a quiet body, a bead amidships with the bridge and a cross-tree of sensors, and a spar of radar ahead."),
    ("monastery", monastery, "Monastery", "Station",
     "A still halo: a thin ring with eight needles pointing outward and nothing at all in the middle. It looks the same from every side."),
]
