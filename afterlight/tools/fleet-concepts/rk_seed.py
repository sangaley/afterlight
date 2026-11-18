"""Recursive Kingdom: THE SEED, the one shape every Recursive Kingdom hull is
built from. Import this to build any RK ship; draw.py does the rest.

Lore: one person copied themselves until the copies became a civilisation.
So every RK hull is made of copies of one shape, and bigger ships are copies
of the seed arranged as the seed again: a chevron of chevrons, a copy nested
in a bigger copy's notch, a chain shrinking toward the stern.

THE SEED (the small one, bow is +x, read from above):

    ##*###/          an arrowhead: a plated NOSE at the tip, two ARMS swept
    E....A#          back at 45 degrees (one row in, one column back), a V
    ERfc...g         NOTCH cut into the stern between them, and at the aft
      #MQ...#        end of each arm a squared-off BARB that sticks out past
       #bO..a#/      the notch line and carries the engines on its aft face.
        #s...T#/     Plates only on the nose and the two shoulders (the
         #MH...g     outer front corner of each arm), never on every step.
        #s...T#/     A corridor climbs each arm as a two-cell staircase
       #bO..a#/      from the keel to the barb.
      #MQ...#
    ERfc...g         Every seed has the same APARTMENT in each arm, the same
    E....A#          person's cabin copied: a diagonal of M Q over b O
    ##*###/          (memory core, quarters / battery, oxygen scrubber) on
                     the notch side of the corridor. Bigger seeds hold more
                     copies of it (small 1 per arm, medium 2, large 3).

SYMMETRY RULE (the developer's, it overrides everything below): every RK
ship's skeleton is mirror-symmetric port/starboard about y = 0: the outline,
the engines and the main guns. Paint the skeleton (copies in mirrored pairs,
or one half), call c.mirror(), and only then add COPY ERRORS with flaw().
Copy errors are small, deliberate details, roughly 10% of a ship at most:
one slightly shorter tip (clip), a single offset pod, a salvaged patch
(patch), a differently laid-out cabin (swap). They never make the silhouette
lopsided and never move an engine or a main gun. finish() enforces it:
engines and weapons must have a mirror image, and at most 10% of the hull
may lack one. An experimental ship may add symmetry (the Mirror is symmetric
about both axes as well), never lose it.

ANCHOR AND AXES: a seed's local (0, 0) is its TIP, the nose cell on the
centreline (a gatling; a cannon on the large seed, which also mounts one on
each arm's front corner: cannons only on the bigger ships). Every other cell
has local x < 0. Local y runs from -half to +half; +y is PORT, -y is
STARBOARD (port is on your left facing the bow). Every seed is
port/starboard symmetric. A ship's centreline is world y = 0: put single
copies on it, and pairs of copies at (x, y) and (x, -y) (the second with
flip=True, so its flaws land mirrored).

SIZES (hull cells, length along x by span along y, from the tip back):

    size      hull  box     band  half  notch_x  stern_x  helm    apartments
    "xs"        66  10x11      6     5       -5       -9  (-4,0)  none (M+Q)
    "small"     91  12x13      7     6       -6      -11  (-4,0)  1 per arm
    "medium"   171  17x19      9     9       -8      -16  (-4,0)  2 per arm
    "large"    257  21x23     11    11      -10      -20  (-4,0)  3 per arm

  band     cells across an arm, counted along one row;
  half     the seed spans local y = -half..+half;
  notch_x  local x of the notch apex, the hull cell at the bottom of the V
           on the centreline (the stern of the keel);
  stern_x  local x of the barbs' aft faces, where the engines sit (2 per
           barb: on rows +-(half-1) and +-(half-2); large has 3 per barb);
  helm     the helm cell. Medium and large carry a 2x3 bridge on the keel
           just aft of the helm, small and xs only the helm.
  Row y (0 <= y <= half - barb rows) runs from x = notch_x - y to x = -y;
  the barb rows run from stern_x to -y. Every seed is 2*half+1 rows tall.

PAINTING:

    from rk_seed import canvas, seed, weld, flaw, finish
    c = canvas("rk_pair")                       # a Canvas, Steel, with patches
    inner = {(-9, -6): "#"}                     # the weld would bury this rocket
    a = seed(c, (0, 7), "small", swap=inner)    # tip at world (0, 7)
    b = seed(c, (0, -7), "small", flip=True, swap=inner)   # its mirror twin
    weld(c, -11, -6, 0, 0)                      # join them at the inner barbs
    c.mirror()                                  # the skeleton is symmetric
    flaw(c, b, clip="nose")                     # one small copy error
    return finish(c)                            # build + RK checks
  (rk_light.py builds every light ship this way; read it for working examples.)

  seed(c, at, size, rot=0, flip=False, nose=True, engines=None, short=None,
       swap=None, patch=None, clip=None) paints one copy, returns a Seed:
    at       world cell of the tip;
    rot      quarter turns counter-clockwise: 0 bow east (+x), 1 north,
             2 west (pointing aft), 3 south;
    flip     mirror local y first (port <-> starboard), so a pair at (x, y)
             and (x, -y) gets its flaws on mirrored sides;
    nose     False: the tip becomes deck and the nose plates go, for a copy
             whose tip is welded into another hull;
    engines  None keeps engines only on a copy facing east (rot 0); engines
             vent aft (-x) in game, so a turned copy's would fire into hull.
             False turns them into plain hull, True keeps them anyway;
    short    SKELETON VARIANT "port" | "stbd" | "both": that arm's barb is cut
             back to the notch line (the arm ends square, 2-3 cells shorter,
             its engines move with it). It changes the outline and engines,
             so it is not a copy error: use "both", or give the mirrored copy
             of the pair the other side, so the ship stays symmetric;
    swap, patch, clip: the copy errors, as for flaw() below. Painted at once,
             so give them here only when no c.mirror() follows.
  Never overlap two copies: plan with outline() first. Leave at least one
  empty cell between copies you don't weld, so each copy's outline reads.

  flaw(c, s, clip=None, swap=None, patch=None) adds COPY ERRORS to a seed
  already painted (call it after c.mirror()); keep them small:
    clip     "nose": the tip is one cell shorter (the nose gun steps back a
             cell, the front two plates go); "port" | "stbd": that barb's
             outer aft corner is cut, one cell or ("port", n) n cells.
             Engines are never touched, but on small and xs seeds more than
             one cell opens their engine room to space (finish() says so);
    swap     {(lx, ly): ch}: change single local cells (' ' deletes), e.g. a
             differently laid-out cabin. Never move an engine or a gun;
    patch    (material, region) or a list of them: salvaged plate in another
             material ("Titanium", "Composite", "AbyssalAlloy"). region is
             "nose", "port", "stbd", "port_barb", "stbd_barb", "port_arm",
             "stbd_arm", "keel" or an iterable of local cells. Deck always
             renders as deck, so patches show on the rim and module cells.

  The Seed it returns: .w(lx, ly) local -> world; .tip, .notch, .helm (world
  cells); .barb("port"|"stbd") the world cells of that barb's aft face;
  .shoulder("port"|"stbd") the arm's outer front corner; .engines (world
  cells of engines kept); .cells {world: char} as painted; .spec.

  outline(size, at=(0, 0), rot=0, flip=False, short=None) is the set of
  world hull cells a seed would cover, without painting: use it to plan.
  engine_rows(...) the world rows the engines fire along: keep every cell
  aft of an engine in its row empty, or finish() reports it.

  weld(c, x0, x1, y0, y1, deck=False) fills empty cells of a rectangle with
  hull (never over a module or another copy) and, with deck=True, lays deck
  along its long middle line so crew can cross. Welds should be short: RK
  copies touch, they don't hang off necks.

  crate(c, tip, band, half, fill=None, rot=0) paints an uncrewed CRATE: the
  seed's chevron without barbs or crew (band cells per row, rows -half..half,
  tip at world `tip`), its inside filled with cargo by default (fill(lx, ly)
  -> char for each inside cell, local to the crate's tip). No one walks in a
  crate, so it may be thinner than 3 cells; put only non-post modules in it
  (cargo k U C, memory M, battery b, fuel f, ammo a, PD drone p). Returns the
  set of world cells painted. Its notch apex is at local x = -(band - 1).

  finish(c) drops plates a weld has left outside a corner of hull, builds,
  applies the material patches, and adds the checks draw.py doesn't make:
  the hull is one piece, no engine fires into its own hull, and the ship
  keeps the SYMMETRY RULE (engines and weapons mirrored port/starboard, at
  most 10% of the hull without a mirror image).

Legend (draw.py): # hull  . deck  / plate  E engine  R reactor  b battery
f fuel  c cooling  B bridge  H helm  M memory core  s shield  A radar
T targeting  Q quarters  K barracks  m medbay  O oxygen  a ammo  y repair
g gatling  * cluster rocket  x heavy missile  n cannon  p PD drone
X salvage arm  t tractor beam  & mining drill  k small cargo  U large cargo
C bulk cargo hold  d door
"""
from collections import deque

from draw import Canvas, DrawError

N4 = ((0, 1), (0, -1), (1, 0), (-1, 0))
MATERIALS = ("Steel", "Titanium", "Composite", "AbyssalAlloy")


class Spec:
    """Geometry of one seed size. Local (0, 0) is the tip."""

    def __init__(self, size, band, half, barb, ext):
        self.size, self.band, self.half, self.barb, self.ext = size, band, half, barb, ext
        self.nrow = half - barb                    # last diagonal row
        self.notch_x = -(band - 1)                 # notch apex, on the centreline
        self.stern_x = self.notch_x - self.nrow - ext
        self.short_x = self.notch_x - self.nrow    # a short arm's aft face

    def span(self, y, short=False):
        """(lo, hi) local x range of row y (either sign)."""
        y = abs(y)
        if y > self.half:
            return None
        if y <= self.nrow:
            return self.notch_x - y, -y
        return (self.short_x if short else self.stern_x), -y

    @property
    def length(self):
        return 1 - self.stern_x

    def __repr__(self):
        return (f"Spec({self.size}: band {self.band}, half {self.half}, barb rows {self.barb}, "
                f"notch_x {self.notch_x}, stern_x {self.stern_x})")


SPECS = {
    "xs": Spec("xs", 6, 5, 3, 2),
    "small": Spec("small", 7, 6, 3, 2),
    "medium": Spec("medium", 9, 9, 3, 2),
    "large": Spec("large", 11, 11, 4, 3),
}
SIZES = tuple(SPECS)

# --------------------------------------------------------------------------- the drawings
# Upper half and the centre row of each size, drawn by hand. Column 0 is the
# stern (x = stern_x), the top line is y = half, the last line is y = 0; the
# lower half is the mirror image. The corridor climbs each arm as a two-cell
# staircase from the keel to the barb; rooms sit either side of it.

ART = {
    "xs": """
##*##/
E..Q.#
ERM...g
  #b...#/
   #O...#/
    #HM..g
""",
    "small": """
##*###/
E....A#
ERfc...g
  #MQ...#
   #bO..a#/
    #s...T#/
     #MH...g
""",
    "medium": """
##*##*##/
E.....mA#
ERfca....g
  #MQ...T.#
   #bO...ms#
    #cRT....g
     #MQ...as#/
      #bO....A#/
       #R.BB..T#/
        #.BBH...g
""",
    "large": """
##*##x##*#/
E..ff.c.mA#
E..........#
ER#MQRb...a.n
   #bOKK...T.#
    #c.s...a.s#
     #MQ.....m.g
      #bOKK...A.#
       #c.y...Ts.#/
        #MQ......a#/
         #bO.BB....#/
          #R.BB.H...n
""",
}

# COPY ERROR: a short arm. The barb rows only, upper half, column 0 is the
# short arm's aft face (x = short_x). The rest of the arm is unchanged.
SHORT = {
    "xs": """
#*#/
E.Q#
EM..g
""",
    "small": """
##*#/
E..A#
ER...g
""",
    "medium": """
#*##*#/
E....A#
ERc....g
""",
    "large": """
#*##x##/
E.f..mA#
E.......#
EMQ....a.g
""",
}

HELM = {"xs": (-4, 0), "small": (-4, 0), "medium": (-4, 0), "large": (-4, 0)}


def _parse(block, x0, ytop):
    out = {}
    for k, line in enumerate(block.strip("\n").split("\n")):
        for i, ch in enumerate(line):
            if ch != " ":
                out[(x0 + i, ytop - k)] = ch
    return out


def _half_art(size, short=False):
    """{(x, y): ch} for y >= 0 of one arm (y > 0) plus the centre row."""
    sp = SPECS[size]
    cells = _parse(ART[size], sp.stern_x, sp.half)
    if short:
        for (x, y) in [p for p in cells if p[1] > sp.nrow]:
            del cells[(x, y)]
        cells.update(_parse(SHORT[size], sp.short_x, sp.half))
    return cells


def _validate():
    """Every drawing must match its outline exactly (catches a slipped column)."""
    for size, sp in SPECS.items():
        for short in (False, True):
            art = _half_art(size, short)
            have = {p for p, ch in art.items() if ch != "/"}
            want = {(x, y) for y in range(0, sp.half + 1)
                    for x in range(sp.span(y, short)[0], sp.span(y, short)[1] + 1)}
            if have != want:
                raise DrawError(f"rk_seed {size} short={short}: drawing doesn't match its outline: "
                                f"missing {sorted(want - have)[:6]} extra {sorted(have - want)[:6]}")
        if _half_art(size).get(HELM[size]) != "H":
            raise DrawError(f"rk_seed {size}: no helm at {HELM[size]}")


_validate()


# --------------------------------------------------------------------------- placing
def _xf(p, rot, flip):
    x, y = p
    if flip:
        y = -y
    for _ in range(rot % 4):
        x, y = -y, x
    return x, y


def _local_cells(size, short=None):
    """The whole seed, both arms, in local coordinates."""
    sp = SPECS[size]
    sides = {"port": short in ("port", "both"), "stbd": short in ("stbd", "both")}
    out = {}
    for side, sign in (("port", 1), ("stbd", -1)):
        for (x, y), ch in _half_art(size, sides[side]).items():
            if y == 0 and side == "stbd":
                continue
            out[(x, sign * y)] = ch
    return out


def _region(size, name):
    sp = SPECS[size]
    cells = _local_cells(size)
    sign = {"port": 1, "stbd": -1}
    test = {
        "nose": lambda x, y: x >= -max(2, sp.band // 2) + abs(y) // 2 and abs(y) <= sp.band // 2,
        "keel": lambda x, y: abs(y) <= 1,
        "port": lambda x, y: y > 0,
        "stbd": lambda x, y: y < 0,
        "port_barb": lambda x, y: y > sp.nrow,
        "stbd_barb": lambda x, y: y < -sp.nrow,
        "port_arm": lambda x, y: 2 <= y <= sp.nrow,
        "stbd_arm": lambda x, y: 2 <= -y <= sp.nrow,
    }
    if name not in test:
        raise DrawError(f"unknown patch region {name!r}; use {sorted(test)} or a list of local cells")
    return {p for p in cells if cells[p] != "/" and test[name](*p)}


class Seed:
    def __init__(self, size, at, rot, flip, short=None):
        self.size, self.at, self.rot, self.flip, self.short = size, at, rot % 4, flip, short
        self.spec = SPECS[size]
        self.cells = {}
        self.engines = []

    def w(self, x, y):
        dx, dy = _xf((x, y), self.rot, self.flip)
        return self.at[0] + dx, self.at[1] + dy

    @property
    def tip(self):
        return self.w(0, 0)

    @property
    def notch(self):
        return self.w(self.spec.notch_x, 0)

    @property
    def helm(self):
        return self.w(*HELM[self.size])

    def barb(self, side):
        s = 1 if side == "port" else -1
        sp = self.spec
        short = self.short in (side, "both")
        return [self.w(sp.span(y, short)[0], s * y) for y in range(sp.nrow + 1, sp.half + 1)]

    def shoulder(self, side):
        s = 1 if side == "port" else -1
        return self.w(-self.spec.half, s * self.spec.half)


def outline(size, at=(0, 0), rot=0, flip=False, short=None):
    """World hull cells (no plates) a seed would cover. Paints nothing."""
    s = Seed(size, at, rot, flip, short)
    return {s.w(*p) for p, ch in _local_cells(size, short).items() if ch != "/"}


def engine_rows(size, at=(0, 0), short=None):
    """World y of every engine on an east-facing seed: keep these rows clear aft of it."""
    s = Seed(size, at, 0, False)
    return sorted({s.w(*p)[1] for p, ch in _local_cells(size, short).items() if ch == "E"})


def canvas(name, material="Steel"):
    """A Canvas for an RK ship, with the patches table finish() reads."""
    c = Canvas(name, material)
    c.patches = {}
    return c


def seed(c, at=(0, 0), size="small", rot=0, flip=False, nose=True, engines=None,
         short=None, swap=None, patch=None, clip=None):
    """Paint one seed with its tip at world `at`. See the module docstring."""
    if size not in SPECS:
        raise DrawError(f"unknown seed size {size!r}; use one of {SIZES}")
    if short not in (None, "port", "stbd", "both"):
        raise DrawError(f"short must be 'port', 'stbd' or 'both', not {short!r}")
    s = Seed(size, at, rot, flip, short)
    local = _local_cells(size, short)
    if not nose:
        local[(0, 0)] = "."
        for p, ch in list(local.items()):
            if ch == "/" and p[0] >= -3 and abs(p[1]) <= 3:
                del local[p]
    keep = (s.rot == 0) if engines is None else engines
    for p, ch in local.items():
        if ch == "E" and not keep:
            ch = "#"
        q = s.w(*p)
        if q in c.cells and c.cells[q] != "/" and ch != "/":
            raise DrawError(f"seed {size} at {at}: overlaps the hull already drawn at {q}")
        if ch == "/" and q in c.cells:
            continue
        c.cells[q] = ch
        s.cells[q] = ch
        if ch == "E":
            s.engines.append(q)
    flaw(c, s, clip=clip, swap=swap, patch=patch)
    return s


def flaw(c, s, clip=None, swap=None, patch=None):
    """Add copy errors to a painted seed (after c.mirror()). See the module docstring."""
    sp = s.spec
    if clip:
        where, n = (clip, 1) if isinstance(clip, str) else clip
        if where == "nose":
            gun = c.cells.get(s.tip)
            for p in ((0, 0), (0, 1), (0, -1)):
                c.cells.pop(s.w(*p), None)
            if gun and gun != "/":
                c.cells[s.w(-1, 0)] = gun
        elif where in ("port", "stbd"):
            sign = 1 if where == "port" else -1
            lo = sp.span(sp.half, s.short in (where, "both"))[0]
            for k in range(n):
                q = s.w(lo + k, sign * sp.half)
                if c.cells.get(q, "#") != "#":
                    raise DrawError(f"clip {where} on the seed at {s.at} would cut '{c.cells[q]}' at {q}")
                c.cells.pop(q, None)
        else:
            raise DrawError(f"clip must be 'nose', 'port' or 'stbd' (or (side, cells)), not {clip!r}")
    for p, ch in (swap or {}).items():
        q = s.w(*p)
        if ch == " ":
            c.cells.pop(q, None)
        else:
            c.cells[q] = ch
        s.cells[q] = ch
    if patch:
        if not hasattr(c, "patches"):
            c.patches = {}
        for mat, region in ([patch] if isinstance(patch[0], str) else patch):
            if mat not in MATERIALS:
                raise DrawError(f"unknown material {mat!r}")
            cells = _region(s.size, region) if isinstance(region, str) else set(region)
            for p in cells:
                c.patches[s.w(*p)] = mat
    return s


def weld(c, x0, x1, y0, y1, deck=False):
    """Fill the empty cells of a rectangle with hull; deck=True also lays deck
    along its long middle line (only on cells that end up with hull on both
    sides). Never overwrites a module or another copy's hull."""
    xa, xb, ya, yb = min(x0, x1), max(x0, x1), min(y0, y1), max(y0, y1)
    for x in range(xa, xb + 1):
        for y in range(ya, yb + 1):
            if c.cells.get((x, y), "/") == "/":
                c.cells[(x, y)] = "#"
    if deck:
        if xb - xa >= yb - ya:
            y = (ya + yb) // 2
            line = [(x, y) for x in range(xa, xb + 1)]
        else:
            x = (xa + xb) // 2
            line = [(x, y) for y in range(ya, yb + 1)]
        for p in line:
            if c.cells.get(p) == "#":
                c.cells[p] = "."


def _cargo(lx, ly):
    """Default crate fill: large cargo (2x1) and small cargo in alternate rows."""
    return "U" if abs(ly) % 2 else "k"


def crate(c, tip, band, half, fill=None, rot=0):
    """Paint an uncrewed chevron crate. See the module docstring."""
    fill = fill or _cargo
    hull = {(x, y) for y in range(-half, half + 1) for x in range(-(band - 1) - abs(y), -abs(y) + 1)}
    out = set()
    for (x, y) in hull:
        inside = all((x + dx, y + dy) in hull for dx, dy in N4)
        ch = fill(x, y) if inside else "#"
        dx, dy = _xf((x, y), rot, False)
        q = (tip[0] + dx, tip[1] + dy)
        if q in c.cells and c.cells[q] != "/":
            raise DrawError(f"crate at {tip}: overlaps the hull already drawn at {q}")
        c.cells[q] = ch
        out.add(q)
    return out


def _prune_plates(c):
    hull = {p for p, ch in c.cells.items() if ch != "/"}
    for (x, y), ch in list(c.cells.items()):
        if ch != "/":
            continue
        n, s, w, e = ((x, y + 1) in hull, (x, y - 1) in hull, (x - 1, y) in hull, (x + 1, y) in hull)
        if (n + s + w + e) != 2 or (n and s) or (w and e):
            del c.cells[(x, y)]


WEAPONS = set("ngrxzpiejP*^")     # draw.py letters of every gun and launcher
LOPSIDED = 0.10        # at most this share of the hull may lack a mirror image


def symmetry(c):
    """Problems with the SYMMETRY RULE: engines must be mirrored port/starboard
    exactly, weapons within one cell fore and aft (a shorter tip may step the
    nose gun back), and at most LOPSIDED of the hull may lack a mirror image."""
    out = []
    hull = {p for p, ch in c.cells.items() if ch != "/"}
    lop = [p for p in hull if (p[0], -p[1]) not in hull]
    if len(lop) > LOPSIDED * len(hull):
        out.append(f"lopsided: {len(lop)} of {len(hull)} hull cells have no port/starboard mirror "
                   f"image (copy errors must stay under {LOPSIDED:.0%})")
    for (x, y), ch in sorted(c.cells.items()):
        if ch in "EN" and c.cells.get((x, -y)) != ch:
            out.append(f"engine at {(x, y)} has no mirror image at {(x, -y)}")
        elif ch in WEAPONS and not any(c.cells.get((x + k, -y)) == ch for k in (0, -1, 1)):
            out.append(f"weapon '{ch}' at {(x, y)} has no mirror image near {(x, -y)}")
    return out


def finish(c):
    """Build, apply material patches, and check the hull is one piece, no
    engine fires into its own hull, and the SYMMETRY RULE holds."""
    _prune_plates(c)
    d = c.build()
    d["_checks"] += symmetry(c)
    hull = {tuple(h["grid_pos"]) for h in d["hull_cells"]}
    for h in d["hull_cells"]:
        mat = getattr(c, "patches", {}).get(tuple(h["grid_pos"]))
        if mat:
            h["material"] = mat
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
        if ch in "EN" and any((xx, y) in hull for xx in range(x - 80, x)):
            d["_checks"].append(f"engine at {(x, y)} vents into hull")
    return d
