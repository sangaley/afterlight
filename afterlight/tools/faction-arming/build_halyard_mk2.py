"""The Halyard Mk II: the Halyard refitted with everything the big-ships branch
built, as the player's ship. Writes designs/halyard_mk2.json; the Halyard
itself is left alone.

- Shells: the 280 mm spinal fires AP (punches through several blocks), the
  twin 225 mm turret fires HE (blasts an area), the 170 mm sponsons fire AP.
- Point defence: the four old PD drones become small turret rings, each with
  a 20 mm CIWS on point defence, traverse motors, a hoist and a magazine.
- Missiles: four Launch Pads (Hornets, heat-seeking), two Rail Launchers
  (command-guided frag) and two VLS Silos (radar-guided heavies with a
  shaped charge), in place of rec rooms, labs, cargo and lights.
- A Decoy Launcher and a Chaff Dispenser, in place of the two brigs.
- Two flank sponsons amidships, each with a point defence laser: a small
  bank (4 pulses/s) feeding a laser turret through a Beam Relay.

Run from afterlight/: python3 tools/faction-arming/build_halyard_mk2.py
"""
import os
import kit

FLUFF = {
    "RecRoom", "TrainingRoom", "Brig", "SpecimenVault", "CryoStorage", "ResearchLab", "WellnessHub", "Searchlight",
    "Floodlight", "MessHall", "LargeCargo", "SmallCargo", "ReinforcedVault", "MaintenanceLocker", "SignalJammer",
}

PD_GUN = {
    "name": "20 mm CIWS",
    # Phalanx in spirit, not in build: the whole gun sits on its 3x3 deck,
    # nothing overhangs, and it is light enough to snap from missile to missile.
    "description": "Close-in point defence, compact and agile: the whole 20 mm gun fits on a 3x3 deck, fires about 26 tiny flak rounds a second and snaps round fast.",
    "parts": [
        {"module_type": "FiringPiece", "offset": [0, 0], "rotation": "East"},
        {"module_type": "GaugeReducer", "offset": [0, 1], "rotation": "East"},
        {"module_type": "GaugeReducer", "offset": [0, -1], "rotation": "East"},
        {"module_type": "GunBarrel", "offset": [1, 0], "rotation": "East"},
        {"module_type": "Loader", "offset": [-1, 0], "rotation": "East"},
        {"module_type": "ClipRack", "offset": [-1, 1], "rotation": "East"},
        {"module_type": "BreechCooler", "offset": [-1, -1], "rotation": "East"},
        {"module_type": "BreechCooler", "offset": [1, 1], "rotation": "East"},
    ],
    "shell": {"base": "None", "casings": 2, "bodies": ["Frag", "ProximityFuse"], "head": "Frag"},
    "point_defence": True,
}

AP = {"base": "Tracer", "casings": 3, "bodies": ["Solid", "Solid"], "head": "Ap"}
AP_LIGHT = {"base": "None", "casings": 3, "bodies": ["Solid"], "head": "Ap"}
HE = {"base": "None", "casings": 2, "bodies": ["He", "He"], "head": "He"}

RAIL_MISSILE = {"motor": "Booster", "body": ["Fuel", "Frag", "Frag", "ProximityFuse"], "head": "CommandLink"}
VLS_MISSILE = {"motor": "Sustainer", "body": ["Fuel", "Fuel", "Fins", "SignalProcessor", "ShapedCharge", "He", "ArmourSheath"], "head": "Radar"}


def clear_and_place(s, t, cell, r="North", extras=None):
    """Put `t` at `cell`, clearing any fluff that covers its cells first."""
    for c in kit.cells_of(t, cell, r):
        assert c in s.hull and s.hull[c] != "Hallway", f"{t} at {cell}: {c} isn't a deck cell"
        m = s.at(c)
        if m is not None:
            assert m["module_type"] in FLUFF, f"{t} at {cell}: {c} holds {m['module_type']}"
            s.remove(m)
    s.place(t, cell, r, extras)


def pd_turret(s, ring, motor, hoist, magazine, motor2=None):
    drone = s.at(ring)
    assert drone and drone["module_type"] == "PointDefenseDrone", f"no PD drone at {ring}"
    s.remove(drone)
    s.place("TurretRingSmall", ring)
    parts = [("TraverseMotor", motor), ("AmmoHoist", hoist), ("Magazine", magazine)]
    if motor2:
        parts.append(("TraverseMotor", motor2))   # two motors: it snaps round
    for t, c in parts:
        clear_and_place(s, t, c)
    s.fit_gun(PD_GUN["name"], turret=ring)


def shell_on(s, turret, shell):
    """Every firing piece on that turret (None: the hull-mounted spinal)."""
    n = 0
    for m in s.d["modules"]:
        if m["module_type"] != "FiringPiece":
            continue
        on = (m.get("extras") or {}).get("turret")
        if (on and tuple(on) == turret) or (turret is None and not on):
            m["extras"] = {**(m.get("extras") or {}), "shell": shell}
            n += 1
    assert n, f"no firing piece on {turret}"


# A point defence laser unit, in its own sponson: (dx, dy, part), dy outboard.
# A small bank -- core, three Q-switches (4 pulses/s), a destabiliser, four
# cavities and four pumps -- feeding a small ring through a Beam Relay.
PD_LASER_UNIT = [
    (2, 3, "TurretRingSmall"), (1, 2, "TraverseMotor"), (3, 2, "TraverseMotor"), (2, 2, "BeamRelay"),
    (0, 1, "QSwitch"), (1, 1, "LaserCoupler"), (2, 1, "LaserCoupler"), (3, 1, "QSwitch"),
    (0, 0, "QSwitch"), (1, 0, "LaserCore"), (2, 0, "LaserDestabiliser"),
    (0, -1, "LaserPump"), (1, -1, "LaserCavity"), (2, -1, "LaserCavity"), (3, -1, "LaserPump"),
    (0, -2, "LaserPump"), (1, -2, "LaserCavity"), (2, -2, "LaserCavity"), (3, -2, "LaserPump"),
]


def add_laser_sponsons(s):
    """A sponson on each flank amidships, over plain plating at x -7..-2,
    with a point defence laser unit in it (alongside the CIWS)."""
    for sgn in (1, -1):
        for x in range(-7, -1):
            for y in range(9, 14):
                s.d["hull_cells"].append({"grid_pos": [x, sgn * y], "layer": "Inner", "material": "Steel"})
    cells = {tuple(c["grid_pos"]): c for c in s.d["hull_cells"]}
    for (x, y), c in cells.items():
        if c["layer"] == "Hallway":
            continue
        edge = any((x + dx, y + dy) not in cells for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)))
        c["layer"] = "Outer" if edge else "Inner"
    s.hull = {k: c["layer"] for k, c in cells.items()}
    x0, y0 = -6, 10
    for sgn in (1, -1):
        for dx, dy, t in PD_LASER_UNIT:
            clear_and_place(s, t, (x0 + dx, sgn * (y0 + dy) if sgn > 0 else -(y0 + dy)))
        ring = [x0 + 2, sgn * (y0 + 3)]
        s.d["modules"].append(kit.module("LaserEmitter", 0, 0, "East", {"turret": ring, "point_defence": True}))
        s.d["modules"].append(kit.module("LaserLens", 1, 0, "East", {"turret": ring}))


def main():
    s = kit.Ship(os.path.join(kit.ROOT, "designs/halyard.json"))
    s.d["name"] = "Halyard Mk II"
    s.d["created_at"] = "tools/faction-arming/build_halyard_mk2.py"
    kit.write_gun("20_mm_ciws", PD_GUN)

    shell_on(s, None, AP)
    shell_on(s, (-4, -2), HE)
    shell_on(s, (8, 4), AP_LIGHT)
    shell_on(s, (8, -6), AP_LIGHT)

    # Midships: on the dorsal/ventral edge, motor-hoist-magazine on the deck inboard.
    pd_turret(s, (6, 8), motor=(5, 7), hoist=(6, 7), magazine=(7, 7), motor2=(7, 8))
    pd_turret(s, (6, -8), motor=(5, -7), hoist=(6, -7), magazine=(7, -7), motor2=(7, -8))
    # Bow: the jammer beside each becomes the hoist, the floodlight past it the magazine.
    pd_turret(s, (25, 2), motor=(24, 2), hoist=(26, 2), magazine=(27, 2))
    pd_turret(s, (25, -2), motor=(24, -2), hoist=(26, -2), magazine=(27, -2))

    for cell in [(-8, 8), (-8, -8), (17, -4), (13, 5)]:
        clear_and_place(s, "LaunchPad", cell, "East")
    for cell in [(-2, 5), (-1, -7)]:
        clear_and_place(s, "RailLauncher", cell, "East", {"missile": RAIL_MISSILE})
    for cell in [(-5, 5), (-5, -7)]:
        clear_and_place(s, "VlsSilo", cell, "North", {"missile": VLS_MISSILE})

    # Missile Warners, fore and aft on both sides: point defence and decoys
    # act only on missiles a warner has seen.
    for cell in [(-17, 3), (-17, -3), (22, 3), (22, -3)]:
        clear_and_place(s, "MissileWarner", cell)

    # Defensive lasers alongside the CIWS, in two flank sponsons.
    add_laser_sponsons(s)

    clear_and_place(s, "DecoyLauncher", (-15, -1))
    clear_and_place(s, "ChaffDispenser", (-9, -5))

    out = os.path.join(kit.ROOT, "designs/halyard_mk2.json")
    s.save(out)
    print(out, len(s.d["hull_cells"]), "hull,", len(s.d["modules"]), "modules +", len(s.d["guns"]), "fitted guns")
    for t in ["LaunchPad", "RailLauncher", "VlsSilo", "TurretRingSmall", "DecoyLauncher", "ChaffDispenser", "MissileWarner", "Magazine", "LaserCore", "LaserEmitter"]:
        print(" ", t, s.count(t))


if __name__ == "__main__":
    main()
