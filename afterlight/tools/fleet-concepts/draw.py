"""Hand-drawn ships: paint a grid of characters, one per cell, and turn it into a
Blueprint. Nothing is placed by a program; every room is where it was drawn.

Draw with rectangles and strings on a Canvas, x to the right (bow), y up:
    c.R(x0, x1, y0, y1)        fill with hull ('#')
    c.R(..., ch='.')           fill with deck
    c.K(x0, x1, y0, y1)        cut back to space
    c.P(x, y, "Rb.K")          write a run of cells starting at (x, y); spaces skip
    c.mirror()                 copy y > 0 onto y < 0 (port/starboard symmetry);
                               anything drawn afterwards is one side only
    c.rot4()                   copy the x>0,y>=0 quadrant round four ways (stations)

Multi-cell modules are drawn as a block of their letter the size of their
footprint (a 3x2 bridge is six B's, either way round); a bigger block of the
same letter is tiled with as many as fit. Guns and launchers face the open
side they sit on; engines vent aft.
"""
from collections import deque

import fleet
import gen
from fleet import footprint

LEGEND = {
    "E": "StandardEngine", "N": "LargeEngine",
    "R": "StandardReactor", "L": "LargeReactor", "F": "FusionReactor", "b": "BatteryBank",
    "f": "FuelTank", "c": "CoolingPump", "v": "HeatVent",
    "B": "BridgeWing", "H": "HelmStation", "M": "MemoryCore", "s": "ShieldEmitter",
    "A": "RadarArray", "W": "AdvancedRadar", "T": "TargetingComputer",
    "Q": "BasicQuarters", "K": "Barracks", "G": "GalleyMess", "S": "SurgicalBay", "m": "MedBay",
    "O": "OxygenScrubber", "a": "AmmoBay", "C": "BulkCargoHold", "k": "SmallCargo", "U": "LargeCargo",
    "n": "Cannon", "g": "Gatling", "r": "Railgun", "x": "HeavyMissile", "z": "Laser",
    "p": "PointDefenseDrone", "i": "IonDisruptor", "e": "EMPPulse", "j": "Coilgun",
    "d": "EmergencyBulkhead", "D": "DroneBay", "y": "RepairBay", "l": "Floodlight",
    "o": "DockingPort", "h": "DockingHub", "t": "TractorBeam", "X": "SalvageArm",
    "u": "AirlockChamber", "q": "OfficerQuarters", "w": "WellnessHub", "V": "CryoStorage",
    "P": "PlasmaCaster", "Z": "CreatureContainment", "Y": "SpecimenVault", "J": "SignalBuoy",
    "I": "ThermalImager", "*": "ClusterRocket", "^": "GuidedMissile", "&": "MiningDrill",
    "~": "StealthCoating", "?": "PassiveRadar", "!": "ResearchLab", "$": "ReinforcedVault",
    "%": "AdvancedRepairBay", "@": "AICombatCore", "+": "EmergencyForceField", "=": "SignalJammer",
}
AIMED = {"Cannon", "Gatling", "Railgun", "HeavyMissile", "Laser", "PointDefenseDrone", "IonDisruptor",
         "EMPPulse", "Coilgun", "Floodlight", "TractorBeam", "SalvageArm", "DockingPort",
         "PlasmaCaster", "ClusterRocket", "GuidedMissile", "MiningDrill"}
ENGINES = {"StandardEngine", "LargeEngine"}
DIRS = {"North": (0, 1), "South": (0, -1), "East": (1, 0), "West": (-1, 0)}
N4 = ((0, 1), (0, -1), (-1, 0), (1, 0))


class DrawError(Exception):
    pass


class Canvas:
    def __init__(self, name, material="Titanium"):
        self.name = name
        self.material = material
        self.cells = {}

    def R(self, x0, x1, y0, y1, ch="#"):
        for x in range(min(x0, x1), max(x0, x1) + 1):
            for y in range(min(y0, y1), max(y0, y1) + 1):
                self.cells[(x, y)] = ch
        return self

    def K(self, x0, x1, y0, y1):
        for x in range(min(x0, x1), max(x0, x1) + 1):
            for y in range(min(y0, y1), max(y0, y1) + 1):
                self.cells.pop((x, y), None)
        return self

    def P(self, x, y, s):
        for i, ch in enumerate(s):
            if ch != " ":
                self.cells[(x + i, y)] = ch
        return self

    def rows(self, x, y, block):
        """Paint a multi-line block, top line at row y, going down."""
        for k, line in enumerate(block.strip("\n").split("\n")):
            self.P(x, y - k, line)
        return self

    def band(self, k, s):
        """Paint square ring k of the first quadrant, walking (1,k)..(k,k) then
        (k,k-1)..(k,0). rot4() turns it into the whole ring."""
        path = [(x, k) for x in range(1, k + 1)] + [(k, y) for y in range(k - 1, -1, -1)]
        if len(s) != len(path):
            raise DrawError(f"band {k} wants {len(path)} cells, got {len(s)}")
        for c, ch in zip(path, s):
            if ch != " ":
                self.cells[c] = ch
        return self

    def mirror(self):
        for (x, y), ch in list(self.cells.items()):
            if y > 0:
                self.cells[(x, -y)] = ch
        return self

    def rot4(self):
        for (x, y), ch in list(self.cells.items()):
            if x > 0 and y >= 0:
                self.cells[(-y, x)] = ch
                self.cells[(-x, -y)] = ch
                self.cells[(y, -x)] = ch
        return self

    # ------------------------------------------------------------------ build
    def build(self):
        hull = {c for c, ch in self.cells.items() if ch != "/"}
        interior = lambda c: c in hull and all((c[0] + dx, c[1] + dy) in hull for dx, dy in N4)
        xs = [c[0] for c in hull]
        ys = [c[1] for c in hull]
        cx, cy = (min(xs) + max(xs)) / 2, (min(ys) + max(ys)) / 2
        hw, hh = max(1, (max(xs) - min(xs)) / 2), max(1, (max(ys) - min(ys)) / 2)
        mods, doors, errors = [], set(), []

        # Plates: an empty cell in an L of hull; rotation from which sides hold it.
        for c, ch in self.cells.items():
            if ch != "/":
                continue
            x, y = c
            n, s, w, e = ((x, y + 1) in hull, (x, y - 1) in hull, (x - 1, y) in hull, (x + 1, y) in hull)
            rot = ("North" if w and s and not n and not e else "East" if w and n and not s and not e
                   else "South" if e and n and not w and not s else "West" if e and s and not w and not n
                   else None)
            if rot is None:
                errors.append(f"plate at {c} isn't in a corner of hull")
                continue
            mods.append(("AngledArmorPlate", c, rot))

        # Modules.
        by_letter = {}
        for c, ch in self.cells.items():
            if ch in LEGEND:
                by_letter.setdefault(ch, set()).add(c)
            elif ch not in "#./":   # every other character is in LEGEND
                errors.append(f"unknown cell '{ch}' at {c}")
        for ch, cells in by_letter.items():
            kind = LEGEND[ch]
            w, h = fleet.SIZES.get(kind, (1, 1))
            for comp in _components(cells):
                if (w, h) == (1, 1):
                    for c in sorted(comp):
                        mods.append((kind, c, self._facing(kind, c, hull, (cx, cy, hw, hh))))
                    continue
                left = set(comp)
                for c in sorted(comp, key=lambda c: (-c[1], c[0])):
                    if c not in left:
                        continue
                    for rot in ("North", "East"):
                        anchor = c if rot == "North" else c
                        fp = footprint(kind, (c[0], c[1] - (h - 1)) if rot == "North" else c, rot)
                        if all(f in left for f in fp):
                            a = (c[0], c[1] - (h - 1)) if rot == "North" else c
                            facing = "East" if kind in ENGINES else rot
                            if kind in ENGINES:
                                # engines keep their footprint, so the long side must run along y
                                facing = rot if rot == "East" else "North"
                            mods.append((kind, a, facing))
                            left -= set(fp)
                            break
                if left:
                    errors.append(f"{kind} block {sorted(comp)[0]}: {len(left)} cells don't make a whole {w}x{h}")
            if ch == "d":
                doors |= cells

        layers = {}
        for c in hull:
            ch = self.cells[c]
            if not interior(c):
                layers[c] = "Outer"          # a deck drawn on the rim is just rim
            elif ch in "#." or ch == "d":
                layers[c] = "Hallway"
            else:
                layers[c] = "Inner"
        design = {
            "name": self.name,
            "hull_cells": [{"grid_pos": list(c), "layer": layers[c], "material": self.material}
                           for c in sorted(hull)],
            "modules": [{"module_type": k, "grid_pos": list(a), "rotation": r} for k, a, r in mods],
            "created_at": "fleet-concepts (hand-drawn)",
            "version": 2,
        }
        design["_checks"] = errors + gen.check(design)
        return design

    def _facing(self, kind, c, hull, frame):
        if kind in ENGINES:
            return "East"           # same as the Pincer: vents aft
        if kind not in AIMED:
            return "North"
        cx, cy, hw, hh = frame
        ox, oy = (c[0] - cx) / hw, (c[1] - cy) / hh
        best, score = None, -9
        for rot, (dx, dy) in DIRS.items():
            n = (c[0] + dx, c[1] + dy)
            if n in hull or self.cells.get(n) == "/":
                continue
            s = dx * ox + dy * oy + (0.15 if rot == "East" else 0)
            if s > score:
                best, score = rot, s
        return best or "East"


def _components(cells):
    left, out = set(cells), []
    while left:
        start = left.pop()
        comp, q = {start}, deque([start])
        while q:
            x, y = q.popleft()
            for dx, dy in N4:
                n = (x + dx, y + dy)
                if n in left:
                    left.discard(n)
                    comp.add(n)
                    q.append(n)
        out.append(comp)
    return out
