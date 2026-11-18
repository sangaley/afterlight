#!/usr/bin/env python3
"""Refit hand-drawn concepts into game designs, the way the current fleet was.

A concept (tools/fleet-concepts/concepts/<slug>/<key>.json, drawn at concept
scale with the old one-block weapons as placeholders) goes through the same
steps as every ship in the game did (tools/ship-scale/scale.py):

- drawn twice as big (stations three times), every cell a 2x2 block;
- the old weapons swapped for the builder's pre-made weapons in the
  faction's style: autocannon turrets, 170 mm and twin 225 mm turrets, VLS
  silos with the faction's missile, laser units, CIWS point defence;
- point defence on every armed ship (about one CIWS per 400 cells), and
  Missile Warners with a decoy and chaff beside one.

Then what this rework adds for each faction (STYLE): its own missile design,
its armour (the newer ablative and reflective plates where it suits), and a
paint job of its own -- three colours and decals -- carried in the design
(Blueprint.paint), so the portrait shows the ship as it will look.

Writes designs/rework/<slug>/<key>.json. Nothing here is loaded by the game's
rosters; these are for the review site until they're approved.

    python3 tools/fleet-rework/refit.py <slug> [key ...]
"""
import json, os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
sys.path.insert(0, os.path.join(ROOT, "tools", "ship-scale"))
sys.path.insert(0, os.path.join(ROOT, "tools", "faction-arming"))
import kit    # noqa: E402
import scale  # noqa: E402

CONCEPTS = os.path.join(ROOT, "tools", "fleet-concepts", "concepts")
OUT = os.path.join(ROOT, "designs", "rework")

# Each faction's own touches on top of the shared refit.
STYLE = {
    "recursive_kingdom": {
        # A salvo of cheap junk rockets: no seeker, high explosive and frag.
        "missile": {"motor": "Booster", "body": ["Fins", "He", "Frag"], "head": "DumbNose"},
        # Scrap plate: stops one shell whatever its size, then breaks.
        "armour": {"ArmorPlate": "AblativeArmor", "AngledArmorPlate": "AngledAblativeArmor"},
        # Sun-bleached scrap, oxidised panels, safety-orange trim.
        "paint": {"main": [0.55, 0.49, 0.40], "second": [0.33, 0.40, 0.36], "trim": [0.88, 0.54, 0.24]},
        # Every copy is numbered: chevrons down the spine, pointing at the bow.
        "decals": {"spine": "Chevron", "every": 6, "colour": [224, 138, 60]},
    },
}

KITS = {
    "autocannon": "Light Autocannon",
    "ciws": "CIWS Turret",
    "compact": "Compact 170 mm Turret",
    "turret": "170 mm Turret",
    "twin": "Twin 225 mm Turret",
    "vls": "VLS Missile Silo",
    "spinal": "280 mm Spinal Gun",
}


def decals(design, style):
    """Chevrons down the middle of the hull, every few cells, bow-ward."""
    d = style.get("decals")
    if not d:
        return []
    hull = {tuple(c["grid_pos"]) for c in design["hull_cells"]}
    taken = set()
    for m in design["modules"]:
        if not (m.get("extras") or {}).get("turret"):
            taken.update(kit.cells_of(m["module_type"], tuple(m["grid_pos"]), m.get("rotation", "North")))
    xs = sorted({x for x, y in hull if y in (0, -1)})
    out = []
    for x in xs[2:-2:d["every"]]:
        for y in (0, -1):
            if (x, y) in hull and (x, y) not in taken:
                # The art is a V pointing down; a quarter turn points it at the bow.
                out.append({"cell": [x, y], "shape": d["spine"], "turn": 1, "flip": False, "colour": d["colour"], "layer": 0})
    return out


def refit(slug, key, station=False):
    src = json.load(open(os.path.join(CONCEPTS, slug, f"{key}.json")))
    src.pop("_checks", None)
    style = STYLE.get(slug, {})
    if style.get("missile"):
        scale.MISSILES[slug] = style["missile"]
    kits = {k: scale.load_kit(v) for k, v in KITS.items()}
    own = dict(kits)
    guns = scale.FACTION_GUNS.get(slug, {})
    if "ciws" in guns:
        own["ciws"] = scale.with_gun(kits["ciws"], guns["ciws"])
    if "turret" in guns:
        own["turret"] = scale.with_gun(kits["turret"], guns["turret"])
    s = scale.Scaled(src, slug, 3 if station else 2)
    s.scale()
    armed = bool(s.slots) or s.exotic
    s.arm(own)
    cells = len(src["hull_cells"])
    if armed and slug != "the_silence":
        s.point_defence(own, 8 if station else max(1, min(8, round(len(s.hull) / 400))))
        s.warners(2 if cells < 300 else 4)
    out = s.design()
    swapped = 0
    for m in out["modules"]:
        new = style.get("armour", {}).get(m["module_type"])
        if new:
            m["module_type"] = new
            swapped += 1
    if style.get("paint"):
        p = style["paint"]
        out["paint"] = {"main": p["main"], "second": p["second"], "trim": p["trim"], "warm": False, "slots": [],
                        "decals": decals(out, style)}
    os.makedirs(os.path.join(OUT, slug), exist_ok=True)
    path = os.path.join(OUT, slug, f"{key}.json")
    json.dump(out, open(path, "w"), indent=1)
    report = {k: n for k, n in s.report.items() if n}
    if swapped:
        report["armour refitted"] = swapped
    # What the review site lists for it.
    json.dump({"concept_hull": cells, "hull": len(out["hull_cells"]), "blocks": len(out["hull_cells"]) + len(out["modules"]),
               "report": report, "missile": style.get("missile"), "armour": style.get("armour")},
              open(os.path.join(OUT, slug, f"{key}.report.json"), "w"), indent=1)
    return path, cells, len(out["hull_cells"]), len(out["modules"]), report


def main():
    slug = sys.argv[1]
    keys = sys.argv[2:] or sorted(f[:-5] for f in os.listdir(os.path.join(CONCEPTS, slug)) if f.endswith(".json"))
    stations = {"hive"}
    for key in keys:
        path, a, b, n, report = refit(slug, key, key in stations)
        line = ", ".join(f"{v} {k}" for k, v in sorted(report.items()))
        print(f"{slug}/{key}: {a} -> {b} hull, {n} blocks | {line or 'unarmed'}")


if __name__ == "__main__":
    main()
