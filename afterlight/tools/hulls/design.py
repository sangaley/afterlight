#!/usr/bin/env python3
"""Four claw hulls, fitted out with real modules.

Right is forward (+x). '#' is hull. Modules are placed by cell afterwards.
Decking is derived the way the game derives it: every enclosed cell that no
module stands on becomes hallway.
"""
import json, os
from collections import deque

MODS = {}
for line in open('/tmp/modules.tsv'):
    _, name, sx, sy, post, cat, cost = line.rstrip('\n').split('\t')
    MODS[name] = dict(w=int(sx), h=int(sy), post=(post == 'true'), cat=cat, cost=int(cost))

# ---------------------------------------------------------------- hulls ----
HULLS = {}

HULLS['pincer'] = """
.....###############..
...##################.
..###################.
..#####...............
.#######..............
.######...............
.#######..............
.######...............
.#######..............
..#####...............
..###################.
...##################.
.....###############..
"""

HULLS['trident'] = """
....################..
..##################..
..#####...............
.#######..............
.######...............
.#####################
.#####################
.#####################
.######...............
.#######..............
..#####...............
..##################..
....################..
"""

HULLS['crab'] = """
.....##############...
...################...
..#################...
..#######.............
.#########............
.########.............
.#########............
.########.............
..#######.............
..#################...
...################...
.....##############...
"""

HULLS['scorpion'] = """
...........###########
........##############
..####################
..####################
..#####...............
.#######..............
.######................
.#######..............
..#####...............
..#################...
..################....
....##########........
"""

def cells_of(art):
    rows = art.strip('\n').split('\n')
    h = len(rows)
    out = set()
    for r, line in enumerate(rows):
        for c, ch in enumerate(line):
            if ch == '#':
                out.add((c, h - 1 - r))
    # normalise so the stern sits near x = -9 and the ship straddles y = 0
    xs = [p[0] for p in out]; ys = [p[1] for p in out]
    ox = -9 - min(xs)
    oy = -((min(ys) + max(ys)) // 2)
    return {(x + ox, y + oy) for (x, y) in out}

# -------------------------------------------------------------- fitting ----
# Non-rectangular footprints, mirroring building::footprints. Reading size.x
# by size.y instead cost an afternoon: the bridge is a T of four cells, not a
# 3x2 block of six, and the two cells my model invented for it were the only
# thing holding an arm onto the body. The game disagreed and the game was
# right.
FOOTPRINTS = {
}

def cells_for(t, x, y):
    """Exactly what ShipGrid::cells_for returns, for rotation North."""
    if t in FOOTPRINTS:
        return [(x+dx, y+dy) for (dx, dy) in FOOTPRINTS[t]]
    d = MODS[t]
    return [(x+dx, y+dy) for dx in range(d['w']) for dy in range(d['h'])]

def occupied(cells, mods):
    taken = set()
    for m in mods:
        taken.update(cells_for(m['t'], m['x'], m['y']))
    return taken

def interior(cells):
    """Cells with hull on all four sides -- the ones that can be decked."""
    return {c for c in cells
            if all((c[0]+d[0], c[1]+d[1]) in cells for d in ((0,1),(0,-1),(-1,0),(1,0)))}

def deck(cells, mods):
    return interior(cells) - occupied(cells, mods)

def posts(mods):
    out = set()
    for m in mods:
        if MODS[m['t']]['post']:
            out.update(cells_for(m['t'], m['x'], m['y']))
    return out

def walkable(cells, mods):
    return deck(cells, mods) | posts(mods)

def connected(cells, mods):
    w = walkable(cells, mods)
    if not w:
        return 0, 0, []
    start = next(iter(sorted(w)))
    seen = {start}; q = deque([start])
    while q:
        cx, cy = q.popleft()
        for d in ((0,1),(0,-1),(-1,0),(1,0)):
            n = (cx+d[0], cy+d[1])
            if n in w and n not in seen:
                seen.add(n); q.append(n)
    stranded = sorted(posts(mods) - seen)
    return len(w), len(seen), stranded

# ------------------------------------------------------------ placement ----
def fits(cells, mods, t, x, y):
    taken = occupied(cells, mods)
    return all(c in cells and c not in taken for c in cells_for(t, x, y))

def deck_is_whole(cells, mods):
    """Is every walkable cell reachable from every other?

    Checked after each placement, because a single machinery module dropped in
    the wrong cell severs an arm from the body -- and an arm whose deck is cut
    off is an arm whose guns can never be crewed.
    """
    w = walkable(cells, mods)
    if not w:
        return False
    start = next(iter(sorted(w)))
    seen = {start}; q = deque([start])
    while q:
        cx, cy = q.popleft()
        for d in ((0,1),(0,-1),(-1,0),(1,0)):
            n = (cx+d[0], cy+d[1])
            if n in w and n not in seen:
                seen.add(n); q.append(n)
    return len(seen) == len(w)

def place(cells, mods, t, ax, ay, outboard=False):
    """Put `t` at the nearest free cell that does not cut the deck in two."""
    d = MODS[t]
    skin = {c for c in cells if c not in interior(cells)}
    cands = []
    for c in sorted(cells):
        if not fits(cells, mods, t, c[0], c[1]):
            continue
        if outboard and c not in skin:
            continue
        if not outboard and c in skin:
            continue
        cands.append((abs(c[0]-ax) + abs(c[1]-ay), c))
    cands.sort()
    if not cands:
        cands = sorted((abs(c[0]-ax)+abs(c[1]-ay), c) for c in sorted(cells)
                       if fits(cells, mods, t, c[0], c[1]))
    for _, c in cands:
        if d['post']:
            # A crewed module has to touch somewhere a crewman can stand, or
            # it is a gun on an arm tip that nobody can ever reach.
            here = walkable(cells, mods)
            cover = set(cells_for(t, c[0], c[1]))
            touching = any((cx+ddx, cy+ddy) in here
                           for (cx, cy) in cover
                           for (ddx, ddy) in ((0,1),(0,-1),(-1,0),(1,0)))
            if not touching:
                continue
        mods.append({'t': t, 'x': c[0], 'y': c[1], 'r': 'North'})
        if d['post'] or deck_is_whole(cells, mods):
            return True
        mods.pop()
    return False

def fit_out(cells, plan):
    """Build a module list from a plan of (type, anchor_x, anchor_y, outboard)."""
    mods = []
    for (t, ax, ay, out) in plan:
        place(cells, mods, t, ax, ay, out)
    return mods
