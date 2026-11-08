"""Shared helpers for the arming scripts: footprints, free cells, placing parts.

Footprints come from src/building/registry.rs, so a 1x3 Rail Launcher or a
2x2 VLS Silo is checked against every cell it covers, the way the game does
(building::GridOccupancy::cells_for). Turret-deck parts don't take hull cells.
"""
import json, os, re

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))


def _sizes():
    src = open(os.path.join(ROOT, "src/building/registry.rs")).read()
    sizes = {}
    for m in re.finditer(r"defs\.insert\(ModuleType::(\w+),", src):
        s = re.search(r"size: IVec2::new\((\d+),\s*(\d+)\)", src[m.end(): m.end() + 1500])
        if s:
            sizes.setdefault(m.group(1), (int(s.group(1)), int(s.group(2))))
    return sizes


SIZES = _sizes()


def rotate(r, x, y):
    return {"North": (x, y), "East": (y, -x), "South": (-x, -y), "West": (-y, x)}[r]


def cells_of(t, at, r):
    w, h = SIZES.get(t, (1, 1))
    out = []
    for i in range(w):
        for j in range(h):
            dx, dy = rotate(r, i, j)
            out.append((at[0] + dx, at[1] + dy))
    return out


def module(t, x, y, r="North", extras=None):
    return {"module_type": t, "grid_pos": [x, y], "rotation": r, "custom_name": None, "subcomponents": None, "extras": extras}


class Ship:
    def __init__(self, path):
        self.path = path
        self.text = open(path).read()
        self.d = json.loads(self.text)
        self.hull = {tuple(c["grid_pos"]): c["layer"] for c in self.d["hull_cells"]}

    def on_deck(self, m):
        return bool((m.get("extras") or {}).get("turret"))

    def taken(self):
        t = {}
        for m in self.d["modules"]:
            if not self.on_deck(m):
                for c in cells_of(m["module_type"], m["grid_pos"], m["rotation"]):
                    t[c] = m
        return t

    def at(self, cell):
        return self.taken().get(tuple(cell))

    def remove(self, m):
        self.d["modules"].remove(m)

    def fits(self, t, cell, r="North"):
        taken = self.taken()
        return all(c in self.hull and self.hull[c] != "Hallway" and c not in taken for c in cells_of(t, cell, r))

    def place(self, t, cell, r="North", extras=None):
        assert self.fits(t, cell, r), f"{t} doesn't fit at {cell} facing {r}"
        self.d["modules"].append(module(t, cell[0], cell[1], r, extras))

    def replace(self, cell, t, r=None, extras=None, kinds=None):
        """The module covering `cell` gives way to `t`, at that module's origin."""
        old = self.at(cell)
        assert old is not None, f"nothing at {cell}"
        assert kinds is None or old["module_type"] in kinds, f"{cell} holds {old['module_type']}"
        self.remove(old)
        self.place(t, tuple(old["grid_pos"]), r or old["rotation"], extras)
        return old["module_type"]

    def free_near(self, cell, avoid=(), reach=1):
        """Free non-hallway hull cells touching `cell` (8-way), nearest first."""
        taken = self.taken()
        out = []
        for dx in range(-reach, reach + 1):
            for dy in range(-reach, reach + 1):
                c = (cell[0] + dx, cell[1] + dy)
                if c != tuple(cell) and c in self.hull and self.hull[c] != "Hallway" and c not in taken and c not in avoid:
                    out.append(c)
        return sorted(out, key=lambda c: (abs(c[0] - cell[0]) + abs(c[1] - cell[1]), c))

    def fit_gun(self, design, turret=None, at=(0, 0), facing="East"):
        self.d.setdefault("guns", []).append({"design": design, "at": list(at), "facing": facing, "turret": list(turret) if turret else None})

    def count(self, t):
        return sum(1 for m in self.d["modules"] if m["module_type"] == t)

    def save(self, path=None):
        indent = 1 if self.text.startswith('{\n "') else 2
        with open(path or self.path, "w") as f:
            json.dump(self.d, f, indent=indent)
            f.write("\n")


def write_gun(name, gun):
    path = os.path.join(ROOT, "designs/guns", name + ".json")
    with open(path, "w") as f:
        json.dump(gun, f, indent=2)
    return path
