"""Writes the builder's pre-made weapons (designs/kits/*.json): whole
installations a beginner puts down in one go, in the gun design format with
`turret: true` on parts that stand on the kit's own ring. Offsets are from
the kit's origin -- a turret's ring centre, or a hull gun's firing piece --
facing East (the bow, +x). The game checks every kit assembles
(combat::gun_design tests).

The turret installs are the ones proven in fights: the Vigil's 3x3 ring and
the Halyard Mk II's twin 225 mm turret. The faction refit (tools/ship-scale)
places these same kits.

Run from afterlight/: python3 tools/kits/make_kits.py
"""
import json, os

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
OUT = os.path.join(ROOT, "designs/kits")
GUNS = os.path.join(ROOT, "designs/guns")


def part(t, x, y, r="East", turret=False):
    p = {"module_type": t, "offset": [x, y], "rotation": r}
    if turret:
        p["turret"] = True
    return p


def gun_parts(name):
    """A saved gun design's parts, to stand on a kit's turret."""
    d = json.load(open(os.path.join(GUNS, name)))
    return d, [part(p["module_type"], p["offset"][0], p["offset"][1], p["rotation"], turret=True) for p in d["parts"]]


# A small ring's hull: ring, one motor, a hoist and its magazine: 2x2.
SMALL_RING = [part("TurretRingSmall", 0, 0, "North"), part("TraverseMotor", 1, 0, "North"),
              part("AmmoHoist", 0, -1, "North"), part("Magazine", 1, -1, "North")]

# The Vigil's 3x3 ring (cells -1..1): three motors, a hoist below, five
# magazines behind it.
RING = ([part("TurretRing", -1, -1, "North")]
        + [part("TraverseMotor", x, y, "North") for x, y in [(-2, -1), (-2, 0), (2, -1)]]
        + [part("AmmoHoist", 0, -2, "North")]
        + [part("Magazine", x, y, "North") for x, y in [(-1, -2), (1, -2), (-1, -3), (0, -3), (1, -3)]])

KITS = []

# 1. Light autocannon: a 60 mm gun on a small turret. Quick and cheap.
KITS.append({
    "name": "Light Autocannon",
    "description": "A 60 mm autocannon on a small turret: ring, motor, hoist and magazine in a 2x2 block, the gun up on the turret. Quick to turn and fast-firing; good against small craft. Place it on empty cells touching your hull.",
    "parts": SMALL_RING + [part("FiringPiece", 0, 0, turret=True), part("Loader", -1, 0, turret=True),
                           part("ClipRack", -1, 1, turret=True), part("BreechCooler", -1, -1, turret=True),
                           part("GunBarrel", 1, 0, turret=True), part("GunBarrel", 2, 0, turret=True)],
    "shell": None,
})

# 2. CIWS: the 20 mm point defence turret, fires itself at missiles.
ciws, ciws_parts = gun_parts("20_mm_ciws.json")
KITS.append({
    "name": "CIWS Turret",
    "description": "Point defence: a 20 mm close-in gun on a small turret that shoots down incoming missiles on its own, firing flak. Put one on each side. Pair it with a Missile Defence Pack so its warner can spot what's coming.",
    "parts": SMALL_RING + ciws_parts,
    "shell": ciws["shell"],
    "point_defence": True,
})

# 3. Compact 170 mm: the same bore on a small ring, in a 2x2 block. Slow to
# turn on its one motor, but it fits where one old cannon stood.
# Three motors round the ring (one would turn its 20 t at 13 deg/s: the
# gun never caught the cursor).
SMALL_RING_HEAVY = SMALL_RING + [part("TraverseMotor", -1, 0, "North"), part("TraverseMotor", 0, 1, "North")]
KITS.append({
    "name": "Compact 170 mm Turret",
    "description": "A 170 mm gun squeezed onto a small turret: ring, three motors, hoist and magazine in a 3x3 cross. Hits as hard as the big ring's gun; a main gun for a small ship.",
    "parts": SMALL_RING_HEAVY + [part("FiringPiece", 0, 0, turret=True), part("GaugeIncreaser", 0, 1, turret=True),
                           part("GaugeIncreaser", 0, -1, turret=True), part("Loader", -1, 0, turret=True),
                           part("ClipRack", -1, 1, turret=True), part("BreechCooler", -1, -1, turret=True),
                           part("Mantlet", 1, 0, turret=True)]
             + [part("GunBarrel", x, 0, turret=True) for x in (2, 3, 4)],
    "shell": None,
})

# 4. 170 mm turret: the Vigil's light gun.
g170, g170_parts = gun_parts("170_mm_turret_gun.json")
KITS.append({
    "name": "170 mm Turret",
    "description": "A 170 mm gun on a 3x3 turret ring with three motors, a hoist and five magazines. The all-round main gun for a mid-size ship.",
    "parts": RING + g170_parts,
    "shell": g170.get("shell"),
})

# 5. Twin 225 mm: the Halyard Mk II's big turret, read from its design.
hal = json.load(open(os.path.join(ROOT, "designs/halyard_mk2.json")))
ring = next(m for m in hal["modules"] if m["module_type"] == "TurretRingLarge")
cx, cy = ring["grid_pos"][0] + 2, ring["grid_pos"][1] + 2
twin = [part("TurretRingLarge", -2, -2, "North")]
for m in hal["modules"]:
    x, y = m["grid_pos"][0] - cx, m["grid_pos"][1] - cy
    ex = m.get("extras") or {}
    if ex.get("turret") == ring["grid_pos"]:
        if m["module_type"] in ("FiringPiece", "GaugeIncreaser", "GunBarrel", "Mantlet", "Loader", "ClipRack", "BreechCooler", "MuzzleBrake", "RecoilAbsorber"):
            twin.append(part(m["module_type"], m["grid_pos"][0], m["grid_pos"][1], m["rotation"], turret=True))
    elif max(abs(x), abs(y)) <= 3 and m["module_type"] in ("TraverseMotor", "AmmoHoist", "Magazine", "AntiFlashDoor", "BeltConveyor"):
        twin.append(part(m["module_type"], x, y, m["rotation"]))
KITS.append({
    "name": "Twin 225 mm Turret",
    "description": "Two 225 mm guns on a large 5x5 turret ring, fed by two hoists through anti-flash doors. Heavy firepower for a big ship.",
    "parts": twin,
    "shell": None,
})

# 6. 280 mm spinal: fixed, aim it by turning the ship. Magazines behind.
g280, g280_parts = gun_parts("280_mm_spinal.json")
spinal = [dict(p, turret=False) for p in g280_parts]
for p in spinal:
    p.pop("turret")
spinal += [part("Magazine", -5, 0, "North"), part("Magazine", -5, 1, "North"), part("Magazine", -5, -1, "North")]
KITS.append({
    "name": "280 mm Spinal Gun",
    "description": "A heavy fixed gun along the hull: three loaders, six metres of barrel, magazines behind the feeder. It fires where the ship points, so lay it along your bow.",
    "parts": spinal,
    "shell": g280.get("shell"),
})

# 7. VLS silo: launches straight up, so it needs no clear lane.
KITS.append({
    "name": "VLS Missile Silo",
    "description": "A 2x2 vertical launch silo: missiles leave straight up out of the deck, so it can sit anywhere on the hull. Long reach, slow reload.",
    "parts": [part("VlsSilo", 0, 0, "North")],
    "shell": None,
})

# 8. Launch pads: a row of four, firing out over the bow.
KITS.append({
    "name": "Launch Pad Battery",
    "description": "Four launch pads in a row, firing out the way they face. Quick reloads, short reach. Their missiles need a clear lane out of the ship: put them on an edge facing out.",
    "parts": [part("LaunchPad", 0, y, "East") for y in (-1, 0, 1, 2)],
    "shell": None,
})

# 9. Rail launchers: two, side by side.
KITS.append({
    "name": "Rail Launcher Pair",
    "description": "Two rail launchers, three cells long, firing out the way they face. Faster missiles than pads, further reach. They need a clear lane out of the ship.",
    "parts": [part("RailLauncher", 0, 0, "East"), part("RailLauncher", 0, 1, "East")],
    "shell": None,
})


def laser(rows, emitter_dir="East"):
    """A laser unit from rows top to bottom; C is the core at the origin."""
    T = {"C": "LaserCore", "K": "LaserCoupler", "V": "LaserCavity", "P": "LaserPump", "Q": "QSwitch",
         "D": "FrequencyDoubler", "X": "LaserDestabiliser", "E": "LaserEmitter", "L": "LaserLens"}
    cy = next(i for i, r in enumerate(rows) if "C" in r)
    cx = rows[cy].index("C")
    return [part(T[ch], x - cx, cy - y, emitter_dir if ch == "E" else "East")
            for y, r in enumerate(rows) for x, ch in enumerate(r) if ch in T]


# 10. PD laser: the Gun Foundry's PD Laser.
KITS.append({
    "name": "PD Laser",
    "description": "A point defence laser unit: four pulses a second at whatever its ship's warners spot. Burns missiles down from 3,500 out, but draws 300 MW while it recharges.",
    "parts": laser(["...PPP...", "...VVV...", "...PVP...", "..QKCKEL.", "...QXQ..."]),
    "shell": None,
    "point_defence": True,
})

# 10b. PD laser turret: the Halyard Mk II's sponson unit. The bank on the
# hull; a Beam Relay beside a small ring feeds the emitter up on the
# turret, which swings all the way round (a hull emitter covers 60 degrees).
PD_LASER_UNIT = [
    (2, 3, "TurretRingSmall"), (1, 2, "TraverseMotor"), (3, 2, "TraverseMotor"), (2, 2, "BeamRelay"),
    (0, 1, "QSwitch"), (1, 1, "LaserCoupler"), (2, 1, "LaserCoupler"), (3, 1, "QSwitch"),
    (0, 0, "QSwitch"), (1, 0, "LaserCore"), (2, 0, "LaserDestabiliser"),
    (0, -1, "LaserPump"), (1, -1, "LaserCavity"), (2, -1, "LaserCavity"), (3, -1, "LaserPump"),
    (0, -2, "LaserPump"), (1, -2, "LaserCavity"), (2, -2, "LaserCavity"), (3, -2, "LaserPump"),
]
KITS.append({
    "name": "PD Laser Turret",
    "description": "Point defence that sees all round: a laser bank on the hull feeding an emitter on a small turret through a Beam Relay. Burns down missiles from 3,500 out, eight pulses a second, but draws 240 MW while it recharges.",
    "parts": [part(t, x - 2, y - 3, "North") for x, y, t in PD_LASER_UNIT]
             + [part("LaserEmitter", 0, 0, "East", turret=True), part("LaserLens", 1, 0, "East", turret=True)],
    "shell": None,
    "point_defence": True,
})

# 11. Cutter: a steady burning beam.
KITS.append({
    "name": "Cutter Laser",
    "description": "A steady beam that slices through armour: a doubler for burn, destabilisers to let it go, two lenses for reach. Hungry: 540 MW while it recharges.",
    "parts": laser([".PVVVVVP..", ".PV.VXVP..", ".PVDCKELL.", ".PV.VXVP..", ".PVVVVVP.."]),
    "shell": None,
})

# 12. Missile defence: see them coming, and confuse them.
KITS.append({
    "name": "Missile Defence Pack",
    "description": "A Missile Warner to spot incoming missiles for your point defence, with a decoy launcher and a chaff dispenser beside it to pull them off course.",
    "parts": [part("MissileWarner", 0, 0, "North"), part("DecoyLauncher", 1, 0, "North"), part("ChaffDispenser", -1, 0, "North")],
    "shell": None,
})

os.makedirs(OUT, exist_ok=True)
for f in os.listdir(OUT):
    if f.endswith(".json"):
        os.remove(os.path.join(OUT, f))
for i, k in enumerate(KITS, 1):
    k = {key: v for key, v in k.items() if v is not None}
    stem = "%02d_%s" % (i, k["name"].lower().replace(" ", "_").replace("/", ""))
    json.dump(k, open(os.path.join(OUT, stem + ".json"), "w"), indent=1)
    print("wrote", stem, len(k["parts"]), "parts")
