#!/usr/bin/env python3
"""Summarise the PERF lines a AFTERLIGHT_PERF run logged (see src/perf.rs).

    tools/perf_summary.py run.log [more.log ...]

Prints each log's PERF SUMMARY line if it has one, then the worst seconds,
judged by sim time (the game's own systems), which a throttled window can't
distort.
"""
import re
import sys

KV = re.compile(r"(\w+)=([-\d.]+)")


def parse(line):
    return {k: float(v) for k, v in KV.findall(line[line.index("PERF"):].replace("ms", ""))}


def summarise(path, warmup=10):
    rows, summary = [], None
    for line in open(path, errors="replace"):
        if "PERF SUMMARY" in line:
            summary = line[line.index("PERF SUMMARY"):].strip()
        elif "PERF t=" in line:
            r = parse(line)
            if r.get("t", 0) > warmup:
                rows.append(r)
    print(f"== {path}")
    if summary:
        print("  " + summary)
    if rows:
        worst = sorted(rows, key=lambda r: -r.get("sim_avg", 0))[:3]
        for r in worst:
            print(f"  worst second t={r['t']:.0f}: sim_avg={r.get('sim_avg', 0):.2f} sim_max={r.get('sim_max', 0):.2f} "
                  f"frame={r['avg']:.2f} entities={r['entities']:.0f} player={r.get('player', 0):.0f} ai={r.get('ai', 0):.0f} wreck={r.get('wreck', 0):.0f}")
    if not rows and not summary:
        print("  no PERF lines")


for p in sys.argv[1:]:
    summarise(p)
