"""designs/lasertest.json: a small ship to try the lasers on.

A Cutter-style bank amidships (19 cavities, 9 pumps, a doubler, two
destabilisers: a steady green beam), a coupler spine running to a hull
emitter at the bow with four lenses, a 3x3 turret carrying an emitter and
two lenses, and a small point defence laser turret -- both fed through Beam
Relays beside their rings. Two Missile Warners so the point defence sees.

Run from afterlight/: python3 tools/laser-lab/build_lasertest.py
"""
import json, os, sys
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "faction-arming"))
import kit

W, H = 26, 5          # x 0..25, y -5..5
hull = []
for x in range(W):
    for y in range(-H, H + 1):
        edge = x in (0, W - 1) or abs(y) == H
        hull.append({"grid_pos": [x, y], "layer": "Outer" if edge else "Inner", "material": "Steel"})
mods = []
def put(t, x, y, r="North", extras=None):
    mods.append(kit.module(t, x, y, r, extras))

for y in (-2, 2):
    put("StandardEngine", 0, y, "West")
put("StandardReactor", 3, 2); put("StandardReactor", 3, -2)
put("HelmStation", 5, 0)
for x in (2, 3):
    for y in (4, -4):
        put("BasicQuarters", x, y)
put("OxygenScrubber", 5, 3); put("OxygenScrubber", 5, -3)
put("MissileWarner", 1, 0); put("MissileWarner", 24, 4); put("MissileWarner", 24, -4)

# The bank: core at (12, 0), a spine of couplers to the bow.
rows = {
    2:  "PVVVVVP",
    1:  "PVVXVVP",
    0:  "PVDC",
    -1: "PVVXVVP",
    -2: "PVVVVVP",
}
part = {"P": "LaserPump", "V": "LaserCavity", "X": "LaserDestabiliser", "D": "FrequencyDoubler", "C": "LaserCore"}
for y, row in rows.items():
    for i, ch in enumerate(row):
        put(part[ch], 9 + i, y)
for x in range(13, 20):
    put("LaserCoupler", x, 0)
put("LaserEmitter", 20, 0, "East")
for x in range(21, 25):
    put("LaserLens", x, 0, "East")

# Weapon turret: a 3x3 ring at (17, 2), a relay below it on the spine.
put("TurretRing", 17, 2)
for c in [(16, 3), (20, 3), (16, 2)]:
    put("TraverseMotor", *c)
put("BeamRelay", 18, 1)
turret = [17, 2]
put("LaserEmitter", 0, 0, "East", {"turret": turret})
put("LaserLens", 1, 0, "East", {"turret": turret})
put("LaserLens", 2, 0, "East", {"turret": turret})

# Point defence: a small ring at (17, -3), fed down a coupler from the spine.
put("TurretRingSmall", 17, -3)
put("TraverseMotor", 16, -3); put("TraverseMotor", 18, -3)
put("LaserCoupler", 17, -1)
put("BeamRelay", 17, -2)
pd = [17, -3]
put("LaserEmitter", 0, 0, "East", {"turret": pd, "point_defence": True})
put("LaserLens", 1, 0, "East", {"turret": pd})

# Sanity: nothing overlaps on the hull deck.
taken = {}
for m in mods:
    if (m.get("extras") or {}).get("turret"):
        continue
    for c in kit.cells_of(m["module_type"], m["grid_pos"], m["rotation"]):
        assert c not in taken, f"{m['module_type']} at {m['grid_pos']} overlaps {taken[c]}"
        taken[c] = m["module_type"]

out = os.path.join(kit.ROOT, "designs/lasertest.json")
json.dump({"name": "Laser Test", "hull_cells": hull, "modules": mods, "created_at": "tools/laser-lab/build_lasertest.py", "version": 2}, open(out, "w"), indent=1)
print(out, len(hull), "hull,", len(mods), "modules")
