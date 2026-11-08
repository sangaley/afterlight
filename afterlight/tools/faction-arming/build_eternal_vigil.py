"""The Vigil: an Eternal Hegemony capital built around the big-ships weapons.
Writes designs/factions/eternal_hegemony/vigil.json and the Eternal gun
designs it fits (designs/guns/eternal_*.json).

Laid out block by block, in the Eternal grammar (tools/fleet-concepts/eternal.py):
- a stepped CROWN BOW: the central spire carries a 280 mm spinal (the Throne
  Lance), the crown points carry CIWS and launch pads, every forward step plated;
- two DOUBLE-TIER SPONSONS a side: a broad gallery with a narrower turret tier
  outboard, each tier a 3x3 ring with a 170 mm AP-HE turret gun (the Lance);
- ENGINE PODS at the stern corners round a recessed centre engine bank;
- a PROCESSIONAL NAVE down the centreline, lined with shield pillars, to the
  throne (the bridge) amidships; command is spread out -- a helm aft, the
  bridge and a helm amidships, combat cores forward -- so one hit can't take it;
- six 20 mm CIWS turrets on point defence (crown points, waist, stern pods),
  four Launch Pads, two Rail Launchers, four VLS Silos, a Decoy Launcher and
  a Chaff Dispenser; eight Missile Warners round the hull to see what's
  coming. Titanium, symmetrical.

Run from afterlight/: python3 tools/faction-arming/build_eternal_vigil.py
"""
import json, os
import kit

OUT = os.path.join(kit.ROOT, "designs/factions/eternal_hegemony/vigil.json")

# ---- silhouette (y >= 0; mirrored) ----------------------------------------
hull = set()
def H(x0, x1, y0, y1):
    for x in range(x0, x1 + 1):
        for y in range(y0, y1 + 1):
            hull.add((x, y))
def K(x0, x1, y0, y1):
    for x in range(x0, x1 + 1):
        for y in range(y0, y1 + 1):
            hull.discard((x, y))

H(2, 46, 0, 6)       # body
K(41, 46, 6, 6)      # forward taper
H(0, 8, 5, 8)        # stern engine pod
H(12, 23, 7, 8)      # aft sponson: gallery...
H(15, 20, 9, 10)     # ...and its turret tier
H(28, 37, 7, 8)      # fore sponson gallery
H(31, 35, 9, 10)     # and tier
H(24, 27, 7, 7)      # waist ledge (CIWS)
H(47, 56, 0, 1)      # crown bow: the spire
H(47, 50, 2, 2)
H(47, 53, 3, 4)      # crown points

corr = set()
corr |= {(x, 0) for x in range(6, 41)}            # the nave
for x in (9, 24, 39):
    corr |= {(x, y) for y in range(0, 7)}         # cross passages
corr |= {(x, 4) for x in range(9, 40)}            # side passage

for c in list(hull):
    hull.add((c[0], -c[1]))
for c in list(corr):
    corr.add((c[0], -c[1]))

# ---- blocks ------------------------------------------------------------------
mods = []   # (type, x, y, rot, extras)
def put(t, x, y, r="North", extras=None):
    mods.append((t, x, y, r, extras))
def mirror_of(t, x, y, r):
    want = sorted((cx, -cy) for cx, cy in kit.cells_of(t, (x, y), r))
    if y == 0 and kit.SIZES.get(t, (1, 1))[1] == 1 and r in ("East", "West"):
        return None
    flip = {"North": "South", "South": "North"}.get(r, r)
    w, h = kit.SIZES.get(t, (1, 1))
    # A square block (a turret ring, a silo) keeps its own facing, so its
    # origin cell stays the corner the game measures it from.
    order = [r, flip] if w == h else [flip, r]
    for rot in order + ["North", "East", "South", "West"]:
        for ox, oy in want:
            if sorted(kit.cells_of(t, (ox, oy), rot)) == want:
                return (ox, oy, rot)
    raise ValueError(f"can't mirror {t} at {(x, y)}")
def both(t, x, y, r="North", extras=None):
    put(t, x, y, r, extras)
    m = mirror_of(t, x, y, r)
    if m and (m[0], m[1]) != (x, y):
        put(t, m[0], m[1], m[2], extras)
def row(y, x0, letters):
    """A run of 1x1 blocks along row y from x0, mirrored; ' ' skips a cell."""
    for i, ch in enumerate(letters):
        if ch != " ":
            both(L[ch], x0 + i, y)

L = {
    "s": "ShieldEmitter", "m": "MemoryCore", "Q": "BasicQuarters", "q": "OfficerQuarters", "+": "MedBay",
    "O": "OxygenScrubber", "o": "CO2Scrubber", "i": "AirCirculator", "k": "OxygenTank", "F": "FireSuppression",
    "y": "RepairBay", "b": "BatteryBank", "c": "Capacitor", "p": "CoolingPump", "v": "HeatVent",
    "R": "StandardReactor", "f": "FuelTank", "t": "TargetingComputer", "A": "RadarArray", "l": "Floodlight",
    "a": "ArmorPlate", "Z": "Magazine", "M": "TraverseMotor", "h": "AmmoHoist", "e": "EngineeringStation",
    "T": "ManeuveringThruster", "S": "SurgicalBay", "W": "AdvancedRadar", "w": "MissileWarner",
}

# Stern: engines vent aft (East, as every Eternal hull has them).
for y in (5, 6, 7, 8):
    both("StandardEngine", 0, y, "East")
put("StandardEngine", 2, 0, "East")
for y in (1, 2, 3, 4):
    both("StandardEngine", 2, y, "East")
put("FusionReactor", 3, -1)
row(1, 6, "RR")
put("HelmStation", 8, 1); put("BatteryBank", 8, -1)        # helm aft
row(2, 3, "pvbeyc")
row(3, 3, "ffRpbf")
row(4, 3, "ffvyOc")
row(5, 1, "fffbbbT")
both("OxygenScrubber", 8, 5)
row(6, 1, "fffccp+i")
row(7, 1, "fffbb")
both("Magazine", 6, 7); both("AmmoHoist", 7, 7)
both("TurretRingSmall", 7, 8); both("TraverseMotor", 8, 8); both("TraverseMotor", 8, 7)   # stern CIWS
row(8, 1, "w T")   # a warner on each stern pod

# Aft block, x 10..23: the nave's pillars, then crew.
row(1, 10, "sQsQsQsQsQsQsm")
both("Barracks", 10, 2); both("Barracks", 12, 2)
row(2, 14, "++")
both("GalleyMess", 16, 2)
row(2, 18, "Oo")
both("Barracks", 20, 2)
row(2, 22, "yF")
row(3, 10, "QQQqOi  QQQkms")
both("VlsSilo", 10, 5)
row(5, 12, "bcst")
row(6, 12, "QQQF")
# Turret A: 3x3 ring on the tier, three motors, a hoist below into five magazines.
both("TurretRing", 16, 8)
row(8, 15, "M   M")
both("TraverseMotor", 15, 9)
row(7, 16, "ZhZ")
row(6, 16, "ZZZ")
row(5, 16, "Amp")
row(5, 19, "bQ+ts")
row(6, 19, "QQO v")
put("DecoyLauncher", 22, 6); put("ChaffDispenser", 22, -6)
row(7, 12, "QQOF")
row(7, 19, "tQQla")
row(8, 12, "wsA")
both("RailLauncher", 20, 8, "East")
row(8, 23, "a")
row(10, 19, "aa")
row(9, 20, "a")

# Amidships: the throne.
put("BridgeWing", 25, 1)
put("HelmStation", 25, -1); put("NavigationConsole", 26, -1); put("TargetingComputer", 27, -1)
put("AdvancedRadar", 25, -2); put("MemoryCore", 26, -2); put("BasicQuarters", 27, -2)
row(3, 25, "tAm")
row(1, 28, "sQsQsQsQsms")
both("Barracks", 28, 2)
row(2, 30, "++Oo")
both("Barracks", 34, 2)
row(2, 36, "ybR")
row(3, 28, "QQQqkiQQFpR")
# Waist CIWS.
both("TurretRingSmall", 25, 7); both("TraverseMotor", 24, 7); both("AmmoHoist", 26, 7); both("Magazine", 26, 6)
row(7, 27, "a")
row(5, 25, "bcs")
row(6, 25, "M Q")
both("VlsSilo", 28, 5)
row(5, 30, "Qb")
row(6, 30, "QF")
# Turret B.
both("TurretRing", 32, 8)
row(8, 31, "M")
both("TraverseMotor", 31, 9); both("TraverseMotor", 35, 9)
row(7, 32, "ZhZ")
row(6, 32, "ZZZ")
row(5, 32, "Amp")
row(5, 35, "b+ts")
row(6, 35, "QOvl")
row(7, 28, "QQOF")
row(7, 35, "tQa")
row(8, 28, "wsA")
both("RailLauncher", 35, 8, "East")
row(10, 35, "a")

# Forward: combat cores, then the spinal's magazines and breech.
row(1, 40, "bcZZ")
put("Magazine", 41, 0); put("Magazine", 42, 0)
both("AICombatCore", 40, 2)
row(2, 42, "tApvs")
put("HelmStation", 40, 3); put("NavigationConsole", 40, -3)
row(3, 41, "mQQOFa")
row(4, 40, "QQ+bcla")
row(5, 40, "QAsbQa")
row(6, 40, "l")
# Crown bow.
row(1, 49, "aaaaaaaa")
put("ArmorPlate", 56, 0)
row(2, 48, "sla")
row(3, 47, "tbZhMaa")
row(4, 47, "wsM")
both("TurretRingSmall", 50, 4)
both("LaunchPad", 52, 4, "East"); both("LaunchPad", 53, 4, "East")
both("ArmorPlate", 51, 4)

# ---- fitted guns -------------------------------------------------------------
LANCE = {
    "name": "Eternal 170 mm Lance",
    "description": "The old pattern's turret gun: a 3x3 deck, four metres of barrel, AP-HE that punches in and bursts.",
    "parts": [
        {"module_type": "ClipRack", "offset": [-2, 0], "rotation": "East"},
        {"module_type": "Loader", "offset": [-1, 0], "rotation": "East"},
        {"module_type": "BreechCooler", "offset": [-1, 1], "rotation": "East"},
        {"module_type": "BreechCooler", "offset": [-1, -1], "rotation": "East"},
        {"module_type": "GaugeIncreaser", "offset": [0, 1], "rotation": "East"},
        {"module_type": "FiringPiece", "offset": [0, 0], "rotation": "East"},
        {"module_type": "GaugeIncreaser", "offset": [0, -1], "rotation": "East"},
        {"module_type": "Mantlet", "offset": [1, 0], "rotation": "East"},
    ] + [{"module_type": "GunBarrel", "offset": [x, 0], "rotation": "East"} for x in (2, 3, 4, 5)],
    "shell": {"base": "Tracer", "casings": 3, "bodies": ["He"], "head": "Ap"},
    "faction": "EternalHegemony",
}
def part(t, x, y):
    return {"module_type": t, "offset": [x, y], "rotation": "East"}
THRONE_LANCE = {
    "name": "Eternal 280 mm Throne Lance",
    "description": "The spinal down the Vigil's spire: three two-metre loaders, six metres of barrel, solid AP that goes through a ship end to end. Aim it by turning the ship.",
    # Loaders two blocks long: a 1.54 m shell won't go in a one-metre loader.
    "parts": [part("ShellFeeder", -5, 0)]
    + [part(t, x, y) for y in (-1, 0, 1) for t, x in (("ClipRack", -4), ("Loader", -3), ("Loader", -2))]
    + [part("BreechCooler", -1, -2), part("GaugeIncreaser", -1, -1), part("RecoilAbsorber", -1, 0),
       part("GaugeIncreaser", -1, 1), part("BreechCooler", -1, 2), part("GaugeIncreaser", 0, -1),
       part("FiringPiece", 0, 0), part("GaugeIncreaser", 0, 1), part("Mantlet", 1, 0)]
    + [part("GunBarrel", x, 0) for x in range(2, 8)],
    "shell": {"base": "Tracer", "casings": 3, "bodies": ["Solid", "Solid"], "head": "Ap"},
    "faction": "EternalHegemony",
}
PD = dict(json.load(open(os.path.join(kit.ROOT, "designs/guns/20_mm_ciws.json"))))
PD.update({"name": "Eternal CIWS", "faction": "EternalHegemony"})

guns = []
def fit(design, at, turret=None):
    guns.append({"design": design, "at": list(at), "facing": "East", "turret": list(turret) if turret else None})
fit(THRONE_LANCE["name"], (48, 0))
for t, x, y, r, _ in mods:
    if t == "TurretRing":
        fit(LANCE["name"], (0, 0), (x, y))
    elif t == "TurretRingSmall":
        fit(PD["name"], (0, 0), (x, y))

# ---- checks --------------------------------------------------------------------
spinal = {(48 + dx, dy) for dx, dy in (tuple(p["offset"]) for p in THRONE_LANCE["parts"])}
owner = {}
for t, x, y, r, _ in mods:
    for c in kit.cells_of(t, (x, y), r):
        assert c in hull, f"{t} at {(x, y)} covers {c}, off the hull"
        assert c not in owner, f"{t} at {(x, y)} overlaps {owner[c]} at {c}"
        assert c not in corr, f"{t} at {(x, y)} blocks the corridor at {c}"
        assert c not in spinal, f"{t} at {(x, y)} is in the spinal's way at {c}"
        owner[c] = (t, x, y)
for c in spinal:
    assert c in hull and c not in corr, f"spinal part at {c} isn't on open deck"

# Plates on the forward steps: an empty cell with hull aft of it (west) and
# beside it toward the centreline, nothing ahead or outboard.
plates = []
for (x, y) in {(x + 1, y + dy) for (x, y) in hull for dy in (-1, 0, 1)}:
    if (x, y) in hull or y == 0:
        continue
    inward = (x, y - 1) if y > 0 else (x, y + 1)
    outward = (x, y + 1) if y > 0 else (x, y - 1)
    if (x - 1, y) in hull and inward in hull and (x + 1, y) not in hull and outward not in hull:
        plates.append(("AngledArmorPlate", x, y, "North" if y > 0 else "East", None))

def layer(c):
    if any((c[0] + dx, c[1] + dy) not in hull for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1))):
        return "Outer"
    return "Hallway" if c in corr else "Inner"

design = {
    "name": "eh_vigil",
    "hull_cells": [{"grid_pos": list(c), "layer": layer(c), "material": "Titanium"} for c in sorted(hull)],
    "modules": [{"module_type": t, "grid_pos": [x, y], "rotation": r, **({"extras": e} if e else {})}
                for t, x, y, r, e in plates + mods],
    "created_at": "tools/faction-arming/build_eternal_vigil.py (laid out by hand)",
    "version": 2,
    "guns": guns,
}
# The ship turns about grid (0, 0): centre it.
shift = -round(sum(x for x, _ in hull) / len(hull))
for c in design["hull_cells"]:
    c["grid_pos"][0] += shift
for m in design["modules"]:
    m["grid_pos"][0] += shift
for g in design["guns"]:
    if g["turret"]:
        g["turret"][0] += shift
    else:
        g["at"][0] += shift

for name, gun in [("eternal_170_mm_lance", LANCE), ("eternal_280_mm_throne_lance", THRONE_LANCE), ("eternal_ciws", PD)]:
    kit.write_gun(name, gun)
with open(OUT, "w") as f:
    json.dump(design, f, indent=1)
    f.write("\n")
count = lambda t: sum(1 for m in mods if m[0] == t)
print(OUT, len(hull), "hull,", len(design["modules"]), "modules,", len(guns), "fitted guns, shift", shift)
for t in ["MissileWarner", "TurretRing", "TurretRingSmall", "LaunchPad", "RailLauncher", "VlsSilo", "Magazine", "BasicQuarters",
          "StandardReactor", "HelmStation", "AICombatCore", "BridgeWing", "StandardEngine"]:
    print(" ", t, count(t))
print("  plates", len(plates))
