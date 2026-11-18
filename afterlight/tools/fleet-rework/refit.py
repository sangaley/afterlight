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

Then what this rework adds for each faction (STYLE): its own missile design
and its armour (the newer ablative and reflective plates where it suits).
Every ship is painted in its faction's in-game colours and pattern (LIVERY),
carried in the design (Blueprint.paint), so the portrait shows it as it will
look.

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

# Every faction's colours, as the game paints it (src/ai_ship/livery.rs and
# components.rs::faction_map_color): (map colour, accent, accent pattern). The
# portraits wear the same scheme (vfx::ship_roof::faction_paint), so a faction
# looks the same on the map, in play and on the review site.
LIVERY = {
    "terran_hegemony": ((0.65, 0.7, 0.8), (0.93, 0.95, 0.98), "Bands"),
    "eternal_hegemony": ((0.95, 0.2, 0.2), (0.95, 0.78, 0.3), "Chevrons"),
    "gilded_throne": ((0.4, 0.45, 0.65), (0.95, 0.8, 0.35), "Bow"),
    "broken_choir": ((0.6, 0.8, 0.65), (0.85, 0.9, 0.85), "Checker"),
    "synthesis_collective": ((0.75, 0.35, 0.95), (0.3, 0.9, 0.8), "Stripe"),
    "corpse_stars": ((0.55, 0.35, 0.9), (0.9, 0.86, 0.75), "Chevrons"),
    "stellar_preserve": ((0.25, 0.85, 0.75), (0.95, 0.97, 0.95), "Split"),
    "the_silence": ((0.9, 0.92, 0.95), (0.25, 0.27, 0.3), "Bands"),
    "shepherd": ((1.0, 0.85, 0.2), (0.5, 0.33, 0.2), "Bow"),
    "recursive_kingdom": ((0.95, 0.55, 0.2), (0.18, 0.16, 0.15), "Chevrons"),
    "merchant_princes": ((0.5, 0.75, 0.4), (0.93, 0.9, 0.75), "Bands"),
    "free_haulers_compact": ((0.8, 0.45, 0.3), (0.95, 0.82, 0.2), "Stripe"),
}

# Each faction's own touches on top of the shared refit.
STYLE = {
    "recursive_kingdom": {
        # A salvo of cheap junk rockets: no seeker, high explosive and frag.
        "missile": {"motor": "Booster", "body": ["Fins", "He", "Frag"], "head": "DumbNose"},
        # Scrap plate: stops one shell whatever its size, then breaks.
        "armour": {"ArmorPlate": "AblativeArmor", "AngledArmorPlate": "AngledAblativeArmor"},
    },
}


def accent_at(pattern, cell, lo, hi):
    """Livery::accent_at, the same cells the game paints in the accent."""
    x, y = cell
    from_bow = hi[0] - x
    off = y - (lo[1] + hi[1]) * 0.5
    if pattern == "Bands":
        return x % 6 < 2
    if pattern == "Chevrons":
        return (x + int(abs(off))) % 7 < 2
    if pattern == "Bow":
        return from_bow < max((hi[0] - lo[0] + 1) // 5, 2)
    if pattern == "Split":
        return off > 0
    if pattern == "Checker":
        return (x // 2 + y // 2) % 2 == 0
    if pattern == "Stripe":
        half = max((hi[1] - lo[1]) * 0.5, 1.0)
        return 0.45 <= abs(off) / half < 0.7
    return False


def paint(slug, design):
    """The faction's scheme (faction_paint: main its map colour a little
    greyed toward plating, second and trim its accent), with its pattern
    laid in the second colour."""
    base, accent, pattern = LIVERY[slug]
    grey = (0.6, 0.6, 0.62)
    main = [round(g + (b - g) * 0.62, 4) for g, b in zip(grey, base)]
    cells = [tuple(c["grid_pos"]) for c in design["hull_cells"]]
    lo = (min(c[0] for c in cells), min(c[1] for c in cells))
    hi = (max(c[0] for c in cells), max(c[1] for c in cells))
    slots = [[list(c), "Second"] for c in cells if accent_at(pattern, c, lo, hi)]
    return {"main": main, "second": [round(a * 0.9, 4) for a in accent], "trim": list(accent), "warm": False,
            "slots": slots, "decals": []}


KITS = {
    "autocannon": "Light Autocannon",
    "ciws": "CIWS Turret",
    "compact": "Compact 170 mm Turret",
    "turret": "170 mm Turret",
    "twin": "Twin 225 mm Turret",
    "vls": "VLS Missile Silo",
    "spinal": "280 mm Spinal Gun",
}


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
    out["paint"] = paint(slug, out)
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
