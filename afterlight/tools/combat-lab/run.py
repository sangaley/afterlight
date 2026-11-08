"""Combat test campaign: each player loadout against each faction's patrol,
muted and offscreen, read back from the game's combat report.

    python3 tools/combat-lab/run.py <out_dir> [secs] [filter]

Loadouts (written to <out_dir>/designs): the starter as it ships (mixed),
guns only (no silos or laser), missiles only (gun turrets swapped for VLS
silos, CIWS kept), and the Halyard Mk II battleship. Each fight: the enemy
patrol 6 km off; the player selects it and holds fire (it fires first, so
factions that wouldn't pick the fight do). Writes <out_dir>/results.json and
results.md. Run from afterlight/.
"""
import json, os, re, subprocess, sys
from collections import Counter

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "faction-arming"))
import kit  # noqa: E402

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
LASER = {"LaserCore", "LaserCoupler", "LaserCavity", "LaserPump", "QSwitch", "FrequencyDoubler",
         "LaserDestabiliser", "LaserEmitter", "LaserLens", "BeamRelay"}
FACTIONS = ["EternalHegemony", "TerranHegemony", "GildedThrone", "BrokenChoir", "CorpseStars",
            "SynthesisCollective", "Shepherd", "RecursiveKingdom"]


def loadouts(out):
    os.makedirs(os.path.join(out, "designs"), exist_ok=True)
    base = json.load(open(os.path.join(ROOT, "designs/starter.json")))
    paths = {}

    def write(name, d):
        d = dict(d, name=name)
        p = os.path.join(out, "designs", name + ".json")
        json.dump(d, open(p, "w"))
        paths[name] = p

    write("mixed", base)
    write("guns", dict(base, modules=[m for m in base["modules"] if m["module_type"] not in LASER | {"VlsSilo"}]))
    # Missiles: every gun turret but the CIWS swapped for a VLS silo where
    # its ring and feed stood.
    mods = [m for m in base["modules"] if m["module_type"] not in LASER]
    pd_rings = {tuple(m["extras"]["turret"]) for m in mods
                if m["module_type"] == "FiringPiece" and (m.get("extras") or {}).get("point_defence")}
    gun_rings = [m for m in mods if m["module_type"].startswith("TurretRing") and tuple(m["grid_pos"]) not in pd_rings]
    keep = []
    freed = set()
    for m in mods:
        ex = m.get("extras") or {}
        if ex.get("turret") and tuple(ex["turret"]) not in pd_rings:
            continue
        near = any(max(abs(m["grid_pos"][0] - r["grid_pos"][0]), abs(m["grid_pos"][1] - r["grid_pos"][1])) <= 1 for r in gun_rings)
        if m in gun_rings or (near and m["module_type"] in ("TraverseMotor", "AmmoHoist", "Magazine")):
            freed |= set(kit.cells_of(m["module_type"], tuple(m["grid_pos"]), m["rotation"]))
            continue
        keep.append(m)
    taken = set()
    for m in keep:
        if not (m.get("extras") or {}).get("turret"):
            taken |= set(kit.cells_of(m["module_type"], tuple(m["grid_pos"]), m["rotation"]))
    silos = 0
    for c in sorted(freed):
        cells = [(c[0] + i, c[1] + j) for i in (0, 1) for j in (0, 1)]
        if all(x in freed and x not in taken for x in cells):
            keep.append(kit.module("VlsSilo", c[0], c[1]))
            taken |= set(cells)
            silos += 1
    write("missiles", dict(base, modules=keep))
    paths["halyard"] = os.path.join(ROOT, "designs/halyard_mk2.json")
    print(f"missiles loadout: {silos} extra silos")
    return paths


LINE = re.compile(r"COMBAT t=(\d+) player hull=([\d.]+) blocks=(\d+)/(\d+) power=(\d+)/(\d+) crew=(\d+) \| (.*)")
ENEMY = re.compile(r"(\w+)/(\w+|-) hull=([\d.]+) blocks=(\d+)/(\d+) d=(\d+) (\w+)")


def fight(out, loadout, path, faction, secs):
    label = f"lab_{loadout}_{faction}"
    env = [f"AFTERLIGHT_DESIGN={path}", "AFTERLIGHT_OFFSCREEN=1", "AFTERLIGHT_MOVETEST=1", "AFTERLIGHT_MOVETEST_ENEMY_SHOOTS=1",
           f"AFTERLIGHT_MOVETEST_FLEET={faction}:Patrol", "AFTERLIGHT_MOVETEST_FLEET_AT=0,20000",
           "AFTERLIGHT_COMBAT_REPORT=1", "AFTERLIGHT_GUN_TRACE=1", f"AFTERLIGHT_KEYS=3:Backslash,4:Space~{secs}"]
    # LAB_ENV: extra game settings for a tuning run, e.g. "AFTERLIGHT_GUN_DAMAGE=6".
    env += [e for e in os.environ.get("LAB_ENV", "").split() if e]
    subprocess.run(["tools/playtest.sh", str(secs), label] + env, cwd=ROOT, capture_output=True, text=True)
    log = open(f"/tmp/afterlight_playtest/{label}/run.log", errors="replace").read()
    log = re.sub(r"\x1b\[[0-9;]*m", "", log)
    rows = []
    for m in LINE.finditer(log):
        enemies = [dict(faction=e[0], cls=e[1], hull=float(e[2]), blocks=int(e[3]), start=int(e[4]), d=int(e[5]), state=e[6])
                   for e in ENEMY.findall(m.group(8))]
        rows.append(dict(t=int(m.group(1)), hull=float(m.group(2)), blocks=int(m.group(3)), start=int(m.group(4)),
                         gen=int(m.group(5)), use=int(m.group(6)), crew=int(m.group(7)), enemies=enemies))
    c = Counter()
    for k, pat in [("player_missiles", r"MISSILE launched from \w+ \(player\)"), ("enemy_missiles", r"MISSILE launched from \w+ \(AI\)"),
                   ("missile_bursts_on_enemy", r"MISSILE burst on"), ("flak_kills", r"FLAK downed a missile"),
                   ("laser_pd_kills", r"LASER .*downed"), ("decoyed", r"MISSILE fooled"), ("laser_hits", r"LASER hit"),
                   ("round_hits", r"BUILT ROUND HIT"), ("panics", r"panicked")]:
        c[k] = len(re.findall(pat, log))
    res = dict(loadout=loadout, faction=faction, secs=secs, counts=dict(c), timeline=rows)
    # Keep the report lines, drop the rest (the trace runs to megabytes).
    keep = "\n".join(l for l in log.splitlines() if "COMBAT t=" in l or "panicked" in l or "STANDOFF" in l)
    open(os.path.join(out, label + ".log"), "w").write(keep)
    subprocess.run(["rm", "-rf", f"/tmp/afterlight_playtest/{label}"])
    return res


def summarise(r):
    tl = r["timeline"]
    if not tl:
        return dict(verdict="NO REPORT")
    last = tl[-1]
    en = last["enemies"]
    first_kill = next((row["t"] for row in tl if any(e["state"] == "DESTROYED" for e in row["enemies"])), None)
    engaged = [e["d"] for row in tl for e in row["enemies"] if e["state"] == "Engaging"]
    dead = sum(1 for e in en if e["state"] == "DESTROYED")
    return dict(
        player_hull=last["hull"], player_blocks=f"{last['blocks']}/{last['start']}", power=f"{last['gen']}/{last['use']}", crew=last["crew"],
        enemies=len(en), killed=dead, enemy_hull_avg=round(sum(e["hull"] for e in en) / max(len(en), 1), 2),
        enemy_blocks_lost=round(1 - sum(e["blocks"] for e in en) / max(sum(e["start"] for e in en), 1), 3),
        player_blocks_lost=round(1 - last["blocks"] / max(last["start"], 1), 3),
        first_kill_s=first_kill, engage_range_median=sorted(engaged)[len(engaged) // 2] if engaged else None,
        behaviors=dict(Counter(e["state"] for e in en)),
    )


def main():
    out = sys.argv[1]
    secs = int(sys.argv[2]) if len(sys.argv) > 2 else 150
    filt = sys.argv[3] if len(sys.argv) > 3 else ""
    os.makedirs(out, exist_ok=True)
    paths = loadouts(out)
    results = []
    resfile = os.path.join(out, "results.json")
    if os.path.exists(resfile):
        results = json.load(open(resfile))
    done = {(r["loadout"], r["faction"]) for r in results}
    # LAB_LOADOUTS / LAB_FACTIONS (comma lists) narrow a campaign.
    pick = lambda var, all_: [x for x in all_ if not os.environ.get(var) or x in os.environ[var].split(",")]
    for loadout, path in [(l, paths[l]) for l in pick("LAB_LOADOUTS", list(paths))]:
        for faction in pick("LAB_FACTIONS", FACTIONS):
            if (loadout, faction) in done or (filt and filt not in f"{loadout}_{faction}"):
                continue
            r = fight(out, loadout, path, faction, secs)
            r["summary"] = summarise(r)
            results.append(r)
            json.dump(results, open(resfile, "w"), indent=1)
            print(loadout, faction, r["summary"], r["counts"], flush=True)
    lines = ["| loadout | enemy | your blocks lost | their blocks lost | power | killed | 1st kill | range | your msl | their msl | PD kills | panics |",
             "|---|---|---|---|---|---|---|---|---|---|---|---|"]
    for r in results:
        s, c = r["summary"], r["counts"]
        lines.append(f"| {r['loadout']} | {r['faction']} | {s.get('player_blocks_lost')} | {s.get('enemy_blocks_lost')} | {s.get('power')} | "
                     f"{s.get('killed')}/{s.get('enemies')} | {s.get('first_kill_s')} | {s.get('engage_range_median')} | "
                     f"{c['player_missiles']} | {c['enemy_missiles']} | {c['flak_kills'] + c['laser_pd_kills']} | {c['panics']} |")
    open(os.path.join(out, "results.md"), "w").write("\n".join(lines) + "\n")
    print("\n".join(lines))


if __name__ == "__main__":
    main()
