"""Arms every faction with the big-ships weapons, each in its own style.

The Terrans (arm_terran.py) and the Eternal Vigil (build_eternal_vigil.py)
came first; this does the rest. Swaps happen in place of old one-block
weapons, so each hand-drawn layout keeps its shape:

- pads:  old missile/gun blocks become Launch Pads, with the faction's own
         missile design (Choir frag screamers, Corpse Stars EMP).
- ciws:  an old block becomes a small turret ring carrying a 20 mm CIWS on
         point defence, with a traverse motor, an ammo hoist and a magazine
         beside it.
- laser: an old block becomes a hull Laser Emitter facing the same way, with
         a compact bank behind it: core, two modifiers, three cavities, three
         pumps. Gilded and Shepherd lasers pulse; Synthesis lasers hold a
         steady burning beam.

Ships that gain point defence get Missile Warners (add_warners.py). A swap
that can't fit is reported and skipped. Refuses any ship that already has
new parts, so it runs once.

Run from afterlight/: python3 tools/faction-arming/arm_factions.py
"""
import json, os, sys
import kit
import add_warners

FACTIONS = os.path.join(kit.ROOT, "designs/factions")
NEW = {"LaunchPad", "RailLauncher", "VlsSilo", "TurretRingSmall", "LaserCore", "LaserEmitter", "MissileWarner"}

# What may be cleared to make room. Never command, power, engines or crew.
CLEARABLE = {
    "Cannon", "Gatling", "Railgun", "HeavyMissile", "Laser", "PointDefenseDrone", "IonDisruptor", "EMPPulse",
    "PlasmaCaster", "ClusterRocket", "GuidedMissile", "AmmoBay", "Floodlight", "Searchlight", "SmallCargo",
    "LargeCargo", "MaintenanceLocker", "RecRoom", "TrainingRoom", "CryoStorage", "ResearchLab", "EmergencyBulkhead",
    "DockingPort", "SalvageArm", "TractorBeam", "SpecimenVault", "ReinforcedVault", "SignalBuoy",
    # Synthesis hulls are mostly repair bays: they regrow themselves.
    "RepairBay", "AdvancedRepairBay",
}

SCREAMER = {"motor": "Booster", "body": ["Frag", "Frag"], "head": "Heat"}
DIRGE = {"motor": "Booster", "body": ["Fins", "He"], "head": "DumbNose"}
LAST_RITES = {"motor": "Booster", "body": ["Emp", "Fuel"], "head": "Radar"}
SAMPLE = {"motor": "Booster", "body": ["Fins", "He"], "head": "Radar"}

# Two modifiers on each laser core.
FLAVOUR = {
    "pulse": ("QSwitch", "FrequencyDoubler"),      # Gilded: hard pulses that burn deep
    "steady": ("FrequencyDoubler", "LaserDestabiliser"),  # Synthesis: a steady cutting beam
    "lash": ("QSwitch", "LaserDestabiliser"),      # Shepherd: heavy, quick-draining pulses
}

CIWS = "20 mm CIWS"
RULES = {
    "eternal_hegemony": {
        "echo_escort": [("pads", "Cannon", 2, None)],
        "herald": [("pads", "Cannon", 4, None), ("ciws", "Gatling", 2, "Eternal CIWS")],
        "reliquary_of_echoes": [("pads", "Cannon", 4, None), ("ciws", "Gatling", 2, "Eternal CIWS")],
    },
    "gilded_throne": {
        "courier": [("laser", "Laser", 1, "pulse")],
        "dividend": [("laser", "Laser", 1, "pulse")],
        "debt_collector": [("laser", "Laser", 2, "pulse")],
        "throne_barge": [("laser", "Laser", 3, "pulse"), ("ciws", "PointDefenseDrone", 4, CIWS)],
        "bullion_freighter": [("laser", "Laser", 1, "pulse")],
    },
    "broken_choir": {
        "shard": [("pads", "HeavyMissile", 1, SCREAMER), ("pads", "ClusterRocket", 1, DIRGE)],
        "chorus": [("pads", "HeavyMissile", 2, SCREAMER), ("pads", "ClusterRocket", 2, DIRGE)],
        "requiem": [("pads", "HeavyMissile", 6, SCREAMER), ("pads", "ClusterRocket", 4, DIRGE), ("ciws", "Gatling", 2, CIWS)],
        "echo_chamber": [("pads", "HeavyMissile", 8, SCREAMER), ("pads", "ClusterRocket", 3, DIRGE), ("ciws", "Gatling", 3, CIWS)],
    },
    "synthesis_collective": {
        "graft": [("laser", "PlasmaCaster", 1, "steady")],
        "chimera": [("laser", "PlasmaCaster", 2, "steady")],
        "assimilator": [("laser", "PlasmaCaster", 3, "steady")],
        "bloom": [("laser", "PlasmaCaster", 1, "steady")],
    },
    "corpse_stars": {
        "mourner": [("pads", "HeavyMissile", 2, LAST_RITES)],
        "pyre": [("pads", "HeavyMissile", 4, LAST_RITES)],
        "dead_sun": [("pads", "HeavyMissile", 4, LAST_RITES)],
        "ossuary": [("pads", "HeavyMissile", 2, LAST_RITES)],
    },
    "stellar_preserve": {
        "net_weaver": [("ciws", "Gatling", 2, CIWS)],
        "seed_ark": [("ciws", "Gatling", 2, CIWS)],
    },
    "merchant_princes": {
        "hauler": [("ciws", "PointDefenseDrone", 1, CIWS)],
        "bulk_freighter": [("ciws", "PointDefenseDrone", 2, CIWS)],
        "mining_barge": [("ciws", "PointDefenseDrone", 1, CIWS)],
        "arms_bazaar": [("ciws", "PointDefenseDrone", 6, CIWS), ("pads", "Gatling", 6, SAMPLE)],
    },
    "free_haulers_compact": {
        "ore_hauler": [("ciws", "PointDefenseDrone", 1, CIWS)],
        "fuel_runner": [("ciws", "PointDefenseDrone", 2, CIWS)],
    },
    "shepherd": {
        "lamb": [("laser", "Laser", 1, "lash")],
        "ram": [("laser", "Laser", 2, "lash")],
        "the_shepherd": [("laser", "Laser", 4, "lash")],
    },
    "terran_hegemony": {
        "homeward": [("ciws", "PointDefenseDrone", 1, "Terran CIWS")],
        "hauler": [("ciws", "PointDefenseDrone", 1, "Terran CIWS")],
    },
}

FACING = {"North": (0, 1), "East": (1, 0), "South": (0, -1), "West": (-1, 0)}


def module_at(s, c):
    return s.at(c)


def free_or_clearable(s, c, keep=()):
    if c not in s.hull or c in keep:
        return False
    m = module_at(s, c)
    return m is None or (m["module_type"] in CLEARABLE and kit.SIZES.get(m["module_type"], (1, 1)) == (1, 1))


def force(s, t, c, r="North", extras=None):
    m = module_at(s, c)
    if m is not None:
        s.remove(m)
    s.d["modules"].append(kit.module(t, c[0], c[1], r, extras))


def spread(cells, n):
    """Pick n of these cells spread over the ship: mirrored pairs first,
    outermost first."""
    cells = sorted(cells, key=lambda c: (-abs(c[1]), -c[0], c[1]))
    picked = []
    for c in cells:
        if len(picked) >= n:
            break
        if c in picked:
            continue
        picked.append(c)
        twin = (c[0], -c[1])
        if twin in cells and twin not in picked and len(picked) < n:
            picked.append(twin)
    return picked[:n]


def sources(s, kind):
    return [tuple(m["grid_pos"]) for m in s.d["modules"] if m["module_type"] == kind and not (m.get("extras") or {}).get("turret")]


def pads(s, kind, n, design):
    done = 0
    for c in spread(sources(s, kind), n):
        old = module_at(s, c)
        force(s, "LaunchPad", c, old["rotation"], {"missile": design} if design else None)
        done += 1
    return done


def ciws(s, kind, n, gun):
    done = 0
    for c in spread(sources(s, kind), len(sources(s, kind))):
        if done >= n:
            break
        x, y = c
        ring_n = [(x + dx, y + dy) for dx in (-1, 0, 1) for dy in (-1, 0, 1) if (dx, dy) != (0, 0)]
        four = [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)]
        # Empty cells first, then ones holding something we may clear.
        rank = lambda c: (module_at(s, c) is not None, c)
        hoists = sorted([h for h in four if free_or_clearable(s, h, keep={c})], key=rank)
        placed = False
        for h in hoists:
            mags = sorted([m for m in [(h[0] + 1, h[1]), (h[0] - 1, h[1]), (h[0], h[1] + 1), (h[0], h[1] - 1)]
                           if m != c and free_or_clearable(s, m, keep={c, h})], key=rank)
            motors = sorted([m for m in ring_n if free_or_clearable(s, m, keep={c, h})], key=rank)
            for mag in mags:
                motor = next((m for m in motors if m != mag), None)
                if motor is None:
                    continue
                force(s, "TurretRingSmall", c)
                force(s, "AmmoHoist", h)
                force(s, "Magazine", mag)
                force(s, "TraverseMotor", motor)
                s.d.setdefault("guns", []).append({"design": gun, "at": [0, 0], "facing": "East", "turret": [x, y]})
                placed = done = done + 1
                break
            if placed:
                break
    return done


def laser(s, kind, n, flavour):
    done = 0
    m1, m2 = FLAVOUR[flavour]
    for c in spread(sources(s, kind), len(sources(s, kind))):
        if done >= n:
            break
        old = module_at(s, c)
        if old is None or old["module_type"] != kind:
            continue
        fx, fy = FACING[old["rotation"]]
        sx, sy = -fy, fx
        # The full bank first, then smaller ones that still count: every
        # cavity touches the core or a cavity, every pump a cavity, every
        # modifier the core. Each is tried on either side.
        layouts = [
            [("E", 0, 0), ("C", 1, 0), ("1", 1, 1), ("2", 1, -1), ("V", 2, 0), ("V", 2, 1), ("V", 2, -1), ("P", 3, 0), ("P", 3, 1), ("P", 3, -1)],
            [("E", 0, 0), ("C", 1, 0), ("1", 1, 1), ("2", 1, -1), ("V", 2, 0), ("P", 3, 0), ("P", 2, 1), ("P", 2, -1)],
            [("E", 0, 0), ("C", 1, 0), ("1", 1, 1), ("V", 2, 0), ("V", 2, 1), ("P", 3, 0), ("P", 3, 1)],
            [("E", 0, 0), ("C", 1, 0), ("1", 1, 1), ("V", 2, 0), ("P", 3, 0), ("P", 2, 1)],
            [("E", 0, 0), ("C", 1, 0), ("1", 0, 1), ("V", 1, 1), ("P", 1, 2), ("P", 2, 1), ("V", 2, 0), ("P", 3, 0)],
        ]
        names = {"E": "LaserEmitter", "C": "LaserCore", "1": m1, "2": m2, "V": "LaserCavity", "P": "LaserPump"}
        for layout in layouts:
            for flip in (1, -1):
                at = lambda back, side: (c[0] - fx * back + sx * side * flip, c[1] - fy * back + sy * side * flip)
                plan = [(names[k], at(b, sd)) for k, b, sd in layout]
                if all(p == c or free_or_clearable(s, p) for _, p in plan):
                    for t, p in plan:
                        force(s, t, p, old["rotation"] if t == "LaserEmitter" else "North")
                    done += 1
                    break
            else:
                continue
            break
    return done


def main():
    report = []
    for slug, ships in RULES.items():
        for key, rules in ships.items():
            path = os.path.join(FACTIONS, slug, key + ".json")
            s = kit.Ship(path)
            if any(m["module_type"] in NEW for m in s.d["modules"]):
                report.append(f"{slug}/{key}: already armed, left alone")
                continue
            did = []
            point_defence = False
            for what, kind, n, arg in rules:
                got = {"pads": pads, "ciws": ciws, "laser": laser}[what](s, kind, n, arg)
                did.append(f"{what} {got}/{n} from {kind}")
                point_defence |= what == "ciws" and got > 0
            s.save()
            if point_defence:
                add_warners.main(path, 4 if len(s.d["hull_cells"]) > 600 else 2)
            report.append(f"{slug}/{key}: " + ", ".join(did))
    print("\n".join(report))


if __name__ == "__main__":
    main()
