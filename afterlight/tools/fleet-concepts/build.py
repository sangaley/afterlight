"""Generate, check and render one faction's concepts: python3 build.py terran"""
import importlib, json, os, sys
import fleet, gen

def run(mod_name, px=14):
    mod = importlib.import_module(mod_name)
    slug = mod.FACTION["slug"]
    out = os.path.join(os.path.dirname(__file__), "concepts", slug)
    os.makedirs(out, exist_ok=True)
    results = []
    for key, fn, *meta in mod.CONCEPTS:
        try:
            d = fn()
        except gen.ShipError as e:
            print(f"{key}: ERROR {e}")
            continue
        checks = d.pop("_checks")
        st = gen.stats(d)
        with open(os.path.join(out, f"{key}.json"), "w") as f:
            json.dump(d, f, indent=1)
        fleet.render(d, px=px, background=(12, 16, 25, 255)).save(os.path.join(out, f"{key}.png"))
        print(f"{key}: {st['hull']} hull {st['modules']} mods {st['plates']} plates {st['w']}x{st['h']}"
              + ("" if not checks else f"  PROBLEMS: {checks}"))
        results.append((key, d, st, checks, meta))
    return mod, results

if __name__ == "__main__":
    run(sys.argv[1])
