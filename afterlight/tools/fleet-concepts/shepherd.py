"""The Shepherd: hand-drawn, top-down only.

Lore: an apex boss (danger 8) that "wants you to become one". It engages
anything that comes within 12,000 units, and what it catches it keeps.

Grammar: everything is built round a shepherd's crook.
  - a long STAFF spine, ending in a great curling HOOK at the bow; the inside
    of the hook is open space, and the hook's inner face is lined with
    tractor beams that pull ships into it;
  - curves are diagonal staircases, and only the front-facing steps (and the
    hook tips) carry plates;
  - smaller HOOKS (barbs, horns, gateposts) repeat the crook at every size;
  - attendants (Lambs) launch from cradles on the staff and orbit it like a
    flock;
  - AbyssalAlloy throughout.

Legend: see terran.py and draw.LEGEND. 't' tractor beam, 'P' plasma caster,
'i' ion disruptor, 'D' drone bay (the flock's cradles).
"""
import math

from draw import Canvas, AIMED, ENGINES, LEGEND
import fleet

FACTION = {
    "slug": "shepherd",
    "name": "The Shepherd",
    "tagline": "An apex hunter that wants you to become one. It engages anything within 12,000 units, and what it catches it keeps.",
    "grammar": [
        "A long staff spine ending in a great curling hook at the bow, open inside the hook",
        "Tractor beams line the hook's inner face; the hook is where ships are pulled in",
        "Curves as diagonal staircases, plates only on the forward steps and the hook tips",
        "Smaller hooks (barbs, horns, gateposts) repeat the crook at every size",
        "Attendants launch from cradles and orbit like a flock; AbyssalAlloy",
    ],
    "accent": "#e8c547",
}

MAT = "AbyssalAlloy"
N4 = ((0, 1), (0, -1), (1, 0), (-1, 0))
POSTS = set(fleet.POSTS) | {"EmergencyBulkhead"}


def finish(c):
    """c.build(), plus checks the shared checker leaves out: every gun has an
    open side to fire from, every engine has clear space aft."""
    d = c.build()
    hull = {p for p, ch in c.cells.items() if ch != "/"}
    for p, ch in c.cells.items():
        kind = LEGEND.get(ch)
        x, y = p
        if kind in AIMED and kind != "DockingPort":
            if all((x + dx, y + dy) in hull or c.cells.get((x + dx, y + dy)) == "/" for dx, dy in N4):
                d["_checks"].append(f"{kind} at {p} has no open side")
        if kind in ENGINES and any((xx, y) in hull for xx in range(x - 40, x)):
            d["_checks"].append(f"{kind} at {p} is blocked aft")
    return d


def put_row(c, y, x0, x1, base, items):
    """One row x0..x1 of `base`, with the named cells overridden."""
    for x in range(x0, x1 + 1):
        c.cells[(x, y)] = items.get(x, base)


def arc(cx, cy, r_in, r_out, th0, th1):
    """Cells of an annular sector, angles in degrees counter-clockwise from +x."""
    out = {}
    for x in range(int(cx - r_out) - 2, int(cx + r_out) + 3):
        for y in range(int(cy - r_out) - 2, int(cy + r_out) + 3):
            r = math.hypot(x - cx, y - cy)
            t = (math.degrees(math.atan2(y - cy, x - cx)) - th0) % 360 + th0
            if r_in <= r <= r_out and t <= th1:
                out[(x, y)] = (r, t)
    return out


def paint_arc(c, cells, centre, walk_r, spokes, rooms, outer_guns, inner_guns, keep=()):
    """Lay out the inside of a curved hull as rooms along a curved corridor:
    a walkway at radius walk_r, radial passages at the `spokes` angles, a gun
    where each passage meets the outer rim and a tractor where it meets the
    inner rim. Between passages each room is one sector of `rooms`:
    (from_angle, inner_post, inner_fill, outer_post, outer_fill); a post
    letter goes only where it touches the corridor."""
    cx, cy = centre
    for p in cells:
        if p not in keep:
            c.cells[p] = "#"
    hull = {p for p, ch in c.cells.items() if ch != "/"}
    inner = lambda p: all((p[0] + dx, p[1] + dy) in hull for dx, dy in N4)
    walk = set()
    for p, (r, t) in cells.items():
        if p in keep or not inner(p):
            continue
        near_spoke = any(abs(math.radians(((t - s + 180) % 360) - 180)) * r < 0.75 for s in spokes)
        if abs(r - walk_r) < 0.8 or near_spoke:
            c.cells[p] = "."
            walk.add(p)
    for p, (r, t) in cells.items():
        if p in keep or not inner(p) or p in walk:
            continue
        room = [rm for rm in rooms if rm[0] <= t][-1]
        ip, if_, op, of = room[1:]
        touches = any((p[0] + dx, p[1] + dy) in walk for dx, dy in N4)
        by_wall = any(not inner((p[0] + dx, p[1] + dy)) for dx, dy in N4)
        post, fill = (ip, if_) if r < walk_r else (op, of)
        c.cells[p] = post if touches else fill if by_wall else "."
    # Guns and tractors at the ends of the passages.
    for k, s in enumerate(spokes):
        for guns, pick in ((outer_guns, max), (inner_guns, min)):
            if not guns:
                continue
            rim = [p for p, (r, t) in cells.items() if p not in keep and not inner(p)
                   and abs(math.radians(((t - s + 180) % 360) - 180)) * r < 1.6
                   and any((p[0] + dx, p[1] + dy) in walk for dx, dy in N4)]
            if not rim:
                continue
            p = pick(rim, key=lambda q: cells[q][0])
            c.cells[p] = guns[k % len(guns)]
    return walk


def plate_front(c, x_min=None, test=None):
    """Plate the forward-facing steps of a staircase edge: every empty cell that
    sits in an L of hull opening forward (+x) or at a tip, limited by `test`."""
    hull = {p for p, ch in c.cells.items() if ch != "/"}
    xs = [p[0] for p in hull]
    for x in range(min(xs) - 1, max(xs) + 2):
        for y in range(min(p[1] for p in hull) - 1, max(p[1] for p in hull) + 2):
            p = (x, y)
            if p in c.cells or (x_min is not None and x < x_min) or (test and not test(p)):
                continue
            n, s, w, e = ((x, y + 1) in hull, (x, y - 1) in hull, (x - 1, y) in hull, (x + 1, y) in hull)
            if w and (n != s) and not e:
                c.cells[p] = "/"
    # A plate must never close a gun's only open side.
    for p, ch in list(c.cells.items()):
        if LEGEND.get(ch) in AIMED:
            nb = [(p[0] + dx, p[1] + dy) for dx, dy in N4]
            if all(q in hull or c.cells.get(q) == "/" for q in nb):
                for q in nb:
                    if c.cells.get(q) == "/":
                        del c.cells[q]
                        break


# --------------------------------------------------------------------------- ships


def the_shepherd():
    """Boss flagship. A colossal crook: an eighty-cell staff, and at its bow a
    hook that curls up, over and back on itself, open inside. Tractor beams
    line the inside of the hook. Three barbed cradles under the staff launch
    the flock."""
    c = Canvas("sh_the_shepherd", MAT)
    X0, X1 = -60, 20
    for a in (-41, -15):                      # two binding bands round the staff
        put_row(c, 6, a, a + 4, "#", {a + 1: "i", a + 3: "z"})
    put_row(c, 5, X0 + 6, X1, "#", {-52: "i", -45: "z", -40: ".", -38: ".", -28: "P", -18: "z",
                                     -14: ".", -12: ".", -2: "P", 8: "i", 14: "z"})
    put_row(c, 4, X0 + 3, X1, ".", {-57: "E", -54: "R", -53: "c", -51: "f", -49: "R", -47: "L", -46: "L",
                               -44: "O", -42: "b", -38: "Q", -36: "Q", -33: "K", -32: "K", -30: "Q", -26: "m",
                               -24: "y", -22: "Q", -20: "Q", -16: "q", -14: "q", -12: "s", -9: "s", -6: "T",
                               -4: "a", 0: "a", 2: "s", 4: "a", 6: "M", 10: "a", 12: "T", 16: "s", 18: "a"})
    put_row(c, 3, X0, X1, ".", {-60: "E", -45: "d", -30: "d", -8: "d", 5: "d"})
    put_row(c, 2, X0, X1, ".", {-60: "N", -57: "L", -56: "L", -54: "b", -52: "c", -50: "b", -47: "R", -45: "c",
                               -43: "R", -40: "f", -38: "K", -37: "K", -35: "O", -33: "K", -32: "K", -30: "m",
                               -28: "S", -27: "S", -26: "S", -24: "y", -22: "O", -20: "Q", -17: "M", -14: "A",
                               -12: "T", -7: "@", -6: "@", -3: "a", -1: "a", 1: "T", 3: "a", 5: "a", 7: "s",
                               9: "M", 11: "b", 13: "a", 15: "a", 17: "T", 19: "s"})
    put_row(c, 1, X0, X1, ".", {-60: "N", -57: "b", -55: "c", -53: "v", -50: "R", -47: "b", -44: "O", -41: "Q",
                               -38: "Q", -35: "Q", -28: "S", -27: "S", -26: "S", -23: "m", -20: "O", -17: "s",
                               -14: "s", -11: "B", -10: "B", -8: "H", -5: "a", -3: "T", -1: "a", 2: "s", 4: "a",
                               6: "c", 8: "a", 11: "s", 13: "a", 15: "b", 17: "a"})
    put_row(c, 0, X0, X1, ".", {-60: "E", -11: "B", -10: "B"})
    put_row(c, -1, X0, X1, ".", {-60: "N", -57: "b", -55: "c", -50: "R", -47: "b", -44: "O", -41: "q", -38: "G",
                                -37: "G", -34: "Q", -31: "Q", -28: "m", -25: "O", -22: "V", -19: "V", -16: "s",
                                -14: "T", -11: "B", -10: "B", -7: "a", -5: "s", -2: "a", 1: "b", 4: "a", 7: "s",
                                10: "a", 13: "b", 16: "a", 19: "s"})
    put_row(c, -2, X0, X1, ".", {-60: "N", -57: "L", -56: "L", -53: "f", -50: "c", -47: "R", -44: "f", -41: "y",
                                -38: "G", -37: "G", -35: "K", -34: "K", -31: "m", -28: "Q", -25: "Q", -22: "O",
                                -19: "M", -15: "A", -12: "@", -11: "@", -8: "T", -5: "a", -2: "s", 1: "a", 4: "T",
                                7: "a", 10: "s", 13: "a", 16: "M", 19: "a"})
    put_row(c, -3, X0, X1, ".", {-60: "E", -45: "d", -30: "d", -8: "d", 5: "d"})
    put_row(c, -4, X0 + 3, X1, ".", {-57: "E", -54: "R", -52: "b", -50: "f", -48: "Q", -46: "O", -42: "Q",
                                -39: "a", -37: "Q", -35: "O", -33: "y", -27: "Q", -25: "k", -22: "Q", -20: "m",
                                -18: "Q", -13: "s", -11: "T", -6: "a", -3: "a", 0: "s", 3: "a", 6: "b", 9: "a",
                                12: "T", 15: "a", 18: "s"})
    put_row(c, -5, X0 + 6, X1, "#", {-54: "z", -38: "i", -23: "P", -8: "z", 2: "i", 10: "P", 17: "z"})
    # The barbed cradles under the staff: a strut, then a hook curling aft.
    for x0 in (-44, -30, -16):
        c.rows(x0 - 6, -5, """
      .
     #.#
#P   #.#
#.####.#
#......#
 #DD#DD
""")
    # The hook: from the bottom of the staff's bow, round the front, over the
    # top and back down, ending above the staff with its mouth open.
    hook = arc(21, 16, 12, 21.4, -90, 200)
    keep = {p for p in hook if p[0] <= X1 and -5 <= p[1] <= 5}
    paint_arc(c, hook, (21, 16), 16.5, spokes=[-62, -30, 0, 28, 56, 84, 112, 140, 168, 194],
              rooms=[(-90, "R", "c", "b", "f"), (-45, "O", "a", "T", "a"), (0, "Q", "s", "a", "a"),
                     (42, "m", "V", "Q", "k"), (98, "y", "b", "O", "a"), (126, "A", "s", "Q", "T"),
                     (154, "O", "T", "a", "a")],
              outer_guns=["P", "i", "P", "z", "P", "i", "z", "P", "i", "P"], inner_guns=["t"], keep=keep)
    for p in ((34, 9), (24, 33), (5, 22)):   # memory cores, round the hook
        c.cells[p] = "M"
    plate_front(c, test=lambda p: (p[0] >= 24 and p[1] <= 26) or (p[0] <= 6 and p[1] <= 12 and p[1] >= 6))
    return finish(c)


def lamb():
    """Attendant. A small crook: a short staff with its bow curled up and back
    into a hook, a tractor on the inside of the curl. Lambs launch from the
    Shepherd's cradles and orbit it like a flock."""
    c = Canvas("sh_lamb", MAT)
    c.rows(-9, 9, """
      #####
     #######/
    ###    ###/
    ##      #.#
    ##      t.#
    #/      #.P
            #.##
###z####i####.##/
E.R.M.OBB..s..M.#/
E......BB........P
E.f.b.QBB.M.a.s.#/
###z#####p#i####/
""")
    return finish(c)


def ram():
    """Escort. Ram's horns: two crooks back to back, curling up and over the
    bow on each side, open in the middle of each curl, tractors facing in.
    Between the horns, a blunt plated ram."""
    c = Canvas("sh_ram", MAT)
    c.rows(-12, 13, """
          ##P###/
         #...O..##/
        #..####..a#
        #.t    t..#
        z.#    #..i
        #.t    t.b#
        #P/    #.s#
              #c..z
             #f..#
 ###z###i####..a..P/
E.R..Q.O.MQ.y.s..T.#/
E...................#/
E.LL.b.c.KK.BB..as...##/
E...........BB.....M...n
""")
    return finish(c.mirror())


def turn(p, k):
    """Rotate a cell a quarter turn counter-clockwise, k times."""
    x, y = p
    for _ in range(k % 4):
        x, y = -y, x
    return (x, y)


def the_fold():
    """Experimental station: the sheepfold. A round wall of four curved
    sections with a gate between each, round a central hub joined to every
    section by a spoke, so the fold is four pens. At each gate the leading wall
    ends in a crook that curls back into the pen, its pocket facing the gate:
    what comes in is caught. Tractor beams line the inside of the wall."""
    c = Canvas("sh_the_fold", MAT)
    hub = """
#####d#####
#M.s...s.M#
#.y..O..m.#
#s.......s#
#..q.HBB..#
d.O...BB.Od
#..q..BB..#
#s.......s#
#.m..O..y.#
#M.s...s.M#
#####d#####
"""
    spoke = ["#i###t###i###P", "aRa.s.Q.s.aRa.", "d.....d.....d.", "a.c.O.a.O.c.a.", "#t###i###t###i"]
    hook = """
 /##
 #sl
#D#     ###
#D#  l####
#s#p###
##s###
 ####
"""
    walls = []
    for k in range(4):
        walls.append({turn(p, k): v for p, v in arc(0, 0, 20, 26.4, -32, 32).items()})
        for dy, row in zip((2, 1, 0, -1, -2), spoke):
            for i, ch in enumerate(row):
                c.cells[turn((6 + i, dy), k)] = ch
        for j, row in enumerate(hook.strip("\n").split("\n")):
            for i, ch in enumerate(row):
                if ch != " ":
                    c.cells[turn((8 + i, 13 - j), k)] = ch
    for w in walls:
        for p in w:
            c.cells.setdefault(p, "#")
    c.rows(-5, 5, hub)
    for k, w in enumerate(walls):
        # arc() angles are absolute, so re-measure them for this section
        cells = {p: (r, (t - 90 * k + 180) % 360 - 180 + 90 * k) for p, (r, t) in
                 ((p, (math.hypot(*p), math.degrees(math.atan2(p[1], p[0])))) for p in w)}
        paint_arc(c, cells, (0, 0), 23.2, spokes=[90 * k + a for a in (-26, -13, 0, 13, 26)],
                  rooms=[(90 * k - 40, "Q", "a", "R", "b"), (90 * k - 13, "m", "s", "O", "c"),
                         (90 * k, "y", "V", "Q", "a"), (90 * k + 13, "O", "s", "R", "f")],
                  outer_guns=["P", "i", "z", "i", "P"], inner_guns=["t"],
                  keep={p for p in w if abs(turn(p, -k)[1]) <= 2 and turn(p, -k)[0] <= 21})
    return finish(c)


CONCEPTS = [
    ("the_shepherd", the_shepherd, "The Shepherd", "Boss flagship",
     "A colossal crook: an eighty-cell staff ending in a hook that curls up, over and back, open inside, its inner face lined with tractor beams. Barbed cradles under the staff launch the flock."),
    ("lamb", lamb, "Lamb", "Attendant",
     "A short staff with its bow curled into a small hook, a tractor inside the curl. Lambs launch from the Shepherd's cradles and orbit it like a flock."),
    ("ram", ram, "Ram", "Escort",
     "Ram's horns: two crooks back to back curling up and over the bow, tractors facing into each curl, a blunt plated ram between them."),
    ("the_fold", the_fold, "The Fold", "Experimental: herding station",
     "A sheepfold: a round wall in four sections round a hub, joined by spokes into four pens. At each gate the wall ends in a crook curling back into the pen, its pocket facing the gate. What comes in is caught."),
]
