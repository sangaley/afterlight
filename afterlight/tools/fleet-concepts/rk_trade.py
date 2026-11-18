"""Recursive Kingdom, the traders: haulers built from the one seed.

Grammar as in rk_light.py (FACTION is imported from there): every hull is
copies of the seed, and the cargo rides in CRATES (rk_seed.crate()), the
seed's chevron without barbs or crew, copies that never woke up, used as
holds. No one walks in a crate; only the seed that tows or carries them is
crewed, so its bridge is the ship's bridge.

  - Ore Chain: a large seed tug towing a chain of crates that GROWS toward
    the stern, each crate a size up from the one in front of it (the Scrap
    Barge's chain shrinks into its notch; this one grows out of it);
  - Hoard Ark: the treasure ship. Holds nested round a crewed core like the
    rings of a tree: three chevrons wrapped round its bow, each bigger than
    the one inside it, and a tail of smaller crates in its notch.

Traders are lightly armed: gatlings (g) and point defence (p), plus tow
beams (t) on the tug. Steel, with salvaged Titanium and Composite on the
cargo.

Legend: see rk_seed.py (and draw.py); $ is a reinforced vault.
"""
import rk_seed as rk
from rk_light import FACTION  # noqa: F401  (the same faction, on the same page)


def hold(c, tip, band, half, rows):
    """Paint a crate (rk_seed.crate) with its cargo drawn by hand. rows[0]
    is the inside of the keel row, read from the front (just behind the
    leading rim) to the back; rows[1:] repeat for every row further out, so
    one pair of rows draws a whole arm. A bulk hold C spans a row pair
    (1, 2), (3, 4)...; on the last row, with no partner left, it becomes
    small cargo. Mirror-symmetric; returns the world cells painted."""
    def fill(lx, ly):
        y = abs(ly)
        r = rows[0] if y == 0 else rows[1 + (y - 1) % (len(rows) - 1)]
        i = -y - 1 - lx
        ch = r[i] if i < len(r) else "k"
        if ch == "C" and y % 2 == 1 and y + 1 >= half:
            ch = "k"
        return ch
    return rk.crate(c, tip, band, half, fill=fill)


def arm(cells, side, beyond=0):
    """The cells of a crate's port (+y) or starboard (-y) arm past row `beyond`."""
    s = 1 if side == "port" else -1
    return {p for p in cells if p[1] * s > beyond}


def couple(c, x, y=0, n=2):
    """A short coupling on the keel: n cells of hull from (x, y) aft. Where
    a notch closes round a coupling (hull on all four sides) it would read
    as a scrap of deck going nowhere; seal() fills those cells with cargo."""
    cells = [(x - k, y) for k in range(n)]
    for p in cells:
        c.cells[p] = "#"
    return cells


def seal(c, cells):
    """Fill the plain hull cells among `cells` (couplings, notch apexes) that
    ended up inside the hull with small cargo."""
    for (x, y) in cells:
        if c.cells.get((x, y)) in ("#", ".") and all(
                c.cells.get((x + dx, y + dy), "/") != "/"
                for dx, dy in ((0, 1), (0, -1), (1, 0), (-1, 0))):
            c.cells[(x, y)] = "k"


def both(swap):
    """The same swap on both arms of a seed, so the skeleton stays symmetric."""
    return {**swap, **{(x, -y): ch for (x, y), ch in swap.items()}}


# Crate insides by band. Bulk holds (C, 2x2) sit on paired rows, staggered
# one cell per row so they follow the 45 degree sweep of the arm.
BULK = {
    4: ["kk", "UU", "kk"],
    5: ["kkk", "kCC", "CCk"],
    6: ["UUUU", "kCCk", "CCkk"],
    7: ["UUkUU", "kCCkk", "CCkUU"],
}

# The large seed's weapons swapped for a trader's: no cannons, no launchers.
TRADER = {(-18, 11): "#", (-15, 11): "#", (-12, 11): "#", (-8, 8): "p"}


def ore_chain():
    """Trader (medium hauler). A large seed tug towing four crates on short
    couplings, each hung from the notch of the one in front and each a
    size up from it (bands and spans 4, 5, 6, 7), so the chain grows toward
    the stern, as if every copy that built the next crate thought it could
    carry more. The first crate rides inside the tug's own notch; the rest
    trail behind its engines, never in their wash (no crate is wider than
    the notch). Gatlings on the tug's nose and arms, tow beams on its
    barbs, point defence on its shoulders and on the last crate's arm ends.
    Copy errors: one of the tug's cabins is a medbay, one crate's arm end is
    two cells short, and two crates wear salvaged plate (titanium,
    composite) on one arm."""
    c = rk.canvas("rk_ore_chain")
    tug = rk.seed(c, (0, 0), "large", swap=both({**TRADER, (0, 0): "g",
                                                 (-15, 11): "t"}))     # tow beams
    crates, tips, joins = [], [], [(-10, 0)]
    apex = -10                                 # the notch the next crate hangs from
    for size in (4, 5, 6, 7):                  # band and half span grow together
        joins += couple(c, apex - 1)
        tip = apex - 3
        crates.append(hold(c, (tip, 0), size, size, BULK[size]))
        tips.append(tip)
        apex = tip - (size - 1)
        joins.append((apex, 0))
    seal(c, joins)
    c.P(tip - size, size, "p")                 # point defence on the last crate's arm ends
    c.mirror()
    rk.flaw(c, tug, swap={(-13, -5): "m"})     # a cabin with a medbay for a bunk
    end = tips[2] - 5 - 6                      # crate 3 (6 x 6): its arm end's aft corner
    for x in (end, end + 1):                   # one arm end two cells short
        del c.cells[(x, -6)]
    c.patches.update({p: "Titanium" for p in arm(crates[1], "port", 1)})
    c.patches.update({p: "Composite" for p in arm(crates[3], "stbd", 3)})
    return rk.finish(c)


def hoard_ark():
    """Trader (large hauler), the kingdom's treasure ship. A large seed, the
    crewed core, nested in the notch of a cargo chevron, nested in a bigger
    one, nested in a bigger one again: three holds wrapped round its bow,
    each the next size up (vaults inside, bulk holds, then large cargo), and
    in its own notch a tail of three crates, each a size down. The hoard
    surrounds the crew front and back, and the bridge sits in the middle of
    it all. Gatlings and point defence on the core's barbs, more point
    defence on the outer hold's leading edges; the core's guns that the
    holds would bury became shield emitters. The vault ring is plated in
    salvaged titanium. Copy errors: one cabin is a medbay, and one arm of
    the outer hold ends two cells short and is patched in composite."""
    c = rk.canvas("rk_hoard_ark")
    core = rk.seed(c, (0, 0), "large", nose=False, swap=both({
        (-18, 11): "p", (-15, 11): "g", (-12, 11): "p",       # barb rims: guns outward
        (-8, 8): "#", (-5, 5): "s",              # buried behind the holds: a shield
    }))
    rings, joins = [], []
    front = 0                                  # the inner copy's leading edge on the keel
    for band, half, rows in ((4, 13, ["$$", "$$", "k$"]),          # vaults
                             (5, 15, BULK[5]),                   # bulk holds
                             (6, 17, ["UUUU", "UUkk", "kUUk"])):  # large cargo
        joins += couple(c, front + 2)
        front += 2 + band
        rings.append(hold(c, (front, 0), band, half, rows))
        joins.append((front - (band - 1), 0))
    end, top = front - (band - 1) - half, half  # the outer hold's arm end: aft corner, row
    c.P(front, 1, "/")                         # plate the bow and the outer shoulders
    c.P(front - half + 1, half, "/")
    for y in (6, 12):                          # point defence along the outer hold
        c.P(front - y, y, "p")
    tail = []
    apex = -10
    joins.append((apex, 0))
    for band, half, rows in ((5, 6, BULK[5]), (4, 4, BULK[4]), (3, 2, ["k", "k"])):
        joins += couple(c, apex - 1)
        tip = apex - 3
        tail.append(hold(c, (tip, 0), band, half, rows))
        apex = tip - (band - 1)
        joins.append((apex, 0))
    seal(c, joins)
    c.mirror()
    rk.flaw(c, core, swap={(-13, -5): "m"})    # a cabin with a medbay for a bunk
    for x in (end, end + 1):                   # the outer hold: one arm end short
        del c.cells[(x, -top)]
    c.patches.update({p: "Titanium" for p in rings[0]})
    c.patches.update({p: "Composite" for p in arm(rings[2], "stbd", 9) if p in c.cells})
    return rk.finish(c)


CONCEPTS = [
    ("ore_chain", ore_chain, "Ore Chain", "Trader (medium hauler)",
     "A big seed tug towing four ore crates, each hung in the notch of the one before and each a size bigger, so the chain grows toward the stern. Gatlings and tow beams on the tug, a few point-defence drones."),
    ("hoard_ark", hoard_ark, "Hoard Ark", "Trader (large hauler)",
     "The kingdom's treasure ship: a crewed seed nested inside three ever-bigger chevrons of holds, vaults innermost, with a tail of shrinking crates in its own notch, so the hoard wraps the crew front and back. Two gatlings and a ring of point defence."),
]
