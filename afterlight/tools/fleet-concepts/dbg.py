"""Print a design as a grid: lower-case letters/'.' = interior, '#' = rim, '/' plate.
python3 dbg.py terran warden [x0 x1 y0 y1]"""
import importlib, sys, json
import fleet
mod = importlib.import_module(sys.argv[1])
fn = dict((k, f) for k, f, *_ in mod.CONCEPTS)[sys.argv[2]]
d = fn(); d.pop("_checks")
hull = {tuple(c["grid_pos"]): c["layer"] for c in d["hull_cells"]}
occ = {}
for m in d["modules"]:
    for c in fleet.footprint(m["module_type"], tuple(m["grid_pos"]), m["rotation"]):
        occ[c] = m["module_type"]
xs = [c[0] for c in hull]; ys = [c[1] for c in hull]
x0, x1, y0, y1 = (map(int, sys.argv[3:7]) if len(sys.argv) > 6 else (min(xs) - 1, max(xs) + 1, min(ys) - 1, max(ys) + 1))
for y in range(y1, y0 - 1, -1):
    row = ""
    for x in range(x0, x1 + 1):
        c = (x, y)
        if c in occ and occ[c] in ("AngledArmorPlate",): row += "/"
        elif c in occ: row += occ[c][0] if hull.get(c) != "Outer" else occ[c][0].lower()
        elif c in hull: row += "." if hull[c] == "Hallway" else "#" if hull[c] == "Outer" else "+"
        else: row += " "
    print(f"{y:4d} {row}")
