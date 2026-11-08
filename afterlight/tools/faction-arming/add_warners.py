"""Fit Missile Warners to a design that has point defence or decoys but no
warner (point defence and decoys act only on missiles a warner has seen).

python3 tools/faction-arming/add_warners.py <design.json> [count]

Each warner goes as far out as it can toward bow, stern, port and starboard
in turn, on a free deck cell or in place of a crew/cargo filler block, so
the ship sees all round. Refuses a design that already has a warner.
"""
import os, sys
import kit

FILLER = {"BasicQuarters", "SmallCargo", "LargeCargo", "MaintenanceLocker", "RecRoom", "Floodlight", "Searchlight"}


def main(path, count=4):
    s = kit.Ship(path)
    if s.count("MissileWarner"):
        sys.exit(f"{path} already has warners")
    taken = s.taken()
    spots = []
    for c, layer in s.hull.items():
        m = taken.get(c)
        if layer == "Hallway" or (m and (m["module_type"] not in FILLER or kit.SIZES.get(m["module_type"], (1, 1)) != (1, 1))):
            continue
        spots.append(c)
    directions = [(1, 0), (-1, 0), (0, 1), (0, -1)]
    placed = []
    for i in range(count):
        dx, dy = directions[i % 4]
        left = [c for c in spots if c not in placed]
        best = max(left, key=lambda c: (c[0] * dx + c[1] * dy, -abs(c[1] * dx + c[0] * dy)))
        m = s.at(best)
        if m:
            s.remove(m)
        s.place("MissileWarner", best)
        placed.append(best)
    s.save()
    print(path, "warners at", placed)


if __name__ == "__main__":
    main(os.path.join(kit.ROOT, sys.argv[1]) if not os.path.isabs(sys.argv[1]) else sys.argv[1], int(sys.argv[2]) if len(sys.argv) > 2 else 4)
