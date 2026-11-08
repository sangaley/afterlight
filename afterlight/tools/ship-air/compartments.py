"""Divide a ship's air: emergency doors across its corridors, an oxygen tank per
compartment. Only ever uses bare corridor tiles, so what was drawn stays put, and
adds nothing to a ship that already meets the target, so it can be run again.

    python3 tools/ship-air/compartments.py [--write] [--show] <design.json> ...

Run from afterlight/. Without --write it only reports. Air is worked out as in
`building::rooms` and `ai_ship::air`: corridors and modules, less inner hull and doors.
"""
import argparse, json, os, re, sys
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "faction-arming"))
from kit import cells_of

DOORS = ("EmergencyBulkhead", "AirlockValve")
ROOT = os.path.join(os.path.dirname(__file__), "..", "..")
# Largest compartment to aim for: this share of the air, or this many tiles.
TARGET_SHARE, TARGET_MIN = 0.15, 48
# Widest corridor worth a door, and the smallest part worth shutting off.
MAX_WIDTH, MIN_PART = 10, 12
TILES_PER_TANK, MAX_TANKS = 80, 4


def no_air():
    s = open(os.path.join(ROOT, "src/components.rs")).read()
    body = re.search(r"pub fn holds_atmosphere\(&self\) -> bool \{\s*!matches!\(\s*self,(.*?)\)\s*\}", s, re.S).group(1)
    return set(re.findall(r"ModuleType::(\w+)", body))


NO_AIR = no_air()


def side(p):
    return ((p[0] + 1, p[1]), (p[0] - 1, p[1]), (p[0], p[1] + 1), (p[0], p[1] - 1))


def parts(cells):
    left, out = set(cells), []
    while left:
        stack = [left.pop()]
        part = set(stack)
        while stack:
            for n in side(stack.pop()):
                if n in left:
                    left.discard(n)
                    part.add(n)
                    stack.append(n)
        out.append(part)
    return sorted(out, key=len, reverse=True)


class Ship:
    def __init__(self, path):
        self.path = path
        self.d = json.load(open(path))
        self.enemy = "factions" in path
        self.hull = {tuple(c["grid_pos"]): c["layer"] for c in self.d["hull_cells"]}
        # Parts on a turret deck are on that turret's grid, not the hull's.
        self.d_hull = [m for m in self.d["modules"] if not (m.get("extras") or {}).get("turret")]
        self.taken = set()
        for m in self.d_hull:
            self.taken.update(map(tuple, cells_of(m["module_type"], m["grid_pos"], m["rotation"])))

    def air(self):
        """Air with every door shut, as the game would flood it."""
        hall = {p for p, l in self.hull.items() if l == "Hallway"}
        inner = {p for p, l in self.hull.items() if l == "Inner"}
        doors, mods = set(), set()
        for m in self.d_hull:
            t = m["module_type"]
            if t in DOORS:
                doors.update(map(tuple, cells_of(t, m["grid_pos"], m["rotation"])))
            elif t not in NO_AIR:
                if self.enemy:
                    mods.update(map(tuple, cells_of(t, m["grid_pos"], m["rotation"])))
                else:
                    mods.add(tuple(m["grid_pos"]))
        return (hall | mods) - doors - inner

    def bare(self, p):
        return self.hull.get(p) == "Hallway" and p not in self.taken

    def add(self, t, p):
        m = {"module_type": t, "grid_pos": list(p), "rotation": "North", "custom_name": None, "subcomponents": None, "extras": None}
        self.d["modules"].append(m)
        self.d_hull.append(m)
        self.taken.add(p)

    def runs(self, cells):
        """Every straight line of bare corridor from wall to wall."""
        for axis in (0, 1):
            lines = {}
            for p in cells:
                lines.setdefault(p[1 - axis], []).append(p[axis])
            for at, xs in lines.items():
                xs.sort()
                start = 0
                for i in range(1, len(xs) + 1):
                    if i == len(xs) or xs[i] != xs[i - 1] + 1:
                        run = [(x, at) if axis == 0 else (at, x) for x in xs[start:i]]
                        start = i
                        if len(run) <= MAX_WIDTH and all(self.bare(p) for p in run):
                            yield run

    def divide(self):
        added = 0
        air = self.air()
        target = max(TARGET_MIN, TARGET_SHARE * len(air))
        while True:
            rooms = parts(air)
            if not rooms or len(rooms[0]) <= target:
                return added
            room = rooms[0]
            best = None
            for run in self.runs(room):
                pieces = parts(room - set(run))
                if len(pieces) >= 2 and len(pieces[1]) >= MIN_PART:
                    score = (len(pieces[1]) / len(room)) / len(run) ** 0.5
                    if best is None or score > best[0]:
                        best = (score, run)
            if best is None:
                return added
            # Ships are drawn mirrored about y = 1/2: door both sides alike.
            run = best[1]
            mirror = [(x, 1 - y) for x, y in run]
            both = set(run) | set(mirror) if all(self.bare(p) for p in mirror) else set(run)
            for p in both:
                self.add("EmergencyBulkhead", p)
            added += len(both)
            air = self.air()

    def tanks(self):
        added = 0
        air = self.air()
        have = [tuple(m["grid_pos"]) for m in self.d_hull if m["module_type"] == "OxygenTank"]
        for room in parts(air):
            if len(room) < MIN_PART:
                continue
            want = min(MAX_TANKS, 1 + len(room) // TILES_PER_TANK) - sum(any(c in room for c in (p, *side(p))) for p in have)
            floor = {p for p in room if self.hull.get(p) == "Hallway"}
            spots = sorted((p for p in floor if self.bare(p)), key=lambda p: (-sum(n not in room for n in side(p)), p))
            for p in spots:
                if want <= 0:
                    break
                # Never where it would cut the corridor in two.
                before = len(parts(floor))
                if len(parts(floor - {p})) > before:
                    continue
                self.add("OxygenTank", p)
                floor.discard(p)
                want -= 1
                added += 1
        return added

    def show(self):
        kinds = {tuple(m["grid_pos"]): m["module_type"] for m in self.d_hull}
        xs = [p[0] for p in self.hull]
        ys = [p[1] for p in self.hull]
        sym = {"Outer": "#", "Inner": "+", "Hallway": ".", "BulkheadDoor": "D", "Void": " "}
        for y in range(max(ys), min(ys) - 1, -1):
            row = ""
            for x in range(min(xs), max(xs) + 1):
                t = kinds.get((x, y))
                row += "E" if t in DOORS else "O" if t == "OxygenTank" else "m" if (x, y) in self.taken else sym.get(self.hull.get((x, y)), " ")
            print(f"{y:5} {row}")


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--write", action="store_true")
    p.add_argument("--show", action="store_true")
    p.add_argument("designs", nargs="+")
    a = p.parse_args()
    for path in a.designs:
        ship = Ship(path)
        before = [len(r) for r in parts(ship.air())][:4]
        doors = ship.divide()
        tanks = ship.tanks()
        after = [len(r) for r in parts(ship.air())]
        print(f"{os.path.basename(path)[:26]:26} air {sum(after):5} largest {before} -> {after[:4]} in {len(after):3} | +{doors} door cells, +{tanks} tanks")
        if a.show:
            ship.show()
        if a.write and (doors or tanks):
            with open(path, "w") as f:
                json.dump(ship.d, f, indent=1)


if __name__ == "__main__":
    main()
