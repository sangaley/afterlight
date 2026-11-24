#!/usr/bin/env python3
"""Armour belts and emergency force fields for the faction stations.

The user's call (2026-11-24): every faction station carries its faction's
armour and emergency force fields. Three stations (Bastion, Growth Node,
Depot) had no plate at all and none had a force field.

Plating follows the fleet rule (memory: selective armour): not every outer
tile. Plate goes on
- the tips: the far ends of arms and prongs, found as edge cells with little
  hull around them (a convex point), and
- a belt over the vitals: outward faces within reach of a reactor, magazine,
  bridge or computer core.
Inner edges (faces turned toward the station's centre), notches and the
cells round docking ports, hangars and outboard guns stay bare.

A plate is flat on a straight face and angled in an L of hull, the same
corners fleet-concepts/draw.py angles. The material is the faction's
(refit.STYLE): ablative for the Kingdom and the Synthesis, reflective for
the Gilded Throne, steel for everyone else.

Force fields sit two or three cells inside the hull under the plated faces,
spread out so their reach (3.5 cells) doesn't overlap, picked in whole
rotations on a four-way station so it stays symmetric.

Works from designs/rework/<slug>/<key>.pre_armour.json (written on the first
run), so running it again gives the same result. Then run integrate.py.

    python3 tools/fleet-rework/station_armour.py
"""
import json, math, os, shutil, sys
from collections import deque

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
REWORK = os.path.join(ROOT, "designs", "rework")
sys.path.insert(0, os.path.join(ROOT, "tools", "faction-arming"))
import kit  # noqa: E402

STATIONS = {
    "recursive_kingdom": ["hive"],
    "terran_hegemony": ["bastion", "gun_platform"],
    "eternal_hegemony": ["citadel"],
    "gilded_throne": ["exchange", "velvet_rope"],
    "broken_choir": ["broken_spire"],
    "synthesis_collective": ["growth_node"],
    "corpse_stars": ["reliquary"],
    "stellar_preserve": ["sanctuary"],
    "the_silence": ["monastery"],
    "shepherd": ["the_fold"],
    "merchant_princes": ["trade_post"],
    "free_haulers_compact": ["depot"],
}
# refit.STYLE's armour, as (flat, angled).
PLATE = {
    "recursive_kingdom": ("AblativeArmor", "AngledAblativeArmor"),
    "synthesis_collective": ("AblativeArmor", "AngledAblativeArmor"),
    "gilded_throne": ("ReflectivePlate", "AngledReflectivePlate"),
}
STEEL = ("ArmorPlate", "AngledArmorPlate")

VITAL = {"SmallReactor", "StandardReactor", "LargeReactor", "FusionReactor", "Magazine", "AmmoBay",
         "HelmStation", "BridgeWing", "MemoryCore", "AICombatCore", "VlsSilo"}
KEEP_CLEAR = {"DockingPort", "Hangar", "SmallHangar", "LargeHangar", "TractorBeam"}

TIP_WINDOW = 4        # 9x9 window round an edge cell
TIP_FILL = 0.36       # below this much hull in the window, it's a point
VITAL_REACH = 7       # cells from a vital module to the face that covers it
CLEAR_REACH = 4       # cells kept bare round docks and hangars
FIELD_SPACING = 8     # Chebyshev cells between two force fields
COVER = 0.40          # at most this share of the outer edge plated, old plate included
N4 = ((1, 0), (-1, 0), (0, 1), (0, -1))


def angled_rot(n, s, w, e):
    if w and s and not n and not e:
        return "North"
    if w and n and not s and not e:
        return "East"
    if e and n and not w and not s:
        return "South"
    if e and s and not w and not n:
        return "West"
    return None


def distance_from(seeds, lo, hi):
    """Grid steps (8-way) from the nearest seed, over the bounding box."""
    dist = {c: 0 for c in seeds}
    q = deque(seeds)
    while q:
        x, y = q.popleft()
        d = dist[(x, y)] + 1
        for dx in (-1, 0, 1):
            for dy in (-1, 0, 1):
                c = (x + dx, y + dy)
                if c in dist or not (lo[0] <= c[0] <= hi[0] and lo[1] <= c[1] <= hi[1]):
                    continue
                dist[c] = d
                q.append(c)
    return dist


def mod(t, at, r="North"):
    return {"module_type": t, "grid_pos": [at[0], at[1]], "rotation": r,
            "custom_name": None, "subcomponents": None, "extras": None}


def armour(slug, key):
    path = os.path.join(REWORK, slug, f"{key}.json")
    orig = os.path.join(REWORK, slug, f"{key}.pre_armour.json")
    if not os.path.exists(orig):
        shutil.copy(path, orig)
    d = json.load(open(orig))

    hull = {tuple(h["grid_pos"]): h["layer"] for h in d["hull_cells"]}
    taken, vital, clear, outboard = set(), set(), set(), set()
    for m in d["modules"]:
        cells = kit.cells_of(m["module_type"], tuple(m["grid_pos"]), m.get("rotation", "North"))
        taken.update(cells)
        if m["module_type"] in VITAL:
            vital.update(cells)
        if m["module_type"] in KEEP_CLEAR:
            clear.update(cells)
        outboard.update(c for c in cells if c not in hull)
    xs = [c[0] for c in hull]
    ys = [c[1] for c in hull]
    pad = TIP_WINDOW + VITAL_REACH + 2
    lo, hi = (min(xs) - pad, min(ys) - pad), (max(xs) + pad, max(ys) + pad)
    centre = ((min(xs) + max(xs)) / 2, (min(ys) + max(ys)) / 2)

    to_vital = distance_from(list(vital), lo, hi) if vital else {}
    to_clear = distance_from(list(clear | outboard), lo, hi) if (clear or outboard) else {}

    flat_t, angled_t = PLATE.get(slug, STEEL)
    plates, plated_faces, candidates = [], [], []
    edge = 0
    for x in range(lo[0], hi[0] + 1):
        for y in range(lo[1], hi[1] + 1):
            c = (x, y)
            if c in hull or c in taken:
                continue
            n, s, w, e = ((x, y + 1) in hull, (x, y - 1) in hull, (x - 1, y) in hull, (x + 1, y) in hull)
            sides = n + s + w + e
            if sides == 0:
                continue
            edge += 1
            if sides > 2 or (n and s) or (w and e):
                continue  # a notch or a slot: bare
            # Which way the face looks: from the hull out through this cell.
            fx = (w - e)
            fy = (s - n)
            out = (x - centre[0], y - centre[1])
            if fx * out[0] + fy * out[1] <= 0:
                continue  # an inner edge, turned toward the centre
            if to_clear.get(c, 99) <= CLEAR_REACH:
                continue
            fill = sum((x + i, y + j) in hull for i in range(-TIP_WINDOW, TIP_WINDOW + 1)
                       for j in range(-TIP_WINDOW, TIP_WINDOW + 1)) / (2 * TIP_WINDOW + 1) ** 2
            tip = fill < TIP_FILL
            belt = to_vital.get(c, 99) <= VITAL_REACH
            if not (tip or belt):
                continue
            m = mod(angled_t, c, angled_rot(n, s, w, e)) if sides == 2 else mod(flat_t, c)
            # Tips first, then the faces nearest a vital. One priority is a
            # whole symmetric set, so the cap never takes half of one.
            want_first = (0, 0) if tip else (1, to_vital.get(c, 99))
            candidates.append((want_first, c, m))

    before = sum(1 for m in d["modules"] if "Armor" in m["module_type"] or "Plate" in m["module_type"])
    room = max(0, int(COVER * (edge + before)) - before)
    candidates.sort(key=lambda t: t[0])
    i = 0
    while i < len(candidates):
        j = i
        while j < len(candidates) and candidates[j][0] == candidates[i][0]:
            j += 1
        if len(plates) + (j - i) > room and plates:
            break
        for _, c, m in candidates[i:j]:
            plates.append(m)
            plated_faces.append(c)
        i = j

    # Force fields: free inner hull two or three cells in from a plated face.
    plated = set(plated_faces)
    to_plate = distance_from(plated_faces, lo, hi) if plated_faces else {}
    free = [c for c, layer in hull.items() if layer == "Inner" and c not in taken
            and 2 <= to_plate.get(c, 99) <= 3]
    four_way = all(rot(c, centre) in hull for c in hull)

    def orbit(c):
        if not four_way:
            return [c]
        o = [c]
        for _ in range(3):
            o.append(rot(o[-1], centre))
        return o

    want = max(4, min(16, len(hull) // 900))
    free_set = set(free)
    fields = []
    # Vitals first, then the tips; nearest the centre line of the arm first.
    free.sort(key=lambda c: (to_vital.get(c, 99), -math.hypot(c[0] - centre[0], c[1] - centre[1]), c))
    for c in free:
        if len(fields) >= want:
            break
        o = orbit(c)
        if len(set(o)) != len(o) or not all(p in free_set for p in o):
            continue
        if any(max(abs(p[0] - f[0]), abs(p[1] - f[1])) < FIELD_SPACING for p in o for f in fields):
            continue
        fields.extend(o)

    d["modules"] = d["modules"] + plates + [mod("EmergencyForceField", f) for f in fields]
    json.dump(d, open(path, "w"), indent=1)
    return {"edge": edge, "plates before": before, "plates added": len(plates),
            "edge plated": f"{(before + len(plates)) / max(edge, 1):.0%}", "fields": len(fields),
            "material": flat_t}


def rot(c, centre):
    """A quarter turn about the station's centre (which may sit on a half cell)."""
    cx2, cy2 = round(centre[0] * 2), round(centre[1] * 2)
    x, y = c
    return ((cx2 - (2 * y - cy2)) // 2, (cy2 + (2 * x - cx2)) // 2)


def main():
    for slug, keys in STATIONS.items():
        for key in keys:
            print(f"{slug:22} {key:13}", armour(slug, key))


if __name__ == "__main__":
    main()
