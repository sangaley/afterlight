"""Lays out the Halyard destroyer by hand and writes data.js for the preview.

Every block below is placed explicitly; nothing is generated except which
hull cells are Outer (the silhouette's edge) and Inner (everything else that
isn't a corridor). Run from afterlight/: python3 tools/ship-preview/build_data.py
"""
import json, re, shutil, os

ROOT = os.path.dirname(os.path.abspath(__file__))
CRATE = os.path.abspath(os.path.join(ROOT, "..", ".."))

# ---- silhouette: half-beam per column, stern (x=0) to bow --------------
SPANS = [(0, 2, 5), (3, 9, 7), (10, 33, 8), (34, 41, 7), (42, 47, 5), (48, 52, 3), (53, 56, 2)]
def half(x):
    for a, b, h in SPANS:
        if a <= x <= b:
            return h
    return None
hull = {}
for x in range(0, 57):
    h = half(x)
    for y in range(-h, h + 1):
        hull[(x, y)] = "Inner"
def inside(x, y): return (x, y) in hull
for (x, y) in list(hull):
    if not all(inside(x + dx, y + dy) for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1))):
        hull[(x, y)] = "Outer"
# Corridors, drawn by hand: the spine, three cross passages, and the loop
# round the big turret's ring.
corr = set()
corr |= {(x, 0) for x in range(6, 20)}
corr |= {(19, y) for y in range(-6, 7)}
corr |= {(x, 4) for x in range(19, 28)} | {(x, -4) for x in range(19, 28)}
corr |= {(27, y) for y in range(-7, 8)}
corr |= {(x, 0) for x in range(27, 38)}
corr |= {(37, y) for y in range(-6, 7)}
corr |= {(9, y) for y in range(-6, 7)}
for c in corr:
    if hull.get(c) == "Inner":
        hull[c] = "Hallway"

# ---- module metadata from the game -----------------------------------------
reg = open(os.path.join(CRATE, "src/building/registry.rs")).read()
meta = {}
for m in re.finditer(r'defs\.insert\(ModuleType::(\w+), ModuleDef \{(.*?)\}\);', reg, re.S):
    t, body = m.group(1), m.group(2)
    g = lambda pat, d=None: (re.search(pat, body).group(1) if re.search(pat, body) else d)
    size = re.search(r'size: IVec2::new\((\d+), (\d+)\)', body)
    meta[t] = {"name": g(r'name: "([^"]*)"', t), "desc": g(r'description: "([^"]*)"', ""),
               "cat": g(r'category: ModuleCategory::(\w+)', "?"), "w": int(size.group(1)), "h": int(size.group(2)),
               "cost": int(g(r'cost: (\d+)', 0)), "gen": float(g(r'power_generation: ([\d.]+)', 0)),
               "use": float(g(r'power_consumption: ([\d.]+)', 0))}
sm = open(os.path.join(CRATE, "src/sprite_map.rs")).read()
body = sm[sm.index("pub fn module_sprite_path"):]
body = body[:body.index("\n}\n")]
paths = {}
for m in re.finditer(r'((?:ModuleType::\w+\s*\|\s*)*ModuleType::\w+)\s*=>\s*"([^"]+)"', body):
    for t in re.findall(r'ModuleType::(\w+)', m.group(1)):
        paths[t] = m.group(2)

def rotate_offset(r, x, y):
    return {"North": (x, y), "East": (y, -x), "South": (-x, -y), "West": (-y, x)}[r]
def cells(t, x, y, r):
    return [(x + rotate_offset(r, i, j)[0], y + rotate_offset(r, i, j)[1]) for i in range(meta[t]["w"]) for j in range(meta[t]["h"])]

R = {"North": 0, "East": 1, "South": 2, "West": 3}
mods = []   # (type, x, y, rot)
def put(t, x, y, r="North"): mods.append((t, x, y, r))
def both(t, x, y, r="North"):
    """The block and its mirror across the centreline -- the mirror of its
    cells, not just its origin, so a 2x2 lands on the matching cells."""
    put(t, x, y, r)
    want = sorted((cx, -cy) for cx, cy in cells(t, x, y, r))
    flip = {"North": "South", "South": "North"}.get(r, r)
    for rot in [flip, r, "North", "East", "South", "West"]:
        for ox, oy in want:
            if sorted(cells(t, ox, oy, rot)) == want:
                put(t, ox, oy, rot)
                return
    raise ValueError(f"can't mirror {t} at {(x, y)}")

# ---- stern: drive ------------------------------------------------------
for y in (-5, -3, -1, 1, 3, 5):
    put("StandardEngine", 0, y, "West")
for y in (-4, -2, 0, 2, 4):
    put("FuelInjector", 1, y)
both("ManeuveringThruster", 12, 8, "North")
both("ManeuveringThruster", 44, 5, "North")
both("Afterburner", 2, 4, "West")
# ---- engineering -------------------------------------------------------
put("FusionReactor", 3, -1)
# The fusion core alone makes 400 MW; two standard reactors carry the rest.
both("StandardReactor", 6, 1); both("StandardReactor", 5, 4)
both("ReactorCooling", 4, 2)
put("EmergencyShutdown", 6, 2); put("PowerRegulator", 6, -2)
both("BatteryBank", 2, 3); both("Capacitor", 5, 3)
for x in (5, 6, 7, 8):
    both("FuelTank", x, 5)
put("EngineeringStation", 7, 2); put("RepairBay", 7, -2)
both("HeatVent", 8, 7)
# ---- crew --------------------------------------------------------------
for x in (10, 11, 12):
    both("BasicQuarters", x, 2)
both("Barracks", 10, 4, "East")
put("OfficerQuarters", 13, 2); put("MedBay", 13, -2)
put("GalleyMess", 14, 1)
put("SurgicalBay", 15, -3)
both("BulkCargoHold", 10, 6)  # (10,6)-(11,7) and its mirror
put("SmallCargo", 12, 6); put("SmallCargo", 12, -6)
both("AirlockValve", 15, 8); both("AirlockChamber", 15, 7)
# ---- life support -------------------------------------------------------
put("OxygenScrubber", 16, 2); put("OxygenScrubber", 17, 2)
put("CO2Scrubber", 16, 3); put("AirCirculator", 17, 3)
both("OxygenTank", 18, 2); put("WasteRecycler", 18, -4)
both("FireSuppression", 18, 5); put("AtmosphereMonitor", 13, -3)
both("EmergencyO2Cache", 16, 6)
# ---- turret C: large ring on the centreline -----------------------------
put("TurretRingLarge", 21, -2)
both("TraverseMotor", 22, 3); put("TraverseMotor", 26, 0)
put("AmmoHoist", 24, 3); put("AntiFlashDoor", 25, 3); put("Magazine", 26, 3)
put("Magazine", 26, -3); put("BeltConveyor", 25, -3)
put("AmmoHoist", 24, -3)  # spare hoist, port side
put("AntiFlashDoor", 23, -3)
# Turret C draws on the port hoist (the first one round its ring): two more
# magazines against the one it reaches make one store of three.
put("Magazine", 26, -2); put("Magazine", 26, -1)
# ---- midships -----------------------------------------------------------
both("ShieldEmitter", 30, 6); both("PointDefenseDrone", 31, 8)
both("RadarArray", 30, 1); put("MemoryCore", 29, 2); put("BatteryBank", 29, -2)
put("DroneBay", 28, 6, "East"); put("AdvancedRepairBay", 28, -6, "East")
both("ArmorPlate", 24, 8); both("ArmorPlate", 25, 8); both("ArmorPlate", 26, 8)
both("CoolingPump", 32, 2)
# ---- turrets A/B: flank sponsons ----------------------------------------
put("TurretRing", 33, 4); put("TurretRing", 33, -6)
both("TraverseMotor", 32, 5)
# Three motors a ring: one alone turns a turret ~9 deg/s, slow enough that
# its gun spends most of a fight holding fire.
both("TraverseMotor", 32, 4); both("TraverseMotor", 36, 5)
both("AmmoHoist", 34, 3); both("AntiFlashDoor", 35, 3); both("Magazine", 36, 3)
both("Magazine", 36, 4)
both("ArmorPlate", 36, 7)
# ---- bridge -------------------------------------------------------------
put("BridgeWing", 40, 3); put("HelmStation", 43, 3); put("NavigationConsole", 43, 4)
put("TargetingComputer", 44, 3); put("AdvancedRadar", 44, 4)
put("AICombatCore", 40, -4, "East"); put("MemoryCore", 42, -3); put("RadarArray", 44, -3)
put("SignalJammer", 43, -4)
# ---- spinal gun ----------------------------------------------------------
put("FiringPiece", 46, 0, "East"); put("Mantlet", 47, 0, "East")
for x in range(48, 56):
    put("GunBarrel", x, 0, "East")
put("MuzzleBrake", 56, 0, "East")
both("RailMagnet", 48, 1); both("RailMagnet", 49, 1)
both("GaugeIncreaser", 46, 1); both("GaugeIncreaser", 45, 1)
put("RecoilAbsorber", 45, 0)
both("BreechCooler", 46, 2); both("BreechCooler", 45, 2)
for y in (-1, 0, 1):
    put("Loader", 43, y, "East"); put("Loader", 44, y, "East")
    put("ClipRack", 41, y); put("ClipRack", 42, y)
    put("ShellFeeder", 40, y); put("Magazine", 39, y)
# ---- bow -----------------------------------------------------------------
for x in range(48, 53):
    both("AngledArmorPlate", x, 3, "North")
for x in range(53, 57):
    both("AngledArmorPlate", x, 2, "North")
both("AngledArmorPlate", 42, 5, "North"); both("AngledArmorPlate", 43, 5, "North")
both("Floodlight", 52, 2)
both("PointDefenseDrone", 50, 2)

# ---- fitting out the rest, by hand -----------------------------------------
# Engineering
both("ReactorFuelRod", 2, 2); both("ContainmentField", 3, 2); both("PowerConduit", 5, 2)
both("Transformer", 8, 2); both("PowerConduit", 7, 1); both("PowerConduit", 8, 1)
put("ThrustVectoring", 2, 0); both("InertialDampener", 2, 1); both("EngineNozzle", 1, 1)
both("RudderAssembly", 1, 3); both("RTG", 3, 3); both("PowerRegulator", 4, 3)
both("CoolingPump", 6, 3); both("HeatVent", 7, 3); both("MaintenanceLocker", 8, 3)
both("FuelTank", 3, 4); both("FuelTank", 4, 4); both("FuelProcessor", 6, 4)
both("BatteryBank", 3, 5); both("Capacitor", 4, 5)
both("FuelTank", 4, 6); both("FuelTank", 5, 6); both("ReactorCooling", 7, 6)
# Crew block
put("RecRoom", 10, 1); put("TrainingRoom", 11, 1)
put("AdvancedOxygenator", 16, 1); put("AirCirculator", 18, 1)
put("Brig", 10, -1); put("MessHall", 11, -1); put("RecRoom", 12, -1)
put("MedBay", 15, -1); put("CryoStorage", 16, -1); put("FireSuppression", 18, -1)
put("BasicQuarters", 11, 3); put("BasicQuarters", 12, 3); put("OfficerQuarters", 14, 3); put("OxygenTank", 18, 3)
put("BasicQuarters", 11, -3); put("BasicQuarters", 12, -3); put("OxygenTank", 18, -3)
put("Barracks", 10, 5); put("Barracks", 12, 5); put("Barracks", 14, 5); put("MessHall", 16, 5); put("RecRoom", 17, 5)
put("Barracks", 11, -5); put("Barracks", 13, -5); put("OfficerQuarters", 15, -5); put("Brig", 16, -5); put("MaintenanceLocker", 17, -5)
put("LargeCargo", 13, 6); put("SmallCargo", 17, 6); put("SmallCargo", 18, 6)
put("ReinforcedVault", 12, 7); put("SmallCargo", 13, 7); put("CryoStorage", 14, 7); put("MaintenanceLocker", 16, 7)
put("LargeCargo", 13, -6); put("SmallCargo", 17, -6); put("SmallCargo", 18, -6)
put("SpecimenVault", 12, -7); put("ReinforcedVault", 13, -7); put("MaintenanceLocker", 16, -7)
both("Floodlight", 17, 8)
# Beside turret C
put("WellnessHub", 20, 5); put("ResearchLab", 23, 5); put("MaintenanceLocker", 25, 5)
put("LargeCargo", 23, 6); put("RepairBay", 25, 6)
put("DroneBay", 20, -5); put("AdvancedRepairBay", 22, -5); put("RepairBay", 24, -5)
put("MaintenanceLocker", 25, -5); put("HullSealer", 26, -5)
put("LargeCargo", 20, -6); put("BulkCargoHold", 22, -7); put("SmallCargo", 24, -6)
# Midships
both("DepthScanner", 31, 1); both("ProximityAlarm", 32, 1)
both("BatteryBank", 28, 2); both("PassiveRadar", 30, 2); both("ThermalImager", 31, 2); both("SignalJammer", 33, 2)
both("HydrophoneArray", 28, 3); both("Capacitor", 30, 3); both("BatteryBank", 31, 3)
both("MaintenanceLocker", 28, 4); both("BatteryBank", 30, 4)
both("ResearchLab", 29, 5); both("PowerRegulator", 31, 5)
both("Capacitor", 29, 6); both("CoolingPump", 31, 6); both("HeatVent", 32, 6)
# Bridge
both("Capacitor", 38, 2); both("FireSuppression", 39, 2); both("AtmosphereMonitor", 41, 2)
put("AutopilotCore", 38, 3); put("MemoryCore", 39, 3); put("RadarArray", 46, 3); put("Searchlight", 47, 3)
put("AutopilotCore", 38, -3); put("BlackBox", 39, -3); put("Capacitor", 40, -3); put("PowerRegulator", 41, -3)
put("TargetingComputer", 43, -3); put("ThermalImager", 46, -3); put("Searchlight", 47, -3)
put("OfficerQuarters", 38, 4); put("OfficerQuarters", 39, 4); put("ProximityAlarm", 45, 4)
put("OfficerQuarters", 38, -4); put("OfficerQuarters", 39, -4); put("MessHall", 41, -4); put("RecRoom", 42, -4)
put("CO2Scrubber", 44, -4); put("OxygenScrubber", 45, -4)
put("TrainingRoom", 38, 5); put("MedBay", 39, 5); put("SmallCargo", 38, -5); put("ReinforcedVault", 39, -5)
both("BasicQuarters", 38, 6); both("BasicQuarters", 39, 6)
both("CoolingPump", 47, 2); both("FireSuppression", 48, 2); both("SignalJammer", 51, 2)
# Bow: more rail magnets along the spinal barrel, and bracing round it
both("RailMagnet", 50, 1); both("RailMagnet", 51, 1)
both("StructuralBrace", 52, 1); both("ReinforcedJoint", 53, 1); both("StructuralBrace", 54, 1); both("ReinforcedJoint", 55, 1)

# ---- turret decks (turret-local cells: (0, 0) at the ring's centre) -------
def gun_225(lane, flip):
    s = 1 if lane > 0 else -1
    p = [("FiringPiece", 0, lane, "East"), ("Mantlet", 1, lane, "East")]
    p += [("GunBarrel", x, lane, "East") for x in range(2, 6)]
    p += [("GaugeIncreaser", x, lane + s, "North") for x in (-1, 0, 1)]
    p += [("Loader", -1, lane, "East"), ("ClipRack", -2, lane, "North"), ("BreechCooler", -3, lane, "North")]
    return p
gun_170 = [("FiringPiece", 0, 0, "East"), ("Mantlet", 1, 0, "East")] + [("GunBarrel", x, 0, "East") for x in (2, 3, 4)] + \
    [("GaugeIncreaser", 0, 1, "North"), ("GaugeIncreaser", 0, -1, "North"), ("Loader", -1, 0, "East"),
     ("ClipRack", -2, 0, "North"), ("BreechCooler", -1, 1, "North")]
turrets = [
    {"id": "C", "label": "Turret C (twin 225 mm)", "ring": [21, -2], "size": 5, "centre": [23, 0], "angle": 0.0,
     "guns": [gun_225(1, False), gun_225(-1, True)]},
    {"id": "A", "label": "Turret A (170 mm)", "ring": [33, 4], "size": 3, "centre": [34, 5], "angle": 0.35, "guns": [gun_170]},
    {"id": "B", "label": "Turret B (170 mm)", "ring": [33, -6], "size": 3, "centre": [34, -5], "angle": -0.35, "guns": [gun_170]},
]

# Sanity: no two blocks share a cell, and everything is on the hull.
owner = {}
for t, x, y, r in mods:
    for c in cells(t, x, y, r):
        assert c in hull, f"{t} at {(x, y)} covers {c}, off the hull"
        assert c not in owner, f"{t} at {(x, y)} overlaps {owner[c]} at {c}"
        owner[c] = (t, x, y)
        assert hull[c] != "Hallway", f"{t} at {(x, y)} blocks the corridor at {c}"
for tu in turrets:
    taken = set()
    for gun in tu["guns"]:
        for t, x, y, r in gun:
            assert (x, y) not in taken, f"turret {tu['id']}: {t} overlaps at {(x, y)}"
            taken.add((x, y))

used = sorted({t for t, *_ in mods} | {t for tu in turrets for g in tu["guns"] for t, *_ in g} | {"TurretRingLarge", "TurretRing"})
out_dir = os.path.join(ROOT, "mod"); os.makedirs(out_dir, exist_ok=True)
for t in used:
    shutil.copy(os.path.join(CRATE, "assets", paths[t]), os.path.join(out_dir, t + ".png"))
tiles = os.path.join(ROOT, "tiles"); os.makedirs(tiles, exist_ok=True)
for n in ("hull_steel", "hull_hallway"):
    for i in range(16):
        shutil.copy(os.path.join(CRATE, "assets/sprites/hull", f"{n}_{i:02d}.png"), tiles)
shutil.copy(os.path.join(CRATE, "assets/sprites/gun_parts/turret_plating.png"), os.path.join(tiles, "turret_plating.png"))

# Small copies: the page draws a cell at about 30 px.
from PIL import Image
for t in used:
    f = os.path.join(out_dir, t + ".png")
    Image.open(f).resize((96 * meta[t]["w"], 96 * meta[t]["h"]), Image.LANCZOS).save(f, optimize=True)
for f in os.listdir(tiles):
    q = os.path.join(tiles, f)
    Image.open(q).resize((64, 64), Image.LANCZOS).save(q, optimize=True)

data = {
    "hull": [[x, y, l] for (x, y), l in sorted(hull.items())],
    "modules": [[t, x, y, R[r]] for t, x, y, r in mods],
    "turrets": [{**tu, "guns": [[[t, x, y, R[r]] for t, x, y, r in g] for g in tu["guns"]]} for tu in turrets],
    "meta": {t: meta[t] for t in used},
}
# ---- the same ship as a game design: designs/halyard.json -------------------
# Turret blocks sit on their ring's turret-local grid ((0, 0) at the ring's
# centre) and name the ring by its origin cell, as the game saves them.
# A ship turns about grid (0, 0), so the design is shifted to put that at
# the middle of the hull (the layout above starts at the stern).
SHIFT = -round(sum(x for x, _ in hull) / len(hull))
def module(t, x, y, r, turret=None):
    return {"module_type": t, "grid_pos": [x, y], "rotation": r, "custom_name": None, "subcomponents": None,
            "extras": {"turret": [turret[0] + SHIFT, turret[1]]} if turret else None}
blueprint = {
    "name": "Halyard",
    "hull_cells": [{"grid_pos": [x + SHIFT, y], "layer": l, "material": "Steel"} for (x, y), l in sorted(hull.items())],
    "modules": [module(t, x + SHIFT, y, r) for t, x, y, r in mods]
             + [module(t, x, y, r, tu["ring"]) for tu in turrets for g in tu["guns"] for t, x, y, r in g],
    "created_at": "tools/ship-preview/build_data.py",
    "version": 2,
}
with open(os.path.join(CRATE, "designs", "halyard.json"), "w") as f:
    json.dump(blueprint, f, indent=1)
open(os.path.join(ROOT, "data.js"), "w").write("window.SHIP=" + json.dumps(data) + ";\n")
print(len(hull), "hull cells,", len(mods), "hull-deck blocks,", sum(len(g) for tu in turrets for g in tu["guns"]), "turret blocks,", len(used), "block types")
