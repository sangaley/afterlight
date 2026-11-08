"""Build the Fleet Concepts page from every faction built so far:
python3 page.py terran [next_faction ...]"""
import html, json, os, sys
import build, cards, lineup

HERE = os.path.dirname(os.path.abspath(__file__))
PINCER_HULL = 210

UPCOMING = []

def weapons_line(w):
    names = {"Cannon": "cannon", "Gatling": "gatling", "Railgun": "railgun", "HeavyMissile": "missile",
             "PointDefenseDrone": "PD drone", "Laser": "laser", "PlasmaCaster": "plasma",
             "IonDisruptor": "ion", "TractorBeam": "tractor", "ClusterRocket": "rocket pod"}
    parts = [f"{n} {names.get(k, k)}{'s' if n > 1 and not names.get(k, k).endswith('s') else ''}"
             for k, n in sorted(w.items(), key=lambda kv: -kv[1])]
    return ", ".join(parts) if parts else "unarmed"

def main(mods):
    factions = []
    files = {}
    for m in mods:
        mod, results = build.run(m)
        slug = mod.FACTION["slug"]
        items = []
        for key, d, st, checks, meta in results:
            name, cls, note = meta
            img = f"img/{slug}/{key}.png"
            os.makedirs(os.path.join(HERE, "site", "img", slug), exist_ok=True)
            w, h = cards.card(d, os.path.join(HERE, "site", img))
            files[img] = os.path.join("site", img)
            items.append({
                "id": f"{slug}__{key}", "name": name, "cls": cls, "note": note, "img": img,
                "iw": w, "ih": h, "hull": st["hull"], "x": round(st["hull"] / PINCER_HULL, 1),
                "mods": st["modules"], "plates": st["plates"], "w": st["w"], "h": st["h"],
                "weapons": weapons_line(st["weapons"]), "engines": st["engines"],
                "reactors": st["reactors"], "shields": st["shields"], "ok": not checks,
                "revised": getattr(mod, "REVISED", {}).get(key, ""),
                "badge": getattr(mod, "REVISED_LABEL", ""),
            })
        lu = f"img/{slug}/_lineup.png"
        order = [os.path.join(HERE, "concepts", slug, f"{k}.json") for k, *_ in mod.CONCEPTS]
        lineup.lineup(slug, px=6, order=order, out=os.path.join(HERE, "site", lu), width=1500)
        files[lu] = os.path.join("site", lu)
        factions.append({**mod.FACTION, "items": items, "lineup": lu})
    data = json.dumps({"factions": factions, "upcoming": UPCOMING})
    tpl = open(os.path.join(HERE, "page.html")).read()
    open(os.path.join(HERE, "site", "fleet-concepts.html"), "w").write(tpl.replace("/*DATA*/null", data))
    json.dump(files, open(os.path.join(HERE, "site", "files.json"), "w"), indent=1)
    print("files:", len(files))

if __name__ == "__main__":
    main(sys.argv[1:])
