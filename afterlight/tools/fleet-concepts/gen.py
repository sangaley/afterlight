"""Ship builder for fleet concepts: compose a hull from rectangles, place modules,
fill rooms, plate selectively, derive decking, and check the result against the
rules the player's ship follows.

A concept is a function that drives a `Ship` and returns `ship.build()`, which
is a Blueprint dict in the game's JSON shape (designs/*.json).
"""
import math
from collections import deque

import fleet
from fleet import footprint

MIRROR_ROT = {"North": "South", "South": "North", "East": "East", "West": "West"}
# A wedge's material sits in its lower-left at North; mirroring top<->bottom swaps
# which corner that is.
MIRROR_WEDGE = {"North": "East", "East": "North", "South": "West", "West": "South"}
FACING = {"North": (0, 1), "East": (1, 0), "South": (0, -1), "West": (-1, 0)}
N4 = ((0, 1), (0, -1), (-1, 0), (1, 0))


class ShipError(Exception):
    pass


class Ship:
    def __init__(self, name, material="Titanium"):
        self.name = name
        self.material = material
        self.hull = set()
        self.mods = []          # (kind, (x, y), rot)
        self.occ = {}           # cell -> module index
        self.corr = set()       # cells reserved as walkway
        self.doors = set()

    # ---------------------------------------------------------------- shape
    def rect(self, x0, x1, y0, y1, mirror=False):
        for x in range(min(x0, x1), max(x0, x1) + 1):
            for y in range(min(y0, y1), max(y0, y1) + 1):
                self.hull.add((x, y))
                if mirror:
                    self.hull.add((x, -y))
        return self

    def cut(self, x0, x1, y0, y1, mirror=False):
        for x in range(min(x0, x1), max(x0, x1) + 1):
            for y in range(min(y0, y1), max(y0, y1) + 1):
                self.hull.discard((x, y))
                if mirror:
                    self.hull.discard((x, -y))
        return self

    def trap(self, x0, x1, y0, y1, front=1, back=1, mirror=False):
        """Ziggurat block: rows y0..y1 (|y| growing outward), each row stepping
        in by `front` cells at the bow end and `back` at the stern end."""
        step = 1 if y1 >= y0 else -1
        for k, y in enumerate(range(y0, y1 + step, step)):
            self.rect(x0 + back * k, x1 - front * k, y, y, mirror)
        return self

    def casemate(self, x0, x1, y0, y1):
        """A gun deck mirrored top and bottom: a rectangle whose outer bow corner
        is cut back one cell so a plate can rake it. Returns the plate cells."""
        self.rect(x0, x1, y0, y1, mirror=True)
        self.cut(x1, x1, y1, y1, mirror=True)
        return {(x1, y1), (x1, -y1)}

    def hammer(self, x0, x1, H, ch=2):
        """Flat bow face |y| <= H, its outer corners chamfered back `ch` steps."""
        for y in range(-H, H + 1):
            xm = x1 - max(0, abs(y) - (H - ch))
            if xm >= x0:
                self.rect(x0, xm, y, y)
        return self

    def rows(self, table, mirror=False):
        """(y, x_min, x_max) rows, like layouts.rs."""
        for y, a, b in table:
            self.rect(a, b, y, y, mirror)
        return self

    def interior(self, c):
        return c in self.hull and all((c[0] + dx, c[1] + dy) in self.hull for dx, dy in N4)

    # ---------------------------------------------------------------- walkways
    def corridor(self, x0, x1, y0, y1, mirror=False):
        for x in range(min(x0, x1), max(x0, x1) + 1):
            for y in range(min(y0, y1), max(y0, y1) + 1):
                for c in ((x, y), (x, -y)) if mirror else ((x, y),):
                    if c not in self.hull:
                        raise ShipError(f"{self.name}: corridor off hull at {c}")
                    if c not in self.occ:
                        self.corr.add(c)
        return self

    def door(self, x, y, mirror=False):
        self.put("EmergencyBulkhead", x, y, "North", mirror, allow_corr=True)
        self.doors.add((x, y))
        if mirror:
            self.doors.add((x, -y))
        return self

    # ---------------------------------------------------------------- modules
    def _place(self, kind, at, rot, allow_corr=False, strict=True):
        cells = footprint(kind, at, rot)
        wedge = kind in fleet.WEDGES
        for c in cells:
            bad = None
            if c in self.occ:
                bad = "overlaps " + self.mods[self.occ[c]][0]
            elif wedge and c in self.hull:
                bad = "plate on a hull cell"
            elif not wedge and c not in self.hull:
                bad = "off hull"
            elif c in self.corr and not allow_corr:
                bad = "in a corridor"
            if bad:
                if strict:
                    raise ShipError(f"{self.name}: {kind} at {at} {rot}: {bad} at {c}")
                return False
        i = len(self.mods)
        self.mods.append((kind, at, rot))
        for c in cells:
            self.occ[c] = i
            if kind != "EmergencyBulkhead":
                self.corr.discard(c)
        return True

    def _mirror_anchor(self, kind, at, rot):
        target = {(x, -y) for x, y in footprint(kind, at, rot)}
        want = MIRROR_WEDGE[rot] if kind in fleet.WEDGES else MIRROR_ROT[rot]
        for r in [want] + [r for r in FACING if r != want]:
            for a in target:
                if set(footprint(kind, a, r)) == target:
                    return a, r
        raise ShipError(f"{self.name}: cannot mirror {kind} at {at}")

    def put(self, kind, x, y, rot="North", mirror=False, allow_corr=True):
        """Explicit placements win over reserved corridors (fill never does)."""
        self._place(kind, (x, y), rot, allow_corr)
        if mirror:
            here = set(footprint(kind, (x, y), rot))
            if {(a, -b) for a, b in here} != here:
                a, r = self._mirror_anchor(kind, (x, y), rot)
                self._place(kind, a, r, allow_corr)
        return self

    def line(self, kind, xs, y, rot="North", mirror=False):
        for x in xs:
            self.put(kind, x, y, rot, mirror)
        return self

    def col(self, kind, x, ys, rot="North", mirror=False):
        for y in ys:
            self.put(kind, x, y, rot, mirror)
        return self

    def _walk(self):
        """Cells crew can stand on right now: walkways, doors, and posts."""
        w = set(self.corr)
        for k, a, r in self.mods:
            if k in fleet.POSTS or k == "EmergencyBulkhead":
                w.update(footprint(k, a, r))
        return w

    def fill(self, region, palette, mirror=False):
        """Greedy room-filling: every free interior cell in `region` (a predicate
        on (x, y)) gets the first palette module that fits there. The palette
        cycles so neighbouring rooms differ. A crewed room (a post) only goes
        where it touches somewhere crew can already walk, so filling never
        strands a post. With mirror, the top half is filled and reflected, then
        the centreline gets modules that lie along it."""
        walk = self._walk()

        def free(f, reg=region):
            return self.interior(f) and f not in self.occ and f not in self.corr and reg(*f)

        def touches(cells):
            return any((x + dx, y + dy) in walk for x, y in cells for dx, dy in N4)

        def run(reg, mirrored):
            k = 0
            for c in sorted((c for c in self.hull if reg(*c)), key=lambda c: (-c[1], c[0])):
                if not free(c, reg):
                    continue
                placed = False
                for step in range(len(palette)):
                    kind = palette[(k + step) % len(palette)]
                    if kind == "_":                     # leave open deck here
                        if c not in self.corr and any(n in walk for n in
                                                       ((c[0] + dx, c[1] + dy) for dx, dy in N4)):
                            self.corr.add(c)
                            walk.add(c)
                            if mirrored and free((c[0], -c[1])):
                                self.corr.add((c[0], -c[1]))
                                walk.add((c[0], -c[1]))
                            k += 1
                            break
                        continue
                    for rot in ("North", "East"):
                        fp = footprint(kind, c, rot)
                        if not all(free(f, reg) for f in fp):
                            continue
                        mir = [(x, -y) for x, y in fp] if mirrored else []
                        if mirrored and not all(free(m) or m in fp for m in mir):
                            continue
                        if kind in fleet.POSTS and not (touches(fp) and (not mir or touches(mir))):
                            continue
                        if mirrored:
                            self.put(kind, c[0], c[1], rot, mirror=True, allow_corr=False)
                        else:
                            self._place(kind, c, rot)
                        if kind in fleet.POSTS:
                            walk.update(fp)
                            walk.update(mir)
                        placed = True
                        break
                    if placed:
                        k += 1
                        break

        if mirror:
            run(lambda x, y: region(x, y) and y > 0, True)
            run(lambda x, y: region(x, y) and y == 0, False)
        else:
            run(region, False)
        return self

    def connect(self):
        """Run a walkway from every crew post that can't reach the main deck to
        the nearest bit of it, through free interior cells. Call after the
        explicit placements and before fill(), which then builds rooms around
        the walkways instead of over them."""
        post = {}
        for i, (k, a, r) in enumerate(self.mods):
            if k in fleet.POSTS or k == "EmergencyBulkhead":
                for c in footprint(k, a, r):
                    post[c] = i

        def walkable(c):
            return c in self.corr or c in post

        def component(start):
            comp, q = {start}, deque([start])
            while q:
                x, y = q.popleft()
                for dx, dy in N4:
                    n = (x + dx, y + dy)
                    if n not in comp and walkable(n):
                        comp.add(n)
                        q.append(n)
            return comp

        main = set()
        for c in list(self.corr):
            if c not in main:
                comp = component(c)
                if len(comp) > len(main):
                    main = comp
        groups = [[c for c, j in post.items() if j == i] for i in range(len(self.mods))]
        seen = set(main)
        for c in sorted(self.corr):
            if c not in seen:
                comp = component(c)
                seen |= comp
                groups.append(sorted(comp))
        for cells in groups:
            if not cells or any(c in main for c in cells):
                continue
            prev = {c: None for c in cells}
            q = deque(cells)
            hit = None
            while q and hit is None:
                x, y = q.popleft()
                for dx, dy in N4:
                    n = (x + dx, y + dy)
                    if n in prev:
                        continue
                    if n in main:
                        prev[n] = (x, y)
                        hit = n
                        break
                    if walkable(n) or (self.interior(n) and n not in self.occ):
                        prev[n] = (x, y)
                        q.append(n)
            if hit is None:
                continue
            c = prev[hit]
            while c is not None:
                if c not in self.occ:
                    self.corr.add(c)
                c = prev[c]
            main |= component(hit)
        return self

    # ---------------------------------------------------------------- armour
    def corners(self):
        """Empty cells sitting in an L of hull: where a wedge makes a raked face."""
        out = []
        cand = {(x + dx, y + dy) for x, y in self.hull for dx, dy in N4} - self.hull
        for c in cand:
            x, y = c
            n, s, w, e = ((x, y + 1) in self.hull, (x, y - 1) in self.hull,
                          (x - 1, y) in self.hull, (x + 1, y) in self.hull)
            rot = None
            if w and s and not n and not e:
                rot = "North"
            elif w and n and not s and not e:
                rot = "East"
            elif e and n and not w and not s:
                rot = "South"
            elif e and s and not w and not n:
                rot = "West"
            if rot:
                out.append((c, rot))
        return sorted(out)

    def armour(self, bow_from=None, belts=(), tips=(), kind="AngledArmorPlate"):
        """Selective plating (user rule, 2026-10-15): bow-facing steps forward of
        `bow_from`, plus any step inside a flank `belt` x-range, plus explicit
        `tips`. Never the whole outline."""
        for c, rot in self.corners():
            if c in self.occ:
                continue
            x, y = c
            bow = bow_from is not None and x >= bow_from and rot in ("North", "East")
            belt = any(a <= x <= b for a, b in belts)
            if bow or belt or c in tips:
                self._place(kind, c, rot)
        return self

    # ---------------------------------------------------------------- output
    def build(self):
        layers = {}
        for c in self.hull:
            if not self.interior(c):
                layers[c] = "Outer"
            elif c in self.occ and c not in self.doors:
                layers[c] = "Inner"
            else:
                layers[c] = "Hallway"
        design = {
            "name": self.name,
            "hull_cells": [{"grid_pos": list(c), "layer": layers[c], "material": self.material}
                           for c in sorted(self.hull)],
            "modules": [{"module_type": k, "grid_pos": list(a), "rotation": r} for k, a, r in self.mods],
            "created_at": "fleet-concepts",
            "version": 2,
        }
        design["_checks"] = check(design)
        return design


# -------------------------------------------------------------------- checks
def check(design):
    hull = {tuple(c["grid_pos"]): c["layer"] for c in design["hull_cells"]}
    problems = []
    occ = {}
    for i, m in enumerate(design["modules"]):
        kind, at, rot = m["module_type"], tuple(m["grid_pos"]), m["rotation"]
        for c in footprint(kind, at, rot):
            if c in occ:
                problems.append(f"overlap {kind}/{design['modules'][occ[c]]['module_type']} at {c}")
            occ[c] = i
            if kind in fleet.WEDGES:
                if c in hull:
                    problems.append(f"plate on hull at {c}")
                elif not any((c[0] + dx, c[1] + dy) in hull for dx, dy in N4):
                    problems.append(f"plate not touching hull at {c}")
            elif c not in hull:
                problems.append(f"{kind} off hull at {c}")
    # Launchers need a clear lane out of the ship.
    for m in design["modules"]:
        if m["module_type"] in fleet.LAUNCHERS:
            dx, dy = FACING[m["rotation"]]
            x, y = m["grid_pos"]
            for k in range(1, 5):
                c = (x + dx * k, y + dy * k)
                if c in hull or c in occ:
                    problems.append(f"entombed launcher at {(x, y)} facing {m['rotation']}")
                    break
    # Memory cores: at least three, at least 3 cells apart.
    cores = [tuple(m["grid_pos"]) for m in design["modules"] if m["module_type"] == "MemoryCore"]
    if len(cores) < 3:
        problems.append(f"only {len(cores)} memory cores")
    for i in range(len(cores)):
        for j in range(i + 1, len(cores)):
            if math.dist(cores[i], cores[j]) < 3:
                problems.append(f"cores too close {cores[i]} {cores[j]}")
    # Every crew post reachable: walk hallways, doors and posts.
    post_cells = {}
    walk = {c for c, l in hull.items() if l == "Hallway"}
    for i, m in enumerate(design["modules"]):
        if m["module_type"] in fleet.POSTS or m["module_type"] == "EmergencyBulkhead":
            for c in footprint(m["module_type"], tuple(m["grid_pos"]), m["rotation"]):
                walk.add(c)
                post_cells[c] = i
    seen = set()
    best = set()
    for start in walk:
        if start in seen:
            continue
        comp = {start}
        q = deque([start])
        while q:
            x, y = q.popleft()
            for dx, dy in N4:
                n = (x + dx, y + dy)
                if n in walk and n not in comp:
                    comp.add(n)
                    q.append(n)
        seen |= comp
        if len(comp) > len(best):
            best = comp
    stranded = sorted({(design["modules"][i]["module_type"], tuple(design["modules"][i]["grid_pos"]))
                       for c, i in post_cells.items() if c not in best})
    if stranded:
        problems.append("posts cut off: " + ", ".join(f"{k}@{p[0]},{p[1]}" for k, p in stranded))
    return problems


def stats(design):
    mods = [m for m in design["modules"] if m["module_type"] not in fleet.WEDGES]
    plates = len(design["modules"]) - len(mods)
    xs = [c["grid_pos"][0] for c in design["hull_cells"]]
    ys = [c["grid_pos"][1] for c in design["hull_cells"]]
    count = lambda *ks: sum(1 for m in mods if m["module_type"] in ks)
    weapons = {}
    for m in mods:
        k = m["module_type"]
        if k in ("Cannon", "Gatling", "Railgun", "Coilgun", "Laser", "PlasmaCaster", "IonDisruptor",
                 "HeavyMissile", "GuidedMissile", "ClusterRocket", "EMPPulse", "PointDefenseDrone",
                 "TractorBeam"):
            weapons[k] = weapons.get(k, 0) + 1
    crew = sum({"BasicQuarters": 1, "Barracks": 4, "OfficerQuarters": 1}.get(m["module_type"], 0)
               for m in mods)
    return {
        "hull": len(design["hull_cells"]),
        "modules": len(mods),
        "plates": plates,
        "w": max(xs) - min(xs) + 1,
        "h": max(ys) - min(ys) + 1,
        "weapons": weapons,
        "engines": count("StandardEngine", "LargeEngine", "SmallEngine"),
        "reactors": count("StandardReactor", "LargeReactor", "FusionReactor", "SmallReactor"),
        "shields": count("ShieldEmitter"),
        "berths": crew,
        "cargo": count("BulkCargoHold", "LargeCargo", "SmallCargo"),
    }
