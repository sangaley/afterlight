"""Balance sweep: a saved binary against every faction, over several seeds, with
what did the damage. Muted and offscreen.

    python3 tools/combat-lab/sweep.py <out_dir> <binary> [--secs 150] [--seeds 3]
        [--workers 3] [--factions A,B] [--fleet Patrol] [--design designs/starter.json]
        [--first-seed 3] [--keep] [--resume] [--env KEY=VALUE ...]

Run from afterlight/, with a copy of the built binary so a rebuild can't change
it mid-run, and its shared libraries in AFTERLIGHT_LAB_LIBS (see train.py).

The player holds still and fires at one target, so read these as a way to compare
builds, not as how a piloted fight goes. `ended` is the last report's game time;
well short of --secs means the player died there. One fight is noisy, so believe
a difference only if it shows across seeds.
"""
import argparse, json, os, re, statistics, subprocess, time
from concurrent.futures import ThreadPoolExecutor

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
ALL = ["EternalHegemony", "TerranHegemony", "GildedThrone", "BrokenChoir", "CorpseStars",
       "SynthesisCollective", "Shepherd", "RecursiveKingdom"]
LINE = re.compile(r"COMBAT t=(\d+) player hull=[\d.]+ blocks=(\d+)/(\d+) .*?\| (.*)")
CREW = re.compile(r"COMBAT t=\d+ player .*?crew=(\d+)")
ENEMY = re.compile(r"(\w+)/(\w+|-) hull=[\d.]+ blocks=(\d+)/(\d+) d=(\d+) (\w+)")
HITS = re.compile(r"COMBAT HITS t=(\d+) (.*?) \| explosions (\d+)")
SOURCE = re.compile(r"(\w+) x(\d+) (\d+)dmg")


def fight(binary, faction, seed, a):
    os.makedirs(a.out, exist_ok=True)
    log = os.path.join(a.out, f"{faction}_{seed}.log")
    if a.resume and finished(log, a.secs):
        return read_fight(log, faction, seed, a, False)
    env = dict(os.environ, AFTERLIGHT_MUTE="1", AFTERLIGHT_SKIP_MENU="1", AFTERLIGHT_OFFSCREEN="1", AFTERLIGHT_MOVETEST="1",
               AFTERLIGHT_MOVETEST_ENEMY_SHOOTS="1", AFTERLIGHT_MOVETEST_FLEET=f"{faction}:{a.fleet}",
               AFTERLIGHT_MOVETEST_FLEET_AT="0,20000", AFTERLIGHT_COMBAT_REPORT="1", AFTERLIGHT_DESIGN=a.design,
               AFTERLIGHT_KEYS=f"3:Backslash,4:Space~{a.secs}", NO_COLOR="1", AFTERLIGHT_FLEET_SEED=str(seed),
               BEVY_ASSET_ROOT=ROOT)
    for kv in a.env:
        k, v = kv.split("=", 1)
        env[k] = v
    if os.environ.get("AFTERLIGHT_LAB_LIBS"):
        env["DYLD_LIBRARY_PATH"] = os.environ["AFTERLIGHT_LAB_LIBS"]
    began = time.time()
    with open(log, "w") as f:
        p = subprocess.Popen([binary], cwd=ROOT, env=env, stdout=f, stderr=subprocess.STDOUT)
        while p.poll() is None and time.time() - began < a.secs + 6:
            time.sleep(1)
        crashed = p.poll() not in (None, 0)
        if p.poll() is None:
            p.terminate()
            try:
                p.wait(10)
            except subprocess.TimeoutExpired:
                p.kill()
    return read_fight(log, faction, seed, a, crashed)


def finished(log, secs):
    """A log from a fight that ran its time: not cut short by a restart."""
    if not os.path.exists(log):
        return False
    text = open(log, errors="replace").read()
    times = re.findall(r"COMBAT t=(\d+) player", text)
    return bool(times) and (int(times[-1]) >= secs - 12 or "Skipped event Destroyed" in text[-2000:])


def read_fight(log, faction, seed, a, crashed):
    text = re.sub(r"\x1b\[[0-9;]*m", "", open(log, errors="replace").read())
    rows = [(int(t), int(alive), int(start), found) for t, alive, start, found in LINE.findall(text)]
    if not rows:
        raise SystemExit(f"{faction} seed {seed}: no report, see {log}\n{text[-400:]}")
    # Ships can leave sensor range: the last line that still shows them.
    shown = [r for r in rows if ENEMY.findall(r[3])]
    found = ENEMY.findall(shown[-1][3]) if shown else []
    total = sum(int(e[3]) for e in found)
    gone = sum(int(e[3]) if e[5] == "DESTROYED" else int(e[3]) - int(e[2]) for e in found)
    t_half = next((t for t, alive, start, _ in rows if alive < start / 2), None)
    t_first_hit = next((int(t) for t, _, _ in HITS.findall(text)), None)
    t_kill = next((r[0] for r in shown if any(e[5] == "DESTROYED" for e in ENEMY.findall(r[3]))), None)
    sources = {}
    for _, hits, _ in HITS.findall(text):
        for name, n, dmg in SOURCE.findall(hits):
            s = sources.setdefault(name, [0, 0])
            s[0] += int(n)
            s[1] += int(dmg)
    errors = len(re.findall(r"panicked|Skipped command", text))
    if not a.keep:
        os.remove(log)
    return dict(faction=faction, seed=seed, you=1 - rows[-1][1] / max(rows[-1][2], 1), them=gone / max(total, 1),
                killed=sum(e[5] == "DESTROYED" for e in found), ships=len(found), classes=[e[1] for e in found],
                shot_off=sum(int(e[3]) - int(e[2]) for e in found) / max(total, 1),
                crew=[int(c) for c in CREW.findall(text)],
                by_class=[(e[1], int(e[3]), int(e[3]) - int(e[2]), e[5] == "DESTROYED") for e in found],
                strikes=re.findall(r"strikes: ([a-z ]+)", text),
                t_half=t_half, t_first_hit=t_first_hit, t_kill=t_kill, sources=sources, last_t=rows[-1][0],
                crashed=crashed, errors=errors)


def mean(xs):
    xs = [x for x in xs if x is not None]
    return statistics.mean(xs) if xs else None


def main():
    p = argparse.ArgumentParser()
    p.add_argument("out")
    p.add_argument("binary")
    p.add_argument("--secs", type=int, default=150)
    p.add_argument("--seeds", type=int, default=3)
    p.add_argument("--workers", type=int, default=3)
    p.add_argument("--factions", default=",".join(ALL))
    p.add_argument("--fleet", default="Patrol")
    p.add_argument("--design", default="designs/starter.json")
    p.add_argument("--first-seed", type=int, default=3)
    p.add_argument("--keep", action="store_true", help="keep each fight's log")
    p.add_argument("--resume", action="store_true", help="reuse the logs of fights that already ran their time")
    p.add_argument("--env", nargs="*", default=[])
    a = p.parse_args()
    binary = os.path.abspath(a.binary)
    a.out = os.path.abspath(a.out)
    subprocess.Popen(["caffeinate", "-di", "-w", str(os.getpid())])
    factions = a.factions.split(",")
    with ThreadPoolExecutor(a.workers) as pool:
        jobs = [pool.submit(fight, binary, f, a.first_seed + s, a) for f in factions for s in range(a.seeds)]
        results = [j.result() for j in jobs]
    os.makedirs(a.out, exist_ok=True)
    json.dump(results, open(os.path.join(a.out, "results.json"), "w"), indent=1)
    fmt = lambda v, s="{:.0f}": "-" if v is None else s.format(v)
    print("| faction | you lost | they lost | killed | half gone at | 1st kill | gun dmg | blast dmg | other | ended | err |")
    print("|---|---|---|---|---|---|---|---|---|---|---|")
    for f in factions:
        rs = [r for r in results if r["faction"] == f]
        total = {}
        for r in rs:
            for k, v in r["sources"].items():
                total[k] = total.get(k, 0) + v[1]
        gun = total.pop("Gunfire", 0) / len(rs)
        blast = total.pop("Explosion", 0) / len(rs)
        other = sum(total.values()) / len(rs)
        print(f"| {f} | {mean([r['you'] for r in rs]):.0%} | {mean([r['them'] for r in rs]):.0%} "
              f"| {sum(r['killed'] for r in rs)}/{sum(r['ships'] for r in rs)} "
              f"| {fmt(mean([r['t_half'] for r in rs]))} | {fmt(mean([r['t_kill'] for r in rs]))} "
              f"| {gun:.0f} | {blast:.0f} | {other:.0f} | {mean([r['last_t'] for r in rs]):.0f} "
              f"| {sum(r['errors'] + r['crashed'] for r in rs)} |", flush=True)
    print(f"| ALL | {mean([r['you'] for r in results]):.0%} | {mean([r['them'] for r in results]):.0%} | | | | | | | | |")
    # `they lost` counts a surrendered ship as all gone: this is what was shot off.
    print(f"blocks actually shot off: {mean([r['shot_off'] for r in results]):.0%}")
    print(f"your crew left at the end: {mean([r['crew'][-1] / max(r['crew'][0], 1) for r in results if r['crew']]):.0%}")
    print("\n| faction | ship | seen | blocks | shot off | finished |\n|---|---|---|---|---|---|")
    by = {}
    for r in results:
        for name, blocks, off, dead in r["by_class"]:
            s = by.setdefault((r["faction"], name), [0, 0, 0, 0])
            s[0] += 1
            s[1] += blocks
            s[2] += off
            s[3] += dead
    for (f, name), s in sorted(by.items(), key=lambda kv: (kv[0][0], -kv[1][1] / kv[1][0])):
        print(f"| {f} | {name} | {s[0]} | {s[1] // s[0]} | {s[2] / max(s[1], 1):.0%} | {s[3]}/{s[0]} |")
    why = {}
    for r in results:
        for w in r["strikes"]:
            why[w.strip()] = why.get(w.strip(), 0) + 1
    print("\nwhy ships gave up:", why)


if __name__ == "__main__":
    main()
