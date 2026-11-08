"""Arms the Terran Hegemony with the new weapons, in place of old ones.

Swaps happen cell for cell, so the hand-drawn layouts keep their shape:
- Lancer, Warden: the bow cannon becomes a Launch Pad (heat-seeking Hornets).
- Bulwark: the cannons on the second pair of side pods become Launch Pads.
- Sovereign: each old point defence drone becomes a small turret ring with a
  60 mm flak gun on point defence ("Terran PD Flak"), a traverse motor
  outboard of it and a hoist + magazine inboard, leaving one side of the
  tower corridor open on every row; the middle pods' cannons become Launch
  Pads; a quarters cell each becomes a Decoy Launcher and a Chaff Dispenser.
- Bastion: its eight Heavy Missile launchers become Launch Pads.

Run from afterlight/: python3 tools/faction-arming/arm_terran.py
It refuses to touch a ship that already has any of the new parts.
"""
import json, os, sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
SHIPS = os.path.join(ROOT, "designs/factions/terran_hegemony")
NEW = {"LaunchPad", "RailLauncher", "VlsSilo", "TurretRingSmall", "AmmoHoist", "DecoyLauncher", "ChaffDispenser"}

# Since 2026-10-28 the Sovereign fits "Terran CIWS" (designs/guns/terran_ciws.json,
# made from build_halyard_mk2.py's 20 mm CIWS); this 60 mm flak was the first.
PD_GUN = {
    "name": "Terran PD Flak",
    "description": "60 mm flak on a light turret: point defence against missiles. Frag with a proximity fuse.",
    "parts": [
        {"module_type": "FiringPiece", "offset": [0, 0], "rotation": "East"},
        {"module_type": "Mantlet", "offset": [1, 0], "rotation": "East"},
        {"module_type": "GunBarrel", "offset": [2, 0], "rotation": "East"},
        {"module_type": "GunBarrel", "offset": [3, 0], "rotation": "East"},
        {"module_type": "Loader", "offset": [-1, 0], "rotation": "East"},
        {"module_type": "ClipRack", "offset": [-2, 0], "rotation": "East"},
        {"module_type": "BreechCooler", "offset": [0, 1], "rotation": "East"},
        {"module_type": "BreechCooler", "offset": [0, -1], "rotation": "East"},
    ],
    "shell": {"base": "None", "casings": 2, "bodies": ["Frag", "ProximityFuse"], "head": "Frag"},
    "faction": "TerranHegemony",
    "point_defence": True,
}


def module(t, x, y, r):
    return {"module_type": t, "grid_pos": [x, y], "rotation": r, "custom_name": None, "subcomponents": None, "extras": None}


def load(name):
    path = os.path.join(SHIPS, name + ".json")
    text = open(path).read()
    return path, text, json.loads(text)


def save(path, text, design):
    indent = 1 if text.startswith('{\n "') else 2
    with open(path, "w") as f:
        json.dump(design, f, indent=indent)


def swap(design, kind, cells, into):
    """Every `kind` module at one of `cells` becomes `into`, same cell and facing."""
    n = 0
    for m in design["modules"]:
        if m["module_type"] == kind and tuple(m["grid_pos"]) in cells:
            m["module_type"] = into
            n += 1
    assert n == len(cells), f"expected {len(cells)} {kind} at {cells}, found {n}"


def free(design, cell):
    hull = {tuple(c["grid_pos"]): c["layer"] for c in design["hull_cells"]}
    taken = {tuple(m["grid_pos"]) for m in design["modules"] if not (m.get("extras") or {}).get("turret")}
    assert cell in hull and cell not in taken, f"{cell} isn't a free hull cell"


def main():
    for name in ["lancer", "warden", "bulwark", "sovereign", "bastion"]:
        _, _, d = load(name)
        if any(m["module_type"] in NEW for m in d["modules"]):
            sys.exit(f"{name} already has new parts; not touching it")

    gun_path = os.path.join(ROOT, "designs/guns/terran_pd_flak.json")
    with open(gun_path, "w") as f:
        json.dump(PD_GUN, f, indent=2)  # superseded by terran_ciws.json

    for name, cell in [("lancer", (6, 0)), ("warden", (10, 0))]:
        path, text, d = load(name)
        swap(d, "Cannon", {cell}, "LaunchPad")
        save(path, text, d)

    path, text, d = load("bulwark")
    swap(d, "Cannon", {(1, 7), (3, 7), (1, -7), (3, -7)}, "LaunchPad")
    save(path, text, d)

    path, text, d = load("sovereign")
    swap(d, "Cannon", {(-5, 16), (-3, 16), (-5, -16), (-3, -16)}, "LaunchPad")
    for x, y in [(8, 7), (-12, 7), (8, -7), (-12, -7)]:
        out = 1 if y > 0 else -1  # toward the pod; the body is the other way
        drone = next(m for m in d["modules"] if m["module_type"] == "PointDefenseDrone" and tuple(m["grid_pos"]) == (x, y))
        d["modules"].remove(drone)
        for cell in [(x, y + out), (x, y - out), (x - 1, y - out)]:
            free(d, cell)
        d["modules"] += [
            module("TurretRingSmall", x, y, "North"),
            module("TraverseMotor", x, y + out, "North"),
            module("AmmoHoist", x, y - out, "North"),
            module("Magazine", x - 1, y - out, "North"),
        ]
        d.setdefault("guns", []).append({"design": "Terran CIWS", "at": [0, 0], "facing": "East", "turret": [x, y]})
    quarters = sorted((tuple(m["grid_pos"]) for m in d["modules"] if m["module_type"] == "BasicQuarters"), key=lambda c: (abs(c[1]), abs(c[0])))
    for (cell, into) in zip(quarters[:2], ["DecoyLauncher", "ChaffDispenser"]):
        swap(d, "BasicQuarters", {cell}, into)
    save(path, text, d)

    path, text, d = load("bastion")
    cells = {tuple(m["grid_pos"]) for m in d["modules"] if m["module_type"] == "HeavyMissile"}
    swap(d, "HeavyMissile", cells, "LaunchPad")
    save(path, text, d)
    print("armed: lancer, warden, bulwark, sovereign, bastion; gun design", gun_path)


if __name__ == "__main__":
    main()
