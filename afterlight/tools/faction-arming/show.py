"""Print a design as a grid: one character per cell, bow (+x) to the right.

python3 tools/faction-arming/show.py designs/halyard.json [Type=c ...]
Hull: '#' outer, '.' inner, ',' hallway (empty). Modules: first letter, or the
character given on the command line. Turret-deck parts aren't drawn.
"""
import json, sys

d = json.load(open(sys.argv[1]))
marks = dict(a.split("=") for a in sys.argv[2:])
hull = {tuple(c["grid_pos"]): c["layer"] for c in d["hull_cells"]}
mods = {}
for m in d["modules"]:
    if (m.get("extras") or {}).get("turret"):
        continue
    mods[tuple(m["grid_pos"])] = m["module_type"]
xs = [p[0] for p in hull]; ys = [p[1] for p in hull]
print("    " + "".join(str(abs(x) % 10) for x in range(min(xs), max(xs) + 1)))
for y in range(max(ys), min(ys) - 1, -1):
    row = ""
    for x in range(min(xs), max(xs) + 1):
        p = (x, y)
        if p in mods:
            t = mods[p]
            row += marks.get(t, t[0].lower())
        elif p in hull:
            row += {"Outer": "#", "Hallway": ","}.get(hull[p], ".")
        else:
            row += " "
    print(f"{y:3} {row}")
