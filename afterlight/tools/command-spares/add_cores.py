"""Gives the big faction ships spare command: two AI Combat Cores (or a spare
Helm Station where only one cell can be had), each
placed as far from the bridge and from each other as the hull allows, at
least two cells in from the outer hull. These ships have no empty interior,
so a core takes the place of filler (see FILLER) lying wholly inside its
cells, and never a corridor.

A ship strikes its colours when every bridge, helm and AI core it was built
with is gone (ai_ship::combat::check_ai_cripple). With all of them in one
bridge cluster, two hits next to each other ended a capital.

Run from afterlight/: python3 tools/command-spares/add_cores.py [--dry-run]
Re-running is safe: a ship that already has three command spots is left alone.
"""
import json, os, re, sys

CRATE = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
SHIPS = [
    # capitals and the boss (ai_ship::roster)
    "terran_hegemony/sovereign", "gilded_throne/throne_barge", "broken_choir/requiem",
    "corpse_stars/dead_sun", "stellar_preserve/seed_ark", "shepherd/the_shepherd",
    # the original dreadnought, standing in as the Eternal Hegemony's boss
    "eternal_hegemony",
    # faction stations (diplomacy::stations)
    "terran_hegemony/bastion", "eternal_hegemony/citadel", "gilded_throne/exchange",
    "broken_choir/broken_spire", "synthesis_collective/growth_node", "corpse_stars/reliquary",
    "stellar_preserve/sanctuary", "the_silence/monastery", "shepherd/the_fold",
    "merchant_princes/trade_post", "free_haulers_compact/depot",
]
COMMAND = {"HelmStation", "BridgeWing", "AICombatCore"}
# Blocks a ship can spare a cell of: berths, small cargo, and -- for hulls
# with none of those far from the bridge -- a memory core, a med bay or one
# of several oxygen scrubbers.
FILLER = {"BasicQuarters", "SmallCargo", "Barracks", "MemoryCore", "MedBay", "OxygenScrubber"}
SPARES = 2
MIN_APART = 6   # cells: closer than this is the same hit, not a spare
DEPTH = 1       # cells of ship between a core and open space (these hulls run outer
                # walls through their interiors, so 2 left almost nowhere)

reg = open(os.path.join(CRATE, "src/building/registry.rs")).read()
SIZE = {}
for m in re.finditer(r'defs\.insert\(ModuleType::(\w+), ModuleDef \{(.*?)\}\);', reg, re.S):
    s = re.search(r'size: IVec2::new\((\d+), (\d+)\)', m.group(2))
    SIZE[m.group(1)] = (int(s.group(1)), int(s.group(2))) if s else (1, 1)


def rot(r, x, y):
    return {"North": (x, y), "East": (y, -x), "South": (-x, -y), "West": (-y, x)}[r]


def cells(t, x, y, r):
    w, h = SIZE.get(t, (1, 1))
    return [(x + rot(r, i, j)[0], y + rot(r, i, j)[1]) for i in range(w) for j in range(h)]


def apart(a, b):
    return max(abs(a[0] - b[0]), abs(a[1] - b[1]))


def clusters(cs):
    """Command cells grouped into spots: cells closer than MIN_APART are one."""
    groups = []
    for c in cs:
        near = [g for g in groups if any(apart(c, d) < MIN_APART for d in g)]
        groups = [g for g in groups if g not in near] + [[c] + [d for g in near for d in g]]
    return groups


def plan(design):
    """[(origin, rotation, cells, indices of the filler modules it replaces, block)]"""
    hull = {tuple(c["grid_pos"]): c["layer"] for c in design["hull_cells"]}
    taken = set()
    filler = {}   # cell -> (index of the filler module, all its cells)
    command = []
    for i, m in enumerate(design["modules"]):
        if (m.get("extras") or {}).get("turret") is not None:
            continue
        cs = cells(m["module_type"], *m["grid_pos"], m["rotation"])
        if m["module_type"] in FILLER:
            for c in cs:
                filler[c] = (i, frozenset(cs))
        else:
            taken |= set(cs)
        if m["module_type"] in COMMAND:
            command += cs
    if not command or len(clusters(command)) >= 3:
        return []

    def deep(c):
        return all(hull.get((c[0] + dx, c[1] + dy)) not in (None, "Outer")
                   for dx in range(-DEPTH, DEPTH + 1) for dy in range(-DEPTH, DEPTH + 1))

    # An AI core where two cells can be had; a spare helm (one cell) where
    # only one can.
    options = []
    for (x, y) in hull:
        for block, r in (("AICombatCore", "North"), ("AICombatCore", "East"), ("HelmStation", "North")):
            cs = cells(block, x, y, r)
            # Free, or filler that lies wholly inside these cells.
            if all(hull.get(c) == "Inner" and c not in taken and deep(c)
                   and (c not in filler or filler[c][1] <= set(cs)) for c in cs):
                options.append(((x, y), r, cs, {filler[c][0] for c in cs if c in filler}, block))
    placed = []
    anchors = list(command)
    used = set()
    for _ in range(SPARES):
        live = [o for o in options if not set(o[2]) & used]
        if not live:
            break
        best = max(live, key=lambda o: (min(apart(a, b) for a in o[2] for b in anchors), o[4] == "AICombatCore"))
        if min(apart(a, b) for a in best[2] for b in anchors) < MIN_APART:
            break
        placed.append(best)
        used |= set(best[2])
        anchors += best[2]
    return placed


if __name__ == "__main__":
    dry = "--dry-run" in sys.argv
    for name in SHIPS:
        path = os.path.join(CRATE, "designs/factions", name + ".json")
        text = open(path).read()
        design = json.loads(text)
        placed = plan(design)
        print(f"{name}:" + ("" if placed else " nothing to add"))
        for (x, y), r, _, gone, block in placed:
            print("   ", block, (x, y), r, "replaces", [design["modules"][i]["module_type"] for i in sorted(gone)] or "nothing")
        if placed and not dry:
            drop = {i for *_, gone, _ in placed for i in gone}
            design["modules"] = [m for i, m in enumerate(design["modules"]) if i not in drop]
            for (x, y), r, _, _, block in placed:
                design["modules"].append({"module_type": block, "grid_pos": [x, y], "rotation": r,
                                          "custom_name": None, "subcomponents": None, "extras": None})
            indent = 1 if text.startswith('{\n "') else 2
            with open(path, "w") as f:
                json.dump(design, f, indent=indent)
