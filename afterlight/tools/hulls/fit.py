"""Fit out the curved claw hulls.

Everything that keeps the ship alive -- reactors, fuel, air, cooling, berths,
the memory cores -- lives in the body, aft of where the hull splits. The arms
carry guns, sensors and the salvage gear and nothing the crew cannot lose.
Lose an arm and you lose teeth, not breathable air.
"""
import sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'design.py')).read())
from shapes import SHAPES, normalise, body_of

def plan_for(cells, body, split):
    bxs = [p[0] for p in body]; bys = [p[1] for p in body]
    bx0, bx1 = min(bxs), max(bxs)
    ys = [p[1] for p in cells]; ytop, ybot = max(ys), min(ys)
    def B(f):  return int(round(bx0 + (bx1 - bx0) * f))    # along the body
    axs = [p[0] for p in cells]
    def A(f):  return int(round(split + (max(axs) - split) * f))   # along the arms
    return [
        # --- body: drive and power, aft ---
        ('StandardEngine', bx0, 1, True), ('StandardEngine', bx0, 0, True),
        ('StandardEngine', bx0, -1, True), ('StandardEngine', bx0, 2, True),
        ('StandardEngine', bx0, -2, True),
        ('FuelTank', B(.22), 2, False), ('FuelTank', B(.22), -2, False),
        ('StandardReactor', B(.38), 3, False), ('StandardReactor', B(.38), -3, False),
        ('CoolingPump', B(.30), 1, False), ('CoolingPump', B(.30), -1, False),
        ('HeatVent', B(.30), ytop, True), ('HeatVent', B(.30), ybot, True),
        # --- body: air, crew, medical ---
        ('OxygenScrubber', B(.55), 2, False), ('OxygenScrubber', B(.55), -2, False),
        ('BasicQuarters', B(.70), 2, False), ('BasicQuarters', B(.70), -2, False),
        ('GalleyMess', B(.72), 4, False),
        ('RepairBay', B(.55), 0, False),
        ('BulkCargoHold', B(.86), -4, False),
        # --- body: the cores, buried where the hull is thickest ---
        ('MemoryCore', B(.30), 0, False), ('MemoryCore', B(.62), 0, False),
        ('MemoryCore', B(.86), 1, False),
        # --- arms: eyes, guns, working gear ---
        ('BridgeWing', A(.10), 0, False),
        ('RadarArray', A(.24), 0, False),
        ('Floodlight', A(.55), 0, True),
        ('ShieldEmitter', A(.15), ytop - 1, False), ('ShieldEmitter', A(.15), ybot + 1, False),
        ('Cannon', A(.45), ytop, True), ('Cannon', A(.45), ybot, True),
        ('Gatling', A(.80), ytop, True), ('Gatling', A(.80), ybot, True),
        ('SalvageArm', A(.66), 1, True),
        ('AirlockChamber', A(.36), -1, True),
    ]

SHIPS = {}
for key in ('pincer', 'crab', 'trident'):
    cells = normalise(SHAPES[key])
    body, split = body_of(cells)
    mods = fit_out(cells, plan_for(cells, body, split))
    total, reach, stranded = connected(cells, mods)
    in_arms = [m['t'] for m in mods
               if m['x'] >= split and MODS[m['t']]['cat'] in ('Power', 'LifeSupport', 'Crew')]
    SHIPS[key] = dict(cells=cells, mods=mods, body=body, split=split)
    print('%-8s hull %3d  mods %2d  deck %3d/%3d  stranded %d  vitals in arms: %s'
          % (key, len(cells), len(mods), reach, total, len(stranded), in_arms or 'none'))
    if stranded:
        print('        ', stranded[:6])

# ------------------------------------------------------------- export -----
def to_blueprint(name, cells, mods):
    """A real design file the game can fly and build-mode can edit."""
    taken = occupied(cells, mods)
    inner = interior(cells)
    hull = []
    for c in sorted(cells):
        if c not in inner:
            layer = "Outer"
        elif c in taken:
            layer = "Inner"
        else:
            layer = "Hallway"
        hull.append({"grid_pos": [c[0], c[1]], "layer": layer, "material": "Steel"})
    out_mods = []
    for m in sorted(mods, key=lambda m: (m['x'], m['y'])):
        out_mods.append({
            "module_type": m['t'],
            "grid_pos": [m['x'], m['y']],
            "rotation": m['r'],
            "custom_name": None,
            "subcomponents": None,
            "extras": None,
        })
    return {"name": name, "hull_cells": hull, "modules": out_mods,
            "created_at": "tools/hulls", "version": 2}


if __name__ == "__main__":
    import json
    dest = "/Users/shhh/afterlight-cascade/afterlight/designs/"
    for key, s in SHIPS.items():
        bp = to_blueprint(key, s['cells'], s['mods'])
        with open(dest + key + ".json", "w") as f:
            json.dump(bp, f, indent=2)
        print("  wrote designs/%s.json  %d hull, %d modules"
              % (key, len(bp['hull_cells']), len(bp['modules'])))
